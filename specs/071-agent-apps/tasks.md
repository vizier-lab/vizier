---
description: "Task list for 071 Agent Apps"
---

# Tasks: Agent Apps

**Input**: Design documents from `specs/071-agent-apps/`: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md) (D1–D17), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: These are included. The plan (research D17) and the constitution's end-to-end gate require them: unit tests for the pure rules, `vizier-http/tests/apps.rs` for the routes, dummyplug cases in `crates/vizier-core/tests/dummyplug_e2e.rs`, and vitest in `webui/` and `design/`. Every check in quickstart.md marked **(T)** maps to a test task below.

**Organization**: One phase per user story, in priority order. All paths are from the repository root.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[USn]**: the user story the task serves (spec.md)

---

## Phase 1: Setup (shared infrastructure)

**Purpose**: The flag, the runtime switch, and the designs still to draw.

- [ ] T001 Run `just checkout 071-agent-apps` so `landing` and `dummyplug` get the matching branch (constitution: matching submodule branches)
- [ ] T002 Add the Cargo feature `subapp` (ungrouped, no dependency) to `crates/vizier-core/Cargo.toml` and list it in `full`. Add the matching `subapp` path entry and flag-list entry to the `changes` job and the per-flag matrix in `.github/workflows/ci.yml`. Add a `subapp` row to the table in `docs/src/configuration/build-flavours.md`
- [ ] T003 [P] Create `crates/vizier-core/src/apps/mod.rs` with `#[cfg(feature = "subapp")]` empty submodules `manifest`, `database`, `requirements`, `context`, `source`, `tools`, `functions`, `schedules`, `files`, `frame`, `events`, `access`, `limits`. Register them in `crates/vizier-core/src/lib.rs`
- [ ] T004 Add `RuntimeOptions.apps: bool` (default `false`) in `crates/vizier-core/src/runtime.rs`. Without the feature it is ignored. With the feature and `true`, `VizierRuntime::start` refuses a non-SQLite backend with "apps need the SQLite backend" (research D1, D3)
- [ ] T005 Set `RuntimeOptions { apps: true, .. }` in `app/src-tauri/src/engine.rs` for both the window runtime and the web-access server. Confirm `vizier-core/full` reaches `subapp` through `app/src-tauri/Cargo.toml`
- [ ] T006 [P] FigCraft pass (constitution, design first) in `Vizier shell.figcraft` › `Shell · chosen`: draw the "from <app>" mark on a person's chat bubble (FR-021a), the version history list (FR-017), and the tables + row counts + download view (FR-018b). Save, and add the frame names to `specs/071-agent-apps/spec.md` › Context

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: Storage, the channel kind, the service core, the per-turn seam, the frame lookup and the reusable chat. No user story can start before this phase is done.

### Engine

- [ ] T007 Add `VizierChannelId::App(String /*user*/, String /*app_id*/)` in `crates/vizier-core/src/schema/session.rs`: slug `app__{user}__{id}`, `from_web` for wire id `app__<id>`, `web_user`, `web_channel_id`, new `app_id()`, `usage_place` `app:{id}`, `usage_activity` → `UsageActivity::App` (research D2)
- [ ] T008 [P] Add `UsageActivity::App` (label "Apps") in `crates/vizier-core/src/schema/usage.rs` (`ALL` length, `as_str`, `label`), `SlugKind::App` and its labels in `crates/vizier-core/src/usage/mod.rs`, and `app: 'Apps'` in `webui/app/lib/usage.ts`
- [ ] T009 [P] Handle `App` in the remaining channel match sites:
  - `crates/vizier-core/src/service/conversations.rs` `kind_of` → `ConversationKind::App` ("App: <name>")
  - `crates/vizier-core/src/service/topics.rs` `readable_channel`
  - the span kind and `vizier.app.id` in `crates/vizier-core/src/agents/agent/mod.rs`
  - `crates/vizier-core/src/ui_pages/verify.rs` (the responder accepts `App`)
  - `crates/vizier-core/src/agents/tools/ui_pages.rs` `offered_in`: no `ui_render` in `App`
  - `crates/vizier-core/src/service/shares.rs` (App conversations not shareable)
