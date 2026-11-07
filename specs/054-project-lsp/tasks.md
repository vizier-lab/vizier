---
description: "Task list for Project Language Intelligence"
---

# Tasks: Project Language Intelligence

**Input**: Design documents from `specs/054-project-lsp/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Included. The plan names them, and the constitution requires the dummyplug end-to-end steps for an agent-observable change. Pure logic gets unit tests in the same file, as the repository does elsewhere.

**Organization**: Tasks are grouped by user story, so each story can be implemented and tested alone.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: The user story the task belongs to

## Path Conventions

- Engine: `crates/vizier-core/src/`, abbreviated below only where a full path has already been given for the folder
- HTTP: `crates/vizier-http/src/`
- Design and WebUI are submodules: `design/`, `webui/`. Commit order is design → webui → vizier, with `just commit`
- Decision references such as R6 are in `research.md`

## Rules that apply to every task

- Errors are `crate::Result`, logging is `tracing` only, no `unwrap()` outside tests.
- No `match` on a language id. Server differences come from the table in `languages.rs` or from negotiated capabilities.
- Do not write `Utc::now` or `DateTime<Utc>`; use `crate::time`.
- Keep `cargo test -p vizier-core --no-default-features` passing.

---

## Phase 1: Setup

**Purpose**: Dependencies, config and the empty module.

- [X] T001 Add `url` to `[workspace.dependencies]` in `Cargo.toml` and to `crates/vizier-core/Cargo.toml`, and add `windows-sys` under `[target.'cfg(windows)'.dependencies]` in `crates/vizier-core/Cargo.toml` with the job-object and handle features. Use versions already in `Cargo.lock`, and run `cargo check` in `app/` so `app/Cargo.lock` stays in step (CI `policy` checks native versions)
- [X] T002 Create `crates/vizier-core/src/projects/lsp/mod.rs` with submodules `rpc`, `position`, `languages`, `search_path`, `server`, `sync`, `diagnostics`, and register `pub mod lsp;` in `crates/vizier-core/src/projects/mod.rs`
- [X] T003 [P] Add `LspConfig { max_servers: usize }` (default 6, `#[serde(default)]`) as `VizierConfig.lsp` in `crates/vizier-core/src/config/mod.rs`, with a test that a config file without the section still loads

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: A server can be started, spoken to and stopped; the switch and overrides exist and reach a turn. No agent-visible tool yet.

**⚠️ CRITICAL**: No user story can begin until this phase is complete.

### Protocol and pure logic

- [X] T004 [P] Implement `Content-Length` framing in `crates/vizier-core/src/projects/lsp/rpc.rs`: an async reader and writer over `serde_json::Value`, case-insensitive headers, a 64 MiB frame limit, message classification (response, server request, notification). Unit-test split reads, several frames in one read, a bad length and an oversized frame
- [X] T005 [P] Implement position conversion in `crates/vizier-core/src/projects/lsp/position.rs`: 1-based line and scalar column to and from LSP positions for `utf-8` and `utf-16`, and `locate_symbol(line_text, symbol, occurrence)` on word boundaries (R15). Unit-test CJK, emoji and CRLF lines, a symbol absent from the line, and the second occurrence
- [X] T006 Add location mapping to `crates/vizier-core/src/projects/lsp/position.rs`: `file://` URI to and from path through `url`, a path inside `cwd` to relative, a path outside to `{outside_project, file}` with no absolute path, results under `.vizier/` dropped (R11), a one-line `preview`. Unit-test each, including a Windows-style path under `cfg(windows)`
- [X] T007 [P] Implement the table in `crates/vizier-core/src/projects/lsp/languages.rs` with all 16 rows of `data-model.md` §3: `LanguageDef` and `RootRule` (`Nearest`, `NearestPreferring` for markers that win at any level, `CargoWorkspace`), `language_for(path)`, `find_root(file, cwd, rule)` stopping at `cwd`, `*.ext` marker patterns, and `initialization_options` with `{"checkOnSave": false}` on the Rust row (R17). Unit-test a tempdir copying Vizier's layout: `crates/x` to the repo root, `app/src-tauri` to `app/`, `webui/` to itself; a Gradle multi-module build; a `*.sln`; a file with no marker to `cwd`; an unknown extension to `None`
- [X] T008 [P] Implement `crates/vizier-core/src/projects/lsp/search_path.rs`: lookup honouring `PATHEXT` on Windows, the login-shell `PATH` and well-known folders read once on a miss with a 3 s limit (R4), `filtered_env()` removing the R11 patterns then adding an override's env. Unit-test lookup in a tempdir, `FOO_API_KEY` and `VIZIER_JWT_SECRET` removal, and override env winning
- [X] T009 [P] Add `bind_to_parent(&mut Command)` to `crates/vizier-core/src/agents/shell/process_tree.rs`: `PR_SET_PDEATHSIG` on Linux, a `KILL_ON_JOB_CLOSE` job object on Windows, a no-op on macOS (R9). Keep all platform code in this file

