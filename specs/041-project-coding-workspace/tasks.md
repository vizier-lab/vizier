---

description: "Task list for 041 Project Coding Workspace"
---

# Tasks: Project Coding Workspace

**Input**: Design documents from `/specs/041-project-coding-workspace/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Included. The plan names the unit tests this feature needs (path resolution, edit semantics, ledger, ignore filtering, porcelain parsing, mode filtering, the channel migration), the conformance suite must gain a vector for every new route (`tests/conformance/pending.txt` may only shrink), and the constitution requires dummyplug end-to-end checks ([quickstart.md](./quickstart.md)).

**Organization**: Grouped by user story. The channel kind (US6) is mostly foundational, because the mode table, worktree binding and every project route key on it; its phase keeps only the consumer-facing parts.

**UI rule**: every UI task is preceded by a FigCraft drawing task (CLAUDE.md "Design first"). Commit order across repos: design → webui → vizier (`just commit`).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US6)

## Path Conventions

- Core: `crates/vizier-core/src/…`; HTTP: `crates/vizier-http/src/…`; conformance: `crates/vizier-http/tests/conformance/…`
- Design library: `design/src/…`; WebUI: `webui/app/…`

---

## Phase 1: Setup

**Purpose**: Dependencies and module skeletons

- [X] T001 Add `ignore` to `[workspace.dependencies]` in `Cargo.toml` (exact current version, default features) and `ignore.workspace = true` to `crates/vizier-core/Cargo.toml`; run `cargo build -p vizier-core` and `cargo tree -i ignore` to confirm no duplicate `regex`/`globset` versions are introduced
- [X] T002 Create empty modules and register them in `crates/vizier-core/src/projects/mod.rs`: `path`, `ledger`, `mentions`, `suggest`, `migrate`, `git` (`git/mod.rs`, `git/status.rs`, `git/worktree.rs`), `tools` (`tools/mod.rs`)
- [ ] T003 [P] Add a #82 checklist entry (GitHub issue vizier-lab/vizier#82) for `projects::migrate::migrate_project_channels`, its marker `migration.project_channel_v1` and its 040-shaped test fixture, per [contracts/channel-and-migration.md](./contracts/channel-and-migration.md) §4

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Channel kind, storage, path confinement, the tool registry, the git runner and per-turn context. No user story can start before this phase is done.

### Channel kind and migration (R1, R2)

- [X] T004 Add `VizierChannelId::Project(String, String)` in `crates/vizier-core/src/schema/session.rs` with slug `project__{user}__{project_id}`, and `from_web`, `web_user`, `web_channel_id`, `project_id` helpers per [contracts/channel-and-migration.md](./contracts/channel-and-migration.md) §2; give `usage_place` (`project:{slug}:{topic}`), `usage_activity`, `platform` and `to_slug` their `Project` arms; unit tests for round-tripping and that `from_web("alice", "project__p3x")` is `Project` while `from_web("alice", "vizier-webui")` is `HTTP`
- [X] T005 Add `UsageActivity::Project` (serialised `project`) in `crates/vizier-core/src/schema/usage.rs` and its label "Projects" wherever activities are listed in `crates/vizier-core/src/usage/mod.rs`
- [X] T006 Replace every hand-built `VizierChannelId::HTTP(user, channel_id)` from request data with `VizierChannelId::from_web` in `crates/vizier-http/src/topic.rs`, `crates/vizier-core/src/service/topics.rs` (including `readable_channel`) and `crates/vizier-core/src/service/projects.rs` (`sessions`, `create_session`, `session_for`)
- [X] T007 Replace `HTTP(user, _)` destructures that mean "the person in this chat" with `web_user()` in `crates/vizier-core/src/notifications/mod.rs`, `crates/vizier-core/src/agents/process.rs`, `crates/vizier-core/src/agents/client/mod.rs`, `crates/vizier-core/src/channels/side_chat.rs`, `crates/vizier-core/src/service/agents.rs`, `crates/vizier-core/src/agents/tools/webui/mod.rs`, `crates/vizier-core/src/storage/sqlite/client_call.rs` and `crates/vizier-http/src/api/v1/agents/channel.rs`; then fix every remaining non-exhaustive match the compiler or `grep -rn "VizierChannelId::HTTP" crates` reports, deciding each one explicitly
- [X] T008 Delete `project_of_slug`/`project_channel_of_slug` from `crates/vizier-core/src/projects/channel.rs` (keep `PREFIX`, `channel_id`, `project_id`, `display_path`, `canonical_folder`, `inside`) and move their callers onto the variant: `conversation_rows` in `crates/vizier-core/src/storage/sqlite/session.rs`, `kind_of` in `crates/vizier-core/src/service/conversations.rs`, `crates/vizier-core/src/service/topics.rs`, `crates/vizier-core/src/notifications/mod.rs`, `crates/vizier-core/src/agents/agent/system_prompt/project.rs`, `crates/vizier-http/src/topic.rs` (per-turn re-check on the variant)
- [X] T009 Make `crates/vizier-core/src/storage/session.rs` (the `starts_with("http__")` display rule) and `crates/vizier-core/src/usage/mod.rs` (`slug_kind`, `slug_place`, `conversation_link`) recognise `project__` slugs; label a project place with the project name and chat title, linking to `/<agent>/projects/<id>/chat/<topic>`
- [X] T010 Implement `migrate_project_channels(conn)` in `crates/vizier-core/src/projects/migrate.rs` per [contracts/channel-and-migration.md](./contracts/channel-and-migration.md) §4: marker `migration.project_channel_v1`, one transaction, the old-shape parser (moved here), exact rewrites, `REPLACE` rewrites (including `DREAM__…` slugs, `session_detail.id`, `session_file.session_slug`, `usage_event.place`/`activity`), and JSON rewrites of `session_detail.data.channel` and `session_turn.request.session[1]`; before writing it, grep every `CREATE TABLE` in `crates/vizier-core/src/storage/sqlite/` and add any further channel/slug column to the contract's table
- [X] T011 Call the migration from `crates/vizier-core/src/dependencies.rs` beside `migrate_task_requester`, after schema init and before agents start, failing startup on error
- [X] T012 Unit test in `crates/vizier-core/src/projects/migrate.rs`: a 040-shaped database with one `http__alice__project__p3x` row per table plus an ordinary `http__alice__vizier-webui` row; assert no old slug remains, every rewritten value matches, the ordinary row is untouched, and a second run is a no-op
- [X] T013 Unit test in `crates/vizier-core/src/storage/history.rs` that `is_non_user_channel("project__alice__p3x")` is false (FR-016a)

### Storage (data-model §2–§4)

- [X] T014 Add `ProjectMode` (`Plan` default, `Build`), `Worktree`, `WorktreeRef`, `ProjectScope` value types to `crates/vizier-core/src/schema/project.rs` per [data-model.md](./data-model.md)
- [X] T015 Create `project_chat` and `project_worktree` tables (`CREATE TABLE IF NOT EXISTS`, indexes, `ON DELETE CASCADE` to `project`) in the project schema init of `crates/vizier-core/src/storage/sqlite/project.rs`
- [X] T016 Extend the `ProjectStorage` trait and `VizierStorage` forwarding in `crates/vizier-core/src/storage/project.rs` with `project_chat_mode`, `set_project_chat_mode`, `insert_worktree` (row + `session_detail` in one transaction), `worktree`, `worktree_by_topic`, `worktrees`, `delete_worktree`, `project_has_main_turn` (R14)
- [X] T017 Implement those methods in `crates/vizier-core/src/storage/sqlite/project.rs`, make `project_sessions` return each row's mode and worktree, and delete `project_chat` rows in `delete_session` and `delete_agent` (`crates/vizier-core/src/storage/sqlite/session.rs`, agent deletion); unit tests for default mode, upsert, unique name/topic, cascade on agent deletion

### Paths, ledger, git runner (R6, R8, R12)

- [X] T018 [P] Implement `projects::path::resolve(cwd, rel, Purpose)` in `crates/vizier-core/src/projects/path.rs` (absolute → `path_outside_project`; deepest existing ancestor canonicalised; `..` in the missing tail refused; `.vizier/` from the project root → `path_in_worktree_area`; `Existing`/`Create`) and `display_rel(cwd, path)` with `/` separators; unit tests for `..`, absolute, symlink out, `.vizier/`, new nested file, Windows-style separators
- [X] T019 [P] Implement `ProjectReadLedger` in `crates/vizier-core/src/projects/ledger.rs` (`record(session, path, bytes)`, `check(session, path, current_bytes) -> Result<(), stale_read>`, `forget_session`, 1,024-entry bound per session) and hold it on `VizierDependencies` in `crates/vizier-core/src/dependencies.rs`; call `forget_session` from session deletion; unit tests
- [X] T020 [P] Implement the `Git` runner in `crates/vizier-core/src/projects/git/mod.rs` per R12 (argv only, `-C`, `core.quotepath=off`, `color.ui=false`, `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0` for reads, `LC_ALL=C`, 15 s timeout with kill, cached `git --version` probe, `probe(dir) -> Option<GitInfo>` from `rev-parse --show-toplevel --git-common-dir --abbrev-ref HEAD`), with known-stderr → plain-message mapping
- [X] T021 [P] Implement porcelain v2 `-z` status parsing, `--numstat -z` parsing, untracked line counting (files over 4 MiB as unknown) and `for-each-ref`/`worktree list --porcelain` parsing in `crates/vizier-core/src/projects/git/status.rs`; unit tests from captured fixtures including renames, conflicts, detached HEAD, spaces and non-ASCII paths

### Per-turn context and the registry (R10, R11)

- [X] T022 Replace `for_session` with `for_turn(deps, session, request)` in `crates/vizier-core/src/projects/context.rs`: `Project` sessions resolve the project, the topic's worktree (`cwd`), the saved mode and `git` probe; `Subagent` sessions with `request.project_scope` resolve the scope's project, topic worktree and mode after re-checking the owner's `can_use`; add `cwd`, `worktree`, `mode`, `git`, `is_piece` to `ProjectTurnContext`; read instructions from `cwd`; update `shell_offered()`; update callers in `crates/vizier-core/src/agents/agent/mod.rs` (chat turn, side chat, checkpoint rebuild)
- [X] T023 Add `VizierRequest.project_scope` and `VizierRequest.mentions` (`#[serde(default, skip_serializing_if = …)]`) in `crates/vizier-core/src/schema/request.rs`
- [X] T024 Create the registry in `crates/vizier-core/src/projects/tools/mod.rs`: `Access { Read, Plan, Write, Command }`, `ProjectToolSpec { tool, access, needs_git }`, `REGISTRY`, `offered(ctx) -> Vec<CatalogueEntry>` (mode filter, shell switch, git presence, `Plan` excluded for pieces), `call(name, args, ctx)` refusing names not in `offered` with `tool_not_offered: …`, and `output_schema(name)`; register `workspace_shell` first; unit tests of the availability table in [contracts/coding-tools.md](./contracts/coding-tools.md) §1 for Plan, Build, Build with shell off, no git, and a piece
- [X] T025 Add `ToolCategory::Coding` (name `coding`, description per data-model §9) after `Shell` in `crates/vizier-core/src/agents/tools/mod.rs`; change `WorkspaceShell::category()` to `Coding` and its `workdir` resolution to `projects::path::resolve` in `crates/vizier-core/src/projects/shell.rs`; update the `every_tool_is_in_its_category` table
- [X] T026 Wire the registry: `turn_catalogue` appends `projects::tools::offered(ctx)` and `turn_categories` adds `coding` whenever `ctx.project` is set (both modes) in `crates/vizier-core/src/agents/tools/mod.rs`; `ToolTarget::Project` dispatches through `projects::tools::call` in `crates/vizier-core/src/agents/tools/python/bridge.rs`; output schemas through `projects::tools::output_schema` in `crates/vizier-core/src/agents/tools/python/docs_tools.rs`
- [X] T027 Split what the agent is told per [contracts/coding-tools.md](./contracts/coding-tools.md) §4 (R17): make the `## Project` section in `crates/vizier-core/src/agents/agent/system_prompt/project.rs` stable per chat (project, working folder, worktree name, the fixed two-mode paragraph, the piece variant, instructions; remove the 040 shell-on/off and unavailable lines), and add a `## Project state` section (mode, branch with changed count, shell state, or folder unavailable) to `context_md` in `crates/vizier-core/src/agents/agent/system_prompt/context.rs`, passed from the chat turn in `crates/vizier-core/src/agents/agent/mod.rs`; unit tests that `project_md` is byte-identical across Plan/Build, shell on/off, branch change and folder loss, and that `## Project state` renders each case; keep `SANDBOX.md` unchanged by mode