- [ ] T010 [P] Define the `AppStorage` trait in `crates/vizier-core/src/storage/app.rs`: app CRUD, versions, approvals, schedules, schedule runs, lock. Define the `RailOrderStorage` trait in `crates/vizier-core/src/storage/rail.rs`. Add both to `VizierStorageProvider` under `cfg(feature = "subapp")`
- [ ] T011 Implement `init_app_schema` and both traits on `SqlStorage` in `crates/vizier-core/src/storage/sqlite/app.rs`, with the tables `app`, `app_version`, `app_approval`, `app_schedule`, `app_schedule_run` and `rail_order` per [data-model.md](data-model.md). Call it from `SqliteStorage::init_schema` in `crates/vizier-core/src/storage/sqlite/mod.rs`
- [ ] T012 [P] Unit tests for `storage/sqlite/app.rs` (create/read/version append/stale base/delete cascade/lock column) in `crates/vizier-core/src/storage/sqlite/app_tests.rs`
- [ ] T013 Implement `apps::database` in `crates/vizier-core/src/apps/database.rs` (research D3, data-model):
  - open `{workspace}/apps/{app_id}/app.sqlite` with WAL, `busy_timeout`, `trusted_schema=OFF`
  - a `rusqlite` authorizer denying `ATTACH`, `load_extension`, `PRAGMA` writes, DDL outside migrations and writes to `_vizier_*`
  - `query(sql, params, mode, limits)` with time and row limits
  - `migrate(new: &[Migration])` in one transaction recording `_vizier_migrations`
  - `snapshot_to(path)` (`VACUUM INTO`)
- [ ] T014 [P] Unit tests for `apps/database.rs`: DDL refused, ATTACH refused, `_vizier_*` write refused, recursive CTE times out, row limit, migration rollback on failure, append-only check. In `crates/vizier-core/src/apps/database_tests.rs`
- [ ] T015 [P] Implement `apps::manifest` in `crates/vizier-core/src/apps/manifest.rs` per [contracts/manifest.md](contracts/manifest.md):
  - the types
  - `ManifestPatch` application
  - validation reporting all problems at once
  - functions through `custom_tools::validate::validate_definition`, with `calls` derived by `custom_tools::calls::parse_calls`
  - limits
  - the append-only migration rule
  - the first-version name and icon rule
- [ ] T016 [P] Unit tests for `apps/manifest.rs` (each rule in contracts/manifest.md) in `crates/vizier-core/src/apps/manifest_tests.rs`
- [ ] T017 [P] Implement `apps::limits` and `apps::access` in `crates/vizier-core/src/apps/limits.rs` and `crates/vizier-core/src/apps/access.rs`: owner-only; `can_drive(owner, agent)` via `access::can_view_agent` plus the chat right
- [ ] T018 Implement the `Apps` service core in `crates/vizier-core/src/apps/mod.rs`:
  - `create_draft`, `get`, `list_for`, `save_version` (page blob `apps/{id}/pages/{sha}.html` + manifest + migrations in one transaction; re-checks `locked`), `set_current`, `remove`, `set_lock`
  - the `last_topic` handling
- [ ] T019 [P] Define `AppEvent` (`DataChanged`, `VersionSaved`, `DriverChanged`, `BadgeChanged`, `LockChanged`) in `crates/vizier-core/src/apps/events.rs`. Add a bus pump arm for `app.{id}.events` in `crates/vizier-core/src/transport/events.rs`
- [ ] T020 Build `Apps` on `VizierDependencies` (`crates/vizier-core/src/dependencies.rs`) only when `RuntimeOptions.apps` is set. Add a startup sweep of drafts older than a day with no history row, and mark leftover `running` schedule runs `failed: interrupted`
- [ ] T021 [P] Extract a `FrameSource` trait into `crates/vizier-core/src/ui_pages/frame_source.rs`, implemented by `UiPages` (`pg_` ids). Implement it for `Apps` (`ap_` ids) in `crates/vizier-core/src/apps/frame.rs` (research D5)
- [ ] T022 Add `ToolContext.app: Option<Arc<AppTurnContext>>` in `crates/vizier-core/src/agents/tools/mod.rs`. Set it in `VizierAgent::chat` (`crates/vizier-core/src/agents/agent/mod.rs`), in `crates/vizier-core/src/agents/agent/side_chat.rs` and in the detached copies
- [ ] T023 Implement `apps::context::for_turn` in `crates/vizier-core/src/apps/context.rs`. It loads the app, re-checks `can_drive`, the manifest, the table list with row counts, the lock and `tab_open`. The requirement report is filled in by US4; until then it is `Ready`
- [ ] T024 Implement `AppSource` (id `app`) in `crates/vizier-core/src/apps/source.rs`. Register it in `sources()` in `crates/vizier-core/src/agents/tools/turn_source.rs` (array of 4). Append `ToolCategory::Apps` at the end in `crates/vizier-core/src/agents/tools/mod.rs` and extend `every_tool_is_in_its_category`
- [ ] T025 [P] Add `system_prompt/app.rs` (the stable section) and the `## App state` section of `context_md_with` in `crates/vizier-core/src/agents/agent/system_prompt/` (contracts/app-tools.md, "What the driver sees")
- [ ] T026 [P] Add `service/apps.rs` in `crates/vizier-core/src/service/apps.rs`: owner checks, `ServiceError` mapping, `404` for apps the caller does not own

### HTTP

