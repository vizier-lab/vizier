---
description: "Task list for Projects Mode Foundation"
---

# Tasks: Projects Mode Foundation

**Input**: Design documents from `/specs/040-projects-mode/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: The feature specification does not request a TDD workflow; test work is folded into the implementation tasks that need it, plus the mandatory `cargo test` / `npm test` / dummyplug verification tasks in Polish.

**Organization**: Tasks are grouped by user story so each can be implemented and validated independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Every task names the exact file path to change

## Path Conventions

This repository is a Rust workspace plus a WebUI and a Tauri shell:

- `crates/vizier-core/src/`, `crates/vizier-http/src/`
- `webui/app/`
- `app/src-tauri/src/`
- feature docs under `specs/040-projects-mode/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Compile-ready scaffolding, shared types, permissions and channel helpers.

- [ ] T001 Add `process` to the workspace `tokio` features in `Cargo.toml` and a Unix-only `libc = "0.2"` dependency to `crates/vizier-core/Cargo.toml` for process-group signalling.
- [ ] T002 [P] Create `crates/vizier-core/src/schema/project.rs` with `Project` (`id`, `agent_id`, `name`, `folder`, `owner_id`, `shell_enabled`, `created_at`, `removed_at`), `ProjectSessionRow` (`topic`, `title`, `is_thinking`, `last_activity`), and the constants `PROJECT_ID_LEN = 10`, `DEFAULT_COMMAND_TIMEOUT = 120s`, `MAX_COMMAND_TIMEOUT = 600s`, `OUTPUT_CAP_BYTES = 64 * 1024`, `INSTRUCTIONS_CAP_BYTES = 32 * 1024`; register and re-export it from `crates/vizier-core/src/schema/mod.rs`.
- [ ] T003 [P] Create `crates/vizier-core/src/projects/mod.rs` and `crates/vizier-core/src/projects/channel.rs` with `PREFIX = "project__"`, `channel_id(project_id)`, `project_id(channel_id)`, `is_project_channel(channel_id)` and `project_of_slug(slug)`; add unit tests proving no `:`/path separators, correct parsing, and `None` for ordinary WebUI/Discord/Telegram slugs.
- [ ] T004 [P] Add `all_agents:projects` and `owned_agents:projects` to `AVAILABLE_PERMISSIONS` in `crates/vizier-core/src/storage/user.rs` and extend the permission-list test so both strings are pinned.
- [ ] T005 [P] Add the Projects permission group labels in `webui/app/lib/permissions.ts` and the `canUseProjects` / `canManageProject` helpers in `webui/app/lib/access.ts`, with unit tests in `webui/app/lib/access.test.ts`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The project entity is stored, authorized, served, listed and reachable from the WebUI. No story-specific shell or UI behavior yet.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T006 Create `crates/vizier-core/src/storage/project.rs` with the `ProjectStorage` trait (`list_projects`, `get_project`, `open_project`, `rename_project`, `set_project_shell`, `remove_project`, `project_sessions`) and its `VizierStorage` forwarding; add the module and `ProjectStorage` to the `VizierStorageProvider` bound in `crates/vizier-core/src/storage/mod.rs`.
- [ ] T007 Implement `crates/vizier-core/src/storage/sqlite/project.rs` plus `init_project_schema` and `mod project;` in `crates/vizier-core/src/storage/sqlite/mod.rs`: table `project(id TEXT PRIMARY KEY, agent_id TEXT NOT NULL, name TEXT NOT NULL, folder TEXT NOT NULL, owner_id TEXT NOT NULL, shell_enabled INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL, removed_at INTEGER, UNIQUE(agent_id, folder))` and `idx_project_agent(agent_id, removed_at)`; implement `open_project` as insert-or-restore (clear `removed_at`, keep the id) and `project_sessions` as a left join of `session_detail` to per-topic `MAX(session_history.timestamp)` filtered by the exact project channel slug, newest first, `last_activity` null for an empty session.
- [ ] T008 [P] Add `delete_agent_projects(&conn, agent_id)` to `crates/vizier-core/src/storage/sqlite/project.rs` and call it from `delete_agent` in `crates/vizier-core/src/storage/sqlite/agent.rs` before `agent_config` is deleted; add a unit test proving a deleted agent leaves no project rows.
- [ ] T009 Create `crates/vizier-core/src/projects/access.rs` with the single permission matrix: `can_use(caller, agent)` = `can_view_agent` AND (`all_agents:projects` OR `owned_agents:projects` + ownership); `can_manage(caller, project)` = project owner OR `all_agents:projects`; `can_create(caller, agent)` = `can_use` AND `principal.method == AuthMethod::Desktop`; add table-driven tests for owner, permitted non-owner, owned-agents scope, system role and no permission.
- [ ] T010 Create `crates/vizier-core/src/service/projects.rs` with the `ProjectSummary`, `ProjectDetail`, `ProjectSession`, `OpenProjectRequest`, `UpdateProjectRequest` and `CreateProjectSessionResponse` DTOs, plus folder validation: canonicalize the folder, require it to exist and be a directory (`422 invalid_folder`), refuse it when equal to or inside the canonical `VizierConfig.workspace` (`422 workspace_folder`), and enforce a trimmed name of 1–120 characters (`422 invalid_name`); add unit tests for `..`, symlink and workspace-descendant refusal.
- [ ] T011 In `crates/vizier-core/src/service/projects.rs`, implement `list`, `get`, `open`, `rename`, `set_shell` and `remove`: `open` returns `(ProjectSummary, created)` and reuses/restores an existing `(agent_id, folder)`; `remove` sets `removed_at` only after checking `TurnStorage::pending_turns(agent_id)` for any turn on the project channel and returns `409 turn_active` otherwise; register `pub mod projects;` in `crates/vizier-core/src/service/mod.rs`.
- [ ] T012 In `crates/vizier-core/src/service/projects.rs`, implement `sessions` (the caller's project channel only) and `create_session` (server-generated 10-character nanoid topic, `session_detail` title `"New session"`, return `channel_id = project__<id>`); update `refresh_session_detail` in `crates/vizier-core/src/agents/process.rs` so `"New session"` is treated as an untitled placeholder and replaced after the first request.
- [ ] T013 Exclude project channels from the regular conversation list: add the project-slug check to `conversation_rows` in `crates/vizier-core/src/storage/sqlite/session.rs` and to `kind_of` in `crates/vizier-core/src/service/conversations.rs`; add tests proving a `project__` channel is absent from rows, counts, search and the picker while an ordinary `http__` channel is unchanged.
- [ ] T014 Create `crates/vizier-http/src/api/v1/agents/projects.rs` with the seven handlers (`GET /`, `POST /`, `GET /{project_id}`, `PATCH /{project_id}`, `DELETE /{project_id}`, `GET /{project_id}/sessions`, `POST /{project_id}/sessions`), map `ServiceError` to the existing `APIResponse`/`ApiError` envelope including `desktop_only`, `invalid_folder`, `workspace_folder`, `invalid_name` and `turn_active`, and nest the router at `/{agent_id}/projects` in `crates/vizier-http/src/api/v1/agents/mod.rs`.
- [ ] T015 Register the seven operations and their DTOs in `crates/vizier-http/src/lib.rs` (`ApiDoc` paths and `components.schemas`), and add `crates/vizier-http/tests/conformance/groups/projects.rs` with one vector per route (happy path, `403`, `404`, `422`, `409`) wired into `crates/vizier-http/tests/conformance/groups/mod.rs`.
- [ ] T016 Create `webui/app/api/endpoints/projects.ts` with typed calls for list/open/get/update/remove/sessions/create-session, and `webui/app/stores/projects.ts` with list/detail/session state, 15-second refresh while visible, keep-previous-data on failure, and optimistic session insertion; add `webui/app/stores/projects.test.ts`.
- [ ] T017 Add `authorize_project_channel` to `crates/vizier-core/src/service/topics.rs`, used by `authorize_topic` and the history/files/detail/delete paths: a project channel resolves the project by `(agent_id, project_id)`, requires non-removed and the T009 matrix, and leaves the channel username as the caller's; add tests for allowed, removed, wrong-agent and permissionless reads.

**Checkpoint**: Project rows can be stored, opened, listed, updated, soft-removed and served; sessions can be listed/created through the API; project channels are invisible to ordinary listings.

---

## Phase 3: User Story 1 - Open a project and work in its folder (Priority: P1) 🎯 MVP

**Goal**: A desktop user opens a folder, accepts the trust disclosure, gets a project session, and the agent can run commands in that folder with full results.

**Independent Test**: Open a temporary folder, run `echo hello` and create a file through the agent; the reply shows stdout/stderr/exit status/duration and the file exists on disk. Cancel the trust dialog and confirm nothing was created.

### Implementation for User Story 1

- [ ] T018 [US1] Create `crates/vizier-core/src/projects/context.rs` with `ProjectTurnContext { project_id, name, folder, available, shell_enabled }` and `for_session(deps, session)` that returns `None` for every non-project channel and, for a project channel, loads the project, computes `available = folder.is_dir()`, and re-reads the session owner's current project permission; add unit tests for project/ordinary/task/dream/subagent channels and for a revoked permission.
- [ ] T019 [US1] Add `pub project: Option<Arc<ProjectTurnContext>>` to `ToolContext` in `crates/vizier-core/src/agents/tools/mod.rs` and populate it in `chat_turn`, side chat and the checkpoint prompt rebuild in `crates/vizier-core/src/agents/agent/mod.rs`; fix every other `ToolContext` literal (`agents/process.rs`, background tests, tool tests) with `project: None`.
- [ ] T020 [US1] Create `crates/vizier-core/src/system_prompt/project.rs` with a `project_md(name, folder, shell_enabled, available)` section and inject it in `prepare_system_prompts` after CORE and before `config.documents`; add a snapshot-style unit test asserting the folder is named for a project session and the section is absent for an ordinary session.
- [ ] T021 [US1] Create `crates/vizier-core/src/agents/shell/outcome.rs` (`ShellCommand { command, workdir, timeout }`, `ShellOutcome { stdout, stderr, exit_code, duration_ms, timed_out, truncated }`) and `crates/vizier-core/src/agents/shell/process_tree.rs` (Unix `process_group(0)` + `libc::killpg`, Windows `taskkill /PID <pid> /T /F`), both wired into `crates/vizier-core/src/agents/shell/mod.rs`.
- [ ] T022 [US1] Add `LocalShell::run(&self, cmd: ShellCommand) -> Result<ShellOutcome>` in `crates/vizier-core/src/agents/shell/local/mod.rs`: `tokio::process` with `kill_on_drop(true)`, host environment, resolved working directory, concurrent stdout/stderr reads capped at `OUTPUT_CAP_BYTES` with `truncated`, the 120 s default / 600 s maximum limit, and process-tree kill on timeout; keep `ShellProvider::exec` delegating to `run` and returning its current text contract. Add tests for stdout, stderr, non-zero exit, truncation, timeout and a killed grandchild.
- [ ] T023 [US1] Create `crates/vizier-core/src/projects/shell.rs` with the `WorkspaceShell` `VizierTool`: input `{ command, workdir?, timeout_secs? }`, output the T021 JSON, `workdir` resolved under the canonical project root and refused with `workdir_outside_project` when it escapes or is absolute; return `project_unavailable` / `shell_disabled` before spawning; register the module in `crates/vizier-core/src/projects/mod.rs` and add tests for each error and the happy path.
- [ ] T024 [US1] Add `ToolTarget::Project` to `crates/vizier-core/src/agents/tools/mod.rs`, append the `workspace_shell` catalogue entry and a `shell` category line in `turn_catalogue`/`turn_categories` only when `ctx.project` is present and `shell_enabled && available`, and dispatch it in `crates/vizier-core/src/agents/tools/python/bridge.rs`; update any exhaustive category/tool table tests.
- [ ] T025 [US1] Make the chat-reply notification project-aware in `crates/vizier-core/src/notifications/mod.rs`: keep the HTTP audience resolution, but build `/{agent_id}/projects/{project_id}/chat/{topic}` for a project channel; add a unit test asserting the ordinary href is unchanged.
- [ ] T026 [US1] Add the `desktop_pick_folder` Tauri command in `app/src-tauri/src/shell.rs` using `tauri-plugin-dialog`'s blocking folder picker, register it in both the debug and release handler lists, and expose `pickFolder(): Promise<string | null>` through `webui/app/api/transport/common.ts`, `tauri.ts` (invoke) and `http.ts` (returns `null`).
- [ ] T027 [US1] Create `webui/app/screens/projects/OpenProjectDialog.tsx` and `TrustDisclosure.tsx`: desktop-only guard (`transport.authMode === 'desktop'`), folder picker, danger-tone disclosure stating host account / no sandbox / trusted folders only, then `POST /projects` followed by `POST /projects/{id}/sessions` and navigation; show server validation errors without closing the dialog.
- [ ] T028 [US1] Create `webui/app/screens/projects/ProjectsSection.tsx` and `ProjectRow.tsx` (name, folder basename, availability marker, session count, expand/collapse, session rows, new-session action) and render the section above `<SessionList>` in `webui/app/screens/shell/AgentSidebar.tsx`, hidden when there is no project permission or no projects; add a design story and axe check in `design/src/components/Shell/Shell.stories.tsx`.
- [ ] T029 [US1] Add the project routes in `webui/app/routes.ts` and `webui/app/routes/projects.tsx` (`/:agentId/projects/:projectId/chat/:topicId`, plus a redirect from `/:agentId/projects/:projectId`), teach `pages.ts`/`AppShell` the `projects` segment, and thread an explicit `channel` argument through `webui/app/routes/chat.tsx`, `webui/app/api/endpoints/chat.ts`, `webui/app/stores/connection.ts` and the topic-list call so project sessions no longer use the hard-coded `vizier-webui`.
- [ ] T030 [US1] Add the desktop empty-conversation "Open a Project" action: an optional action/children slot on `design/src/components/ChatGreeting/ChatGreeting.tsx`, rendered from `webui/app/screens/chat/MessageList.tsx` when the conversation is empty, the desktop transport is active, and the agent has no projects; add a component test.

**Checkpoint**: US1 works end to end on the desktop: pick → trust → project session → shell result → file on disk.

---

## Phase 4: User Story 2 - Keep several sessions in one project (Priority: P2)

**Goal**: A project holds several independent personal sessions, with live markers, a shell switch, rename and soft removal from the sidebar.

**Independent Test**: Create three sessions in one project, switch between them, confirm their histories are separate, confirm none appears in regular Sessions, toggle the shell off and observe the tool disappear, remove the project and confirm files remain and reopening restores the sessions.

### Implementation for User Story 2

- [ ] T031 [US2] Complete the project-session UI in `webui/app/screens/projects/ProjectRow.tsx` and `ProjectsSection.tsx`: create a session from the project row (including a project with no sessions), expand/switch sessions, show `is_thinking` live markers, show the `New session` placeholder, and keep the regular Sessions list unchanged; add Vitest coverage for grouping, ordering and the empty-project state.
- [ ] T032 [US2] Add the project menu actions in `webui/app/screens/projects/ProjectRow.tsx` (and the project header): Rename (inline validation, 1–120 chars), Toggle workspace shell (`PATCH shell_enabled`), disabled unless the caller owns the project or has `all_agents:projects`; refresh the project store after each write.
- [ ] T033 [US2] Add the Remove project flow in `webui/app/screens/projects/RemoveProjectDialog.tsx`: confirmation that files and history are kept, `DELETE /projects/{id}`, `409 turn_active` messaging that keeps the project, and a store refresh that removes the row and its sessions from the sidebar.
- [ ] T034 [US2] Add service and WebUI tests for removal/restoration/unavailability in `crates/vizier-core/src/service/projects.rs` and `webui/app/stores/projects.test.ts`: soft removal keeps files and history, reopening restores the same id/sessions, a missing folder marks the project unavailable while history stays readable, and a removed project disappears from listings.
- [ ] T035 [US2] Add a concurrency test proving two sessions of one project never serialise: in `crates/vizier-core/src/projects/shell.rs` (or the service test module), run two `WorkspaceShell` calls with a short delay and assert the wall-clock is closer to the longer call than to the sum; assert no project-level lock exists.

**Checkpoint**: US2 is usable independently: multiple sessions, shell switch, rename and remove/restore all work from the sidebar.

---

## Phase 5: User Story 3 - Reach existing projects from a browser (Priority: P2)

**Goal**: A permitted browser user can list, open and use existing projects, sees a host-execution notice, cannot create a project, and never sees another person's sessions.

**Independent Test**: With web access on, sign in as a person with the project permission, open a project session, run a command and see the output plus the host notice. Confirm no creation action exists and a direct POST returns `403 desktop_only`. A user without the permission sees no section and a direct URL is refused.

### Implementation for User Story 3

- [ ] T036 [US3] Enforce project authorization on the WebSocket path in `crates/vizier-http/src/topic.rs`: `TopicConnection::open` resolves the project and applies the T009 matrix, and `handle_inbound` re-checks the caller's permission before `transport.send_request`; add tests for open refusal, removed project, wrong agent, and revocation between two inbound frames.
- [ ] T037 [US3] Apply the same project check to every REST entry point that accepts a channel id: `POST /agents/{id}/chat`, `POST /agents/{id}/btw` in `crates/vizier-core/src/service/agents.rs`, and the topic-jobs routes in `crates/vizier-core/src/service/jobs.rs`; add tests that a permissionless or removed-project call is `403`/`404` and starts no turn.
- [ ] T038 [US3] Gate the Projects UI on permission: hide `ProjectsSection`, its create action and project routes when `canUseProjects` is false, and render the standard refused/empty state on a direct project URL; add Vitest coverage in `webui/app/lib/access.test.ts` and the project section test.
- [ ] T039 [US3] Create `webui/app/screens/projects/HostExecutionNotice.tsx` and render it in the project session header and above the composer whenever `transport.authMode !== 'desktop'`; add a test proving it is present for HTTP mode and absent in desktop mode.
- [ ] T040 [US3] Ensure the browser never offers creation and handles a forced attempt gracefully: `OpenProjectDialog`/`ProjectsSection` render no create action when `authMode !== 'desktop'`, and a `403 desktop_only` response is surfaced as a clear message; add a test with a mocked transport.
- [ ] T041 [US3] Add per-person isolation tests: two users with the project permission see the same project but each `GET …/sessions` returns only their own channel's sessions, and a bookmarked other-user project URL returns history only for the caller's constructed channel (no leak); cover this in `crates/vizier-core/src/service/projects.rs` and the HTTP conformance vectors.
- [ ] T042 [US3] Finish the remote session experience in `webui/app/routes/projects.tsx` and `webui/app/screens/projects/ProjectsSection.tsx`: a notification href opens the project route, the mobile drawer shows the same Projects section, and a revoked-permission reload shows the refused state without a read-only leak; add the assertion in `webui/app/screens/projects/ProjectsSection.test.tsx`.

**Checkpoint**: Remote access is complete and safe: permitted users can work, creation is impossible, and no cross-person session leakage exists.

---

## Phase 6: User Story 4 - Give the agent the project's context (Priority: P3)

**Goal**: A project turn tells the agent its folder and reads the folder's instruction document; ordinary conversations receive neither.

**Independent Test**: Put `AGENTS.md` in the folder and ask a real model where it is working; it names the folder and follows the instruction. Ask the same agent in an ordinary conversation; it claims no project context.

### Implementation for User Story 4

- [ ] T043 [US4] Create `crates/vizier-core/src/projects/instructions.rs`: read `AGENTS.md` first, then `CLAUDE.md`, from the project root only, UTF-8 lossy, cap at `INSTRUCTIONS_CAP_BYTES` on a UTF-8 boundary and report `truncated`; a missing/unreadable file is `None` with no error; add tests for precedence, missing, invalid UTF-8 and a 40 KiB file cut to 32 KiB with the truncation flag.
- [ ] T044 [US4] Extend `ProjectTurnContext` in `crates/vizier-core/src/projects/context.rs` with `instructions: Option<String>` and `instructions_truncated: bool`, and extend `crates/vizier-core/src/system_prompt/project.rs` to add the `## Project Instructions` section plus the truncation note; add tests for present, truncated, missing and unavailable-folder cases.
- [ ] T045 [US4] Add exclusion and inheritance tests: a project session's prompt contains the folder and instructions; ordinary, task, dream, subagent and inter-agent prompts contain neither; a side chat inside a project session inherits the section; a background report waking in the project session inherits it while a background piece does not. Put these in `crates/vizier-core/src/system_prompt/project.rs` and `crates/vizier-core/src/projects/context.rs`.

**Checkpoint**: US4 is complete; the agent knows its project and conventions, and no other session kind receives them.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, generated artifacts, and the mandatory quality gates.

- [ ] T046 [P] Regenerate the API surface with `just openapi` (writes `webui/openapi.json` and `webui/app/api/schema.d.ts`) and verify the seven project operations and DTOs are present; do not hand-edit the generated file.
- [ ] T047 [P] Add `docs/src/configuration/projects.md` (what a project is, desktop-only creation, trust disclosure, permissions, workspace shell, removed/unavailable behavior) and link it from the configuration index/navigation.
- [ ] T048 [P] Update `CLAUDE.md` with the projects module map: channel identity, project storage/service, per-turn workspace shell, prompt section and the `project__` listing exclusion.
- [ ] T049 Run `cargo clippy --workspace` and `cargo test --workspace` from the repository root; fix every warning, failure and `unwrap()`/`expect()` outside tests introduced by this feature.
- [ ] T050 Run `cd webui && npm run typecheck && npm test`; fix type errors and failing tests, and run the design Shell story (both themes, axe) for the new Projects components.
- [ ] T051 Run `cd app && cargo test`; add/extend a smoke test that creates a project through the embedded `Dispatcher` (desktop principal) and asserts the network router refuses creation with `403 desktop_only`.
- [ ] T052 Walk every section of `specs/040-projects-mode/quickstart.md` against a running binary: the dummyplug shell/listing/permission/removal steps, the desktop creation/trust/restore steps, and the one live-provider instruction check; record any deviation in the feature notes.
- [ ] T053 Final scope audit against `specs/040-projects-mode/spec.md`: every FR-001…FR-030 is satisfied or explicitly deferred, no git/diff/editor/worktree/terminal/sandbox code was added, and the Projects section, host notice, permission catalogue and OpenAPI all agree.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup; **blocks all user stories**.
- **US1 (Phase 3)**: depends on Foundational.
- **US2 (Phase 4)**: depends on Foundational and builds on the US1 project UI/routes.
- **US3 (Phase 5)**: depends on Foundational and the US1 routes/UI.
- **US4 (Phase 6)**: depends on Foundational and the US1 context/prompt code.
- **Polish (Phase 7)**: depends on the stories that are in scope.

### User Story Dependencies

- **US1 (P1)**: independent after Foundational; MVP.
- **US2 (P2)**: after Foundational; reuses US1's sidebar/routes but its session, rename, switch and remove behavior is independently testable.
- **US3 (P2)**: after Foundational; reuses US1/2's UI, but its authorization and host-notice behavior is independently testable.
- **US4 (P3)**: after Foundational; reuses US1's `ProjectTurnContext` and prompt, and is independently testable through prompt tests and the live-provider check.

### Within Each User Story

- Storage/service/API before UI (Foundational covers the shared part).
- Context resolver before `ToolContext`/prompt wiring.
- Shell outcome/process-tree before `LocalShell::run` before `WorkspaceShell` before `ToolTarget::Project`.
- Authorization before remote UI.
- Instructions reader before the instruction prompt section.

### Parallel Opportunities

- Setup: T002–T005 are independent files and can run in parallel after T001.
- Foundational: T006 is the storage trait; T007 follows it. T008, T009, T010 and T013 can run in parallel once their prerequisites exist. T014 after T010–T012; T015 and T016 can then run in parallel; T017 in parallel with T014–T016.
- US1: T021/T022 (shell) and T026 (desktop picker) are independent of T018–T020; T023–T024 must follow T022 and T018; T027–T030 are parallel UI files after T016.
- US2: T031–T033 are different screens/dialogs and can run in parallel; T034/T035 are test-only.
- US3: T036/T037 (server authorization) and T038–T040 (UI gating) can run in parallel; T041/T042 after them.
- US4: T043 is independent; T044 follows T018 and T020; T045 follows T044.
- Polish: T046–T048 are independent documentation/generation tasks; T049–T052 run after their scopes are complete.

---

## Parallel Example: User Story 1

```text
# Shell work, independent of the prompt/context work:
T021 ShellCommand/ShellOutcome + process_tree
T022 LocalShell::run
T026 desktop_pick_folder + transport

# UI work, after the project store exists:
T027 OpenProjectDialog + TrustDisclosure
T028 ProjectsSection + ProjectRow
T029 project routes + channel-aware chat
T030 empty-conversation CTA
```

## Parallel Example: User Story 3

```text
# Server and client halves can proceed together:
T036 WebSocket project authorization
T037 REST chat/btw/jobs project authorization
T038 permission gating in the Projects UI
T039 HostExecutionNotice
T040 browser creation refusal UX
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (T001–T005).
2. Complete Phase 2 (T006–T017).
3. Complete Phase 3 (T018–T030).
4. **STOP and VALIDATE**: run the US1 quickstart section — desktop open, trust, session, `echo hello`, file creation, timeout and cwd refusal.
5. Ship the desktop foundation.

### Incremental Delivery

1. Setup + Foundational → storage/API ready.
2. US1 → desktop project + workspace shell (MVP).
3. US2 → multiple sessions, shell switch, rename/remove/restore.
4. US3 → browser access, permission gating, host notice.
5. US4 → project folder and instructions in the prompt.
6. Polish → docs, generated OpenAPI, full gates and e2e.

### Parallel Team Strategy

- One developer on storage/service/API (T006–T017).
- One on the shell toolchain (T021–T024) after T001/T002.
- One on the WebUI shell/creation (T026–T030) after T016.
- One on remote authorization (T036–T037) after T017.
- US4 can proceed once US1's context/prompt tasks are in.

---

## Notes

- [P] means different files with no dependency on an incomplete task.
- Every project task that stores or returns paths must use canonical paths; display may strip the Windows `\\?\` prefix.
- The workspace shell is always local; do not route it through the Docker shell backend.
- Project sessions must never appear in `GET /agents/{id}/conversations` or session search.
- Do not add a client-supplied system-prompt field; project instructions are read server-side.
- Commit after each logical group with `just commit "feat: ..."` (conventional commit format).
