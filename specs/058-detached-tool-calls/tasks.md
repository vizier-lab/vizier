---

description: "Task list for detached scripts (replaces background jobs)"
---

# Tasks: Detached scripts

**Input**: Design documents from `/specs/058-detached-tool-calls/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: The spec does not ask for TDD, but the constitution's gates (dummyplug end to end, spec 057 fence tests, conformance vectors) require the test tasks below; they sit in the story they verify and are written with the code, not before it.

**Organization**: grouped by user story. US1 is the MVP. US2 removes the old feature and cannot ship without US1. US3 adds control, visibility and the WebUI.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1, US2, US3 (maps to the spec's user stories)
- Paths are from the repository root. `core/` = `crates/vizier-core/src/`, `http/` = `crates/vizier-http/src/`

---

## Phase 1: Setup

- [ ] T001 Create branch `058-detached-tool-calls` in the `design` and `webui` submodules with `just checkout 058-detached-tool-calls` (constitution: matching submodule branches)
- [ ] T002 [P] Confirm the sandbox time limit is wall-clock for a script that mostly waits on tools: read `core/sandbox/mod.rs` (`SandboxLimits.timeout`, documented as Monty's CPU clock limit) and `core/sandbox/runtime.rs`, write a test in `core/sandbox/runtime.rs` that runs a script awaiting a 2 s bridge call under a 1 s limit, and record the answer in `research.md` D3; if it is CPU-only, add a wall-clock bound around `sandbox::execute` in `core/agents/tools/python/mod.rs` (plan risk 4)
- [ ] T003 [P] Confirm session hooks tolerate a closed turn: read `core/agents/hook/` and write a failing-first test that a hook publishing to a closed `ResponseSender` returns without error (plan risk 1), fixing the hook if it does not

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the storage, types and seams all three stories use. No behaviour change for agents yet.

- [ ] T004 Add the value types in `core/schema/detached.rs`: `DetachedCall`, `DetachedCallState` (`running`, `succeeded`, `failed`, `timed_out`, `cancelled`, `interrupted`), `DetachedCallSnapshot`, `DetachedCallDetail`, handle id generator (`d_` + 8 base36), constants `DEFAULT_TIMEOUT_SECS` 600, `MAX_TIMEOUT_SECS` 3600, `MAX_BACKGROUND_DEPTH` 3, outcome and tool-call caps; re-export from `core/schema/mod.rs` (reuse `Canceller`)
- [ ] T005 Define `DetachedCallStorage` in `core/storage/detached_call.rs` (open, record_progress, close_final, cancel with cascade ids, claim_deliverable, list, get, sweep_interrupted, delete_for_session, deliverable_sessions) and add it to the `VizierStorageProvider` supertrait list in `core/storage/mod.rs`
- [ ] T006 Implement `DetachedCallStorage` for `SqlStorage` in `core/storage/sqlite/detached_call.rs`: every write calls `check_fence` in its own transaction and stores `owner_node`/`owner_token`; each transition out of `running` is one guarded update; `claim_deliverable` is one transaction that sets `delivered_at` and returns the rows (data-model.md)
- [ ] T007 Create the table and indexes in the SQLite schema init (`core/storage/sqlite/mod.rs`, new `init_detached_call_schema`) and drop `background_job` and `background_piece` there with `DROP TABLE IF EXISTS` (marked as #82 code)
- [ ] T008 [P] Add PostgreSQL migration `core/storage/sql/postgres/migrations/0008_detached_calls.sql` (create `detached_call`, drop the two old tables) and register it in `core/storage/sql/postgres/migrate.rs`
- [ ] T009 Add one fence test per `DetachedCallStorage` method to `core/storage/sqlite/fence_tests.rs` (a newer owner's claim makes `open`, `record_progress`, `close_final`, `cancel` and `claim_deliverable` fail with `OwnershipLost`; a gateway with no fence is never refused), plus race tests: cancel versus close, and two claimers
- [ ] T010 [P] Add `detached: bool` (default false) to `ToolContext` in `core/agents/tools/mod.rs`, update every literal constructor (`core/agents/tools/python/bridge.rs` tests, `core/tests_support.rs`, other `ToolContext {` sites found by `cargo check`), and make `turn_catalogue` drop entries with `ends_turn` when `ctx.detached`
- [ ] T011 [P] Split `ExecutePython::call` in `core/agents/tools/python/mod.rs` into `run_script(code, limits, bridge) -> (ExecutionReport, attachments)` (build the bridge, run, redact) and the inline wrapper; no behaviour change, existing tests stay green
- [ ] T012 [P] Add `VizierRequestContent::DetachedDelivery` (no payload, non-durable) and an optional `parent_detached_id` (`serde(skip)`, never accepted from clients) to `core/schema/request.rs`; update `is_durable` and the `Default`/literal sites

**Checkpoint**: `cargo test -p vizier-core fence` and `cargo clippy -p vizier-core` pass; nothing uses the new code yet.

---

## Phase 3: User Story 1 - Run a slow script without blocking the agent (Priority: P1) 🎯 MVP

**Goal**: `execute_python` with `detached: true` returns a handle at once; the script's report is delivered exactly once, in the next inner round of a running turn or as a wake-up turn when idle.

**Independent Test**: quickstart steps 2 to 5, 9 and 11 against a dummyplug agent.

- [ ] T013 [US1] Implement `DetachedCalls` in `core/agents/detached/mod.rs`: `launch` (depth and unattended refusals, row, `tokio::spawn`, per-call cancel `watch`), the task (context clone with `background_depth + 1`, `interactive: false`, `client: None`, `detached: true`, fresh attachments; `run_script` under the call's limit; progress writes at most once per second; final transition; send `DetachedDelivery`), `detached_script` span as a child of the launching span (research D3)
- [ ] T014 [P] [US1] Implement `render_delivery` in `core/agents/detached/delivery.rs` (the `[Background work finished]` text per contracts/detached-flag.md §3: id, intent, elapsed, state, result, stdout, error or reason, session files named, 32 KiB cut marker) with unit tests for each state
- [ ] T015 [US1] Construct `DetachedCalls` in `core/dependencies.rs` (storage, transport, notifications) and expose it on `VizierDependencies`; add `pub mod detached;` to `core/agents/mod.rs`
- [ ] T016 [US1] Add `detached` and `timeout_secs` to `ExecutePythonInput` in `core/agents/tools/python/mod.rs`: validate (`timeout_secs` needs `detached`, 1..=3600), on `detached` call `DetachedCalls::launch` and return the handle `{"detached": true, "id", "state", "timeout_secs"}`; refusal messages exactly as contracts/detached-flag.md §2
- [ ] T017 [US1] In `core/agents/tools/python/bridge.rs` delete the `tool == "parallel_subagents"` cancel special case and add the progress hook that reports each finished `ToolInvocationRecord` to `DetachedCalls` for a detached bridge; keep the secret-scrub test green
- [ ] T018 [US1] Deliver at the round boundary in `prompt_with` (`core/agents/agent/mod.rs`): after a round's tool results and before the next completion, call `claim_deliverable(session)`, and with rows add one user message from `render_delivery` after the results (the loaded-images pattern), saved as a history entry with `sender: background`; leave the closing-completion path untouched
- [ ] T019 [US1] Handle `DetachedDelivery` in `core/agents/process.rs`: claim; no rows means no turn and a `debug` log line `detached delivery: nothing to deliver`; rows run a turn through the `Chat` arm (zero auto-context passages, queued behind a running turn, republished on the session-event broadcast as `BackgroundReport` was); add the arm to `is_interactive_turn` and `chat_turn_prompt` in `core/agents/tools/mod.rs`/`process.rs`
- [ ] T020 [US1] Save files a detached script produced as session files (same path the loop uses: `send_file_resolve`, `send_file_upload`, `save_session_file`) from the task in `core/agents/detached/mod.rs`, and name them in the delivery
- [ ] T021 [P] [US1] Write the guidance: when to detach, that results arrive later, what is unavailable inside, in `core/agents/agent/system_prompt/boot.rs` and `execute_python`'s description (`execute_python_description` in `core/agents/tools/python/mod.rs`)
- [ ] T022 [P] [US1] Make a detached `execute_python` result render as "running in the background": `ExecutionReport::ending_call` in `core/sandbox/` must ignore a handle; in the WebUI update `webui/app/lib/trail.ts`, `webui/app/screens/chat/MessageList.tsx`, `webui/app/api/frames.ts` and add tests in `webui/app/lib/trail.test.ts`
- [ ] T023 [P] [US1] Notifications: add category `background_work` in `core/schema/notification.rs`, create one notification per final non-cancelled script through `core/notifications/mod.rs` with wording in `core/notifications/text.rs`; confirm `core/channels/woken.rs` republishes delivery turns to Discord and Telegram unchanged
- [ ] T024 [P] [US1] Unit tests in `core/agents/detached/mod.rs`: launch refusals (scheduled-task session, dream session, depth 3), time-limit abort, exactly-once claim with two racing claimers, redaction of stored `intent`/`code`/`tool_calls`/`outcome`
- [ ] T025 [P] [US1] Test in `core/agents/tools/python/bridge.rs` that a detached script's catalogue has no `ask_question`, `ask_sensitive_input`, client or turn-ending functions and `list_tools()` agrees
- [ ] T026 [US1] Add dummyplug end-to-end tests to `core/tests/dummyplug_e2e.rs` for quickstart steps 2, 3, 4, 5, 9 and 11 (handle at once, idle delivery, mid-turn delivery without an extra turn, ten close races, timeout, redaction)

**Checkpoint**: a detached script returns at once and is delivered exactly once in both situations; `cargo test -p vizier-core detached` passes. US1 is shippable alone behind the still-present old feature.

---

## Phase 4: User Story 2 - Delegate to other agents without a dedicated tool (Priority: P1)

**Goal**: sub-agent fan-out is a detached script, and the old background-jobs feature is gone (SC-007).

**Independent Test**: quickstart steps 6 and 13: one detached script fans out to a second agent, one delivery carries both answers, and the removal grep finds only the allowed leftovers.

- [ ] T027 [US2] Delete `core/agents/tools/background_jobs.rs` and the `background_subagents` half of `core/agents/tools/subagents.rs` (keep `parallel_subagents`), remove their registration from `BaseTools::new()` and any `DREAM_TOOL_NAMES`/category-table entries in `core/agents/tools/mod.rs`, and add `background_subagents`, `list_background_jobs`, `cancel_background_job` to `RETIRED_TOOL_NAMES` with the hint text from contracts/detached-flag.md §5
- [ ] T028 [US2] Delete `core/agents/background/` (`mod.rs`, `report.rs`, `wait.rs`), `core/schema/background.rs`, `core/storage/background_job.rs` and `core/storage/sqlite/background_job.rs`; remove the `BackgroundReport` request kind and every arm that matches it (`core/agents/process.rs`, `core/agents/tools/mod.rs`, `core/recovery.rs`, `core/recovery_tests.rs`), the `BackgroundJobStorage` supertrait entry, and the references in `core/storage/sqlite/{mod,session,history}.rs`, `core/storage/mod.rs`, `core/dependencies.rs`, `core/usage/mod.rs`, `core/utils/mod.rs`, `core/schema/{request,project,mod}.rs`, `core/service/{topics,agents}.rs`, `core/notifications/` (old category), `core/projects/` and `core/questions/` mentions found by `cargo check`
- [ ] T029 [US2] Delete `core/service/jobs.rs`, `http/api/v1/agents/jobs.rs` and its routes in `http/api/v1/agents/mod.rs`; remove the job frame (`SessionFrame::Job`) from `core/transport/mod.rs`, `http/topic.rs`, `http/state.rs`, `http/lib.rs`, `http/api/v1/{agents/channel.rs,notifications.rs,catalogue.rs,providers/mod.rs}` where they mention it; `cargo check --workspace` is green
- [ ] T030 [P] [US2] Fix the old-job fence rows in `core/storage/sqlite/fence_tests.rs` (remove the `background_job` rows; the new ones are T009) and the references in `scripts/client-tools-e2e.mjs`
- [ ] T031 [P] [US2] Rewrite CLAUDE.md's "Background jobs" paragraphs (Agents, Storage, Tools sections) for detached scripts, update `docs/src/configuration/agents.md` (the detached option, what is unavailable inside, delivery timing) and add `docs/src/` page text if a sub-agents page mentions jobs
- [ ] T032 [US2] Add the dummyplug end-to-end test for quickstart step 6 (one detached script calling `parallel_subagents` to a second agent; one delivery, both answers, each piece in its own `Subagent` session) to `core/tests/dummyplug_e2e.rs`
- [ ] T033 [US2] Run the SC-007 grep from quickstart step 13 and fix what it finds until only `RETIRED_TOOL_NAMES`, the `DROP TABLE` lines and the #82 note remain

**Checkpoint**: the old feature is gone, the workspace builds, and fan-out works through a detached script.

---

## Phase 5: User Story 3 - Inspect, cancel and see what is running (Priority: P2)

**Goal**: the agent and people can list, inspect and cancel detached scripts; restarts mark them interrupted; the Background work panel shows them like the activity trail shows a script.

**Independent Test**: quickstart steps 7, 8, 10 and 15.

### Engine, tools and recovery

- [ ] T034 [US3] Add cancel to `DetachedCalls` in `core/agents/detached/mod.rs`: guarded `running → cancelled`, signal the task (`SandboxBridge::cancel`, drop the future), cascade to rows with `parent_id`, never deliver; plumb `parent_detached_id` from the launching script into sub-agent requests (`core/agents/tools/subagents.rs`, `core/agents/process.rs`)
- [ ] T035 [P] [US3] Add `list_detached_calls`, `get_detached_call` and `cancel_detached_call` in `core/agents/tools/detached.rs` (category `agents`, `status` results, not in `DREAM_TOOL_NAMES`), register them in `BaseTools::new()` and add them to the table pinned by `every_tool_is_in_its_category`
- [ ] T036 [US3] On claim in `core/recovery.rs` `recover_agent`: mark the agent's `running` rows `interrupted` and re-send `DetachedDelivery` for every session with a deliverable row; add rows to `core/recovery_tests.rs` (interrupted once, delivered once, nothing for cancelled)
- [ ] T037 [P] [US3] Tests for cancel in `core/agents/detached/mod.rs`: cancel stops a running tool and a `parallel_subagents` wait, cascade to a nested script, no delivery, repeat cancel is `already_finished`, cancel loses to a finished script

### Service, HTTP and frames

- [ ] T038 [US3] Add `core/service/detached.rs` (list, get detail, cancel; access: the person must see the agent and own the conversation; `ServiceError` kinds) and register it in `core/service/mod.rs`
- [ ] T039 [US3] Add `http/api/v1/agents/detached.rs` with the three routes of contracts/http-and-frames.md (`utoipa` annotations, `409` with the snapshot on an already-final cancel) and mount it in `http/api/v1/agents/mod.rs`
- [ ] T040 [US3] Publish `SessionFrame::DetachedCall(DetachedCallSnapshot)` after commit on launch, final and cancel (`core/transport/mod.rs`, `core/agents/detached/mod.rs`), map it in `http/topic.rs`, and cancel plus delete a conversation's scripts in the conversation-delete path (`core/service/topics.rs`)
- [ ] T041 [P] [US3] Add conformance vectors `http/tests/conformance/groups/detached.rs` (state-free: cancel only against a final script) and shrink `tests/conformance/pending.txt`; regenerate `webui/openapi.json` with `webui/scripts/openapi.mjs`
- [ ] T042 [P] [US3] Integration test for the routes in `http/tests/` (list, detail with `code`/`tool_calls`/`report`, cancel 200 and 409, a non-owner gets 404)

### Design system and WebUI (order: design, then webui, then pins)

- [ ] T043 [US3] In `design/` (branch `058-detached-tool-calls`): add `src/components/BackgroundWork/` (collapsible entry: intent title, handle and elapsed time, trash icon while running, chevron, body is `RunDetail`), stories for running, done, failed, timed out, interrupted and cancelled, matching frames "058 · A/B" of `Vizier shell.figcraft`; remove `BackgroundJob`; rename `StatusStripItemId` `jobs` to `work` in `src/components/StatusStrip/StatusStrip.tsx` with its stories
- [ ] T044 [US3] In `webui/` regenerate `app/api/schema.d.ts`, replace the job frames and endpoints (`app/api/frames.ts`, `app/api/endpoints/chat.ts`), and add a reducer for `detached_call` frames in `app/lib/frames.ts` with tests in `app/lib/frames.test.ts`
- [ ] T045 [US3] In `webui/app/screens/chat/ChatStatusStrip.tsx` replace the jobs item and panel with Background work (design `StatusPanel`, `BackgroundWork` entries, search, All/Running/Done/Stopped tabs from `app/lib/statusFilters.ts`, lazy detail fetch on first expand, trash calls cancel, closes on ×, Escape and outside press); update `statusFilters.ts`, `ChatStatusStrip.test.tsx`, `statusFilters.test.ts`
- [ ] T046 [P] [US3] Make the notification list tolerate old `background_job` rows and render `background_work` (`webui/app/api/endpoints/notifications.ts`, `webui/app/lib/notifications.ts`, with a test)
- [ ] T047 [US3] Run `cd webui && npm run typecheck && npx vitest run`, then bump pins design → webui → vizier (`just commit`), checking the panel against the FigCraft frames in both themes
- [ ] T048 [US3] Add dummyplug end-to-end tests to `core/tests/dummyplug_e2e.rs` and `core/tests/agent_host.rs` for quickstart steps 7, 8 and 10 (inspect, cancel, cascade, refusals, restart marks `interrupted` and wakes once)

**Checkpoint**: all three stories work; quickstart steps 1 to 15 pass.

---

## Phase 6: Polish and cross-cutting

- [ ] T049 [P] Regenerate `crates/vizier-core/public-api.txt` and review the removed and added public items
- [ ] T050 [P] Add the #82 checklist entries: the two `DROP TABLE` migrations, the retired tool names, and tolerance for old `background_job` notifications
- [ ] T051 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features` and each flag alone as CI does (`channel-discord`, `channel-telegram`, `addon-docker`, …), fixing what the removal broke
- [ ] T052 Run the whole quickstart (steps 1 to 15) once against a built binary, including the **live** step 14, and record the result in `specs/058-detached-tool-calls/quickstart.md`
- [ ] T053 Open the PR with the title `feat(agents)!: replace background jobs with detached scripts` and a description listing the breaking removal; link the design and webui PRs (constitution: conventional PR titles, linked branches)

---

## Dependencies and execution order

- **Phase 1** has no dependency; T002 and T003 can run alongside Phase 2.
- **Phase 2** blocks everything. Inside it: T004 → T005 → T006 → T007/T008 → T009; T010, T011, T012 are independent of the storage chain.
- **US1** needs Phase 2. T013 → T015 → T016 → T017; T018 and T019 need T013 and T014; T020 needs T013. T021, T022, T023, T024, T025 are parallel once their code exists. T026 closes the story.
- **US2** needs US1 (the replacement must work before the old feature is deleted). T027 → T028 → T029 must go in that order to keep the workspace compiling; T030, T031 are parallel; T032 and T033 last.
- **US3** needs US2 for the removal of the old routes and frames. Engine tasks (T034 to T037) and service/HTTP tasks (T038 to T042) are independent of the design and WebUI chain (T043 → T044 → T045 → T047). T043 can start as soon as Phase 2 is done, since the design is already drawn.
- **Polish** needs everything.

## Parallel examples

- Phase 2: T010, T011, T012 and T008 together while T004 to T007 run.
- US1: T014, T021, T022, T023, T024 and T025 together once T013 and T016 exist.
- US3: T035, T037, T041, T042, T046 together; T043 (design repo) in parallel with T034 to T042 (Rust).

## Implementation strategy

1. **MVP**: Phases 1, 2 and 3. An agent can detach a script and get its result delivered; the old background jobs still exist beside it, so nothing is lost. Stop and run quickstart steps 2 to 5, 9 and 11.
2. **Replace**: Phase 4 removes the old feature once US1 is proven. This is the breaking change; do it on the branch, not behind a flag.
3. **Control and UI**: Phase 5. The Rust half (T034 to T042) and the design half (T043) can be built by different people; the WebUI waits for the design and the regenerated OpenAPI.
4. **Polish and PR** last, with the full CI matrix.
