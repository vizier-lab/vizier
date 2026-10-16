# Tasks: Agent Self-Managed Integrations

**Input**: Design documents from `specs/017-agent-self-integrations/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R8), data-model.md, contracts/ (http-api.md, agent-tools.md, webui.md), quickstart.md

**Tests**: the spec doesn't ask for TDD. Following repo convention, unit tests live as in-file `#[cfg(test)]` modules next to the code they cover, and are listed with the task that writes that code. End-to-end checks are the dummyplug quickstart (constitution gate), in the Polish phase.

**Organization**: Phase 2 holds the per-agent model (FR-020–FR-023), the shared write service and the new tables, which every story needs. The user stories follow in spec priority order.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US4 from spec.md

---

## Phase 1: Setup

**Purpose**: Confirm the baseline builds, so later failures are attributable.

- [ ] T001 Run `cargo build`, `cargo clippy` and `cargo test`, and `cd webui && npm run typecheck`, on branch `017-agent-self-integrations`; note any pre-existing failures in `specs/017-agent-self-integrations/tasks.md` under this task before changing code

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**:
- the per-agent integration model (no scope, no attach, legacy rows read-only);
- `creator` as `Requester`;
- the grant and event tables;
- the one `integrations::service` write path, used by HTTP now and by the agent tools later.

**⚠️ CRITICAL**: no user-story work can begin until this phase is complete.

### Schema and storage

- [ ] T002 In `src/schema/integration.rs`:
  - delete `IntegrationScope` and the `scope` field from `Integration`;
  - replace `created_by: Option<String>` with `creator: Option<Requester>` (`crate::schema::task::Requester`);
  - add `IntegrationGrant { agent_id: AgentId, family: IntegrationFamily, granted_by: String, granted_at: DateTime<Utc> }`;
  - add `EventAction` (snake_case serde: `Created, Edited, Enabled, Disabled, Deleted, Unlinked, GrantOn, GrantOff`, with `as_str`/`parse`);
  - add `IntegrationEvent { id, agent_id, integration_id: Option<String>, integration_name, family, actor: Requester, action: EventAction, fields: Vec<String>, session: Option<String>, at }`;
  - add `EventDraft { actor: Requester, action: EventAction, fields: Vec<String>, session: Option<String> }`;
  - add helpers on `Integration`: `owner_agent() -> Option<&AgentId>` (`Some` iff `agents.len() == 1`) and `is_legacy() -> bool` (`agents.len() != 1`);
  - re-export the new types from `src/schema/mod.rs`.
- [ ] T003 In `src/storage/sqlite/mod.rs` `init_integration_schema`:
  - add `CREATE TABLE IF NOT EXISTS integration_grant` and `integration_event`, with both indexes, exactly as in data-model.md §3–§4;
  - add the nullable column `integration.creator TEXT` using the existing add-column-if-missing pattern in that file (check `pragma table_info` first).
- [ ] T004 In `src/storage/integration.rs`, change the `IntegrationStorage` trait and its `VizierStorage` forwarding impl per data-model.md §5:
  - `save_integration(&Integration, attach_to: Option<&str>, event: &EventDraft) -> Result<Vec<AgentId>>`;
  - `delete_integration(id, event: &EventDraft)`;
  - replace `set_attachment` with `unlink(id, agent_id, event: &EventDraft) -> Result<Vec<AgentId>>`;
  - add `list_grants(agent_id) -> Result<Vec<IntegrationGrant>>`;
  - add `set_grant(agent_id, family, on: bool, granted_by: &str) -> Result<bool>` (returns whether anything changed);
  - add `list_events(agent_id, integration_id: Option<&str>, before: Option<(DateTime<Utc>, String)>, limit: usize) -> Result<Vec<IntegrationEvent>>`;
  - update the doc comments: a create links to exactly one agent; attach is gone.