### Test server

- [X] T010 Create `crates/vizier-core/examples/fake_lsp.rs`, a stdio server using only `serde_json`, with `--script` modes used by `quickstart.md`: `diagnostics` (reports an error when the text contains `ERROR`, and one in another file), `pull` (advertises `diagnosticProvider`), `slow`, `progress`, `answers` (definition, references, hover, symbols, 60 references, an outside location), `hang`, `slow-start`, `crash`, `dump-env`, `apply-edit`, `utf16`, `no-init-exit`. It writes what it received to a path given by `--log`

### Host

- [X] T011 Implement one server process in `crates/vizier-core/src/projects/lsp/server.rs`: spawn with `process_tree::isolate`, `bind_to_parent`, `process::no_window`, `kill_on_drop` and `filtered_env`; a reader task and a writer channel; a pending-request map with a 10 s limit (`lsp_timeout`); `initialize` advertising `utf-8` then `utf-16`, pull diagnostics and work-done progress, with `initialized` sent straight after the answer (R9); `$/progress` tracking; readiness per R8; answers to server requests per R11 including `applied: false`; a capped 8 KiB stderr ring; `-32801`/`-32802` retried once; stop as `shutdown`, `exit`, 2 s, kill
- [X] T012 Implement `LspHost` in `crates/vizier-core/src/projects/lsp/mod.rs`: a registry keyed `(project_id, root, language)`, `ensure()` that starts in a spawned task and never waits, the `closed` set (R10), `missing` vs `failed` classification so a command that exits before answering `initialize` with a "not installed" pattern is `missing` (R3), restarts limited to 3 in 10 minutes, `status(project_id)`, `stop_project`, `stop_root`, `stop_language`, `restart`, `stop_all`, and a state-change callback. No capacity or idle logic yet (US4)
- [X] T013 Write `crates/vizier-core/tests/lsp_host.rs` against `fake_lsp`, finding it next to the test binary: handshake, `ready` after quiet, `starting` while progress is open, `slow-start` not blocking `ensure`, `crash` to `failed` after 3, `hang` affecting only its own calls, `no-init-exit` reading `missing`, `dump-env` holding no secrets, `apply-edit` answered `applied: false`, no process left after `stop_all`

### Data, service and turn context

