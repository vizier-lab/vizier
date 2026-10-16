# Contract: HTTP API changes

All routes are under `/api/v1` and authenticated. Errors keep the 016 shape `{status, message, data: {error: <code>, …}}`.

## Grants (new)

### `GET /agents/{agent_id}/integrations/grants`

The caller must be able to view the agent; otherwise `404`.

```json
{
  "can_manage": true,
  "grants": {
    "mcp":     { "on": true,  "granted_by": "alice", "granted_at": "2026-10-16T14:00:00Z" },
    "addon":   { "on": false },
    "channel": { "on": false }
  }
}
```

`can_manage` tells the form whether the switches are editable (FR-004).

### `PUT /agents/{agent_id}/integrations/grants`

Body: any subset of `{"mcp": bool, "addon": bool, "channel": bool}`.

| Case | Response |
|---|---|
| Caller manages the agent's integrations | `200` with the same shape as `GET`. Each changed family records a `grant_on`/`grant_off` event and publishes `IntegrationChanged { agents: [agent_id] }`. Nothing restarts (FR-006). |
| Caller can view the agent but not manage it | `403 {"error":"forbidden","required":["all_agents:integrations:manage","owned_agents:integrations:manage (own agents)"]}`. Nothing is saved (FR-002). |
| Caller can't view the agent | `404` |

`PUT /agents/{id}` and `POST /agents` do not accept or touch grants. A body carrying a `self_management` key is refused with `422 {"error":"use_grants_endpoint"}`, the same treatment as `moved_to_integrations`, so a client can't believe it saved one.

## Events (new)

### `GET /agents/{agent_id}/integrations/events?integration_id=&before=&limit=`

The caller must be able to view the agent. `limit` defaults to 50 (max 200). `before` is the cursor (`at|id`) from the previous page.

```json
{
  "items": [
    { "id": "…", "integration_id": "…", "integration_name": "github", "family": "mcp",
      "actor": {"agent": "dp-a"}, "actor_name": "Vizier", "action": "edited",
      "fields": ["uri"], "session": "webui__…", "at": "…" }
  ],
  "next": "2026-10-16T14:00:00Z|…"
}
```

## Changed 016 routes

| Route | Change |
|---|---|
| `GET /agents/{id}/integrations` | Returns only integrations linked to this agent. `IntegrationItem` loses `attached`, `can_attach`, `also_used_by` and `others`. It gains `creator` (`{"user":…}` / `{"agent":…}`, plus `creator_name`) and `legacy_linked` (other agents' names the caller can see, plus `others`, present only on legacy rows). `IntegrationView.scope` is removed. |
| `POST /agents/{id}/integrations` | Access: `can_manage_on_agent` only (no scope). Records `created` with the person as actor. For an addon kind the agent already has, answers `409 addon_installed` with the existing id (the client edits it instead). |
| `PUT /agents/{id}/integrations/{iid}/attachment` | `{attached:true}` → `410 {"error":"attach_removed"}`. `{attached:false}` works only on a legacy multi-linked integration (all-agents managers): it unlinks this agent and records `unlinked`. On a single-linked integration → `409 {"error":"use_delete"}`. |
| `PUT /integrations/{id}` | Access per research R3. Records `edited` / `enabled` / `disabled`. |
| `DELETE /integrations/{id}` | Access per R3. `?confirm` keeps its meaning. Records `deleted`. |
| `DELETE /agents/{id}/integrations/{iid}` | **New**, used by the pages. If the integration is linked only to this agent it is deleted; if it is legacy multi-linked, it is unlinked from this agent (all-agents only). |

Every write goes through `integrations::service`, so the HTTP layer does no validation of its own.
