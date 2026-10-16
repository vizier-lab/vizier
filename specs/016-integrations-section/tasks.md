---

description: "Task list for 016 Integrations Section"
---

# Tasks: Integrations Section

**Input**: Design documents from `specs/016-integrations-section/`

**Prerequisites**:
- [plan.md](plan.md), [spec.md](spec.md)
- [research.md](research.md) (R1–R12), [data-model.md](data-model.md)
- contracts: [http-api.md](contracts/http-api.md), [runtime.md](contracts/runtime.md), [webui.md](contracts/webui.md)
- [quickstart.md](quickstart.md)

**Tests**: Unit tests are included where plan.md and research R11 call for them (storage, access, kinds, compose). End-to-end checks are the dummyplug steps in quickstart.md (constitution gate).

**Organization**: Tasks are grouped by user story.
- **Execution order:** US1 → US2 → US3 → US5 (US4, the upgrade migration, was dropped: no migration, spec Assumptions / research R7).
- **Removing old fields:** each story deletes the `AgentConfig` fields it replaces. Stored rows that still carry them load fine, because unknown keys are ignored.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US5 from spec.md

---

## Phase 1: Setup

**Purpose**: Module skeletons and assets that everything else builds on.

- [X] T001 Create the module skeleton `src/integrations/{mod.rs,kind.rs,access.rs,kinds/mod.rs}` and register `mod integrations;` in `src/main.rs` (or `src/lib.rs`, wherever top-level modules are declared)
- [X] T002 [P] Add embedded icons `assets/integrations/{brave_search,discord,telegram,mcp}.svg`, and embed them through the same mechanism `build.rs` / `src/catalogue/` uses for catalogue logos (no runtime file reads, Principle III)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The integration model, storage, kind trait, live-refresh plumbing, generic HTTP API and WebUI shell. Every story needs these.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

### Model and storage

- [X] T003 [P] Create `src/schema/integration.rs` with the types from data-model.md §3, and export them from `src/schema/mod.rs`:
  - `IntegrationFamily`, `IntegrationScope`, `Integration`, `IntegrationStatus`;
  - `IntegrationChanged { integration_id, family, agents }`.
- [X] T004 [P] Define the `IntegrationStorage` trait in `src/storage/integration.rs` (methods per data-model.md §3) and add it to the `VizierStorageProvider` supertrait in `src/storage/mod.rs`
- [X] T005 Add `init_integration_schema(conn)` to `src/storage/sqlite/mod.rs`:
  - tables `integration` and `integration_attachment` (FK `ON DELETE CASCADE`, index on `agent_id`), **no** `UNIQUE(kind, name)` (research R12);
  - call it from `init_schema`.
- [X] T006 Implement `IntegrationStorage for SqliteStorage` in `src/storage/sqlite/integration.rs`. Inside the write transaction, enforce:
  - channel token uniqueness per kind (`token_in_use`);
  - at most one attachment per channel integration (attach = move);
  - one connection per platform per agent (`platform_in_use_on_agent`);
  - one attached configuration per addon kind per agent (attach replaces);
  - no two attached `mcp` integrations with the same `name` on one agent (`name_in_use_on_agent`).

  Return the affected agent ids from `save`/`delete`/`set_attachment`. Model the typed errors on `VizierError` messages the HTTP layer maps to 409.
- [X] T007 Forward every `IntegrationStorage` method through `VizierStorage` in `src/storage/mod.rs`. Make `AgentStorage::delete_agent` call `delete_agent_attachments` (in `src/storage/sqlite/agent.rs`).
- [X] T008 [P] Unit tests in `src/storage/sqlite/integration.rs` (in-memory connection via `init_integration_schema`):
  - round-trip;
  - attachments and cascade on delete;
  - channel move;
  - `token_in_use`, `platform_in_use_on_agent`, `name_in_use_on_agent`;
  - addon replace-on-attach;
  - duplicate MCP names allowed on different agents.

### Kinds and access