- [ ] T027 Mount `/api/v1/apps*` and `/api/v1/me/rail-order` only when the runtime has apps on, and add `GET /api/v1/utils/capabilities` → `{"apps": bool}`, in `crates/vizier-http/src/api/v1/mod.rs`, `crates/vizier-http/src/api/v1/apps.rs` and `crates/vizier-http/src/api/v1/utils.rs`
- [ ] T028 Resolve `/ui-frames/{ticket}` through `FrameSource` in `crates/vizier-http/src/ui_frame.rs`. Add boot data `app: {id, abilities, conversation}` for app frames
- [ ] T029 [P] Create the test harness `crates/vizier-http/tests/apps.rs` (apps runtime on a temp workspace, network router + dispatcher, fixture owner and second person). Add the SC-009 test: router with `apps: false` has no `/apps` route, no `apps` capability and no app tool

### WebUI and design

- [ ] T030 [P] Add the `capabilities` endpoint client and `useCapabilities()` in `webui/app/api/endpoints/utils.ts` and `webui/app/stores/capabilities.ts`. App surfaces render only when `apps` is `true`
- [ ] T031 Extract `ChatView({agentId, channel, topicId, header, onNavigate, chrome})` from `webui/app/routes/chat.tsx` into `webui/app/screens/chat/ChatView.tsx`, keeping `routes/chat.tsx` a thin route. Move `ConversationHeader`'s hard-coded `/${agentId}/chat/...` navigation behind `onNavigate`. Add the channel to `topicKey` in `webui/app/stores/connection.ts` (research D13, FR-011)
- [ ] T032 [P] Create the `SplitInsets` design component (two insets, a keyboard separator with `aria-orientation`, min sizes, double-click reset, 16 px gap) with a story in `design/src/components/SplitInsets/`, and export it from `design/src/index.ts`
- [ ] T033 [P] Generalise the rail tile type to `RailItem = {kind:'agent',…} | {kind:'app', appId, name, icon?, badge?, draft?, status?, locked?}` in `design/src/components/Shell/Rail.tsx`. App tiles are an `Icon` in a tile, with a dashed monogram for drafts and for unknown icons. Update the stories
- [ ] T034 [P] Create the `AppHeader` design component (icon, name, subline, lock button, history, ⋯) with stories in `design/src/components/AppHeader/`
- [ ] T035 Add the routes `/apps/:appId` and `/apps/:appId/:topicId` in `webui/app/routes.ts` and `webui/app/routes/app.tsx`. Exclude `apps` in `AppShell.routeAgent`, add `viewOf` → `app` in `webui/app/screens/shell/pages.ts`, and teach the chat-route regex in `webui/app/layouts/AppShell.tsx` and `MobileNavDrawer.tsx` about `^/apps/`
- [ ] T036 Generalise `SandboxedPage` in `webui/app/screens/chat/client/SandboxedPage.tsx` to take a `ticket()` function and an `abilities` handler object. Keep spec 063's chat card working unchanged

**Checkpoint**: an apps-enabled desktop runtime starts; the server has no app surfaces; the chat works through `ChatView`.

---

## Phase 3: User Story 1 - Create an app (Priority: P1) 🎯 MVP

**Goal**: + → New app opens a draft that becomes an app on its first version, and a chat page can be added to the rail.

**Independent Test**: quickstart §1. Press + → New app and send "a habit tracker": the draft tile becomes a named app. Add a chat page to the rail: it appears, and deleting the chat leaves it.

### Tests for User Story 1

- [ ] T037 [P] [US1] Route tests in `crates/vizier-http/tests/apps.rs`:
  - `POST /apps` creates a draft;
  - `DELETE ?if_draft_unused=1` works, then `409` once a request exists;
  - `POST /apps/from-page` copies a ui_page version and refuses a conversation the caller does not own;
  - another person's app is `404`.
- [ ] T038 [P] [US1] dummyplug case `apps_create` in `crates/vizier-core/tests/dummyplug_e2e.rs`:
  - in a draft's conversation, `context` lists `apps` and `## App state` shows `draft`;
  - `app_edit` with page, name, icon and a migration saves v1;
  - a first version without a name or icon is `invalid`;
  - an ordinary chat gets `NameError` for `app_query`.

### Implementation for User Story 1