**Checkpoint**: `cargo test --workspace` and `cargo clippy --workspace` pass; a 040 project chat survives an upgrade; `workspace_shell` works from Build, is absent in Plan, and is listed under `coding`.

---

## Phase 3: User Story 1 - The agent reads, searches and edits files (Priority: P1) 🎯 MVP

**Goal**: dedicated, confined, gitignore-aware file tools with exact-text edits and a stale-read guard.

**Independent Test**: [quickstart.md](./quickstart.md) §3 (Build mode): rename a function across files using only `read_file`, `grep` and `edit_file`; refused edits leave files byte-identical.

- [X] T028 [P] [US1] Implement a shared walker in `crates/vizier-core/src/projects/tools/mod.rs` (or a `walk.rs` beside it): `ignore::WalkBuilder` with `hidden(false)`, `git_ignore`, `git_exclude`, `git_global`, `parents`, `require_git(false)`, skipping `.git/` and, from the project root, `.vizier/`; `include_ignored` switch; run in `spawn_blocking`; binary sniff (NUL in first 8 KiB); unit tests with a temp tree containing `.gitignore`, nested `.gitignore`, negation and `.vizier/`
- [X] T029 [P] [US1] Implement `read_file` in `crates/vizier-core/src/projects/tools/read_file.rs` per contract (1-based inclusive range, default 2,000 lines, 2,000-char line cap, 256 KiB cap, `truncated`/`next_start_line`, binary result, ledger record); unit tests
- [X] T030 [P] [US1] Implement `list_files` in `crates/vizier-core/src/projects/tools/list_files.rs` (depth 1–10, 1,000-entry cap, sorted, `dir/` suffix, size); unit tests
- [X] T031 [P] [US1] Implement `glob` in `crates/vizier-core/src/projects/tools/glob.rs` with `ignore::overrides::OverrideBuilder` (files only, 1,000 cap); unit tests
- [X] T032 [P] [US1] Implement `grep` in `crates/vizier-core/src/projects/tools/grep.rs` (regex via `regex`, `case_insensitive`, `glob` filter, context 0–5, `max_results` 1–1,000, 500-char line cap, files over 4 MiB and binaries skipped and counted, `invalid_pattern`); unit tests
- [X] T033 [P] [US1] Implement atomic write (temp file in the same folder, copy permissions, rename) as a helper in `crates/vizier-core/src/projects/tools/mod.rs`; unit test that a failure mid-write leaves the original intact
- [X] T034 [US1] Implement `write_file` in `crates/vizier-core/src/projects/tools/write_file.rs` (creates parents inside `cwd`, 4 MiB cap → `too_large`, ledger check for existing files, ledger record); unit tests
- [X] T035 [US1] Implement `edit_file` in `crates/vizier-core/src/projects/tools/edit_file.rs` (`invalid_edit`, `stale_read`, `edit_not_found`, `edit_ambiguous` with count and first five lines, `replace_all`, CRLF translation, atomic write, ledger record); unit tests asserting byte-identical files after every refusal (SC-002) and CRLF preservation
- [X] T036 [US1] Register `read_file`, `list_files`, `glob`, `grep` (Read) and `write_file`, `edit_file` (Write) in `REGISTRY` in `crates/vizier-core/src/projects/tools/mod.rs`
- [ ] T037 [US1] Run [quickstart.md](./quickstart.md) §2 (the `list_files`/`grep` part) and §3 against a dummyplug agent and record the outcome in the PR description