- [X] T009 Define in `src/integrations/kind.rs`:
  - the `IntegrationKind` trait (research R4: `id`, `family`, `listing`, `validate`, `redact`, `merge_secrets`, `contribute`, `reader`);
  - `KindListing { name, description, icon, fields: Vec<KindField> }`;
  - `Contribution { native: VizierToolSet, mcp: Option<(String, Arc<McpServer>)>, addon: Option<&'static str> }`;
  - `AgentToolContext` (agent id, storage, file manager, current snapshot's MCP servers for reuse).
- [X] T010 In `src/integrations/mod.rs`, implement the `IntegrationKinds` registry (`builtin()` with the registration list, initially empty; `get(id)`, `by_family(f)`) plus shared redaction helpers (`mask_secret(&str) -> "••••<last4>"`, a `merge_secrets` default helper). Hold an `Arc<IntegrationKinds>` on `VizierDependencies` in `src/dependencies.rs`.
- [X] T011 [P] Implement `integration_access(user, integration, agent_config) -> Access { see, edit, attach }` in `src/integrations/access.rs` (research R6), plus `can_manage_on_agent(user, agent_config)` and `scope_for_creator(user)`
- [X] T012 Update `AVAILABLE_PERMISSIONS` in `src/storage/user.rs`: add `all_agents:integrations:manage` and `owned_agents:integrations:manage`, remove `agents:mcp_config`. Make the superadmin seed in `src/dependencies.rs` (`migrate_users`) include the new permissions. Don't rewrite other stored roles (no migration).
- [X] T013 [P] Unit tests for `integration_access` in `src/integrations/access.rs`:
  - all-scope;
  - owned-scope on own and on someone else's agent;
  - shared vs another owned user's integration;
  - a view-only user.

### Live refresh plumbing (contracts/runtime.md)

- [X] T014 Add an `IntegrationChanged` broadcast to `src/transport.rs` (`publish_integration_change`, `subscribe_integration_changes`), mirroring the `SessionEvent` broadcast
- [X] T015 Add `AgentCommand::RefreshIntegrations { agent_id }` and `AgentCommand::IntegrationStatus { agent_id, resp }` to `src/schema/commands.rs`
- [X] T016 Split `VizierTools::new` in `src/agents/tools/mod.rs` into:
  - `BaseTools::new` (everything that isn't an integration, unchanged);
  - `IntegrationTools { native, addons: HashMap<&'static str, VizierToolSet>, mcp }`;
  - `fn compose(base: &BaseTools, integrations: Arc<IntegrationTools>) -> VizierTools`, which builds the `ToolRouter` (default + user + integration native, addon toolsets, MCP), the `sandbox_toolset` and `categories`.

  Keep the existing Discord/Telegram/Brave/MCP wiring calls in place for now. Each story moves its own.
- [X] T017 Add `IntegrationTools::build(kinds, integrations, ctx, previous: Option<&IntegrationTools>)` in `src/integrations/mod.rs`:
  - calls each enabled, complete integration's `kind.contribute`;
  - reuses `previous`'s `Arc<McpServer>` when `(name, settings)` are equal, and calls `warm_up()` on new servers.
- [X] T018 Change `VizierAgent` in `src/agents/agent/mod.rs` to hold `base: BaseTools` and `tools: ArcSwap<VizierTools>`:
  - at construction, read `list_agent_integrations(agent_id)` and compose;
  - in `chat`, `prompt`, `dream_chat`, `generate_handover_message` and every helper that used `self.tools`, call `self.tools.load_full()` once at entry and pass the snapshot down (guarantee G2).
- [X] T019 Implement `VizierAgent::refresh_integrations()` in `src/agents/agent/mod.rs`, per contracts/runtime.md: re-read storage, `IntegrationTools::build` with the previous snapshot, compose, `store`; on error keep the old snapshot and log `warn!`.
- [X] T020 Route `RefreshIntegrations` / `IntegrationStatus` through `VizierAgents` (`src/agents/mod.rs`) and `Supervisor` (`src/agents/supervisor.rs`):
  - a running instance → call it;
  - otherwise no-op, or `Unknown` status.

  Subscribe `VizierAgents` to `subscribe_integration_changes()` and fan out `RefreshIntegrations` to each listed agent. Treat `Lagged` as "refresh all running agents".
- [X] T021 [P] Unit test in `src/integrations/mod.rs`:
  - `IntegrationTools::build` reuses the same `Arc<McpServer>` when an unrelated integration changes;
  - it replaces the server when the server's own settings change;
  - `compose` yields identical `categories` for identical inputs (G4).

### Generic HTTP API (contracts/http-api.md)

- [X] T022 Create `src/channels/http/api/v1/agents/integrations.rs` and nest it at `/{agent_id}/integrations` in `src/channels/http/api/v1/agents/mod.rs`, with these routes:
  - `GET ?family=` (visible items, `attached`, `also_used_by` with `others` count, `can_edit`/`can_attach`, redacted settings, status via `AgentCommand::IntegrationStatus`);
  - `POST` (create + attach, `scope_for_creator`, kind `validate`);
  - `PUT /{integration_id}/attachment`.

  Map storage errors to `409` with the codes in the contract (`incomplete` uses kind `listing().fields` required keys). Publish `IntegrationChanged` after commit.
- [X] T023 [P] Create `src/channels/http/api/v1/integrations.rs` and mount it at `/integrations` in `src/channels/http/api/v1/mod.rs`, with these routes:
  - `GET /{id}`;
  - `PUT /{id}` (with `merge_secrets`, `affected_agents`);
  - `DELETE /{id}` (needs `?confirm=true` when attached);
  - `GET /kinds?family=`;
  - `GET /kinds/{id}/icon` (embedded assets from T002).

  Publish `IntegrationChanged` after every write.

### WebUI shell (contracts/webui.md)

- [X] T024 [P] Add `Integration`, `IntegrationItem`, `KindListing`, `IntegrationStatus` types to `webui/app/interfaces/types.ts`
- [X] T025 [P] Create `webui/app/hooks/integrationStore.tsx`:
  - state `{agentId, family, items, kinds, statuses, loading}`;
  - actions `load`, `attach`, `detach`, `create`, `update`, `remove`, `refreshStatus`;
  - 10 s status polling while a page is mounted.
- [X] T026 Update `webui/app/routes.ts`:
  - add `:agentId/integrations/channels`, `:agentId/integrations/addons`, `:agentId/integrations/mcp`;
  - serve `routes/agent-settings.tsx` at `:agentId/config`;
  - make `:agentId/settings` a redirect to `:agentId/config`.

  Create placeholder route files `webui/app/routes/integrations-{channels,addons,mcp}.tsx`.
- [X] T027 Rework the sidebar in `webui/app/layout.tsx`:
  - per-agent list: Home, Chat, Core, Memory, Tasks, Skills, Dreams;
  - an "Integrations" `nav-section` heading with Channels, Addons, MCP;
  - a "Misc" heading with Usage and Config.

  Disable these with no agent selected. Config keeps the `canEditAgent` rule. Update `getCurrentView()` for the new paths. Add the section-heading style (collapsing to a divider under `.collapsed`) to `webui/app/app.css`.
- [X] T028 [P] Update `webui/app/components/RolesSection.tsx`: list "Manage integrations (all agents)" / "Manage integrations (own agents)" and drop "Configure Agent MCP"

**Checkpoint**: storage, kinds registry, live refresh, generic API and WebUI shell are in place. Stories can begin.

---

## Phase 3: User Story 1 — MCP servers in one place, attached live (Priority: P1) 🎯 MVP

**Goal**: Add, edit, attach and detach MCP servers from Integrations › MCP for the selected agent. Agents pick up changes on their next turn, with no restart.

**Independent Test**: quickstart.md §2 (dummyplug `dp-a`/`dp-b`: attach → `list_tool_functions` shows `fs_*`; an edit mid-turn doesn't abort the turn; detach removes the tools; health shows no restart).

- [X] T029 [P] [US1] Implement `McpKind` in `src/integrations/kinds/mcp.rs`:
  - settings `{transport: local|http, command, args, env | uri}` mapped to the existing `McpClientConfig`;
  - `validate` checks the name pattern `^[a-z0-9][a-z0-9_-]{0,47}$` and that a command or uri is present;
  - `redact` / `merge_secrets` mask every `env` value;
  - `contribute` returns `mcp: Some((name, McpServer))`, reusing the server via `AgentToolContext`.

  Register it in `IntegrationKinds::builtin()`.
- [X] T030 [P] [US1] Add `McpServer::status() -> McpStatus { state, tools, last_error }` in `src/agents/mcp/mod.rs`, read from `ServerState` without connecting (`try_lock`; report `Connecting` if it's busy)
- [X] T031 [US1] Answer `IntegrationStatus` for MCP entries from the current snapshot's `McpServer::status()` in `src/agents/agent/mod.rs`
- [X] T032 [US1] Remove the `tools.mcp_servers` loop from `BaseTools::new` in `src/agents/tools/mod.rs`. Delete `mcp_servers` from `AgentToolsConfig` in `src/schema/agent.rs`. Fix the users:
  - the `dummyplug.rs` test fixture;
  - the `api/v1/agents/mod.rs` sanitising;
  - the agent `PUT` handling.
- [X] T033 [US1] Delete `migrate_agent_tools` and its call in `src/dependencies.rs`. Its only effect is copying the legacy global `mcp_servers` into the removed per-agent field (research R7). Also remove `GlobalConfigValue::McpServers` in `src/schema/` if nothing else reads it.
- [X] T034 [P] [US1] Unit tests for `McpKind` in `src/integrations/kinds/mcp.rs`: validate (bad name, missing command), redact masks env, merge keeps unchanged env values and replaces sent ones
- [X] T035 [US1] Build the MCP page in `webui/app/routes/integrations-mcp.tsx`, per contracts/webui.md and the mockup:
  - **Table:** server, status (Connected / Unreachable with error / Not in use), also used by, and an On-for-agent switch (attach/detach).
  - **"+ Add server" form:** name, then local (command, args, env) or http (uri).
  - **Edit panel:** env values redacted with Replace buttons; the shared-edit warning when `also_used_by` isn't empty; read-only when `!can_edit`.
  - **Delete:** confirmation when the server is attached.
  - **Errors:** `409 name_in_use_on_agent` shows a toast naming the conflicting server.
- [X] T036 [US1] Run quickstart.md §2 steps 1–9 against `just run` with two dummyplug agents. Record the results in `specs/016-integrations-section/pr-notes.md`.

**Checkpoint**: MCP is fully managed through Integrations, live, per agent.

---

## Phase 4: User Story 2 — Discord / Telegram from Channels (Priority: P1)

**Goal**: Bots are channel integrations, run by a process-wide manager. Token changes, disabling and moving touch only that bot, and the agent never restarts.

**Independent Test**: quickstart.md §4: a failed token leaves the agent running; moving a bot changes who answers; an agent `PUT` doesn't reconnect the bot. Steps 1, 3 and 4 need real tokens; the offline variant uses a dummy token.

- [X] T037 [P] [US2] Implement `DiscordKind` in `src/integrations/kinds/discord.rs`:
  - settings `{token}`, with `redact` / `merge_secrets` on `token`;
  - `contribute` = the six tools from `new_discord_tools` (`src/agents/tools/discord/mod.rs`);
  - `reader` = `DiscordChannelReader::new(agent_id, token, deps)`.

  Register it.
- [X] T038 [P] [US2] Implement `TelegramKind` in `src/integrations/kinds/telegram.rs` the same way (three tools from `new_telegram_tools`, `TelegramChannelReader`). Register it.
- [X] T039 [US2] Create `ChannelConnections` in `src/channels/connections.rs`:
  - `reconcile()` per contracts/runtime.md (desired = enabled channel integrations with one attachment), keyed by integration id, with a token fingerprint (sha256 prefix via an existing hashing dep, or a plain string compare kept in memory);
  - `health_for(agent_id)` and `status(integration_id)`;
  - `shutdown_all()`.

  Reduce `src/channels/readers.rs` to the per-reader `supervise` loop plus the health types, reused by `connections.rs`. `FatalChannelError` → `Failed` until the token changes.
- [X] T040 [US2] Own and start `ChannelConnections` in `VizierChannels` (`src/channels/mod.rs`):
  - a full `reconcile()` at start;
  - subscribe to `subscribe_integration_changes()` and reconcile on `Channel`-family events or `Lagged`;
  - `shutdown_all()` on process stop.

  Put `ChannelConnections` (an `Arc`) on `VizierDependencies` or in the transport, so the agent health check can read it.
- [X] T041 [US2] Remove the `ChannelReaders::start` / shutdown calls from `agent_process` in `src/agents/process.rs`. Fill `AgentHealthStatus.channels` from `ChannelConnections::health_for` in `src/agents/supervisor.rs` (the shape is unchanged, so `vizier agent ps` keeps working). Answer channel entries of `IntegrationStatus` from `ChannelConnections::status`.
- [X] T042 [US2] Remove the Discord/Telegram wiring from `BaseTools::new` in `src/agents/tools/mod.rs`. Delete these fields and fix their users (`dummyplug.rs` fixture, agent API, `channels/readers.rs` leftovers):
  - `discord_token`, `telegram_token` from `AgentConfig`;
  - `tools.discord`, `tools.telegram` from `AgentToolsConfig`, in `src/schema/agent.rs`.
- [X] T043 [P] [US2] Unit tests in `src/integrations/kinds/discord.rs` and `telegram.rs`: token redaction and merge. Also a `reconcile()` decision test in `src/channels/connections.rs`, using a fake reader through the `VizierChannel` trait: start, stop on disable/unassign, restart on token or agent change, leave an unchanged failed reader alone.
- [X] T044 [US2] Build the Channels page in `webui/app/routes/integrations-channels.tsx`:
  - one card per connection attached to the agent: platform, bot name or handle when known, status, and the error box when failed;
  - Replace token, Disable/Enable, "Move to agent…" (agent picker → attachment on the target agent), and Delete;
  - a "Connect a bot" card offering only the platforms the agent doesn't have yet (new token, or take over an unassigned connection);
  - a toast on `token_in_use` / `platform_in_use_on_agent`.
- [X] T045 [US2] Run quickstart.md §4: the offline variant always, the real-token steps when tokens are available. Record the results in `pr-notes.md`.

**Checkpoint**: Bots live under Integrations and are independent of agent restarts.

---

## Phase 5: User Story 3 — Brave Search as an addon (Priority: P2)

**Goal**: Brave Search is an installable addon with namespaced tools (`brave_search_web`, `brave_search_news`, category `brave_search`), presented as an app-store catalogue.

**Independent Test**: quickstart.md §3 (dummyplug: install on `dp-a` → `list_tool_functions {"category":"brave_search"}` lists both tools, `dp-b` has none; an incomplete install returns 409; a key-preserving edit).

- [X] T046 [P] [US3] Rename the Brave tools in `src/agents/tools/brave_search/mod.rs`: `NAME` `web_search` → `brave_search_web`, `news_search` → `brave_search_news`; `category()` → `None`. Record auth failures (401/403) and successes in `IntegrationHealth`, via an optional integration id passed at construction.
- [X] T047 [P] [US3] Add `IntegrationHealth` (a `RwLock<HashMap<String, String>>` with `fail`, `clear`, `get`) on `VizierDependencies` in `src/dependencies.rs`
- [X] T048 [US3] Implement `BraveSearchKind` in `src/integrations/kinds/brave_search.rs`:
  - settings `{api_key, safesearch}`, with `api_key` required and secret;
  - `listing` = Brave Search, "Search the web", icon `brave_search`, fields;
  - `contribute` = both tools with `addon: Some("brave_search")`.

  Register it.
- [X] T049 [US3] Addon categories in `src/agents/tools/mod.rs`:
  - `compose` lists the addon toolsets' tools under the addon id as category, described by `KindListing.description`;
  - `mcp_category_name` also reserves addon ids (`brave_search` → `brave_search_mcp`);
  - `assign_names` reserves addon tool names with the native ones.

  Update `every_tool_is_in_its_category` / `every_tool()` and add tests: an MCP server named `brave_search` gets `brave_search_mcp`, and every addon tool is named `<addon id>_…`.
- [X] T050 [US3] Remove the Brave wiring from `BaseTools::new` and delete `tools.brave_search` from `AgentToolsConfig` in `src/schema/agent.rs`. Remove `BraveSearchConfig`'s per-agent use and keep the struct only if the kind uses it (`src/config/tools/mod.rs`).
- [X] T051 [P] [US3] Update the display labels for the renamed tools: `src/utils/mod.rs` (`web_search`/`news_search` → `brave_search_web`/`brave_search_news`) and `webui/app/routes/chat.tsx` (the `case 'web_search'` / `'news_search'` labels)
- [X] T052 [US3] Answer addon entries of `IntegrationStatus` in `src/agents/agent/mod.rs`: `Incomplete` from the kind's required fields, `Failed` from `IntegrationHealth`, otherwise `Running`
- [X] T053 [US3] Build the Addons page in `webui/app/routes/integrations-addons.tsx`, per the mockup:
  - a search box;
  - a tile grid from `GET /integrations/kinds?family=addon` (icon from `/kinds/{id}/icon`, name, description, an Installed ✓ / Get badge for this agent);
  - a "more addons" placeholder tile;
  - a details panel: Install on <agent> / Uninstall from <agent>, description, shared settings (secret field with Replace, other fields from `fields`), and "also installed on".

  Install on an incomplete addon (409 `incomplete`) opens the settings form first, then creates or updates the integration and attaches it. When several configurations of one addon exist, show a configuration picker in the panel.
- [X] T054 [US3] Run quickstart.md §3 (steps 1–4, 4a, 4b, 6; step 5 needs network and is marked). Record the results in `pr-notes.md`.

**Checkpoint**: Addons catalogue works, with namespaced tools.

---

## Phase 6: User Story 5 — Misc › Config shows built-in tools only (Priority: P3)

**Goal**: The agent form no longer edits integrations and lives at Misc › Config, and the agent API refuses the moved fields.

**Independent Test**: quickstart.md §6: Config has no token, Brave or MCP sections; `/:agentId/settings` redirects; the sidebar order is right. quickstart.md §1 and §5's last row: the agent API returns 422 for moved fields.

- [X] T055 [US5] Remove these sections from `webui/app/components/AgentForm.tsx` and drop the related form state (`discord_token`, `telegram_token`, `tools.discord`/`telegram`/`brave_search`/`mcp_servers`, MCP form state):
  - "Channel Tokens";
  - the Discord/Telegram entries of "Enabled Tools";
  - "Brave Search";
  - "MCP Servers".

  Update the matching fields in `webui/app/interfaces/types.ts`.
- [X] T056 [US5] Retitle `webui/app/routes/agent-settings.tsx` as "Config" (header, document title); check the create-agent flow (`webui/app/routes/agent-new.tsx`) no longer sends the removed fields
- [X] T057 [US5] Make `POST /agents` and `PUT /agents/{id}` in `src/channels/http/api/v1/agents/mod.rs` reject a raw body containing `discord_token`, `telegram_token`, `tools.discord`, `tools.telegram`, `tools.brave_search` or `tools.mcp_servers` with `422 {"error":"moved_to_integrations","fields":[...]}`. Inspect the raw JSON before typed deserialisation.
- [X] T058 [US5] `cd webui && npm run typecheck`, then walk quickstart.md §6 in the browser. Record the results in `pr-notes.md`.

**Checkpoint**: All user stories are complete.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T059 [P] Update `CLAUDE.md` architecture notes:
  - **Tools:** integration tools come from `IntegrationKind`, `BaseTools` + `compose` + `ArcSwap`; addon namespacing; Brave tool names.
  - **Channels:** `ChannelConnections` replaces per-agent readers.
  - **Storage:** `IntegrationStorage`, two tables.
  - **Config layering:** tokens, Brave and MCP are no longer on `AgentConfig`.
  - **Permissions.**

  Also update the "Adding a new tool" note to cover addons.
- [X] T060 [P] Write `specs/016-integrations-section/pr-description.md`. List the `[**breaking**]` items:
  - the agent API rejects the moved fields;
  - Brave tools renamed with no alias;
  - `agents:mcp_config` removed;
  - no upgrade migration: bots, Brave keys and MCP servers must be re-entered under Integrations, and roles other than superadmin need the new permissions granted.

  List the behaviour changes:
  - a token with its tools off now gets the tools;
  - an agent update no longer reconnects bots.
- [X] T061 `cargo clippy`, `cargo test`, `cd webui && npm run typecheck`. Fix everything.
- [X] T062 End-to-end check with dummyplug agents on a running binary (constitution e2e gate): walk all of quickstart.md §1–§6 in one session on a fresh build, plus a 014 regression (`tools` lists the 4 code-mode tools; `execute_python` `memory_list(limit=1)` → `ok: true`)

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup. Blocks all stories.
- **US1 (Phase 3)**: Foundational only. This is the MVP.
- **US2 (Phase 4)**: Foundational only. Independent of US1.
- **US3 (Phase 5)**: Foundational only.
- **US5 (Phase 6)**: after US1, US2 and US3, because their removals of `AgentConfig` fields change `types.ts`, and AgentForm must drop the same fields.
- **Polish (Phase 7)**: after all stories.

### Within stories

- The kind implementation (`[P]`) comes before the field removal (it needs the kind wiring in place), which comes before the page and then the quickstart run.
- T016 → T017 → T018 → T019 → T020 are sequential (same agent/tools files).
- T022 and T023 are independent files and can run in parallel after T006, T010, T011 and T014.

### Parallel opportunities

- **Setup:** T001 and T002.
- **Foundational:** T003, T004, T011 and T013 together; T008 after T006; T024, T025 and T028 (WebUI) alongside the Rust work.
- **US1:** T029, T030 and T034 together.
- **US2:** T037, T038 and T043 together.
- **US3:** T046, T047 and T051 together.
- **Across stories:** after Foundational, US1, US2 and US3 can proceed in parallel. They touch different kind files. Each edits `BaseTools::new` in `src/agents/tools/mod.rs`, so serialise those three removal tasks: T032, then T042, then T050.

## Parallel Example: User Story 1

```text
Task: "T029 [P] [US1] Implement McpKind in src/integrations/kinds/mcp.rs"
Task: "T030 [P] [US1] Add McpServer::status() in src/agents/mcp/mod.rs"
Task: "T034 [P] [US1] Unit tests for McpKind in src/integrations/kinds/mcp.rs"   # after T029's types exist
```

## Implementation Strategy

### MVP (US1 only)

1. Setup and Foundational (T001–T028).
2. US1 (T029–T036): MCP managed per agent, live.
3. **Stop and validate:** quickstart.md §2 on dummyplug. This proves the whole live-refresh design (G1–G5).

### Incremental delivery

1. US2: channels and `ChannelConnections` (the second runtime path, G6–G8).
2. US3: Addons catalogue and namespaced Brave tools.
3. US5: Config clean-up and the agent API's 422.
4. Polish: docs, PR notes, full e2e.

### Notes

- Commit per task or per logical group with conventional commits (`feat:`, `refactor:`, `doc:`). Mark the agent API, Brave rename, permission removal and old-field removal commits `[**breaking**]`.
- Never call `unwrap()` / `expect()` outside tests. Log with `tracing`.