- [ ] T005 In `src/storage/sqlite/integration.rs`, implement the new trait:
  - drop `scope` from `COLUMNS` and `from_row`. Write the literal `'owned'` into `scope` on insert. Read `creator` as JSON `Requester`, falling back to `Requester::User(created_by)` when null.
  - `save_integration` inserts the attachment only on create, refuses `attach_to` for an id that already exists, and writes the `integration_event` row in the same transaction. For each event, `agent_id` is the attach target on create, otherwise every linked agent (one row each).
  - `unlink` refuses when the integration is linked to fewer than 2 agents; it deletes the one attachment and records `unlinked`.
  - Remove the attach-to-another-agent branch and `ChannelBelongsToAgent` from the attach rules. Keep `check_mcp_name`, the platform rule and token uniqueness for creates and updates. Add an `IntegrationConflict::AddonInstalled { integration_id, name }` (code `addon_installed`) for creating a second configuration of an addon kind on one agent, replacing the old "attach replaces" behaviour.
  - `delete_agent_integrations` deletes integrations whose only link is that agent, removes its link from multi-linked rows, and deletes its `integration_grant` and `integration_event` rows.
  - Implement `list_grants`, `set_grant` (insert/delete the grant row and write a `grant_on`/`grant_off` event in one transaction, `integration_name` = family name; return `false` and write nothing when unchanged) and `list_events` (keyset on `(at, id)` DESC).
  - Update the existing tests in this file: `attaching_an_addon_replaces_the_agents_other_configuration` becomes a test that a second addon config on one agent is `addon_installed`. Add tests for the `set_grant` no-op, the `unlink` refusal on a single-linked row, and `delete_agent_integrations` on a legacy row.

### Access

- [ ] T006 Rewrite `src/integrations/access.rs` per research R3:
  - keep `Caller`, `MANAGE_ALL`, `MANAGE_OWNED` and `can_manage_on_agent`;
  - delete `Access.attach`, `scope_for_creator` and every `IntegrationScope` use;
  - `integration_access(caller, &Integration, agents: &HashMap<AgentId, AgentConfig>) -> Access { see, edit }`:
    - `edit` = (`owner_agent` is some agent the caller `can_manage_on_agent`) || (`is_legacy` && `manages_all`);
    - `see` = `manages_all` || any linked agent the caller can view (pass a `can_view: impl Fn(&AgentConfig) -> bool` so `access.rs` does not depend on HTTP);
  - add `agent_may(agent_id, &Integration, granted: &[IntegrationFamily]) -> bool` = `owner_agent == Some(agent_id)` && `granted.contains(&integration.family)`;
  - rewrite the tests module: owned manager on own agent / other agent, all-agents manager on a legacy row, owned manager on a legacy row (read-only), `agent_may` with and without the grant, on a legacy row, and on another agent's integration.

### Shared write service