- [X] T014 Add `Project.lsp_enabled`, `ServerOverride` and `LspStatusSnapshot` (`data-model.md` §6, deriving `ToSchema` behind `utoipa`) in `crates/vizier-core/src/schema/project.rs`
- [X] T015 Extend `ProjectStorage` in `crates/vizier-core/src/storage/project.rs` and implement it in `crates/vizier-core/src/storage/sqlite/project.rs`: `add_column_if_missing(project, lsp_enabled, INTEGER NOT NULL DEFAULT 0)`, `open_project` writing the value, `set_project_lsp`, table `project_lsp_override`, `lsp_overrides`, `set_lsp_override`, `delete_lsp_override`, with `env` sealed through the cipher using the AAD in `data-model.md` §2. Add tests: an existing row reads off, a new row reads on, the raw `env` column is an envelope, a delete cascades
- [X] T016 [P] Add `crates/vizier-core/src/storage/sql/postgres/migrations/0005_project_lsp.sql` matching T015, confirm the `postgres_tests!` cases pass, and confirm `vizier storage migrate` copies the new table (fix it if it uses a fixed table list)
- [X] T017 Update `crates/vizier-core/src/service/projects.rs`: `OpenProjectRequest.lsp_enabled` defaulting to `true`, `UpdateProjectRequest.lsp_enabled` under `can_manage`, `lsp_enabled` on the summary and detail, and on switch-off `deps.lsp.stop_project` before returning, with switch-on clearing the `closed` entry
- [X] T018 Create `crates/vizier-core/src/service/project_lsp.rs` and register it in `crates/vizier-core/src/service/mod.rs`: `status` (`can_use`; adds `can_manage`, `languages` and `overrides` only for managers), `set_override` and `delete_override` (`can_manage`, validated per `data-model.md` §2, then `stop_language`), `restart` (`can_manage`, listed roots only, `409 lsp_disabled` when off), with `ServiceError` codes from `contracts/http-api.md`
- [X] T019 Add the four routes to `crates/vizier-http/src/api/v1/agents/projects.rs` with OpenAPI entries, translating only, and add vectors for them to `crates/vizier-http/tests/conformance/groups/projects.rs` (a vector must not change state on a second send), then re-record the baseline
- [X] T020 Add `lsp: Arc<LspHost>` to `crates/vizier-core/src/dependencies.rs`, built with `config.lsp`, and add `lsp_enabled`, `lsp_overrides` and `lsp` to `ProjectTurnContext` in `crates/vizier-core/src/projects/context.rs`, read once in `for_turn`. Update every `ProjectTurnContext` literal in tests (`projects/tools/edit_file.rs`, `projects/tools/write_file.rs`, `projects/shell.rs`, `projects/mentions.rs`, `tests/dummyplug_e2e.rs` and any others the compiler names)
- [X] T021 Add `Language servers: on.` or `off.` to `project_state_md` in `crates/vizier-core/src/agents/agent/system_prompt/project.rs`, with a test that the switch changes only this per-message block and never `project_md`

**Checkpoint**: `cargo test --workspace` passes, a server can be driven from tests, and the switch and overrides work over HTTP. Agents see nothing new except the state line.

---

## Phase 3: User Story 1 - The agent learns that its edit broke something (Priority: P1) 🎯 MVP

**Goal**: A successful edit's result reports whether the code still holds together, without ever falsely claiming it is clean.

**Independent Test**: In a project with an override to `fake_lsp --script diagnostics`, an edit introducing `ERROR` returns `diagnostics.status: current` with one error; reverting it returns `errors: 0`; `--script slow` returns `pending` in under 4 s (`quickstart.md` §3).

### Tests for User Story 1

- [X] T022 [P] [US1] Add to `crates/vizier-core/tests/lsp_host.rs`: `diagnostics` gives `current` with items; `pull` gives `current` and covers another open document; `slow` gives `pending` by 3 s; `progress` gives `pending` even with a clean pull; a push from before the edit is never reported as current
- [X] T023 [P] [US1] Create `crates/vizier-core/tests/lsp_real.rs`, skipping with a visible message when the binary cannot answer `initialize`: clangd with an undeclared identifier, and rust-analyzer with a type error plus a signature change breaking a call site in an already-read file, both `current` within 3 s

### Implementation for User Story 1