- [ ] T039 [US1] Implement `app_edit` (create path: v1 from `page`, name/icon/description, `ManifestPatch`, migrations through `apps::database::migrate`, verification through `ui_pages::verify` on the app session) in `crates/vizier-core/src/apps/tools.rs`. Offer it from `AppSource`. Add its `guide()` from `crates/vizier-core/assets/app-kit/GUIDE.md` (first draft: app model and v1 example)
- [ ] T040 [US1] Implement `POST /apps`, `POST /apps/from-page`, `GET /apps`, `GET /apps/{id}`, `PATCH /apps/{id}` (name, icon, description) and `DELETE /apps/{id}?if_draft_unused` in `crates/vizier-http/src/api/v1/apps.rs` over `service/apps.rs`
- [ ] T041 [P] [US1] Add the apps API client and store in `webui/app/api/endpoints/apps.ts` and `webui/app/stores/apps.ts` (list, detail, create, from-page, delete, events). Regenerate `webui/app/api/schema.d.ts` with `scripts/openapi.mjs`
- [ ] T042 [US1] Add app tiles to the rail in `webui/app/screens/shell/ShellRail.tsx`. They are appended after the agents until US7 adds the server order. The + becomes a `Floating` + `Menu` (New agent / New app, frame H) through `Rail`'s `onCreate` when `apps` is on; otherwise the current `addHref` is unchanged
- [ ] T043 [US1] Build the draft view in `webui/app/screens/apps/DraftEmpty.tsx` (one line "Tell <driver> what to build", composer focused, frame I). Discard on leave when unused, in `webui/app/routes/app.tsx`, through the pure rule in `webui/app/lib/apps/draft.ts` (+ vitest)
- [ ] T044 [US1] Add **Add to rail** to the page card header and its confirm popover (frame B) in `webui/app/screens/chat/UiPages.tsx`, desktop with `apps` only

**Checkpoint**: apps can be created both ways and show on the rail.

---

## Phase 4: User Story 2 - Use the app on its own (Priority: P1)

**Goal**: The app fills the main panel, saves its data with no model call, and sits beside a resizable chat inset.

**Independent Test**: quickstart §2. Insert from the page, restart, and the data is still there with no `usage_event`. Resize and reset the inset.

### Tests for User Story 2

- [ ] T045 [P] [US2] Route tests for `POST /apps/{id}/db/query` in `crates/vizier-http/tests/apps.rs`:
  - write → `changes: 1` plus an `app_data_changed` frame, and no usage row;
  - DDL, ATTACH and `_vizier_*` writes refused;
  - timeout and row limit.
- [ ] T046 [P] [US2] Vitest for the new runtime abilities' message parsing (`db.*`, `conversation`) in `webui/app/lib/uiPage/host.test.ts`

### Implementation for User Story 2

- [ ] T047 [US2] Implement `POST /apps/{id}/db/query` with the page limits, publishing `AppEvent::DataChanged`, in `crates/vizier-http/src/api/v1/apps.rs` and `crates/vizier-core/src/service/apps.rs`
- [ ] T048 [US2] Relay `app_data_changed`, `app_version_saved` and `app_lock` frames to sockets open on `app__…` channels in `crates/vizier-http/src/api/v1/agents/channel.rs`
- [ ] T049 [US2] Add the `db` and `conversation` abilities (`vizier.db.query/exec/onChange`, `vizier.conversation.id/onChange`) and the `db.changed` / `conversation` host messages in `webui/app/kit/runtime.ts` and `webui/app/lib/uiPage/host.ts` (`parseFrameMessage`), per [contracts/frame-protocol.md](contracts/frame-protocol.md)
- [ ] T050 [US2] Build `AppView` in `webui/app/screens/apps/AppView.tsx`: `AppHeader` + the `SandboxedPage` with app abilities + `SplitInsets` with `ChatView` on the `app__<id>` channel. The width is persisted per app in localStorage (`vizier.app.chatWidth.{appId}`). It reloads on `app_version_saved`. Mount it from `webui/app/routes/app.tsx`
- [ ] T051 [P] [US2] Phone layout: below `lg`, the app takes the screen and the chat opens as a bottom `SlideOver`, in `webui/app/screens/apps/AppView.tsx` (FR-010)

**Checkpoint**: an app is usable with persistent data and the ordinary chat beside it.

---

## Phase 5: User Story 3 - Change the app by talking to its driver (Priority: P1)

**Goal**: The driver reads and edits the app with versions, verification and rollback, across several conversations. It can wake them. Replies notify. The owner can lock.

**Independent Test**: quickstart §3, §3a, §3b, §3c, §3d.

### Tests for User Story 3

- [ ] T052 [P] [US3] dummyplug case `apps_change` in `crates/vizier-core/tests/dummyplug_e2e.rs`:
  - edits make v2;
  - `stale_base`;
  - a migration appends and a failing migration saves nothing;
  - rollback keeps the data;
  - `app_read` page and diff;
  - `describe_tool_function("app_edit")` has a guide;
  - `ui_describe_elements` is present and `ui_render` absent in app conversations;
  - no `apps` category in a dream or a scheduled task.
- [ ] T053 [P] [US3] dummyplug case `apps_conversations_and_wake` in `crates/vizier-core/tests/dummyplug_e2e.rs`:
  - two topics share the data;
  - the driver's scheduled task wakes an app conversation and the app tools are there;
  - a wake from a non-driver is `not_wakeable`.
- [ ] T054 [P] [US3] dummyplug case `apps_lock` and route tests in `crates/vizier-core/tests/dummyplug_e2e.rs` and `crates/vizier-http/tests/apps.rs`:
  - locked → `app_edit` absent, with read/query/functions still working;
  - an in-flight save is refused `locked`;
  - rollback works while locked;
  - a draft can't be locked (`draft_cannot_lock`).
