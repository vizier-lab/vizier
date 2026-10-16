# Research: Agent Self-Managed Integrations

**Feature**: `017-agent-self-integrations` | **Spec**: [spec.md](spec.md)

Every decision below was taken against the code as of `04b7cfa` (016 complete).

## R1. Where the three switches live: not on `AgentConfig`

**Decision**: Store the switches as **grants** in a new table `integration_grant (agent_id, family, granted_by, granted_at)`, one row per family switched on, behind `IntegrationStorage`. They are read and written through their own endpoint (`/agents/{id}/integrations/grants`), not through `PUT /agents/{id}`.

**Rationale**:
- `PUT /agents/{id}` sends `AgentCommand::Update`, which builds a new instance and **aborts the running turns** (#52). FR-006 forbids that for a switch change. A grant change publishes `IntegrationChanged` instead, which only makes the agent recompose its tools (`refresh_integrations`), exactly as an integration edit does.
- The permission check (FR-002) has one obvious home, the grant endpoint, guarded by `access::can_manage_on_agent`. `PUT /agents` never sees the fields, so FR-003 holds without any "did it change?" diffing.
- `granted_by`/`granted_at` come with the row and feed the change record (FR-018).

**Alternatives considered**:
- *A field on `AgentToolsConfig`*: matches where the built-in tool toggles live, but every save restarts the agent, and `update_agent` would need to diff the old and new config to decide whether the integration permission applies. Rejected.
- *Fields on `AgentConfig` plus a no-restart fast path in the supervisor when only these fields differ*: this adds a second update path to the supervisor. Rejected under Principle I.

The WebUI still shows the switches on Misc › Config (spec Assumptions). The form saves them with a separate request (contracts/webui.md).

## R2. One service for every integration write, shared by HTTP and the agent tools

**Decision**: Move the write logic now in the HTTP handlers into `src/integrations/service.rs`: `create`, `update` and `delete`/unlink, plus grant set/clear. Each one validates through the kind, merges secrets, saves, records an `integration_event`, and publishes `IntegrationChanged` after commit. Each takes an `Actor` (`Person(Caller)` or `Agent(AgentId)`) and asks `access.rs` whether that actor may do it. The HTTP handlers and the new agent tools both become thin callers. Errors are a small `ServiceError` (`Forbidden`, `NotFound`, `Invalid(String)`, `Conflict(IntegrationConflict)`, `Storage`). HTTP maps it to 403/404/422/409/500, and the tools map it to readable text.

**Rationale**: Principle II and FR-011 (the same rules for agents as for people). If the tools re-implemented validate/merge/save/publish, they would be a second copy of `create`/`update_integration`, and every future rule would have to be remembered twice.

**Alternatives considered**: tools that call the HTTP API over loopback. That needs a token for the agent and a JWT identity it doesn't have, and adds an HTTP round-trip inside a tool call. Rejected.

## R3. Access model: per-agent, no scope (FR-020–FR-022)

**Decision**: Rewrite `access.rs` around one question: *which agents does this actor manage?*
- A person manages agent A iff `manages_all()`, or `manages_owned()` and they own A. (`can_manage_on_agent` stays as it is.)
- An integration's **owner agent** is its only linked agent. An integration linked to 0 agents or to 2 or more is **legacy** (FR-023).
- A person may edit or delete an integration iff it has an owner agent they manage, or it is legacy and they `manages_all()`.
- A person may see an integration iff they may view any agent it is linked to, or they `manages_all()`.
- An agent may act on an integration iff it is that integration's owner agent **and** the grant for its family exists. A legacy integration is invisible to the agent: it is filtered out of the tools' listing, and an id reference to it is `NotFound`.
- `created_by` grants nothing.

`IntegrationScope`, `scope_for_creator`, `Access.attach` and the scope branch of `can_see` are deleted. The `scope` column stays in the table, because there are no migrations (FR-023 / 016 precedent). New rows write the constant `'owned'` and reads ignore it.

**Rationale**: This is the user's stated model. It also makes the agent rule and the person rule the same rule ("manages the owner agent"), so the agent's authority never exceeds that of the person who granted it.

## R4. Linking: create links once; attach is gone, detach remains for legacy rows

**Decision**:
- `save_integration(integration, attach_to)` keeps its signature. A create links to exactly one agent, and an update never changes links.
- `PUT /agents/{id}/integrations/{iid}/attachment` with `attached: true` is **removed**, which is breaking. `attached: false` is kept only to unlink a legacy multi-linked integration from one agent (all-agents managers only), which is how FR-023's "deleting it from one agent's page removes only that agent's link" works.
- The agent-scoped list returns only integrations linked to that agent (no more "not in use" rows from elsewhere).
- Addons: **install** creates the agent's configuration. Because there is one configuration per addon per agent, a second install edits the existing one. **Uninstall** deletes it. The Addons page loses its configuration picker.
- `delete_agent_integrations`: deletes every integration whose only link is the deleted agent, and removes that agent's link from legacy multi-linked rows.

**Rationale**: FR-020. Removing attach outright beats leaving a toggle that can only fail.

**Alternatives considered**: adding `integration.agent_id NOT NULL` and dropping `integration_attachment`. That needs a data migration, which FR-023 rules out. The attachment table can express "exactly one" without one.

## R5. Agent tools: five generic tools in a new `integrations` category

**Decision**: Five tools, built in the **integration layer** (so a grant change recomposes them without a restart) and only when at least one grant exists:

| Tool | What it does |
|---|---|
| `list_integration_kinds {family?}` | The kinds in the agent's granted families, with their fields (key, label, type, required, options). Addons are marked installed or not. |
| `list_integrations {family?}` | The agent's own non-legacy integrations in granted families: id, kind, name, enabled, status, `missing`, redacted settings, creator. |
| `create_integration {kind, name, settings, enabled?}` | Create on self. |
| `update_integration {id, name?, settings?, enabled?}` | Patch. Secrets follow `merge_secrets`, so a redacted value sent back is kept. |
| `delete_integration {id}` | Delete. |

They sit in a new `ToolCategory::Integrations` ("Manage your own MCP servers, addons and channel connections"), between `Shell` and the addon/MCP categories. Each tool's description is built at compose time and names the granted families. Calls on a family that isn't granted (checked against storage at call time, FR-008) return a refusal and change nothing. The MCP server tool listing (FR-007) is the existing `list_tool_functions {category: <server>}`; `list_integrations` gives the category name for each MCP server.

**Rationale**:
- Five tools instead of fifteen, because the operations are identical across families and the kind's field list already describes what each needs (Principle I).
- Building them in the integration layer means "no grant → no tools, no category" (SC-006) and "grant → tools from the next turn" (FR-006) with no new mechanism: `summarise_categories` already lists only categories that are present.
- The tools are absent from `DREAM_TOOL_NAMES`, so the dream cycle can't reach them (FR-016) without any extra code.

**Spec adjustment**: FR-015's "sees only that family's tools" becomes "sees only tools that act on the families switched on" (the same tools serve every granted family, and refuse the others).

**Alternatives considered**: one tool set per family (`add_mcp_server`, `install_addon`, …). These read more naturally, but they triple the code and the docs, and each new kind would need new tools. Rejected.

## R6. Change record

**Decision**: A new table, `integration_event`:

| Column | Type | Notes |
|---|---|---|
| `id` | TEXT | |
| `agent_id` | TEXT | |
| `integration_id` | TEXT | nullable for grant events |
| `integration_name` | TEXT | |
| `actor` | TEXT | JSON `Requester` (`{"user": "…"}` / `{"agent": "…"}`) |
| `action` | TEXT | |
| `fields` | TEXT | JSON array of setting keys changed, **names only, never values** |
| `session` | TEXT | slug, for agent actions |
| `at` | TEXT | |

`action` is one of `created`, `edited`, `enabled`, `disabled`, `deleted`, `unlinked`, `grant_on`, `grant_off`. Each row is written in the same transaction as the change it describes. Rows are kept after the integration is deleted, and deleted with the agent. They are served by `GET /agents/{id}/integrations/events?integration_id=&before=&limit=`, which feeds the Activity tab.

`Requester` (`schema/task.rs`) is reused as the actor type, so there is no new "user or agent" enum. The integration's creator is stored the same way, in a new nullable column `creator` (JSON `Requester`). A legacy row with only `created_by` reads as `User(created_by)`. Columns are added with the existing `ALTER TABLE … ADD COLUMN` if-missing pattern; that is a schema addition, not a data migration.

**Rationale**:
- FR-012 and FR-018. Storing changed field names lets the Activity tab say "edited uri" without ever persisting a secret twice.
- The session slug answers "why did the agent do this?" (the conversation is reachable from it).

## R7. Self-lockout warning for channels

**Decision**: When `update_integration` (with `enabled: false`) or `delete_integration` acts on a channel connection whose platform matches the current session's channel platform, the tool still makes the change. Its result carries a `warning` field: "you will stop receiving messages on this connection; this reply may not be delivered". The tool description states this up front. The platform comparison goes through a new `IntegrationKind::platform() -> Option<ChannelPlatform>` (default `None`; Discord and Telegram override it), so the tool never branches on a kind id.

## R8. What is deliberately not built

- **A chat-visible line when the agent changes an integration** was raised during spec review and left undecided. It is out of scope here: the agent's tool result already tells it what happened, and the Activity tab covers operators. It can be added later as a `SessionFrame` without touching this design.
- **An approval step for each change** is not built, per the spec's Assumptions.
- **A data migration** is not built, per FR-023.