- [X] T024 [US1] Implement `crates/vizier-core/src/projects/lsp/sync.rs` document handling: `didOpen`, full-text `didChange` when the sha256 differs, `didSave`, `didClose`, at most 256 open documents with the oldest closed, `file_unsupported` over 2 MiB or for non-UTF-8 (R7). File snapshots come in T054
- [X] T025 [US1] Implement `crates/vizier-core/src/projects/lsp/diagnostics.rs`: the per-URI cache, push handling, `wait_after_write` per R6 (pull where `diagnosticProvider` exists, including other open documents when `interFileDependencies` is true, at most 20; versioned or post-sync push otherwise; 3 s limit; 300 ms progress grace), `DiagnosticsResult` per `contracts/lsp-tools.md`, errors before warnings, hints dropped, capped per FR-023, `other_files`
- [X] T026 [US1] Add `after_write(ctx, file, bytes) -> Option<Value>` in `crates/vizier-core/src/projects/tools/mod.rs`, returning `None` when the switch is off or there is no language, `unavailable` for a missing or failed server, `pending` while starting (and ensuring a start), otherwise `wait_after_write`. It never returns an error
- [X] T027 [US1] Call `after_write` after a successful write and merge a `diagnostics` field in `crates/vizier-core/src/projects/tools/edit_file.rs`, and update its description. Existing refusals and existing fields stay as they are, and the existing tests must pass unchanged
- [X] T028 [P] [US1] Do the same in `crates/vizier-core/src/projects/tools/write_file.rs`
- [X] T029 [P] [US1] In `crates/vizier-core/src/projects/tools/read_file.rs`, after a successful text read, ensure the server in the background and open the document if the server is ready, never delaying or failing the read (FR-017, R6)
- [X] T030 [US1] Run `quickstart.md` §3 through a dummyplug agent and fix what it finds

**Checkpoint**: The MVP works: with a server installed, an agent's edit tells it what it broke.

---

## Phase 4: User Story 2 - A person can see what it is doing and turn it off (Priority: P1)

**Goal**: The popup from the status strip shows servers live and holds the switch; every path that ends a project's use also ends its servers; a missing server is explained.

**Independent Test**: A new project is on with no server running; touching a file with no server installed shows `missing` and the hint; the strip updates with no reload; the switch stops everything (`quickstart.md` §1, §2, §6, §9).

### Tests for User Story 2

- [X] T031 [P] [US2] Add service tests in `crates/vizier-core/src/service/project_lsp.rs`: a viewer gets no `overrides` or `default_command`; a viewer cannot `PATCH lsp_enabled`, restart or set an override; switch-off leaves no process; a mid-turn switch-off gives `lsp_disabled`
- [X] T032 [P] [US2] Add `judge_with` tests in `crates/vizier-core/src/service/inbox.rs`: the frame reaches someone who can use the project and not someone who cannot

### Implementation for User Story 2