- [ ] T055 [P] [US3] Notification tests in `crates/vizier-core/src/notifications/mod.rs` tests:
  - an app-conversation reply → `chat_reply` with `href` `/apps/{id}/{topic}`;
  - `fill_titles` gives `<app> · <conversation>`.

### Implementation for User Story 3

- [ ] T056 [US3] Complete `app_edit` for later versions (edits against `base_version` via `ui_pages::edits::apply_edits`, `stale_base`, `rewrite`, `migration_failed`, `too_large`, the `locked` refusal) in `crates/vizier-core/src/apps/tools.rs` and `crates/vizier-core/src/apps/mod.rs`
- [ ] T057 [P] [US3] Implement `app_read` (version, part, function, `diff_from` unified diff using the existing `storage/diff.rs`) in `crates/vizier-core/src/apps/tools.rs`
- [ ] T058 [P] [US3] Implement `app_query` (driver limits) and `app_functions` (list only, availability filled in by US4/US6) in `crates/vizier-core/src/apps/tools.rs`
- [ ] T059 [US3] Write the full `crates/vizier-core/assets/app-kit/GUIDE.md` (app model, abilities, migrations, functions and approvals, schedules, page tools, limits, worked example). Offer `ui_describe_elements` and `ui_examples` in `App` sessions in `crates/vizier-core/src/agents/tools/ui_kit.rs`. Extend `scripts/ui-kit-reference.test.mjs` to compare GUIDE.md's ability list with `webui/app/kit/runtime.ts`
- [ ] T060 [US3] Version history and rollback: `GET /apps/{id}/versions`, `POST /apps/{id}/current` in `crates/vizier-http/src/api/v1/apps.rs`. Add `webui/app/screens/apps/VersionHistory.tsx` (drawn in T006) behind the header's history button, with a confirm dialog
- [ ] T061 [US3] Conversations: the conversation switcher and the new-conversation button at the top of the chat inset (frames J, K). They reuse the agent conversation list for channel `app__<id>`, through `ChatView`'s `header` slot, in `webui/app/screens/apps/AppConversations.tsx`. Update `app.last_topic` on open (`PATCH`). Send the page a `conversation` message on change
- [ ] T062 [US3] Waking: accept `app__{user}__{id}` in `crates/vizier-core/src/agents/tools/sessions/mod.rs` `wakeable_channel` when the caller is the current driver and `belongs_to_person` holds for the owner. List app conversations in `list_sessions` with the app name
- [ ] T063 [US3] Notifications: add `AppScheduled` to `notifies_chat` in `crates/vizier-core/src/agents/process.rs`. Add an `App` branch to `conversation_href` in `crates/vizier-core/src/notifications/mod.rs`. Title app conversations in `crates/vizier-core/src/service/notifications.rs` `fill_titles`. Sum the app's unread chats on its rail tile in `webui/app/screens/shell/ShellRail.tsx`
- [ ] T064 [US3] Locking: `PATCH /apps/{id} {"locked"}` in `crates/vizier-http/src/api/v1/apps.rs`. `AppSource::entries` hides `app_edit` when locked (`crates/vizier-core/src/apps/source.rs`). Add the `locked` line in `## App state`. Add the header lock button, "locked" subline and the note with Unlock above the composer (frame L) in `webui/app/screens/apps/AppView.tsx`

**Checkpoint**: the full build-and-change loop works. With US1 and US2 this is the MVP.

---

## Phase 6: User Story 4 - Switch the agent that drives the app (Priority: P2)

**Goal**: A driver picker with an exact requirement check. Switching opens a new conversation with the new driver.

**Independent Test**: quickstart §4. A ready agent is marked ready and the other lists its exact gap. After switching, the new driver works on the app.

### Tests for User Story 4

- [ ] T065 [P] [US4] A matrix of at least 10 requirement combinations for `apps::requirements::check` (no false ready, no false missing; SC-005) in `crates/vizier-core/src/apps/requirements_tests.rs`
- [ ] T066 [P] [US4] Route tests for `GET /apps/{id}/drivers` and `PUT /apps/{id}/driver` (new conversation, old one readable, non-sensitive approvals carried and sensitive ones not) in `crates/vizier-http/tests/apps.rs`

### Implementation for User Story 4