- [ ] T007 Create `src/integrations/service.rs` and add `pub mod service;` in `src/integrations/mod.rs`:
  - `pub enum Actor<'a> { Person { caller: Caller<'a> }, Agent { agent_id: &'a str, session: Option<String>, granted: Vec<IntegrationFamily> } }` with `requester()`.
  - `pub enum ServiceError { NotFound, Forbidden(String), Invalid(String), Conflict(IntegrationConflict), Storage(anyhow::Error) }`, with `code()` returning the 016 codes (`not_found`, `forbidden`, `invalid`, the conflict's code, `error`). A `from_storage(anyhow::Error)` downcasts to `IntegrationConflict`.
  - The functions take `&VizierDependencies` (storage, `integration_kinds`, transport):
    - `create(deps, actor, agent: &AgentConfig, agent_id, CreateIntegration { kind, name, settings, enabled }) -> Result<Integration, ServiceError>`. It checks access (a person: `can_manage_on_agent`; an agent: `agent_id` is itself and the kind's family is granted), trims and validates the name, calls `kind.validate`, sets `creator`, saves with a `created` event, and calls `publish`.
    - `update(deps, actor, id, UpdateIntegration { name, settings, enabled })`. It loads the integration (and maps hidden to `NotFound`), checks `integration_access(..).edit` or `agent_may`, applies `merge_secrets`, validates, and computes changed setting keys (top-level keys whose value differs; names only). It saves with an `edited` event when the name or settings changed, plus an `enabled`/`disabled` event when `enabled` changed. It publishes.
    - `delete(deps, actor, id, from_agent: Option<&str>)`. On a legacy multi-linked row with `from_agent` set it calls `unlink` (person `manages_all` only); otherwise it deletes with a `deleted` event. It publishes.
    - `set_grants(deps, caller, agent_id, &AgentConfig, changes: &[(IntegrationFamily, bool)])`, which refuses with `Forbidden` unless `can_manage_on_agent`. It calls `set_grant` per family and publishes `IntegrationChanged { integration_id: String::new(), family, agents: vec![agent_id] }` when anything changed.
  - Move `publish` here (from `src/channels/http/api/v1/integrations.rs`) as `pub fn publish(transport, &Integration, agents)`.

### HTTP adapts to the per-agent model

- [ ] T008 In `src/channels/http/api/v1/integrations.rs`:
  - remove `scope` from `IntegrationView` and `view()`; add `creator: Option<Requester>` and `creator_name: Option<String>` (the agent's name or the user id);
  - add `pub(crate) fn service_error(ServiceError) -> Response` mapping to 404/403/422/409/500, keeping the `storage_error` body shape for conflicts;
  - rewrite `update_integration` and `delete_integration` as thin calls to `service::update` / `service::delete` with `Actor::Person`;
  - delete `channel_owner` and the scope branch of `can_see` (use `access::integration_access(..).see`);
  - `IntegrationDetail` keeps `agents`.
- [ ] T009 In `src/channels/http/api/v1/agents/integrations.rs`:
  - `list` returns only integrations whose `agents` contains `agent_id` (via `list_agent_integrations`);
  - `IntegrationItem` drops `attached`, `also_used_by`, `others` and `can_attach`, and gains `legacy_linked: Option<LegacyLinked { agents: Vec<AgentRef>, others: usize }>` (set only when `is_legacy`, listing the other agents the caller can view);
  - `create` calls `service::create` with `Actor::Person`;
  - `set_attachment` answers `{attached:true}` with `410 {"error":"attach_removed"}` and maps `{attached:false}` to `service::delete(.., from_agent: Some(agent_id))`, which on a single-linked row answers `409 {"error":"use_delete"}`;
  - add `DELETE /{integration_id}`, which calls `service::delete(.., Some(agent_id))`;
  - register the routes and update the utoipa annotations.
- [ ] T010 In `src/agents/agent/mod.rs` and `src/agents/mod.rs`:
  - make the agent build its integration layer only from integrations whose `owner_agent()` is itself **or** that are legacy rows linked to it. Legacy rows keep working for each linked agent (FR-023), so `list_agent_integrations` is unchanged here; just confirm and comment it.
  - Make sure `VizierAgents::run` treats an `IntegrationChanged` with an empty `integration_id` (a grant change) like any other change: refresh the listed agents.

### WebUI adapts to the per-agent model

- [ ] T011 [P] In `webui/app/interfaces/types.ts`:
  - update `IntegrationItem`: remove `attached`, `can_attach`, `also_used_by`, `others` and `scope`; add `creator: {user: string} | {agent: string} | null`, `creator_name?: string` and `legacy_linked?: { agents: AgentRef[]; others: number }`;
  - add `IntegrationGrants` (`{ can_manage: boolean; grants: Record<'mcp' | 'addon' | 'channel', { on: boolean; granted_by?: string; granted_at?: string }> }`);
  - add `IntegrationEvent` (fields per contracts/http-api.md § Events).
- [ ] T012 [P] In `webui/app/services/vizier.ts`:
  - remove the attach call (keep `setIntegrationAttachment` only if needed for legacy unlink, otherwise delete it);
  - add `deleteAgentIntegration(agentId, integrationId)`, `getIntegrationGrants(agentId)`, `setIntegrationGrants(agentId, changes)` and `listIntegrationEvents(agentId, { integrationId?, before?, limit? })`.
- [ ] T013 In `webui/app/hooks/integrationStore.tsx`, remove `attach`/`detach`, add `remove(integrationId)` (calls `deleteAgentIntegration`, then reloads), and keep everything else (depends on T011, T012).
- [ ] T014 In `webui/app/components/integrations.tsx`:
  - replace `SharedEditWarning` with `LegacyLinkedNotice({ item })`, using the text in contracts/webui.md;
  - add `CreatorLine({ item, detail })`, rendering `<detail> · added by <creator_name>`, with a 🤖 marker for an agent creator;
  - change `StatusBadge` so it no longer reads `item.attached`.
- [ ] T015 In `webui/app/routes/integrations-mcp.tsx`:
  - the per-row attach checkbox becomes an enabled switch calling the existing update (`PUT /integrations/{id} {enabled}`);
  - render `CreatorLine` and `LegacyLinkedNotice`;
  - delete uses `store.remove`;
  - drop every reference to `attached`/`can_attach`/`also_used_by`.
- [ ] T016 In `webui/app/routes/integrations-addons.tsx`:
  - remove the configuration picker (`chosen`/`configs` selection);
  - for each addon kind, show the agent's one configuration or nothing;
  - **Install** creates it (on `409 addon_installed`, open the existing one for editing), and **Uninstall** confirms, then `store.remove`;
  - render `CreatorLine`/`LegacyLinkedNotice`.
- [ ] T017 In `webui/app/routes/integrations-channels.tsx`, stop using `item.attached` (every listed item is the agent's), render `CreatorLine`, and use `store.remove` for delete.

**Checkpoint**: `cargo clippy`, `cargo test` and `npm run typecheck` pass. The integration pages work per agent, with no scope and no attach. Writes record events, though nothing shows them yet.

---

## Phase 3: User Story 1 - Grant an agent self-management of one integration family (Priority: P1) 🎯 MVP

**Goal**: someone who manages an agent's integrations can switch MCP/Addons/Channels self-management on or off for that agent. Nobody else can. Nothing restarts.

**Independent test**: quickstart §1, steps 2–6 and step 8, plus §6.1–2.

- [ ] T018 [US1] In `src/channels/http/api/v1/agents/integrations.rs`, add `GET /grants` and `PUT /grants` per contracts/http-api.md:
  - `GET` needs a viewable agent and returns `{ can_manage, grants: {mcp, addon, channel} }` from `list_grants`;
  - `PUT` takes `{ mcp?, addon?, channel? }` and calls `service::set_grants`. `Forbidden` maps to `403` with `required` listing both permissions;
  - register the routes and add utoipa docs.
- [ ] T019 [US1] In `src/channels/http/api/v1/agents/mod.rs` `parse_agent_body`, refuse a body with a top-level `self_management` key with `422 {"error":"use_grants_endpoint"}`, using the same `fail` helper as `moved_to_integrations`.
- [ ] T020 [US1] In `webui/app/components/AgentForm.tsx`, add a "Self-management" section in the Tools tab after "Tool Settings":
  - styled like the existing `h4` + bordered-box sections, with three checkboxes (MCP servers, Addons, Channels);
  - the MCP warning "Includes local servers, which run commands on the vizier host." is always shown, and the Channels note per contracts/webui.md;
  - load the grants with `getIntegrationGrants` when editing an existing agent, and hide the section for a new agent;
  - when `can_manage` is false, disable the checkboxes and show the notice "Only people who can manage this agent's integrations can change these.";
  - on save, after the agent save succeeds, call `setIntegrationGrants` with only the changed families, and show a section-level error if it fails.

**Checkpoint**: the switches persist, are permission-gated, and never restart the agent. Agents have no tools yet, so SC-006 still holds.

---

## Phase 4: User Story 2 - Agent manages its own MCP servers in conversation (Priority: P1)

**Goal**: with the MCP grant on, the agent lists, creates, edits, enables/disables and deletes its own MCP servers through five code-mode tools. Changes take effect from its next turn and are recorded and shown as agent-made.

**Independent test**: quickstart §2, plus §6.3.

- [ ] T021 [US2] In `src/agents/tools/mod.rs`:
  - add `ToolCategory::Integrations` (name `integrations`, description "Manage your own MCP servers, addons and channel connections") after `Shell`, and extend `ALL` to 14;
  - add the five tool names to the `every_tool_is_in_its_category` table;
  - add `mod self_integrations;`;
  - make `compose` add the integration layer's self-management tools (from T023) to the router's `integration_toolset`, so `category_summaries` lists `integrations` only when they exist;
  - add a unit test: `summarise_categories` without these tools has no `integrations` category.
- [ ] T022 [US2] Create `src/agents/tools/self_integrations.rs` with the five `VizierTool`s from contracts/agent-tools.md: `ListIntegrationKinds`, `ListIntegrations`, `CreateIntegration`, `UpdateIntegration`, `DeleteIntegration`. Each holds `agent_id`, `deps: VizierDependencies` and `granted: Vec<IntegrationFamily>`, and has category `Some(ToolCategory::Integrations)`. Input/Output types are `JsonSchema` with field descriptions (dummyplug shows them).
  - `description()` is built at construction and names the granted families. The MCP note about local servers is included when `mcp` is granted, and the channel lockout note when `channel` is granted.
  - Every `call` re-reads `deps.storage.list_grants(agent_id)` (FR-008) and builds `Actor::Agent { agent_id, session: Some(ctx.session.to_slug()), granted }`.
  - List tools filter to the agent's own non-legacy integrations in granted families, render settings with `kind.redact`, and include `status` (via `agent_status`, reusing what `integration_status` uses), `missing`, `creator` and `tool_category` (the MCP category name from `ToolRouter::catalogue()` naming, or the addon id; `null` for channels).
  - `list_integration_kinds` marks addons `installed`.
  - Write tools call `service::create/update/delete`. A reference to an integration that isn't the agent's own, or is legacy, gives `"no integration '<id>'"`. A family that isn't granted gives `"managing <family> is not enabled for this agent; ask someone who manages its integrations to turn it on"`. Other `ServiceError`s map to their text plus `code`. Successful writes add `note: "available from your next turn"`.
  - Return errors as `VizierError` so they are catchable in scripts.
- [ ] T023 [US2] In `src/integrations/mod.rs`, make `IntegrationTools` carry a `self_management: VizierToolSet`, built in `IntegrationTools::build` from a new `grants: &[IntegrationGrant]` parameter (empty set when there are no grants; the set is rebuilt every time, since it's cheap). Update the callers in `src/agents/agent/mod.rs`: the initial build and `refresh_integrations` read `list_grants(agent_id)` next to `list_agent_integrations`, and a grants read failure keeps the current tools, like the existing refresh-failure path.
- [ ] T024 [US2] Confirm that none of the five tool names are in `VizierTools::DREAM_TOOL_NAMES` (in `src/agents/tools/mod.rs`), and add a one-line test asserting it (FR-016).
- [ ] T025 [P] [US2] In `src/channels/http/api/v1/agents/integrations.rs`, add `GET /events?integration_id=&before=&limit=` per contracts/http-api.md (viewable agent; `limit` default 50, max 200; cursor `at|id`; `actor_name` resolved to the agent's name or user id).
- [ ] T026 [US2] In `webui/app/components/integrations.tsx`, add `ActivityTab({ agentId, integrationId })`, which loads `listIntegrationEvents` newest-first with a "Load more" button for `next` and renders actor (🤖 for agents), action, `fields` joined, time and, for agents, a link to the session. In `webui/app/routes/integrations-mcp.tsx`, give the edit panel **Settings | Activity** tabs (depends on T025).

**Checkpoint**: an agent with the MCP grant can manage its MCP servers end to end. Without the grant, it sees no tools.

---

## Phase 5: User Story 3 - Agent manages its own addons (Priority: P2)

**Goal**: with the Addons grant on, the agent installs, configures and uninstalls addons on itself.

**Independent test**: quickstart §3.

- [ ] T027 [US3] In `src/agents/tools/self_integrations.rs`, confirm the addon path works end to end:
  - `list_integration_kinds {family:"addon"}` returns `installed`;
  - a second `create_integration` for an installed addon returns `addon_installed` with the existing id in the message, so the agent can switch to `update_integration`;
  - `tool_category` for an addon is the addon id.

  Adjust the messages if they're unclear.
- [ ] T028 [P] [US3] In `webui/app/routes/integrations-addons.tsx`, add the same **Settings | Activity** tabs to the addon detail panel, reusing `ActivityTab`.

**Checkpoint**: addon self-management works, and people see agent-made addon changes.

---

## Phase 6: User Story 4 - Agent manages its own channel connections (Priority: P3)

**Goal**: with the Channels grant on, the agent manages its Discord/Telegram connections, and is warned when a change would cut off the conversation it is in.

**Independent test**: quickstart §4 (live Telegram token).

- [ ] T029 [P] [US4] In `src/integrations/kind.rs`, add `fn platform(&self) -> Option<ChannelPlatform> { None }` to `IntegrationKind`. Override it in `src/integrations/kinds/discord.rs` (`Some(ChannelPlatform::Discord)`) and `src/integrations/kinds/telegram.rs` (`Some(ChannelPlatform::Telegram)`).
- [ ] T030 [US4] In `src/agents/tools/self_integrations.rs`, when `update_integration` with `enabled: false` or `delete_integration` targets an integration whose `kind.platform()` equals the current session's channel platform (add a small `VizierSession`/`VizierChannelId` helper returning `Option<ChannelPlatform>` if none exists, in `src/schema/`), make the change and add `warning: "you will stop receiving messages on this connection; this reply may not be delivered"` to the output (research R7). Depends on T029.
- [ ] T031 [P] [US4] In `webui/app/routes/integrations-channels.tsx`, add **Settings | Activity** tabs to the connection editor, reusing `ActivityTab`.

**Checkpoint**: all four stories work independently.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T032 [P] Update `CLAUDE.md`:
  - the Integrations section: per-agent model, no scope, attach removed, legacy multi-linked rows, `integrations::service` as the single write path, grants and the `integration_event` record;
  - the Tools section: the `Integrations` category (14 variants), the five self-management tools contributed by the integration layer, and their absence from `DREAM_TOOL_NAMES`.
- [ ] T033 [P] Grep for leftovers: `IntegrationScope`, `scope_for_creator`, `set_attachment`, `can_attach`, `also_used_by`, `SharedEditWarning` and `created_by` uses (other than the legacy read fallback) across `src/` and `webui/app/`, and remove or adapt each one.
- [ ] T034 Run `cargo clippy`, `cargo test` and `cd webui && npm run typecheck`; fix everything.
- [ ] T035 End-to-end with a `dummyplug` agent on a running binary (`just run`): walk quickstart.md §1, §2, §3, §5 and §6, and §4 if a Telegram token is available (marked live). Record any deviations under this task.
- [ ] T036 Prepare the commit as `feat: 017 agents manage their own integrations; integrations are per-agent [**breaking**]`. The body lists the breaking changes: attach removed (`410`), `scope` removed from API views, the Addons picker removed, uninstall now deletes the configuration.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup and blocks every story.
  - T002 → T003 → T004 → T005 → T006 → T007 → T008/T009/T010.
  - The WebUI tasks T011/T012 can start once the contracts are fixed. T013 depends on T011 and T012, and T014 to T017 depend on T013.
- **US1 (Phase 3)**: after Foundational. T018 → T019 (same area); T020 after T018.
- **US2 (Phase 4)**: after Foundational. It also needs US1's grant endpoint to exercise its test (grants can be seeded through storage for development).
  - T023 → T022 → T021 (compose wiring), then T024.
  - T025 is independent; T026 depends on T025.
- **US3 (Phase 5)**: after US2 (it reuses the tools).
- **US4 (Phase 6)**: after US2 (it reuses the tools). T029 is independent; T030 depends on T029.
- **Polish**: after the desired stories.

### User story dependencies

- **US1**: independent once Foundational is done.
- **US2**: needs grants, and so US1's storage. Its tools are generic, so US3 and US4 build on them.
- **US3** and **US4**: each depends on US2 only, and they are independent of each other.

### Parallel opportunities

- Phase 2: T011 and T012 (WebUI types and services) in parallel with the Rust work, from T002 onward.
- US2: T025 (events endpoint) in parallel with T021–T023.
- US3 and US4: T028, T029 and T031 in parallel.
- Polish: T032 and T033 in parallel.

## Parallel Example: User Story 2

```text
# Rust tool work (sequential: same module graph)
T023 IntegrationTools carries the self-management set
T022 the five tools in src/agents/tools/self_integrations.rs
T021 ToolCategory::Integrations + compose wiring

# In parallel with the above
T025 GET /agents/{id}/integrations/events
```

## Implementation Strategy

### MVP first (Foundational + US1 + US2)

1. Phase 2: the per-agent model, the service and the tables. This is the largest phase and already ships value: no scope, simpler pages.
2. US1: grants and their permission gate.
3. US2: the tools, with MCP as the first family.
4. **Stop and validate**: quickstart §1, §2, §5 and §6.

### Incremental delivery

- Add US3 (addons), validate with quickstart §3.
- Add US4 (channels), validate with quickstart §4 (live).
- Polish and commit as one breaking `feat`.