- [X] T033 [US2] Stop servers on project removal and worktree deletion in `crates/vizier-core/src/service/projects.rs` (`remove`, `delete_worktree`, before the folder is removed) and when `for_turn` finds the folder gone in `crates/vizier-core/src/projects/context.rs`
- [X] T034 [P] [US2] Stop servers on agent delete in `crates/vizier-core/src/agents/mod.rs`, near `storage.delete_agent`, and on `VizierRuntime::shutdown` in `crates/vizier-core/src/runtime.rs`
- [X] T035 [US2] Add `InboxEvent::ProjectLsp` in `crates/vizier-core/src/transport/mod.rs`, publish it from the host's state-change callback with progress throttled to 250 ms per project, and add `InboxFrame::Lsp` with delivery through `judge_with` in `crates/vizier-core/src/service/inbox.rs` (`contracts/http-api.md`)
- [X] T036 [P] [US2] In `design/src/components/StatusStrip/StatusStrip.tsx`, add the `lsp` id, `dot` and `display` to `StatusStripItem`, keeping `busy` as `live`, and update `StatusStrip.stories.tsx`
- [X] T037 [P] [US2] In `design/src/components/StatusStrip/StatusPanel.tsx`, add `headerAction` between `meta` and the ×, with a story
- [X] T038 [US2] Regenerate `webui/app/api/schema.d.ts` with `scripts/openapi.mjs`, and add `lsp_enabled` and the LSP calls in `webui/app/api/endpoints/projects.ts`
- [X] T039 [P] [US2] Create `webui/app/lib/lsp.ts` (group by language, worst dot, meta line, relative root display) with vitest tests in `webui/app/lib/lsp.test.ts`
- [X] T040 [US2] Handle the `lsp` frame in `webui/app/lib/inboxFrames.ts`, and keep per-project status in `webui/app/stores/projects.ts`, re-reading `GET …/lsp` on `hello` and `resync`, with tests
- [X] T041 [US2] Create `webui/app/screens/chat/LspPanel.tsx` from `specs/054-project-lsp/mockups/lsp-status-popup.html`: the switch in the header, groups, states, the install hint with Copy, **Restart** and **Check again**, Nothing yet, Off and View only. **Do not add any warning text**, by decision
- [X] T042 [US2] Add the `lsp` item to `webui/app/screens/chat/ChatStatusStrip.tsx` for project chats only, and pass it from `webui/app/routes/chat.tsx`, with a test that an ordinary chat has no item
- [X] T043 [US2] Add `lsp_enabled` to the open-project call in `webui/app/screens/projects/OpenProjectDialog.tsx` only if the API needs it explicit; the default of on needs no new control. Leave `TrustDisclosure.tsx` unchanged, by decision
- [ ] T044 [US2] Run `quickstart.md` §1, §2, §6 and §9 and fix what they find, including the `kill -9` step on macOS and a Finder launch of the desktop app

**Checkpoint**: A person can see and stop everything, and a missing server explains itself.

---

## Phase 5: User Story 3 - The agent asks about meaning instead of matching text (Priority: P2)

**Goal**: Five tools answer definition, references, hover, symbols and diagnostics, in Plan and Build.

**Independent Test**: In a fixture with two same-named functions, `lsp_references` returns only real uses, in Plan mode, with relative paths (`quickstart.md` §4).

### Tests for User Story 3

- [X] T045 [P] [US3] Add tool tests to `crates/vizier-core/tests/lsp_host.rs` with `--script answers`: every `status` in `contracts/lsp-tools.md`, no absolute path in any output, 60 references cut to 50 with `truncated` and `hint`, `symbol_not_on_line`, `lsp_starting` in under 1 s, `--script utf16` positions
- [X] T046 [P] [US3] Extend `crates/vizier-core/tests/lsp_real.rs` with `lsp_definition`, `lsp_references` and `lsp_hover` on rust-analyzer and clangd

### Implementation for User Story 3

- [X] T047 [US3] Add a shared query helper in `crates/vizier-core/src/projects/lsp/mod.rs`: resolve path, language and root; ensure; return the `status` for anything not ready; sync the file; locate the symbol; send the request; map locations
- [X] T048 [P] [US3] Create `crates/vizier-core/src/projects/tools/lsp_definition.rs` and `crates/vizier-core/src/projects/tools/lsp_references.rs` per `contracts/lsp-tools.md`
- [X] T049 [P] [US3] Create `crates/vizier-core/src/projects/tools/lsp_hover.rs` and `crates/vizier-core/src/projects/tools/lsp_symbols.rs` (a file, or `query` with a required `path`)
- [X] T050 [P] [US3] Create `crates/vizier-core/src/projects/tools/lsp_diagnostics.rs` (a file asks directly; a folder reports what running servers published, with `scope: reported`)
- [X] T051 [US3] In `crates/vizier-core/src/projects/tools/mod.rs`, add `needs_lsp` to `ProjectToolSpec` and false to the existing entries, add the five entries as `Access::Read`, gate them in `allowed()` and word `refusal()` per the contract. In `crates/vizier-core/src/agents/agent/system_prompt/project.rs`, add the fixed sentence to `project_md`: prefer `lsp_*` to `grep` when offered, read `diagnostics` on an edit, run the build before finishing when the shell is on (R17). Test mode filtering: offered in Plan and Build, absent when off