- [ ] T067 [US4] Implement `apps::requirements` (derived plus declared requirements; `AgentFacts` from the `agent.{id}.tool_names` view, `IntegrationStorage`, secret names and catalogue model caps; gaps with fix targets) in `crates/vizier-core/src/apps/requirements.rs`. Use it in `apps::context::for_turn` and `app_functions`
- [ ] T068 [US4] Implement `GET /apps/{id}/drivers` and `PUT /apps/{id}/driver` (carrying approvals, publishing `AppEvent::DriverChanged`) in `crates/vizier-http/src/api/v1/apps.rs` and `crates/vizier-core/src/apps/mod.rs`
- [ ] T069 [US4] Build the driver picker (frame D) in `webui/app/screens/apps/DriverPicker.tsx`, opened from the driver line at the top of the chat inset: requirements, ready / missing N, expandable gaps, Set up links to the agent's integrations, secrets or model page

**Checkpoint**: drivers can be switched with an honest readiness picture.

---

## Phase 7: User Story 5 - An app outlives its driver (Priority: P2)

**Goal**: Deleting the driver keeps the app, which works with its chat paused.

**Independent Test**: quickstart §5.

### Tests for User Story 5

- [ ] T070 [P] [US5] Tests in `crates/vizier-http/tests/apps.rs` and `crates/vizier-core/src/storage/sqlite/app_tests.rs`:
  - deleting the driving agent sets `driver_agent_id = NULL` and records `app_driver_lost`;
  - app, versions, database file and files are intact;
  - `POST /apps/{id}/chat` → `no_driver`.

### Implementation for User Story 5

- [ ] T071 [US5] In `delete_agent` (`crates/vizier-core/src/storage/sqlite/mod.rs` or its agent module), null `app.driver_agent_id` in the same transaction. Add the `app_driver_lost` notification in `crates/vizier-core/src/notifications/mod.rs` and `crates/vizier-core/src/notifications/text.rs`
- [ ] T072 [US5] Treat "owner may no longer chat with the driver" as no driver in `apps::context::for_turn`, `service/apps.rs` and the picker's `current` flag
- [ ] T073 [US5] Build the no-driver state (frame E: "No driver" at the top of the chat, the explanation with Choose a driver, the disabled composer, the amber dot on the rail tile) in `webui/app/screens/apps/NoDriver.tsx` and `webui/app/screens/shell/ShellRail.tsx`

**Checkpoint**: apps survive their drivers.

---

## Phase 8: User Story 6 - The app does work without a conversation (Priority: P2)

**Goal**: Approved page function calls in the driver's host, schedules, and badges.

**Independent Test**: quickstart §6.

### Tests for User Story 6

- [ ] T074 [P] [US6] Approval matrix tests (unapproved, newly added, post-switch sensitive; SC-006) with `FakeAgentsClient` in `crates/vizier-http/tests/apps.rs`
- [ ] T075 [P] [US6] dummyplug case `apps_functions_and_schedules` in `crates/vizier-core/tests/dummyplug_e2e.rs`:
  - a page call through `AgentsClient::app_function` writes a row with no turn;
  - a `run` schedule fires each minute with nothing open;
  - an `ask` schedule posts into `sched-<name>`;
  - three failures give one episode notification;
  - `app_schedules` shows the episode;
  - `app_set_badge` updates the tile.

### Implementation for User Story 6