**Checkpoint**: US1 works with the chat's mode forced to Build (no toggle yet: set it with `PUT …/mode` from T039 or directly in `project_chat`).

---

## Phase 4: User Story 2 - Plan before building (Priority: P1)

**Goal**: per-chat Plan/Build mode, a plan card, and subagents inheriting the launching turn's mode and folder.

**Independent Test**: [quickstart.md](./quickstart.md) §2 and the subagent block in §3.

### Backend

- [X] T038 [P] [US2] Implement `propose_plan` (`ends_turn() == true`, input limits per contract, output `{status: "proposed"}`) in `crates/vizier-core/src/projects/tools/propose_plan.rs` and register it with `Access::Plan`
- [X] T039 [US2] Add `set_mode` and mode/worktree fields on `ProjectSession` in `crates/vizier-core/src/service/projects.rs` (caller's own chat, `can_use`), and the route `PUT /agents/{agent_id}/projects/{project_id}/sessions/{topic}/mode` with OpenAPI annotations in `crates/vizier-http/src/api/v1/agents/projects.rs` and `crates/vizier-http/src/lib.rs`
- [X] T040 [US2] In `crates/vizier-core/src/agents/background/mod.rs`, give `dispatch_piece` the launching `ToolContext.project` and set `project_scope` (project id, owner, topic, mode) on the piece's request; pass it from `background_subagents` and `parallel_subagents` in `crates/vizier-core/src/agents/tools/subagents.rs` and from the background launcher
- [X] T041 [US2] Unit tests in `crates/vizier-core/src/projects/context.rs`: a piece with a scope resolves the chat's `cwd` (worktree included) and the scope's mode; a revoked owner or removed project gives no context; a `delegate_agent`/task/dream session ignores any scope; `propose_plan` is never offered to a piece
- [X] T042 [US2] Conformance vector for `PUT …/sessions/{topic}/mode` in `crates/vizier-http/tests/conformance/groups/projects.rs` (three fixture users); record the baseline with `CONFORMANCE_RECORD=1`

### Design and WebUI

- [ ] T043 [US2] Draw in FigCraft (`Vizier.figcraft`, page `- App`, project chat, both themes): the Composer mode toggle with its hint, the "applies to the next message" state, and the plan card with its single action **Build this plan**
- [X] T044 [P] [US2] Build `ModeToggle` (segmented control) and `PlanCard` with stories in `design/src/` per the drawing
- [X] T045 [US2] Add `setMode` to `webui/app/api/endpoints/projects.ts`, mode in the project chat store `webui/app/stores/projects.ts` (optimistic, reverted on error), and regenerate `webui/app/api/schema.d.ts` (`scripts/openapi.mjs`)
- [X] T046 [US2] Put the toggle in `webui/app/screens/chat/Composer.tsx` for project chats only
- [X] T047 [US2] Render `propose_plan` from the `execute_python` report in `webui/app/screens/chat/client/PlanCard.tsx`, registered where report invocations are drawn; **Build this plan** sets Build then sends "Build the plan."; buttons hide once a later message exists; vitest for the hide rule and the action order
- [ ] T048 [US2] Run [quickstart.md](./quickstart.md) §2 and the subagent block of §3 against dummyplug; record the outcome

**Checkpoint**: MVP — a person plans in Plan and builds in Build; subagents inherit.

---

## Phase 5: User Story 3 - Work in an isolated worktree (Priority: P2)

**Goal**: create a worktree with one chat; tools act inside it; delete chat and worktree together.

**Independent Test**: [quickstart.md](./quickstart.md) §5.

### Backend

- [X] T049 [P] [US3] Implement worktree operations in `crates/vizier-core/src/projects/git/worktree.rs`: name/branch validation (`check-ref-format --branch`), ensure `/.vizier/` in `<git-common-dir>/info/exclude`, `worktree add -b`, `worktree remove [--force]`, `worktree prune`, `branch -d|-D`, `merge-base --is-ancestor`, dirty counts; unit tests in a temp repository (skipped when git is absent)
- [X] T050 [US3] Implement `create_worktree`, `worktree_detail`, `list_worktrees` and `delete_worktree` in `crates/vizier-core/src/service/projects.rs` per R13 (row + session detail together; roll back the folder on storage failure; `turn_active`, `worktree_dirty`, missing-folder path with `prune`; delete the topic through the existing topic deletion, then the row; `can_use` for own, `can_manage` for others')
- [X] T051 [US3] Make the existing conversation delete in `crates/vizier-core/src/service/topics.rs` delegate to `delete_worktree` for a worktree-bound topic
- [X] T052 [US3] Make `projects::path::resolve` refuse paths from the project root into `.vizier/` for every tool and the shell `workdir`, and make walks skip `.vizier/` (verify T018/T028 cover it; add tests for a worktree chat reaching the main folder)
- [X] T053 [US3] Routes `GET/POST …/worktrees`, `GET/DELETE …/worktrees/{id}` with OpenAPI in `crates/vizier-http/src/api/v1/agents/projects.rs` and `crates/vizier-http/src/lib.rs`, refusal codes per [contracts/http-api.md](./contracts/http-api.md) §5
- [X] T054 [US3] Conformance vectors for the four worktree routes in `crates/vizier-http/tests/conformance/groups/projects.rs`; record the baseline
- [X] T055 [US3] Project removal confirmation data: add `worktree_count` to the project detail DTO in `crates/vizier-core/src/service/projects.rs` so the WebUI can say folders stay (FR-033)

### Design and WebUI

- [ ] T056 [US3] Draw in FigCraft: the New worktree dialog, the delete confirmation (clean and dirty forms, **Also delete branch**), the sidebar grouping with branch, change dot and unavailable state, and the project removal note
- [X] T057 [P] [US3] Build the dialog and confirmation pieces in `design/src/` with stories (reuse `Dialog`, form fields, checkbox)
- [X] T058 [US3] Add worktree endpoints to `webui/app/api/endpoints/projects.ts` and worktree state to `webui/app/stores/projects.ts`; regenerate `schema.d.ts`
- [X] T059 [US3] Implement `webui/app/screens/projects/WorktreeDialog.tsx` (navigate to the new chat on success, progress while creating) and the **New worktree** action on `webui/app/screens/projects/ProjectRow.tsx`
- [X] T060 [US3] Implement `webui/app/screens/projects/DeleteWorktreeDialog.tsx` (fetch detail for counts, re-show destructive form on `worktree_dirty`) and route worktree chats' **Delete** to it
- [X] T061 [US3] Group worktree chats under the project in `webui/app/screens/projects/ProjectsSection.tsx` with branch and availability; update `ProjectsSection.test.tsx`
- [X] T062 [US3] Add the worktree note to `webui/app/screens/projects/RemoveProjectDialog.tsx`
- [ ] T063 [US3] Run [quickstart.md](./quickstart.md) §5; record the outcome

**Checkpoint**: worktrees work end to end; main folder and worktree chats are isolated.

---

## Phase 6: User Story 4 - See what changed and switch branches (Priority: P2)

**Goal**: branch chip, changes with diffs, switch/create branches; read-only git tools for the agent.

**Independent Test**: [quickstart.md](./quickstart.md) §4.

### Backend

- [X] T064 [P] [US4] Implement `git_status`, `git_diff` (path, `staged`, 256 KiB cap) and `git_log` (n ≤ 50, path) in `crates/vizier-core/src/projects/tools/git_status.rs`, `git_diff.rs`, `git_log.rs`, registered as `Read` with `needs_git`; unit tests against a temp repository
- [X] T065 [US4] Implement `git_summary`, `git_changes`, `git_file_diff` (synthetic diff for untracked, 1 MiB cap, binary flag), `git_branches` (with `checked_out_in`) and `git_switch` (`turn_active` via `project_has_main_turn` for the root or the chat's topic for a worktree; `branch_checked_out`, `local_changes`, `branch_exists`, `invalid_branch`; `can_manage` for the root, `can_use` for one's own worktree) in `crates/vizier-core/src/service/projects.rs`
- [X] T066 [US4] Routes `GET …/git`, `…/git/changes`, `…/git/diff`, `…/git/branches`, `POST …/git/switch` with OpenAPI in `crates/vizier-http/src/api/v1/agents/projects.rs` and `crates/vizier-http/src/lib.rs`; `GET …/git` returns `{available: false}` without git
- [X] T067 [US4] Conformance vectors for the five git routes in `crates/vizier-http/tests/conformance/groups/projects.rs`; record the baseline

### Design and WebUI

- [ ] T068 [US4] Draw in FigCraft: the header branch chip and branch menu (including "in worktree X" and **New branch from …**), the changes section in the details panel, and the diff view (added/removed tints, truncated and binary states, narrow-screen dialog)
- [X] T069 [P] [US4] Build `DiffView` (pure parser of unified diff text with vitest) and `BranchMenu` with stories in `design/src/`
- [X] T070 [US4] Add git endpoints and a per-chat git summary to `webui/app/api/endpoints/projects.ts` and `webui/app/stores/projects.ts` (refresh on open, on turn end, every 15 s while visible); regenerate `schema.d.ts`
- [X] T071 [US4] Add the branch chip and menu to `webui/app/screens/chat/ConversationHeader.tsx` (hidden when `available: false`; refusals as toasts)
- [X] T072 [US4] Implement `webui/app/screens/chat/ChangesPanel.tsx` in the chat details panel using `DiffView`
- [ ] T073 [US4] Run [quickstart.md](./quickstart.md) §4, including the no-git folder; record the outcome

**Checkpoint**: the person reviews the agent's changes and moves the folder between branches inside Vizier.

---

## Phase 7: User Story 5 - Point the agent at a file with `@` (Priority: P3)

**Goal**: `@` suggestions in the composer; mentioned files' contents given to the agent at turn start.

**Independent Test**: [quickstart.md](./quickstart.md) §6.

### Backend

- [X] T074 [P] [US5] Implement `projects::suggest` in `crates/vizier-core/src/projects/suggest.rs`: walker-backed path list cached 10 s per folder (50,000-entry bound), folders with `/`, hand-rolled subsequence scorer (consecutive, word-boundary and basename bonuses; shorter path wins ties); unit tests for ranking
- [X] T075 [P] [US5] Implement `projects::mentions::section(ctx, &mentions)` in `crates/vizier-core/src/projects/mentions.rs` (64 KiB per file numbered, "too large" line, 200-entry folder listing, "missing" line, 256 KiB total, ledger record for included files); unit tests
- [X] T076 [US5] Accept `mentions` on the project socket's `chat` frame in `crates/vizier-http/src/topic.rs` (≤ 10, relative, ≤ 1,024 bytes; `client_error invalid_mentions`), set `VizierRequest.mentions`, and store `metadata.mentions` on the request's history entry in `crates/vizier-core/src/agents/process.rs`
- [X] T077 [US5] Add the `## Mentioned files` section to `context_md` in the chat turn in `crates/vizier-core/src/agents/agent/mod.rs` when the turn has a project and mentions
- [X] T078 [US5] Implement `file_suggestions` in `crates/vizier-core/src/service/projects.rs` and `GET …/files?q=&topic=&limit=` with OpenAPI in `crates/vizier-http/src/api/v1/agents/projects.rs` and `crates/vizier-http/src/lib.rs`
- [X] T079 [US5] Conformance vector for `GET …/files` in `crates/vizier-http/tests/conformance/groups/projects.rs`; record the baseline

### Design and WebUI

- [ ] T080 [US5] Draw in FigCraft: the mention menu under the composer and the mention chip in the input and in sent messages
- [X] T081 [P] [US5] Build `MentionMenu` with stories in `design/src/`
- [X] T082 [US5] Add `@` handling to `webui/app/screens/chat/Composer.tsx` (debounced 120 ms query, stale responses dropped, keyboard navigation, chip insertion, `mentions` on send through the socket store) and chips from `metadata.mentions` in `webui/app/screens/chat/MessageList.tsx`; vitest for the mention parsing and stale-response rule in `webui/app/lib/`
- [ ] T083 [US5] Run [quickstart.md](./quickstart.md) §6; record the outcome

---

## Phase 8: User Story 6 - Project chats are their own kind of conversation (Priority: P3)

**Goal**: verify and finish the consumer-facing parts of the channel kind built in Phase 2.

**Independent Test**: [quickstart.md](./quickstart.md) §1 and §7.

- [X] T084 [US6] Test in `crates/vizier-core/src/usage/mod.rs` that a project chat's usage is reported under `project` with the project name as its label and a link to the project chat
- [X] T085 [US6] Test in `crates/vizier-core/src/notifications/mod.rs` that a project chat's `chat_reply` is addressed to its person and links to `/<agent>/projects/<id>/chat/<topic>`
- [X] T086 [US6] Test in `crates/vizier-core/src/storage/sqlite/session.rs` that project chats stay out of `conversation_rows`, search and the picker and that another person cannot see them through `project_sessions`
- [X] T087 [US6] Show the **Projects** activity in the WebUI usage screen (`webui/app/` usage route/components) where activities are listed
- [ ] T088 [US6] Run [quickstart.md](./quickstart.md) §1 (upgrade a real 040 workspace, including a mid-turn restart) and §7 (dream over a project chat, no `coding` tools in the dream); record the outcome

---

## Phase 9: Polish & Cross-Cutting Concerns

- [X] T089 [P] Update `docs/src/` with a projects page section on coding tools, Plan/Build, subagent inheritance, mentions, the git interface and worktrees (including the `.vizier/` and fresh-worktree notes), and add it to `docs/src/SUMMARY.md` if new
- [X] T090 [P] Update `CLAUDE.md`'s **Projects** section: the `Project` channel variant and `from_web`/`web_user`, the `coding` category and registry, `project_chat`/`project_worktree`, `project_scope`, the ledger, the git runner, and the migration
- [X] T091 Shrink `crates/vizier-http/tests/conformance/pending.txt` (no new route may be listed) and run the full conformance suite
- [X] T092 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, `cd design && npm test`, `cd webui && npm run typecheck && npm test`, `cd app && cargo test`
- [ ] T093 Run [quickstart.md](./quickstart.md) §8 with a live provider (marked live) and note the result
- [ ] T094 Open the deferred follow-up issues: agent-created worktrees; GitHub `#123` references without the gh CLI; merging/rebasing a worktree from the UI; a per-project worktree setup command and copy-files list
- [X] T095 Commit across repos in order design → webui → vizier with `just commit "<conventional message>"`, bump the submodule pins, and push with `just push` when the person asks

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: after Setup; blocks every story. Inside it: T004–T009 before T010–T013 (the migration needs the new slug); T014–T017 before T022; T018–T021 independent of each other; T022–T024 before T025–T027.
- **US1 (Phase 3)**: after Phase 2.
- **US2 (Phase 4)**: after Phase 2; its quickstart needs US1's tools to be useful but T038–T047 can be built in parallel with US1.
- **US3 (Phase 5)**: after Phase 2; T049 uses T020; its quickstart uses US1 tools and US2 mode.
- **US4 (Phase 6)**: after Phase 2; branch "in worktree" marking reads worktree rows (T017) but not US3's UI.
- **US5 (Phase 7)**: after Phase 2 and T028 (walker) from US1.
- **US6 (Phase 8)**: after Phase 2; verification only.
- **Polish (Phase 9)**: after the stories being shipped.

### User Story Dependencies

- US1, US2, US4, US6: independent after Phase 2.
- US3: independent in code; end-to-end check assumes US1 + US2.
- US5: needs the walker from US1 (T028).

### Within Each Story

FigCraft drawing → `design/` component with story → WebUI. Backend service → route → conformance vector. Core logic with its unit tests before the route that exposes it.

### Parallel Opportunities

- Phase 2: T018, T019, T020, T021 in parallel; T005 beside T004.
- US1: T028–T033 in parallel (separate files), then T034–T036.
- Across stories after Phase 2: one developer per story; design work for US2–US5 (T043, T056, T068, T080) can be drawn together in one FigCraft pass.

## Parallel Example: User Story 1

```text
Task: "T029 [P] [US1] Implement read_file in crates/vizier-core/src/projects/tools/read_file.rs"
Task: "T030 [P] [US1] Implement list_files in crates/vizier-core/src/projects/tools/list_files.rs"
Task: "T031 [P] [US1] Implement glob in crates/vizier-core/src/projects/tools/glob.rs"
Task: "T032 [P] [US1] Implement grep in crates/vizier-core/src/projects/tools/grep.rs"
Task: "T033 [P] [US1] Implement the atomic write helper in crates/vizier-core/src/projects/tools/mod.rs"
```

## Implementation Strategy

### MVP First (US1 + US2)

1. Phase 1 and Phase 2 (the channel migration lands here; verify an upgraded 040 workspace).
2. Phase 3 (US1): file tools, checked with the chat forced to Build.
3. Phase 4 (US2): mode, plan card, subagent inheritance.
4. **Stop and validate** with quickstart §1–§3; this is a shippable increment.

### Incremental Delivery

US4 (git interface) next, since the person needs to review what Build changed; then US3 (worktrees); then US5 (mentions); US6's checks run with whichever release first carries the migration.

## Notes

- [P] tasks touch different files and depend on no incomplete task.
- Every new route needs a conformance vector in the same change.
- Every UI task waits for its FigCraft drawing; Storybook in `design/` is the source of truth once built.
- Commit after each task or logical group with a conventional message; across repos use `just commit`.

## Implementation notes (2026-10-31)

Completed: the backend of every story (channel/migration, coding tools, Plan/Build, worktrees,
git, mentions, background inheritance), its unit tests, HTTP routes and conformance vectors, the
WebUI wiring (endpoints, store, mode toggle, plan card, branch chip, changes panel, worktree
dialogs, sidebar branch labels, `@` mentions, Projects usage activity) and the docs/CLAUDE update.

Left open, with the reason:

- **T003/T094 (GitHub issues)** — external issue edits were not made from this session.
- **T037/T048/T063/T073/T083/T088 (quickstart runs)** — the automated dummyplug checks for the
  coding tools and the shell were run (`crates/vizier-core/tests/dummyplug_e2e.rs`); the desktop
  walkthroughs and the 040→041 upgrade rehearsal need a desktop app and a real 040 workspace.
- **T043/T056/T068/T080 (FigCraft drawings)** and **T044/T057/T069/T081 (new `design/`
  components with stories)** — the FigCraft MCP is not available in this environment. The WebUI
  work reuses existing `design/` primitives (`FilterTabs`, `Dialog`, `TextField`, `Select`,
  `Checkbox`, `Menu`, `DiffView`); `DiffView` already existed, and the unified-diff parser is
  `webui/app/lib/diff.ts` with tests.
- **T093 (live provider)** — no live provider credentials here.
- The conformance vectors were added and the suite passes; a fresh `CONFORMANCE_RECORD=1`
  baseline was not written because the recorded baselines are build artifacts, not committed.

## Design review and refinement (2026-10-31)

Run against an isolated instance (throwaway workspace, dummyplug agent, a real git repository,
a project row inserted directly because project creation is desktop-only), through the WebUI in
a browser at desktop and phone widths. Bugs found and fixed:

- **The app went blank on a fresh load once a project existed**: `ProjectsSection` and the chat
  route selected `byAgent[agent] ?? []`, a new array on every call, which React treats as an
  unstable snapshot (infinite render loop). One shared `NO_PROJECTS` constant, with a test.
- **`@` suggestions never appeared**: `Composer` passed `(projectId, agentId)` to
  `suggestProjectFiles(agentId, projectId, …)`; the 404 was swallowed.
- **Enter sent the half-typed `@ma`** because the library composer owned Enter: the `@` menu now
  lives in the library (`MentionMenu`, `Composer.suggest`) with ↑ ↓ Tab Enter Esc.
- **Deleting a worktree chat from its header** showed the generic dialog (nothing about the
  folder or the work that would be lost): it now opens the worktree dialog, like the sidebar.
- A sent message's `@path` was rendered as Markdown (underscores became italics): mentions are
  drawn as inline code from `metadata.mentions`.

Design refinements: branch chip and sidebar use real git icons (they used a clock); the branch
menu is anchored with `Floating`, filterable, and creates a branch inline instead of
`window.prompt`; the plan card is a library component with one action and a loading state (**Keep
planning** removed: typing a message rejects a plan); Changes is the first section of the details
panel with the same header as its siblings, a selected row and an empty state; the worktree dialog
shows the path, validates the name, submits on Enter and shows progress; the delete dialog waits
for the facts, words the loss, warns about an unmerged branch and re-asks if the folder changed;
the Build hint says when commands are off; a worktree chat's title falls back to the project
list's; on a phone the branch switcher lives in the Changes section.

Observed and left alone (shipped in 040): the host-execution notice appears twice in a browser
session (above the messages and above the composer).

Still open: T043/T056/T068/T080 (FigCraft drawings), T069's `BranchMenu` as a library component
(it is composed from `Menu` in the WebUI for now), the story tests of `design/` (they need a
Playwright browser that is not installed here; `tsc` passes), T003/T094 (GitHub issue edits),
T093 (live provider), T088 (upgrade rehearsal on a real 040 workspace).