**Checkpoint**: Stories 1, 2 and 3 work independently, and Plan mode has semantic answers but no writes.

---

## Phase 6: User Story 4 - A repository holding several projects just works (Priority: P2)

**Goal**: The right server for each part of a repository, shared where it should be, bounded overall, and correct after changes made outside the tools.

**Independent Test**: In a fixture with two independent workspaces and a package of another language, touching one file in each runs exactly three servers; a chat touching one part starts one (`quickstart.md` §5).

### Tests for User Story 4

- [X] T052 [P] [US4] Add to `crates/vizier-core/tests/lsp_host.rs`: two chats and a subagent piece on one root share a server; a worktree gets its own; `max_servers: 1` evicts the least recently used; a server with a request in flight is not evicted and the new one reads `lsp_busy`; an idle server stops; `grep`, `glob` and `list_files` start nothing

### Implementation for User Story 4

- [X] T053 [US4] Add capacity and the idle sweep to `LspHost` in `crates/vizier-core/src/projects/lsp/mod.rs`: limit from `config.lsp.max_servers`, `missing` and `failed` entries not counted, a 10-minute idle stop on a task started with the first server
- [X] T054 [US4] Add file snapshots to `crates/vizier-core/src/projects/lsp/sync.rs`: a path-to-mtime walk with `ignore` when a server becomes ready, and `notify_changes(root)` sending `workspace/didChangeWatchedFiles`, restarting the server above 5,000 changes (R7)
- [X] T055 [US4] Call `notify_changes` in the background after a command in `crates/vizier-core/src/projects/shell.rs` and after `git_switch` in `crates/vizier-core/src/service/projects.rs`, never delaying the result, with a `fake_lsp --log` test in `crates/vizier-core/tests/lsp_host.rs`
- [ ] T056 [US4] Run `quickstart.md` §5 and fix what it finds

**Checkpoint**: A repository like Vizier's own needs no setup, and memory stays bounded.

---

## Phase 7: User Story 5 - A person points a language at a specific server (Priority: P3)

**Goal**: The override form. The storage and routes exist from Phase 2, because the end-to-end steps use them.

**Independent Test**: Set an override to a fixture server and it starts instead of the default; remove it and the default returns (`quickstart.md` §3, §7).

### Tests for User Story 5

- [X] T057 [P] [US5] Add to `crates/vizier-core/tests/lsp_host.rs`: an override wins over the table; a missing override command is `missing` naming that command, with no fallback; changing an override stops running servers of that language in every root; a binary inside the project folder is not used unless set by an override; a config file in the project changes nothing

### Implementation for User Story 5

- [X] T058 [US5] Add **Configure** and **Add override** forms to `webui/app/screens/chat/LspPanel.tsx`, per the mockup's Override state: command, arguments, environment, **Save and restart**, **Use default**, the `override` tag, managers only, with tests
- [ ] T059 [US5] Run `quickstart.md` §7 and §8 and fix what they find