- [ ] T076 [US6] Add `AgentsClient::app_function(agent, AppCall) -> AppCallOutcome` in `crates/vizier-core/src/agents_client.rs`, for both `LocalAgentsClient` and `FakeAgentsClient`. Handle `AgentCommand::AppFunction` in the agent host (`crates/vizier-core/src/agents/mod.rs`), building the app-session `ToolContext` and running through `apps::functions`. Append the operation to `specs/048-agent-service-seam/contracts/agents-client.md` from [contracts/agents-client.md](contracts/agents-client.md)
- [ ] T077 [US6] Implement `apps::functions` (call from a turn or from the page through `custom_tools::run::run_with_timeout`, with availability and the redactor) in `crates/vizier-core/src/apps/functions.rs`. Offer the app's functions from `AppSource` under the app's category
- [ ] T078 [US6] Implement `POST /apps/{id}/functions/{name}/call` (ownership, `page_callable`, approval, per-app rate limit, then the seam) and `GET/PUT /apps/{id}/approvals` in `crates/vizier-http/src/api/v1/apps.rs` and `crates/vizier-core/src/service/apps.rs`
- [ ] T079 [US6] Add the `fn` ability (`vizier.fn.call`) in `webui/app/kit/runtime.ts` and `webui/app/lib/uiPage/host.ts`. Add an approval prompt in `webui/app/screens/apps/Approvals.tsx`, reached from the app menu and from a refused call's toast
- [ ] T080 [US6] Implement `apps::schedules` (`tick(now)` from the scheduler's minute tick in `crates/vizier-core/src/scheduler/mod.rs`; `run` via the seam, `ask` as `VizierRequestContent::AppScheduled` in topic `sched-<name>`; runs and episodes; skipped with a reason when there is no driver or a requirement is missing) in `crates/vizier-core/src/apps/schedules.rs`. Add `AppScheduled` to `crates/vizier-core/src/schema/request.rs` with frontmatter `via: app_schedule`, in the history arm of `handle_request`
- [ ] T081 [P] [US6] Implement `app_schedules` and `app_set_badge`, and the failing-schedule lines in `## App state`, in `crates/vizier-core/src/apps/tools.rs` and `crates/vizier-core/src/agents/agent/system_prompt/`. Add the `app_schedule_failed` notification in `crates/vizier-core/src/notifications/`
- [ ] T082 [US6] Add `GET /apps/{id}/schedules` and push `app_badge` / `app_driver_changed` on the inbox stream for rail tiles, in `crates/vizier-http/src/api/v1/apps.rs` and `crates/vizier-core/src/service/inbox.rs`

**Checkpoint**: apps do real work unattended, within the approvals.

---

## Phase 9: User Story 7 - Arrange the rail (Priority: P3)

**Goal**: One mixed, per-person, server-stored rail order with drag and keyboard reordering.

**Independent Test**: quickstart §7.

### Tests for User Story 7

- [ ] T083 [P] [US7] Vitest for the pure merge rule `arrange(stored, visible)` in `webui/app/lib/apps/rail.test.ts`, and route tests for `GET/PUT /me/rail-order` (unknown ids ignored) in `crates/vizier-http/tests/apps.rs`

### Implementation for User Story 7

- [ ] T084 [US7] Implement `GET/PUT /api/v1/me/rail-order` over `RailOrderStorage` in `crates/vizier-http/src/api/v1/me.rs`
- [ ] T085 [US7] Switch `webui/app/screens/shell/ShellRail.tsx` from localStorage `vizier.rail.order` to the server order, through `webui/app/lib/apps/rail.ts`. Upload the old localStorage order once, then remove it. Add a checklist entry for this shim on issue #82
- [ ] T086 [US7] Make `Rail`'s `Reorder.Group` reorder agents and apps together, with a keyboard move (for example Alt+↑/↓ on a focused tile), in `design/src/components/Shell/Rail.tsx` (+ story). Show apps in rail order in `webui/app/screens/shell/AgentSwitcherSheet.tsx` and `webui/app/screens/home/MobileHome.tsx`

**Checkpoint**: the rail is arranged per person.

---

## Phase 10: User Story 8 - Files and messages go from the app to its chat (Priority: P3)

**Goal**: App files, chat uploads, and page-sent chat messages with the gesture rule.

**Independent Test**: quickstart §8.

### Tests for User Story 8

- [ ] T087 [P] [US8] Route tests in `crates/vizier-http/tests/apps.rs`:
  - files `to=app` / `to=conversation`, keep, share;
  - `POST /apps/{id}/chat` creates a `Chat` with `app_origin` and frontmatter `via: app`;
  - an old `gesture_at_ms` → `no_gesture`;
  - the 21st send in a minute → `rate_limited`.
- [ ] T088 [P] [US8] Vitest for `SendGate` with `chat.send` and for `files.upload` message parsing in `webui/app/lib/uiPage/host.test.ts`

### Implementation for User Story 8

- [ ] T089 [US8] Implement `apps::files` (blob `apps/{id}/files/{af_id}`, the `_vizier_files` table, size limit, `keep`, `share`) in `crates/vizier-core/src/apps/files.rs`, and the `/apps/{id}/files*` routes in `crates/vizier-http/src/api/v1/apps.rs`
- [ ] T090 [US8] Implement `app_files` in `crates/vizier-core/src/apps/tools.rs`
- [ ] T091 [US8] Add `VizierRequest.app_origin: Option<AppOrigin>` (journaled; cleared in `stamp_on_receipt`) in `crates/vizier-core/src/schema/request.rs` and `crates/vizier-http/src/api/v1/agents/topic.rs`. Add frontmatter `via: app` / `app: <id> v<n>`. Add `POST /apps/{id}/chat` with the gesture window and rate limit in `crates/vizier-http/src/api/v1/apps.rs`
- [ ] T092 [US8] Add the `chat` and `files` abilities (`vizier.chat.send`, `vizier.files.pick/upload/list/keep/share`) in `webui/app/kit/runtime.ts` and `webui/app/lib/uiPage/host.ts`
- [ ] T093 [US8] Draw the "from <app>" mark on a person's bubble (designed in T006) in `design/` (with a story), and use it in `webui/app/screens/chat/MessageList.tsx` for requests with `app_origin`

**Checkpoint**: apps exchange files and messages with their chat.

---

## Phase 11: User Story 9 - The driver can act on what the person sees (Priority: P3)

**Goal**: Manifest tools answered by the open page, offered only in the app's conversation.

**Independent Test**: quickstart §9.

### Tests for User Story 9

- [ ] T094 [P] [US9] Tests with a fake tab: a manifest tool is answered; an undeclared `evil_tool` is dropped; a tool with no handler is `unavailable`; an ordinary chat gets no app tools. In `crates/vizier-http/tests/apps.rs` and `scripts/apps-e2e.mjs` (modelled on `scripts/ui-pages-e2e.mjs`)

### Implementation for User Story 9

- [ ] T095 [US9] Filter client-tool declarations on `app__…` channels against the current version's manifest in `bind_client_tools` (`crates/vizier-http/src/api/v1/agents/channel.rs`)
- [ ] T096 [US9] Add per-topic registrations to `webui/app/lib/clientTools.ts` (`declaredClientTools(topicKey)`). Have `AppView` register the manifest's tools for its conversation
- [ ] T097 [US9] Add the `tools` ability (`vizier.tools.handle`, `tools.ready`, `tool.call` / `tool.result`, immediate `unavailable` with no handler) in `webui/app/kit/runtime.ts`, `webui/app/lib/uiPage/host.ts` and `SandboxedPage.tsx`

**Checkpoint**: every user story is complete.

---

## Phase 12: Polish & cross-cutting

- [ ] T098 [P] Owner database view (designed in T006): `GET /apps/{id}/db/tables` and `GET /apps/{id}/db/download` in `crates/vizier-http/src/api/v1/apps.rs`, and `webui/app/screens/apps/Tables.tsx` from the app menu (FR-018b)
- [ ] T099 [P] App removal: `DELETE /apps/{id}` (rows, blobs, database file, approvals, schedules, rail entry), and conversations shown as "App: <name> (removed)" in `crates/vizier-core/src/apps/mod.rs` and `crates/vizier-core/src/service/conversations.rs`. Add a confirm dialog naming what is lost in `webui/app/screens/apps/RemoveApp.tsx` (FR-034)
- [ ] T100 [P] Usage: check that page function calls and schedule runs record under `UsageActivity::App` and place `app:{id}` (FR-035), in `crates/vizier-core/tests/dummyplug_e2e.rs`
- [ ] T101 [P] Docs: `docs/src/configuration/apps.md` (what apps are, desktop only, drivers, conversations, data, functions and approvals, schedules, locking, limits) and its entry in `docs/src/SUMMARY.md`
- [ ] T102 [P] Update `CLAUDE.md` with an **Apps** section (module map, the `subapp` flag, `RuntimeOptions.apps`, the seam operation, the `App` channel, the WebUI pieces), matching the other spec sections
- [ ] T103 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, `cargo test -p vizier-core --no-default-features --features subapp` and `just web-check`, and fix what they find
- [ ] T104 Run [quickstart.md](quickstart.md) end to end on `just app-dev` (fresh and existing workspace, §0) and through web access in the built-in browser. Record the results in `specs/071-agent-apps/checklists/verification.md`

---

## Dependencies & execution order

### Phase dependencies

- **Setup (Phase 1)** comes first. T006 (FigCraft) must finish before T060, T093 and T098.
- **Foundational (Phase 2)** depends on Setup and **blocks every story**.
- **US1 → US2 → US3** (P1) are the MVP, done in that order:
  - US2 needs the app route from US1.
  - US3 needs `AppView` from US2.
- **US4, US5 and US6** (P2) each depend only on Foundational plus US1–US3's tools and view. US5 reuses the picker from US4 (Choose a driver), so finish US4 before T073.
- **US7, US8 and US9** (P3) are independent of each other and of the P2 stories.
- **Polish** comes after the stories it touches.

### Within each story

Write the tests first and see them fail. Then storage and service before routes, routes before WebUI, and design components (`design/`) before their use in `webui/`.

### Parallel opportunities

- **Phase 1**: T003 and T006 alongside T002, T004 and T005.
- **Phase 2**: T008, T009, T010, T012, T014, T015, T016, T017, T019, T021, T025, T026, T029, T030, T032, T033 and T034 touch different files.
- **Inside each story**: every test task marked [P].
- **After the MVP**: US4–US9 can be split across people, with US7/US8/US9 fully parallel.

## Parallel example: Phase 2

```text
T010 storage traits      T013 apps/database.rs     T015 apps/manifest.rs
T021 FrameSource         T032 SplitInsets (design) T033 RailItem (design)
T034 AppHeader (design)  T030 capabilities store   T026 service/apps.rs
```

## Parallel example: User Story 3

```text
T052 apps_change e2e   T053 conversations+wake e2e   T054 lock tests   T055 notification tests
T057 app_read          T058 app_query/app_functions
```

## Implementation strategy

1. **MVP**: Setup → Foundational → US1 → US2 → US3. Then stop and validate with quickstart §0–§3d on `just app-dev`. This alone is a complete "build an app by talking to your agent" product.
2. **P2**: US4 (drivers), US5 (no driver), US6 (functions, approvals, schedules), each verified with its quickstart section.
3. **P3**: US7 (rail order), US8 (files and messages), US9 (page tools), in any order.
4. **Polish**: T098–T104. The PR is ready when the T103 commands pass and T104 is recorded.
