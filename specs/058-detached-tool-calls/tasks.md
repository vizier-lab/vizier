---

description: "Task list for detached scripts (replaces background jobs)"
---

# Tasks: Detached scripts

**Input**: Design documents from `/specs/058-detached-tool-calls/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: The spec does not ask for TDD, but the constitution's gates (dummyplug end to end, spec 057 fence tests, conformance vectors, running the binary for migrations) require the test tasks below; they sit in the story they verify and are written with the code.

**Organization**: grouped by user story. US1 is the MVP. US2 removes the old feature and cannot ship without US1. US3 adds control, lifecycle handling and the WebUI panel. **Cancelling from the WebUI is deferred** (spec Assumptions); only the agent cancels.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1, US2, US3 (maps to the spec's user stories)
- Paths are from the repository root. `core/` = `crates/vizier-core/src/`, `http/` = `crates/vizier-http/src/`

---

## Phase 1: Setup

- [ ] T001 Create branch `058-detached-tool-calls` in the workspace and every submodule (`design`, `webui`, `landing`, `dummyplug`) with `just checkout 058-detached-tool-calls`, and confirm with `just status` (constitution: matching submodule branches)
- [ ] T002 [P] Confirm the sandbox time limit is wall-clock for a script that mostly waits on tools: read `core/sandbox/mod.rs` (`SandboxLimits.timeout`, documented as Monty's CPU clock limit) and `core/sandbox/runtime.rs`, add a test in `core/sandbox/runtime.rs` that runs a script awaiting a 2 s bridge call under a 1 s limit, and record the answer in `research.md` D3; if it is CPU-only, add a wall-clock bound around `sandbox::execute` in `core/agents/tools/python/mod.rs`
- [ ] T003 [P] Confirm session hooks tolerate a closed turn: read `core/agents/hook/`, add a test that a hook publishing to a closed `ResponseSender` returns without error, and fix the hook if it does not

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the storage, types and seams all three stories use. No behaviour change for agents yet.

- [ ] T004 Add the value types in `core/schema/detached.rs`: `DetachedScript`, `DetachedScriptState` (`running`, `succeeded`, `failed`, `timed_out`, `cancelled`, `interrupted`), `DetachedScriptSnapshot`, `DetachedScriptDetail`, the handle id generator (`d_` + 8 base36), constants `DEFAULT_TIMEOUT_SECS` 600, `MAX_TIMEOUT_SECS` 3600, `MAX_BACKGROUND_DEPTH` 3, and the outcome and tool-call caps (32 KiB, 500 characters per argument preview); re-export from `core/schema/mod.rs` (reuse `Canceller`)
- [ ] T005 Define `DetachedScriptStorage` in `core/storage/detached_script.rs` (open, record_progress, close_final, cancel returning the cascaded ids, claim_deliverable, list, get, interrupt_running (by agent, optionally by id set), delete_for_session, delete_for_agent, deliverable_sessions) and add it to the `VizierStorageProvider` supertrait list in `core/storage/mod.rs`
- [ ] T006 Implement `DetachedScriptStorage` for `SqlStorage` in `core/storage/sqlite/detached_script.rs`: every write calls `check_fence` in its own transaction and stores `owner_node`/`owner_token`; each transition out of `running` is one guarded update (`WHERE id = ? AND state = 'running'`); `claim_deliverable` is one transaction that sets `delivered_at` and returns the rows; `record_progress` returns the row's current state so a task can see it was cancelled
- [ ] T007 In the SQLite schema init (`core/storage/sqlite/mod.rs`, new `init_detached_script_schema`): create the `detached_script` table and its three indexes, `DROP TABLE IF EXISTS background_job` and `background_piece`, and `DELETE FROM notification WHERE category = 'background_job'` (all three marked as #82 code)
- [ ] T008 [P] Add PostgreSQL migration `core/storage/sql/postgres/migrations/0008_detached_scripts.sql` (create the table and indexes, drop the two old tables, delete `background_job` notifications) and register it in `core/storage/sql/postgres/migrate.rs`
- [ ] T009 Add one fence test per `DetachedScriptStorage` write to `core/storage/sqlite/fence_tests.rs` (a newer owner's claim makes `open`, `record_progress`, `close_final`, `cancel`, `claim_deliverable` and `interrupt_running` fail with `OwnershipLost`; a process holding no fence is never refused), plus race tests (cancel versus close, two claimers); run them on both backends: `cargo test -p vizier-core fence` and the same with `VIZIER_TEST_DATABASE_URL` set (quickstart §1)
- [ ] T010 [P] Add `detached: bool` (default false) and `parent_detached_id: Option<String>` to `ToolContext` in `core/agents/tools/mod.rs`, update every literal constructor (`core/agents/tools/python/bridge.rs` tests, `core/tests_support.rs`, and the other `ToolContext {` sites `cargo check` finds), and make `turn_catalogue` drop entries with `ends_turn` when `ctx.detached`
- [ ] T011 [P] Split `ExecutePython::call` in `core/agents/tools/python/mod.rs` into `run_script(code, limits, bridge) -> (ExecutionReport, attachments)` (build the bridge, run, redact) and the inline wrapper; no behaviour change, existing tests stay green
- [ ] T012 [P] Add `VizierRequestContent::DetachedDelivery` (no payload, non-durable) and `parent_detached_id: Option<String>` (`serde(skip)`, never accepted from clients) to `VizierRequest` in `core/schema/request.rs`; update `is_durable`, `Default` and the literal sites; make the turn's `ToolContext` take `parent_detached_id` from its request where contexts are built (`core/agents/process.rs`)

**Checkpoint**: `cargo test -p vizier-core fence` passes on SQLite and PostgreSQL and `cargo clippy -p vizier-core` is clean; nothing uses the new code yet.

---

## Phase 3: User Story 1 - Run a slow script without blocking the agent (Priority: P1) 🎯 MVP

**Goal**: `execute_python` with `detached: true` returns a handle at once; the script's report is delivered exactly once, in the next inner round of a running turn or as a wake-up turn when idle.

**Independent Test**: quickstart steps 2 to 5, 9, 10c, 11 and 12 against a dummyplug agent.

- [ ] T013 [US1] Implement `DetachedScripts` in `core/agents/detached/mod.rs`: `launch` (refuse a scheduled-task or dream session and depth 3; open the row; `tokio::spawn`; one cancel `watch` per live task, kept per agent so an agent's tasks can be found), and the task: context clone with `background_depth + 1`, `interactive: false`, `client: None`, `detached: true`, `parent_detached_id` = its own id, fresh `pending_attachments`; `run_script` under the script's limit; progress writes at most once per second that stop the task if the row reads `cancelled`; the final transition; then send `DetachedDelivery`. Instrument with a `detached_script` span created inside the launching span (research D3)
- [ ] T014 [P] [US1] Implement `render_delivery` in `core/agents/detached/delivery.rs` (the `[Background work finished]` text of contracts/detached-flag.md §3: id, intent, elapsed, state, result, stdout, error or reason, session files named, 32 KiB cut marker) with a unit test per state
- [ ] T015 [US1] Construct `DetachedScripts` in `core/dependencies.rs` (storage, transport, notifications) and expose it on `VizierDependencies`; add `pub mod detached;` to `core/agents/mod.rs`
- [ ] T016 [US1] Add `detached` and `timeout_secs` to `ExecutePythonInput` in `core/agents/tools/python/mod.rs`: validate (`timeout_secs` needs `detached`, 1..=3600); on `detached` call `DetachedScripts::launch` and return the handle `{"detached": true, "id", "state", "timeout_secs"}`; refusal messages exactly as contracts/detached-flag.md §2
- [ ] T017 [US1] In `core/agents/tools/python/bridge.rs` delete the `tool == "parallel_subagents"` cancel special case and add the progress hook that reports each finished `ToolInvocationRecord` (redacted, argument previews cut to 500 characters) to `DetachedScripts` for a detached bridge; make `parallel_subagents` copy `ctx.parent_detached_id` onto the requests it sends (`core/agents/tools/subagents.rs`); keep the secret-scrub test green
- [ ] T018 [US1] Deliver at the round boundary in `prompt_with` (`core/agents/agent/mod.rs`): after a round's tool results and before the next completion, call `claim_deliverable(session)`, and with rows add one user message from `render_delivery` after the results (the loaded-images pattern), saved as a history entry with `sender: background`; leave the closing-completion path untouched
- [ ] T019 [US1] Handle `DetachedDelivery` in `core/agents/process.rs`: claim; no rows means no turn and a `debug` log line `detached delivery: nothing to deliver`; rows run a turn through the `Chat` arm (zero auto-context passages, queued behind a running turn, republished on the session-event broadcast); in `core/agents/tools/mod.rs` make `is_interactive_turn` return **true** for `DetachedDelivery` in a conversation with a person on it (research D2), and render the turn's prompt in `chat_turn_prompt`
- [ ] T020 [US1] Save files a detached script produced as session files (the loop's path: `send_file_resolve`, `send_file_upload`, `save_session_file`) from the task in `core/agents/detached/mod.rs`, and name them in the delivery
- [ ] T021 [P] [US1] Write the guidance (when to detach, that results arrive later, what is unavailable inside, inspect and cancel with the three tools) in `core/agents/agent/system_prompt/boot.rs` and `execute_python_description` in `core/agents/tools/python/mod.rs`
- [ ] T022 [P] [US1] Make a detached `execute_python` result read as "running in the background": `ExecutionReport::ending_call` in `core/sandbox/` ignores a handle (unit test); in `webui/` update `app/lib/trail.ts`, `app/screens/chat/MessageList.tsx` and `app/api/frames.ts`, with tests in `app/lib/trail.test.ts`
- [ ] T023 [P] [US1] Notifications: add `NotificationCategory::BackgroundWork` (`background_work`) in `core/schema/notification.rs`, create one notification per final non-cancelled script through `core/notifications/mod.rs` with wording in `core/notifications/text.rs`; confirm `core/channels/woken.rs` republishes delivery turns to Discord and Telegram unchanged
- [ ] T024 [P] [US1] Unit tests in `core/agents/detached/mod.rs`: launch refusals (scheduled-task session, dream session, depth 3), time-limit abort, exactly-once claim with two racing claimers, a task stopping when its row reads `cancelled`, redaction of stored `intent`, `code`, `tool_calls` and `outcome`
- [ ] T025 [P] [US1] Test in `core/agents/tools/python/bridge.rs` that a detached script's catalogue has no `ask_question`, `ask_sensitive_input`, client or turn-ending functions and that `list_tools()` agrees
- [ ] T026 [US1] Add dummyplug end-to-end tests to `core/tests/dummyplug_e2e.rs` for quickstart steps 2, 3, 4, 5, 9, 11 and 12 (handle at once, idle delivery, mid-turn delivery without an extra turn, ten close races, timeout, redaction, files)
- [ ] T027 [US1] Add a Workspace test for quickstart step 10c (a detached script launched from a mention wakes the agent in its topic session and the reply reaches the timeline) to `crates/vizier-core/tests/workspaces_service.rs` or `scripts/workspace-e2e.mjs`
- [ ] T028 [US1] Commit the WebUI change of T022 on the `webui` branch and bump vizier's pin (design → webui → vizier, `just commit`), so the MVP checkpoint is a consistent workspace

**Checkpoint**: a detached script returns at once and is delivered exactly once in both situations; `cargo test -p vizier-core detached` passes. US1 is shippable alone beside the still-present old feature.

---

## Phase 4: User Story 2 - Delegate to other agents without a dedicated tool (Priority: P1)

**Goal**: sub-agent fan-out is a detached script, and the old background-jobs feature is gone (SC-007).

**Independent Test**: quickstart steps 6 and 13: one detached script fans out to a second agent, one delivery carries both answers, and the removal grep finds only the allowed leftovers.

- [ ] T029 [US2] Delete `core/agents/tools/background_jobs.rs` and the `background_subagents` half of `core/agents/tools/subagents.rs` (keep `parallel_subagents`), remove their registration from `BaseTools::new()` and any `DREAM_TOOL_NAMES` or category-table entries in `core/agents/tools/mod.rs`, and add `background_subagents`, `list_background_jobs` and `cancel_background_job` to `RETIRED_TOOL_NAMES` with the hint of contracts/detached-flag.md §5
- [ ] T030 [US2] Delete `core/agents/background/` (`mod.rs`, `report.rs`, `wait.rs`), `core/schema/background.rs`, `core/storage/background_job.rs` and `core/storage/sqlite/background_job.rs`; remove the `BackgroundReport` request kind and every arm matching it (`core/agents/process.rs`, `core/agents/tools/mod.rs`, `core/recovery.rs`, `core/recovery_tests.rs`), the `BackgroundJobStorage` supertrait entry, `NotificationCategory::BackgroundJob` (`core/schema/notification.rs`, `core/notifications/`), and the references in `core/storage/sqlite/{mod,session,history}.rs`, `core/storage/mod.rs`, `core/dependencies.rs`, `core/usage/mod.rs`, `core/utils/mod.rs`, `core/schema/{request,project,mod}.rs`, `core/service/{topics,agents}.rs`, `core/projects/` and `core/questions/` that `cargo check` finds
- [ ] T031 [US2] Delete `core/service/jobs.rs`, `http/api/v1/agents/jobs.rs` and its routes in `http/api/v1/agents/mod.rs`; remove `SessionFrame::Job` from `core/transport/mod.rs` and its uses in `http/topic.rs`, `http/state.rs`, `http/lib.rs` and `http/api/v1/{agents/channel.rs,notifications.rs,catalogue.rs,providers/mod.rs}`; `cargo check --workspace` is green
- [ ] T032 [P] [US2] Remove the old `background_job` rows from `core/storage/sqlite/fence_tests.rs` (the new ones are T009) and the job references in `scripts/client-tools-e2e.mjs`
- [ ] T033 [P] [US2] Rewrite CLAUDE.md's "Background jobs" paragraphs (Agents, Storage, Tools) for detached scripts, and update `docs/src/configuration/agents.md` (the detached option, what is unavailable inside, delivery timing, the agent's three tools, WebUI cancel deferred)
- [ ] T034 [US2] Add the dummyplug end-to-end test for quickstart step 6 (one detached script calling `parallel_subagents` to a second agent; one delivery with both answers; each piece in its own `Subagent` session) to `core/tests/dummyplug_e2e.rs`
- [ ] T035 [US2] Run the SC-007 grep of quickstart step 13 and fix what it finds until only `RETIRED_TOOL_NAMES`, the migration lines and the #82 note remain

**Checkpoint**: the old feature is gone, the workspace builds, and fan-out works through a detached script.

---

## Phase 5: User Story 3 - Inspect, cancel and see what is running (Priority: P2)

**Goal**: the agent lists, inspects and cancels its scripts; restarts and agent updates mark them interrupted and deletes remove them; people see each script in the Background work panel the way the activity trail shows it. No cancel from the WebUI.

**Independent Test**: quickstart steps 7, 8, 10, 10b and 15.

### Engine, tools and lifecycle

- [ ] T036 [US3] Add cancel to `DetachedScripts` in `core/agents/detached/mod.rs`: guarded `running → cancelled` with cascade to rows whose `parent_id` is a cancelled one; signal every cascaded task held in this process (`SandboxBridge::cancel`, then drop the future); tasks in other processes stop at their next progress write (T013); never deliver a cancelled script
- [ ] T037 [P] [US3] Add `list_detached_scripts`, `get_detached_script` (slim `tool_calls`) and `cancel_detached_script` in `core/agents/tools/detached.rs` (category `agents`, `status` results, not in `DREAM_TOOL_NAMES`), register them in `BaseTools::new()`, and add them to the table pinned by `every_tool_is_in_its_category` in `core/agents/tools/mod.rs`
- [ ] T038 [US3] Stop scripts with their agent: when the supervisor stops an instance (update, restart) in `core/agents/supervisor.rs`/`core/agents/mod.rs`, cancel the agent's in-process tasks and move their rows `running → interrupted` with `interrupt_running`, then send `DetachedDelivery` for those sessions once the new instance is up; on delete, stop the tasks and call `delete_for_agent` inside `delete_agent` (`core/storage/sqlite/agent.rs`)
- [ ] T039 [US3] On claim in `core/recovery.rs` `recover_agent`: `interrupt_running` for the agent and re-send `DetachedDelivery` for every session with a deliverable row; add rows to `core/recovery_tests.rs`: interrupted once and delivered once, nothing for cancelled, and a 50-row case across several sessions with an exactly-once count (SC-002)
- [ ] T040 [P] [US3] Tests for cancel and lifecycle in `core/agents/detached/mod.rs` and `core/tests/agent_host.rs`: cancel stops a running tool and a `parallel_subagents` wait, cascade to a nested script, no delivery, repeat cancel is `already_finished`, cancel loses to a finished script, an agent update interrupts and wakes once, an agent delete leaves no rows

### Service, HTTP and frames (read-only)

- [ ] T041 [US3] Add `core/service/detached.rs` (list and get detail; access: the person must see the agent and own the conversation; `ServiceError` kinds) and register it in `core/service/mod.rs`
- [ ] T042 [US3] Add `http/api/v1/agents/detached.rs` with the two read routes of contracts/http-and-frames.md (`utoipa` annotations) and mount it in `http/api/v1/agents/mod.rs`; no cancel route
- [ ] T043 [US3] Publish `SessionFrame::DetachedScript(DetachedScriptSnapshot)` after commit on launch, progress (count changes), final and cancel (`core/transport/mod.rs`, `core/agents/detached/mod.rs`), map it in `http/topic.rs`, and cancel plus delete a conversation's scripts in the conversation-delete path (`core/service/topics.rs`)
- [ ] T044 [US3] Add conformance vectors `http/tests/conformance/groups/detached.rs` and shrink `tests/conformance/pending.txt`; then regenerate `webui/openapi.json` with `webui/scripts/openapi.mjs`
- [ ] T045 [P] [US3] Integration test for the routes in `http/tests/` (list, detail with `code`, `tool_calls` and `report`, a non-owner gets 404)

### Design system and WebUI (order: design, then webui, then pins)

- [ ] T046 [P] [US3] In `design/` (branch `058-detached-tool-calls`): add `src/components/BackgroundWork/` (collapsible entry: intent title, handle and elapsed time, chevron, body is `RunDetail`; **no cancel control**) with stories for running, done, failed, timed out, interrupted and cancelled, matching frames "058 · A" and "058 · B" of `Vizier shell.figcraft`; remove `BackgroundJob`; rename `StatusStripItemId` `jobs` to `work` in `src/components/StatusStrip/StatusStrip.tsx` with its stories
- [ ] T047 [US3] In `webui/` (after T044 and T046) regenerate `app/api/schema.d.ts`, replace the job frames and endpoints (`app/api/frames.ts`, `app/api/endpoints/chat.ts`), and add a reducer for `detached_script` frames in `app/lib/frames.ts` with tests in `app/lib/frames.test.ts`
- [ ] T048 [US3] In `webui/app/screens/chat/ChatStatusStrip.tsx` replace the jobs item and panel with Background work (design `StatusPanel` and `BackgroundWork` entries, search, All/Running/Done/Stopped tabs from `app/lib/statusFilters.ts`, detail fetched on first expand and refetched on a frame, no cancel control, closes on ×, Escape and an outside press); update `statusFilters.ts`, `ChatStatusStrip.test.tsx` and `statusFilters.test.ts`
- [ ] T049 [US3] Run `cd webui && npm run typecheck && npx vitest run`, check the panel against the FigCraft frames in both themes, then bump pins design → webui → vizier (`just commit`)
- [ ] T050 [US3] Add dummyplug end-to-end tests to `core/tests/dummyplug_e2e.rs` and `core/tests/agent_host.rs` for quickstart steps 7, 8, 10 and 10b (inspect, agent cancel, cascade, refusals, server restart, agent update and delete)

**Checkpoint**: all three stories work; quickstart steps 1 to 15 pass.

---

## Phase 6: Polish and cross-cutting

- [ ] T051 [P] Regenerate `crates/vizier-core/public-api.txt` and review the removed and added public items
- [ ] T052 [P] Add the #82 checklist entries: the `DROP TABLE` migrations (SQLite init and PostgreSQL `0008`), the `background_job` notification delete, and the retired tool names
- [ ] T053 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, each flag alone as CI does (`channel-discord`, `channel-telegram`, `addon-docker`, …), and the PostgreSQL run of quickstart §1; fix what the removal broke
- [ ] T054 Run the whole quickstart (steps 1 to 15, including 10b, 10c and the **live** step 14) once against a built binary on an existing workspace, and record the result in `specs/058-detached-tool-calls/quickstart.md`
- [ ] T055 Open a follow-up issue for **cancelling from the WebUI** (the gateway-to-owner path through `AgentsClient` and the bus, a `POST …/cancel` route, the trash icon) and link it from the spec's Assumptions
- [ ] T056 Open the PR titled `feat(agents)!: replace background jobs with detached scripts`, describing the breaking removal and the deferred WebUI cancel, and link the design and webui PRs

---

## Dependencies and execution order

- **Phase 1** has no dependency; T002 and T003 can run alongside Phase 2.
- **Phase 2** blocks everything. Inside it: T004 → T005 → T006 → T007/T008 → T009; T010, T011 and T012 are independent of the storage chain.
- **US1** needs Phase 2. T013 → T015 → T016 → T017; T018 and T019 need T013 and T014; T020 needs T013. T021 to T025 run in parallel once their code exists. T026 and T027 close the story; T028 follows T022.
- **US2** needs US1 (the replacement works before the old feature is deleted). T029 → T030 → T031 in that order to keep the workspace compiling; T032 and T033 in parallel; T034 and T035 last.
- **US3** needs US2 (the old routes and frames are gone). Engine and lifecycle (T036 to T040) and service/HTTP (T041 to T045) are independent of the design chain. T046 can start as soon as Phase 2 is done; **T047 needs T044 (regenerated OpenAPI) and T046**; then T048 → T049.
- **Polish** needs everything.

## Parallel examples

- Phase 2: T008, T010, T011 and T012 together while T004 to T007 run.
- US1: T014, T021, T022, T023, T024 and T025 together once T013 and T016 exist.
- US3: T037, T040, T045 together; T046 (design repo) in parallel with T036 to T045 (Rust).

## Implementation strategy

1. **MVP**: Phases 1, 2 and 3. An agent can detach a script and get its result delivered; the old background jobs still exist beside it, so nothing is lost. Stop and run quickstart steps 2 to 5, 9, 10c, 11 and 12.
2. **Replace**: Phase 4 removes the old feature once US1 is proven. This is the breaking change; do it on the branch, not behind a flag.
3. **Control, lifecycle and UI**: Phase 5. The Rust half (T036 to T045) and the design half (T046) can be built by different people; the WebUI waits for both.
4. **Polish, the WebUI-cancel follow-up issue, and the PR** last, with the full CI matrix.