**Checkpoint**: All five stories work.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T060 [P] Add a section to `docs/src/configuration/projects.md`: installing a server per language, the switch, overrides, the not-installed state, and **that a server may execute project code, including in Plan mode, and that rust-analyzer's `cargo check` is off by default** (FR-042, R17)
- [X] T061 [P] Document `lsp.max_servers` in `docs/src/configuration/main-config.md`
- [X] T062 [P] Add language intelligence to the Projects section of `CLAUDE.md`, and commit the plan pointer already changed there
- [X] T063 Verify every install hint in `crates/vizier-core/src/projects/lsp/languages.rs` against the server's own documentation, and run any server that can be installed. Add platform-neutral wording where a `brew` hint would mislead on Windows
- [X] T064 Regenerate `crates/vizier-core/public-api.txt` with `just public-api`
- [ ] T065 Confirm the entry for `add_column_if_missing(project, lsp_enabled)` is ready for #82, and ask the person before editing the issue
- [X] T066 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, `cargo test -p vizier --no-default-features --features role-worker`, `scripts/048-agent-state-read-check.sh`, `cd design && npm test`, `cd webui && npm run typecheck && npm test`, and `cd app && cargo test`
- [ ] T067 Confirm the Windows build compiles `bind_to_parent` and the `windows-sys` features through CI's `app` job and the Windows leg of the engine tests
- [ ] T068 Run all of `quickstart.md`, including the live-provider step in §11, and record anything that differs from `research.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: None.
- **Foundational (Phase 2)**: Depends on Setup. Blocks every story.
- **US1 (Phase 3)** and **US2 (Phase 4)**: Depend on Foundational. Independent of each other.
- **US3 (Phase 5)**: Depends on Foundational and on T024 (`sync.rs`) from US1, because a question needs the file synced.
- **US4 (Phase 6)**: Depends on Foundational and T024. Sharing and root behaviour exist from Phase 2; this phase bounds them and adds change notification.
- **US5 (Phase 7)**: Depends on T041 (`LspPanel.tsx`) from US2.
- **Polish (Phase 8)**: Depends on the stories being shipped.

### Within Foundational

T004, T005, T007, T008 and T009 can run together. T006 follows T005. T011 needs T004, T008 and T009. T012 needs T007 and T011. T010 comes before T013. T014 → T015 → T016 and T017 → T018 → T019. T020 needs T012 and T015. T021 needs T020.

### Within Each User Story

Write the story's tests first and see them fail. Then engine, then service, then design, then WebUI, then the quickstart step.

### Parallel Opportunities

- **Phase 2**: T004, T005, T007, T008, T009; later T016.
- **US1**: T022 and T023; T028 and T029 after T026.
- **US2**: T031 and T032; T034; T036 and T037 (one PR in `design/`); T039.
- **US3**: T045 and T046; T048, T049 and T050.
- **Stories**: After Phase 2 and T024, US1, US2, US3 and US4 touch mostly separate files. `projects/tools/mod.rs` (T026, T051) and `lsp/mod.rs` (T012, T047, T053) are the shared ones, so do those one at a time.

---

## Parallel Example: Phase 2 pure logic

```text
T004 rpc.rs framing
T005 position.rs conversion
T007 languages.rs table and roots
T008 search_path.rs lookup and env filter
T009 process_tree.rs bind_to_parent
```

## Parallel Example: User Story 3 tools

```text
T048 lsp_definition.rs, lsp_references.rs
T049 lsp_hover.rs, lsp_symbols.rs
T050 lsp_diagnostics.rs
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Phases 1 and 2.
2. Phase 3.
3. **Stop and validate** with `quickstart.md` §3 and `tests/lsp_real.rs`. There is no UI yet, so turning it off and seeing status go through the API.

Do not release on this alone. New projects are on by default, so **Story 2's popup should ship in the same release as Story 1**, or a person has no visible way to see or stop what is running.

### Incremental Delivery

1. Phases 1-2: nothing visible beyond the state line.
2. US1 and US2 together: the first release-worthy state.
3. US3: semantic questions.
4. US4: bounded memory, and correctness after shell commands. **Worth doing before a release that reaches large repositories**, since each rust-analyzer can peak at 2.4 GB on a repository this size.
5. US5: the override form.

### Parallel Team Strategy

After Phase 2 and T024: one developer on US1, one on US2's service tasks and the UI, one on US3's tools. US4 and US5 follow.

---

## Notes

- A `[P]` task touches different files and has no dependency on an incomplete task.
- Commit after each task or logical group, in design → webui → vizier order, using `just commit`.
- `quickstart.md` steps are the gate for "done": a passing `cargo test` is not enough for an agent-observable change.
- Stop at any checkpoint to validate that story alone.
