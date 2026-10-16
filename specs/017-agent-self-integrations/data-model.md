# Data Model: Agent Self-Managed Integrations

All tables are SQLite, in `init_integration_schema` (`src/storage/sqlite/mod.rs`). Additions only; no data migration (FR-023).

## 1. `integration` (changed)

| Column | Change |
|---|---|
| `scope` | Kept for the DDL; written as `'owned'`, never read. `IntegrationScope` is deleted from `schema/integration.rs`. |
| `created_by` | Kept; legacy, a user id. Not written for new rows once `creator` exists. |
| `creator` | **New**, `TEXT NULL`: JSON `Requester` — `{"user":"alice"}` or `{"agent":"dp-a"}`. Read as `Requester::User(created_by)` when null. Informational only (FR-012). |

`Integration` (Rust): drops `scope`; replaces `created_by: Option<String>` with `creator: Option<Requester>`; keeps `agents: Vec<AgentId>` (from `integration_attachment`).

Derived, not stored:

- **owner agent** = `agents[0]` when `agents.len() == 1`, else none.
- **legacy** = `agents.len() != 1`. Legacy rows are never created any more; they are left from 016 multi-attach or detach (FR-023).

## 2. `integration_attachment` (unchanged DDL, new invariant)

Every integration created from now on has exactly one row here, written by the create. Updates never touch it. Rows are removed only by: deleting the integration (cascade), unlinking a legacy integration from one agent, or deleting the agent.

## 3. `integration_grant` (new)

```sql
CREATE TABLE IF NOT EXISTS integration_grant (
    agent_id    TEXT NOT NULL,
    family      TEXT NOT NULL,          -- 'mcp' | 'addon' | 'channel'
    granted_by  TEXT NOT NULL,          -- user id
    granted_at  TEXT NOT NULL,
    PRIMARY KEY (agent_id, family)
);
```

- A row means the switch for that family is on. No row means off (the default for existing agents, FR-005).
- Deleted with the agent (`delete_agent_integrations`).
- Not cascaded from users: losing permission or being deleted leaves the grant (spec Edge Cases).

Rust: `IntegrationGrant { agent_id, family: IntegrationFamily, granted_by: String, granted_at }`.

## 4. `integration_event` (new)

```sql
CREATE TABLE IF NOT EXISTS integration_event (
    id               TEXT PRIMARY KEY,
    agent_id         TEXT NOT NULL,
    integration_id   TEXT,               -- null for grant_on / grant_off
    integration_name TEXT NOT NULL,      -- or the family for grant events
    family           TEXT NOT NULL,
    actor            TEXT NOT NULL,      -- JSON Requester
    action           TEXT NOT NULL,
    fields           TEXT NOT NULL DEFAULT '[]',  -- JSON array of changed setting keys (no values)
    session          TEXT,               -- session slug when the actor is an agent
    at               TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_integration_event_agent ON integration_event(agent_id, at DESC);
CREATE INDEX IF NOT EXISTS idx_integration_event_integration ON integration_event(integration_id, at DESC);
```

`action` ∈ `created | edited | enabled | disabled | deleted | unlinked | grant_on | grant_off`. An `update` that changes both settings and `enabled` writes one `edited` row carrying the field names and one `enabled`/`disabled` row.

The event is written **in the same transaction** as the change it describes. Rows survive the integration's deletion, so the agent's history stays whole, and are deleted with the agent.

## 5. `IntegrationStorage` (trait changes)

| Method | Change |
|---|---|
| `save_integration(&Integration, attach_to, &EventDraft)` | Takes the event to record. On create (`attach_to = Some`) it refuses an integration that already exists. |
| `delete_integration(id, &EventDraft)` | Records `deleted`. |
| `set_attachment` | Becomes `unlink(id, agent_id, &EventDraft)`: removes one link of a legacy integration. Attach is removed. |
| `delete_agent_integrations(agent_id)` | Deletes integrations whose only link is the agent, unlinks it from legacy ones, and deletes its grants and events. |
| `list_grants(agent_id)` | **New**. |
| `set_grant(agent_id, family, on, granted_by)` | **New**. Records `grant_on`/`grant_off`, and is a no-op when nothing changes. |
| `list_events(agent_id, integration_id?, before?, limit)` | **New**. Keyset on `(at, id)`. |

`EventDraft { actor: Requester, action, fields: Vec<String>, session: Option<String> }`. The storage fills in `id`, `agent_id`, `integration_*` and `at`.

## 6. Runtime types

- `integrations::service::Actor<'a>`: `Person { caller: Caller<'a> }` or `Agent { agent_id: &'a str, session: &'a VizierSession }`. `Actor::requester()` gives `Requester`.
- `integrations::service::ServiceError`: `NotFound | Forbidden(String) | Invalid(String) | Conflict(IntegrationConflict) | Storage(anyhow::Error)`.
- `IntegrationKind::platform() -> Option<ChannelPlatform>`: **new**, default `None`.
- `ToolCategory::Integrations`: **new**, the 14th variant, after `Shell`.

## 7. State and validation rules (from the spec)

| Rule | Where it is enforced |
|---|---|
| Grant change only by someone who manages the agent (FR-002) | `service::set_grant` → `access::can_manage_on_agent` |
| Agent acts only on its own, non-legacy integrations in granted families (FR-008–FR-010) | `access::agent_may(agent_id, &Integration, &grants)`, checked against storage at call time |
| One connection per platform per agent, token once, one config per addon per agent, MCP names unique per agent (FR-011) | unchanged, in the storage transaction |
| Kind validation (FR-011) | `kind.validate` in `service` |
| Secrets never returned in full (FR-014) | `kind.redact` on every outgoing view, HTTP or tool |
| Legacy rows: all-agents only, unlink per agent, invisible to agents (FR-023) | `access` + `service::delete` on a multi-linked row = `unlink` |
