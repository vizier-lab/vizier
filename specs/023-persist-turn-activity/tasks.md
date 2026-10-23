# Tasks: Durable Turns: Live Progress That Survives Navigation and Restarts

**Input**: Design documents from `/specs/023-persist-turn-activity/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/websocket-and-replies.md, quickstart.md

**Tests**: The plan names unit tests as gates (see quickstart.md § Gates), so they are included
inside the story that needs them. End-to-end checks are dummyplug steps from quickstart.md.

**Organization**: Tasks are grouped by user story. US1 fixes the reported bug on its own. US2
builds on US1, because resumption reads the steps US1 records. US3 and US4 build on US2.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US5 from spec.md

## Path Conventions

Single repo: Rust in `src/`, WebUI in `webui/app/`, docs in `docs/src/`, scripts in `scripts/`.

## Implementation notes that apply throughout

- Errors: `crate::Result`/`VizierError`, never `unwrap()`/`expect()` outside tests. Logging is
  `tracing` only. A failure to record durability state (row insert, step save) is logged and
  never fails the request or the turn.
- One small deviation from plan.md: the step and stop helpers get their own module,
  `src/agents/steps.rs`, rather than living in `process.rs`. Both `agents/agent/mod.rs` and
  `agents/process.rs` need them, and `process.rs` is already 1,800 lines.
- A new `VizierRequest`/`VizierResponse` literal needs `..Default::default()` (see CLAUDE.md
  § Telemetry).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: a baseline to compare against for FR-002 (the step-by-step record must equal today's).

- [X] T001 Run `cargo test`, `cargo clippy` and `cd webui && npm run typecheck` on the branch before changes, and note any pre-existing failures in `specs/023-persist-turn-activity/baseline.md`. With `just run` and a dummyplug agent `plug` (local shell enabled), send `{"tool":"execute_python","arguments":{"intent":"ref","code":"shell_exec(command=\"echo hi\")"}}` on a fresh topic, and save that topic's `GET …/history` entries (content kinds and order only, no uids or timestamps) to `specs/023-persist-turn-activity/reference-history.json`. Confirm `shell_exec`'s argument name with `describe_tool_function` and note it in `baseline.md`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the step frame and the stop helpers that US1 (abort sealing) and US2 (resume sealing) both use.

**⚠️ CRITICAL**: no user story work begins until this phase is complete.

- [X] T002 [P] Add `SessionFrame::HistoryEntry(SessionHistory)` to `src/transport.rs`, carrying a saved entry with its uid, in the shape `GET …/history` returns. In `handle_socket` (`src/channels/http/api/v1/agents/channel.rs`), forward it as `{"history_entry": <entry>}`. Make every other `match` on `SessionFrame` ignore the new variant: `src/channels/woken.rs`, plus anything `grep -rn "SessionFrame::" src` finds.
- [X] T003 [P] In `src/schema/history.rs`, add a pure `pub fn open_tool_calls(entries_after_request: &[SessionHistory]) -> Vec<String>`. It returns the `call_id`s of `ToolCall` entries that have no `ToolResult` with the same `call_id` later in the slice, in call order. Add unit tests for: none open, one open among answered, several open, and a result before its call (that call is not open).
- [X] T004 Create `src/agents/steps.rs` (register it in `src/agents/mod.rs`) with:
  - `pub async fn record_step(storage: &VizierStorage, transport: &VizierTransport, session: &VizierSession, content: SessionHistoryContent) -> Option<String>`: saves through `storage::history::save_with_busy_retry`, logs on error like `VizierAgent::save_history_or_log`, and on success publishes `SessionFrame::HistoryEntry` with the uid.
  - `pub enum StopReason { Restart, Aborted, AgentRestarted, InternalError }` with `fn notice(&self) -> &'static str`, using the exact texts in `contracts/websocket-and-replies.md` §3.
  - `pub async fn seal_open_calls(storage, transport, session, request_uid: Option<&str>, reason: StopReason) -> usize`. It reads the session history, takes the entries after the entry whose uid is `request_uid` (or, when `None`, after the last `Request` entry), calls `open_tool_calls`, records one `ToolResult { call_id, content: reason.notice() }` per open call through `record_step`, and returns how many it sealed.

**Checkpoint**: shared helpers compile, and the `open_tool_calls` tests pass.

---

## Phase 3: User Story 1 - Leaving and returning mid-turn keeps the activity list (Priority: P1) 🎯 MVP

**Goal**: every step is in history as it happens and is pushed as `history_entry`. The WebUI
builds the running trail from history merged by uid. An abort or stop mid-call never leaves a
call without a result.

**Independent Test**: quickstart §1, §2 and §6b steps 1–3, plus §11.1–2 in the WebUI. Mid-turn:
navigate away and back, reload, and open a second tab. Each shows every step once and keeps
updating. After an abort mid-call, the call shows as stopped and the next message works.

### Implementation for User Story 1

- [X] T005 [US1] In `src/agents/steps.rs`, add `pub struct StepRecorder { session, recorded: usize }` with:
  - `async fn record(&mut self, storage: &VizierStorage, transport: &VizierTransport, message: &Message)`: converts one model message with `messages_to_history_entries(std::slice::from_ref(message))`, records each entry through `record_step`, and increments `recorded`.
  - `async fn flush(&mut self, …, delta: &[Message])`: records `delta[self.recorded..]`.

  Keep the conversion pure and separate from the IO (a `fn entries_for(&[Message]) -> Vec<SessionHistoryContent>` step), so that `cargo test` can check the following against a scripted delta (assistant with narration and 2 tool calls, 2 results, a second assistant with 1 call, 1 result, final assistant text): recording message by message as `prompt_with` would, then flushing all but the final message, yields exactly `messages_to_history_entries(&delta[..len-1])`.
- [X] T006 [US1] In `prompt_with` (`src/agents/agent/mod.rs`, ~811), add a parameter `steps: Option<&mut StepRecorder>` and update every caller (`grep -n "prompt_with(" src`). Only `chat` passes `Some`.
  - When `Some`, record the `Message::Assistant(choices)` right after it is pushed to `full_history`, but only when it has tool calls (a text-only final answer is saved by `chat` as the `Response`).
  - Record each tool's `Message::ToolResult` as soon as that call's result is built, before the next call runs.
  - Keep the record order identical to the order the messages end up in `full_history`.
  - The tool-response user messages carrying images (`pending_images`) are `Message::User` and produce no entries, as today.
- [X] T007 [US1] In `VizierAgent::chat` (`src/agents/agent/mod.rs`, ~613–780):
  - Create the `StepRecorder` and pass it to `prompt_with`.
  - Success path: replace the end-of-turn `messages_to_history_entries(&new_messages[..len-1])` save loop with `recorder.flush(&new_messages[..len-1])`, which records only what wasn't recorded.
  - Error path: replace the partial-history save loop with `recorder.flush(partial_history[original_history_len..])`, which picks up the sealing results `fail_turn` adds. The `Error` response save is unchanged.
  - Check that the per-call `ToolResult` saved during the loop and the sealing result from `fail_turn` can't both exist for one call: a failure after a call's result was recorded must not seal that call again.
- [X] T008 [US1] In `src/agents/process.rs`, seal on every stop the turn can't clean up after (research Decision 8). In each case, run after the task is stopped and before the session is reported free:
  - `abort_session_notify`, and the abort performed for `checkpoint`/`lobotomy` → `steps::seal_open_calls(.., None, StopReason::Aborted)`.
  - The `turns.stop_all` loop at the end of `agent_process` → `StopReason::AgentRestarted`. Today's drop behaviour is kept until T027.
  - A panicked turn in `join_next` (`TurnOutcome` panicked, before `recover_failed_turn`) → `StopReason::InternalError`.

  Pass `&deps.storage` and `&deps.transport`.
- [X] T009 [P] [US1] In `webui/app/interfaces/types.ts`, add `HistoryEntryFrame = { history_entry: ChatMessage }`. In `webui/app/hooks/connectionStore.tsx`, recognise `history_entry` frames before the `lastMessage` fallthrough and deliver them to a listener set: `subscribeHistoryEntries(listener) => unsubscribe`, following the `reactionListeners` pattern in the same file.
- [X] T010 [P] [US1] In `webui/app/lib/trail.ts`:
  - Add `mergeEntries(prev: ChatMessage[], incoming: ChatMessage[]): ChatMessage[]`. It dedupes by `uid` (an incoming entry replaces the one with the same uid), keeps the stored history order, and appends new uids after it.
  - Export a helper `openTurn(turns)` that returns the last turn when it has no outcome.
  - Make an aborted turn (no outcome, followed by a `Command("abort")` entry) identifiable, e.g. `turn.aborted = true` set in `groupHistory`.
  - Add tests in `webui/app/lib/trail.test.ts` for: merge with overlap, merge with a gap, an open turn with an unanswered `ToolCall`, and an aborted turn.
- [X] T011 [US1] In `webui/app/routes/chat.tsx`:
  - Subscribe with `subscribeHistoryEntries` and `setMessages(prev => mergeEntries(prev, [entry]))`, only when the entry's session topic is the page's.
  - Re-read history every time `connected` turns true (after the socket opens), merging with `mergeEntries` instead of replacing. Keep the loading skeleton for the first read of a topic only.
  - Draw `ThinkingIndicator`'s trail from the open turn of `groupHistory` (the same `TrailEvent` mapping the stored trails use), not from `liveTrail`.
  - Remove `liveTrail`, `closedTrails`, `closeTurn`, `clearLiveTrail` and the `thinking`/`tool_choice`/`tool_response` → `appendLiveEvent` branch. Those frames are ignored for the trail; the indicator still turns on at `thinking_start`.
  - On `abort`, only stop the indicator and clear `queuedMessages`. The trail stays and renders as an unanchored aborted trail.
  - Remove `appendLiveEvent`/`liveEvent` from `lib/trail.ts` if nothing else uses them.
- [X] T012 [P] [US1] In `webui/app/components/ActivityTrail.tsx`, show an "Aborted" label in the summary line when the trail's turn is aborted (T010), and show a tool call whose result is one of the stop notices as stopped rather than failed or running. Match the notices on the shared prefix `Stopped:`/`Interrupted:`.
- [ ] T013 [US1] Dummyplug check on a running binary: quickstart §1, §2 and §6b steps 1–3 (by hand or with the first part of T041's script). Compare §1's final entry kinds with `reference-history.json` from T001. Then do quickstart §11.1–2 in the WebUI.

**Checkpoint**: the reported bug is fixed and shippable on its own. No restart work is needed for it.

---

## Phase 4: User Story 2 - A conversation interrupted by a restart carries on (Priority: P1)

**Goal**: durable request rows, startup re-send, resume from the last step with cut-off calls
sealed, the crash-loop guard, resumed replies delivered and marked, and the same resume on an
agent settings or credentials change.

**Independent Test**: quickstart §3, §4 and §6b steps 4–5. `kill -9` and `just shutdown`
mid-call: the turn resumes, the call is sealed with the restart notice and not re-run, the reply
is marked `resumed`, and `session_turn` is empty afterwards. A settings change mid-call resumes
on the same socket.

### Implementation for User Story 2

- [X] T014 [P] [US2] Create `src/storage/turn.rs` with `PendingTurn { id: i64, session: VizierSession, request: VizierRequest, state: TurnState, request_uid: Option<String> }`, `enum TurnState { Queued, Running }`, and `#[async_trait] pub trait TurnStorage` with the methods listed in `data-model.md` (`accept_turn`, `start_turn`, `set_turn_request_uid`, `note_resume`, `finish_turn`, `cancel_session_turns`, `pending_turns`, `sessions_with_pending_turns`). Add it to the `VizierStorageProvider` supertrait and hand-forward every method on `VizierStorage` in `src/storage/mod.rs`.
- [X] T015 [US2] SQLite implementation:
  - Add `pub fn init_turn_schema(conn)` in `src/storage/sqlite/mod.rs` (table and index exactly as `data-model.md`), called from `init_schema` after `init_scratch_schema`.
  - Implement `TurnStorage for SqliteStorage` in a new `src/storage/sqlite/turn.rs`. `pending_turns` orders by `id`.
  - `note_resume(id, steps)`: if `steps == steps_at_resume` and `resume_count > 0`, then `resume_count += 1`; otherwise `resume_count = 1` and `steps_at_resume = steps`. Return `resume_count`.
  - Delete rows in `delete_session` (`src/storage/sqlite/session.rs`, matching `agent_id`, `channel` slug and `topic`) and in `delete_agent` (`src/storage/sqlite/agent.rs`).
  - Unit tests against an in-memory connection (the pattern in `src/storage/sqlite/task_run.rs`): lifecycle queued→running→deleted, `pending_turns` order, `note_resume` reset-on-progress and count-otherwise, and deletion with the session and with the agent.
- [X] T016 [P] [US2] In `src/schema/request.rs`, add `pub fn is_durable(&self, session: &VizierSession) -> bool`. It is false for `Command("abort")` and `Command("dream")`, and for sessions whose channel is `VizierChannelId::Dream(..)`; true otherwise. Add unit tests.
- [X] T017 [P] [US2] In `src/schema/response.rs`, add `#[serde(default, skip_serializing_if = "std::ops::Not::not")] pub resumed: bool` to `VizierResponse`, plus the matching field on its utoipa schema if one is derived. Fix any literal that doesn't use `..Default::default()`.
- [X] T018 [US2] Make the transport durable, in `src/transport.rs` and `src/dependencies.rs`:
  - `VizierTransport::new(storage: Arc<VizierStorage>)`. In `VizierDependencies` construction, wrap storage in `Arc` before building the transport and reuse that `Arc`. Update other `VizierTransport::new()` callers (tests, `grep -rn "VizierTransport::new" src`).
  - Add `turn_id: Option<i64>` and `resumed: bool` to `VizierRequestEnvelope`.
  - `send_request`: when `req.is_durable(&session)`, call `accept_turn` (on `Err`, `tracing::warn!` and continue with `turn_id: None`) and set `turn_id`.
  - Add `pub async fn resend(&self, envelope: VizierRequestEnvelope) -> Result<()>`, which sends without inserting.
  - Move `BackgroundJobs::broadcast_sender` (`src/agents/background/mod.rs` ~506) to `VizierTransport::broadcast_sender(&self, session) -> flume::Sender<VizierResponse>` and use it from `BackgroundJobs`.
- [X] T019 [US2] Row lifecycle in `src/agents/process.rs`:
  - Carry `turn_id` and `resumed` from the envelope into `start_session_job`, the session queue tuples and `handle_request` (a small `TurnRef { id: Option<i64>, resumed: bool }`).
  - `start_session_job` → `start_turn(id)`.
  - When a current turn finishes in `join_next` (after T008's sealing for a panic) → `finish_turn(id)`.
  - Pass `TurnRef` on to `VizierAgent::chat`. After it saves the `Request` entry, call `set_turn_request_uid(id, uid)`.
- [X] T020 [US2] Resumption, in `src/schema/history.rs` and `src/agents/agent/mod.rs`:
  - Add a pure `pub fn split_at_request(entries: &[SessionHistory], request_uid: &str) -> Option<(&[SessionHistory], &SessionHistory, &[SessionHistory])>` in `src/schema/history.rs`, with tests.
  - Add `VizierAgent::resume(session, request, turn: TurnRef, request_uid, response_tx)`. It:
    - loads history and splits it; when the uid isn't found, falls back to `chat`
    - builds the system prompts and the prior messages the same way `chat` does (checkpoint handover included), then the request `Message` with `context_md` recomputed (auto-context, skills, reaction digest), without re-saving the request or re-storing attachments
    - calls `steps::seal_open_calls(.., Some(request_uid), StopReason::Restart)`, then reloads the entries after the request
    - converts the progress with `history_entries_to_messages`, and calls `prompt_with` so that the progress follows the request message. Add a `progress: Vec<Message>` parameter to `prompt_with`: when non-empty, the first completion is sent with history = prior + request + progress, and the loop continues from there. The recorder starts with `recorded = progress.len()`.
    - sets `resumed = true` on the final `Response` before saving and returning it.
  - In `handle_request` (`src/agents/process.rs`), route `turn.resumed` with a stored `request_uid` to `resume`, and everything else to `chat`. A resumed row without a `request_uid` runs as a fresh `chat`.
  - Add a unit test for the progress-to-messages step: after sealing, every `ToolCall` in the progress has a result, and the message sequence passes the existing tool-pairing invariant test helper in `src/schema/history.rs`.
- [X] T021 [US2] Crash-loop guard in `src/agents/process.rs`: before running a resumed turn, count the entries after its `request_uid` and call `note_resume(id, count)`. If the result is > `MAX_RESUMES_WITHOUT_PROGRESS` (3, a const in `src/agents/steps.rs`), save an `Error` response ("This turn was stopped: it brought vizier down 3 times without making progress."), send it on the response sender, `finish_turn(id)`, and let the queue continue. Add a unit test with the guard's decision as a pure function.
- [X] T022 [US2] Startup recovery in `src/agents/mod.rs` (`VizierAgents` boot, after every agent's request channel is registered and before supervisors build). For each agent, read `pending_turns(agent_id)` and `transport.resend` each in `id` order with `response_tx: Some(transport.broadcast_sender(session))`, `resumed: state == Running`, `turn_id: Some(id)`. Log a count per agent like the other startup sweeps.
- [X] T023 [US2] Startup clean-up in `src/dependencies.rs`:
  - Replace `clear_all_scratch` with clearing only sessions not in `sessions_with_pending_turns()`. Add `clear_scratch_except(&[VizierSession])` to `ScratchStorage` (`src/storage/scratch.rs`, `src/storage/sqlite/scratch.rs`) or loop `clear_scratch` over the difference, whichever is smaller.
  - Reset `is_thinking` to false for every session with no `running` row (one `UPDATE` in `src/storage/sqlite/session.rs` behind a `SessionStorage` method).
- [X] T024 [P] [US2] Show `resumed` replies:
  - WebUI: add `resumed?: boolean` to the response types in `webui/app/interfaces/types.ts`, and a small "continued after a restart" label in `webui/app/components/chat_bubble.tsx` when the reply's stored `Response` has it.
  - Discord/Telegram: in the shared place where a response's text is turned into messages (find the renderer `src/channels/woken.rs` hands frames to, and the person-turn renderer in `src/channels/discord/mod.rs` and `src/channels/telegram/mod.rs`), prefix `_continued after a restart_` when `response.resumed`. Use one helper in `src/channels/mod.rs` if both renderers can share it.
- [ ] T025 [US2] Dummyplug check: quickstart §3 and §4 (`kill -9`, then `just shutdown`).
- [X] T026 [US2] Agent update/restart stop reason, in `src/agents/supervisor.rs` and `src/agents/mod.rs`: make the instance shutdown signal say why it is stopping. Change `watch::Receiver<bool>` to `watch::Receiver<Option<StopKind>>` with `enum StopKind { Replace, Delete }`, or add a sibling field read when the bool flips, whichever touches fewer call sites. Set `Replace` for `AgentCommand::Update`/`Restart` (including the provider-credentials restart) and `Delete` for delete. Process exit is unaffected (`std::process::exit`).
- [X] T027 [US2] Resume across an agent update (FR-019), in `src/agents/process.rs` at the end of `agent_process`:
  - On `Replace`: for each turn returned by `turns.stop_all`, run T008's `seal_open_calls(AgentRestarted)`, **don't** call `notify_turn_stopped` (no `AgentUnavailable`), and hand the turn on **ahead of** its session's queue as a `resumed` envelope (same `turn_id`) with its original `response_tx` (`SessionTurns::stop_all` must return it). Update `hand_on_queued` to `transport.resend` queued envelopes with their `turn_id`.
  - On `Delete`: today's behaviour, plus `cancel_session_turns` for each session.
- [X] T028 [US2] Update `CLAUDE.md`:
  - Architecture › Agents (#52 sentence): turns stopped by an update or restart now resume on the new instance; delete drops them.
  - Add a short paragraph on durable turns: `session_turn` and `TurnStorage`, steps recorded as they happen, `history_entry` frames, startup re-send, `steps.rs` (`StopReason`, `seal_open_calls`), and `resumed`.
- [ ] T029 [US2] Dummyplug check: quickstart §6b steps 4–5 (settings change and delete mid-call).

**Checkpoint**: restarts and agent updates no longer lose a running conversation.

---

## Phase 5: User Story 3 - Queued messages survive a restart (Priority: P2)

**Goal**: queued rows come back in order behind the resumed turn. Abort clears them.

**Independent Test**: quickstart §5 and §6.

- [X] T030 [US3] In `src/agents/process.rs`, call `cancel_session_turns(session)` wherever the queue is discarded: `abort_session_silent`/`abort_session_notify`, and the handover commands that abort. Do it after T008's sealing and before the `abort` frame is sent.
- [X] T031 [US3] Check that re-sent queued envelopes keep handover priority. They arrive in `id` order through the receive arm in `agent_process`, which applies the same `is_handover_command` insertion as live requests. Add a unit test on the queue-insertion step: a sequence `[running X, queued msg A, queued handover H, queued msg B]` re-sent in `id` order yields queue `[H, A, B]`, the same as the live order produced. Extract the insertion into a small fn if needed.
- [ ] T032 [US3] Dummyplug check: quickstart §5 and §6.

**Checkpoint**: nothing waiting in a queue is lost to a restart.

---

## Phase 6: User Story 4 - Work started on the agent's behalf resumes (Priority: P2)

**Goal**: background jobs and task runs backed by a pending turn reconnect by session and
complete exactly once. Only work with no backing row is swept to `interrupted`.

**Independent Test**: quickstart §8 and §9.

- [X] T033 [US4] In `src/transport.rs`, add `pub async fn await_session_outcome(&self, session: &VizierSession, since: DateTime<Utc>) -> PieceOutcome`:
  1. Subscribe to session events.
  2. Check stored history for a terminal `Response` after `since`, classified with `crate::agents::background::classify`.
  3. Otherwise wait for a `SessionFrame::Response` on that session that `classify` treats as terminal.

  Reuse `PieceOutcome` from `src/agents/background/mod.rs`. Don't create a second outcome type.
- [X] T034 [US4] In `src/agents/background/mod.rs`, add `BackgroundJobs::recover()`:
  - For each `running` job, rebuild the runner: pieces already final keep their state, and unfinished pieces are awaited with `await_session_outcome` under a fresh `timeout_secs` limit. Then continue exactly as `run_job` does (the transition to `reporting`, then `deliver`).
  - For each `reporting` job, call `deliver` again unless a `BackgroundReport` for that job id is already pending (in `session_turn` for the launching session, matched on the request JSON's job id) or is in the launching session's history. Add the query to `TurnStorage`/`HistoryStorage` as needed.
  - Factor the shared part of `run_job` instead of copying it (constitution II).
- [X] T035 [US4] In `src/dependencies.rs`, limit `interrupt_orphaned_background_jobs` to jobs where none of the unfinished pieces' sessions (nor, for `reporting`, the launching session's report) has a pending turn row. Add the storage query next to `interrupt_open_background_jobs` in `src/storage/sqlite/background_job.rs`. In `src/cli/run.rs`, call `deps.background_jobs.recover()` once after `VizierAgents` has re-sent pending turns (T022).
- [ ] T036 [US4] In `src/scheduler/mod.rs`, add `VizierScheduler::recover()`, called at the start of `run()` before the first tick. For each `running` task run whose session has a pending turn row, spawn the closing logic `fire` uses (`close_run` with `answered`/`no_response`), fed by `await_session_outcome`. Get the run's session from the stored run. If `task_run` doesn't store enough to rebuild `VizierChannelId::Task`, add the slug column with `add_column_if_missing` in `init_task_run_schema` and an #82 checklist entry. Then limit the startup task-run sweep in `src/dependencies.rs` to runs with no pending turn row.
- [ ] T037 [US4] Dummyplug check: quickstart §8 and §9. Also check a blocking `consult_agent` cut off by `kill -9`: the caller's call is sealed with the restart notice, the target's turn resumes in its own session, and nothing is answered twice.

**Checkpoint**: background jobs and task runs survive restarts.

---

## Phase 7: User Story 5 - Missed schedules still run once (Priority: P3)

**Goal**: keep today's catch-up, and never fire on top of a resuming run.

**Independent Test**: quickstart §9 step 3 (one run only) and §10.

- [ ] T038 [US5] In `src/scheduler/mod.rs`, confirm and add a comment that `is_running(task)` sees the still-`running` row of a resumed run (T036 keeps it open), so `tick_tasks` skips that task until the run closes. Add a unit test or a debug assertion around `tick_tasks` if a seam exists. Otherwise rely on T039.
- [ ] T039 [US5] Dummyplug check: quickstart §10 (cron every minute, 3 minutes down, then exactly one run) and §9 step 3 (exactly one run).

**Checkpoint**: all user stories are complete.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T040 [P] Update `docs/src/api-integration/rest-api.md` § WebSocket chat per `contracts/websocket-and-replies.md`: the `history_entry` row and its exactly-once recipe, `resumed` on replies, the stop notices, and a short "requests survive restarts" note.
- [ ] T041 [P] Write `scripts/durable-turns-e2e.mjs`, modelled on `scripts/client-tools-e2e.mjs` (Node ≥ 22, no dependencies, `VIZIER_URL`/`VIZIER_TOKEN`/`AGENT`, plus `VIZIER_START`/`VIZIER_STOP`/`VIZIER_PID` for restarts), covering quickstart §1–§6b and §8–§10. Exit non-zero naming the first failed check.
- [ ] T042 Run `cargo clippy` and `cargo test`, `cd webui && npm run typecheck`, and `npx vitest run app/lib/trail.test.ts`. Fix everything new against `baseline.md`.
- [ ] T043 End-to-end check with a `dummyplug` agent on a running binary (constitution e2e gate): run `scripts/durable-turns-e2e.mjs` start to finish, then walk quickstart §11 on a live provider (marked live) in the WebUI and on a Discord or Telegram agent.
- [ ] T044 Run the quickstart.md validation. Mark each § done or note deviations in `specs/023-persist-turn-activity/quickstart.md`, and delete `baseline.md`/`reference-history.json` once they've served their purpose.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (T001)** → **Foundational (T002–T004)** → **US1 (T005–T013)**.
- **US2 (T014–T029)** depends on US1: resumption reads the steps US1 records, and T027 reuses T008.
- **US3 (T030–T032)** and **US4 (T033–T037)** depend on US2 (rows, re-send, resume), not on each other.
- **US5 (T038–T039)** depends on US4 (T036 keeps resumed runs open).
- **Polish** after the stories it documents or tests. T040 and T041 can start once US2's contracts are fixed.

### Within stories

- US1: T005 → T006 → T007. T008 depends on T004 only. The WebUI tasks (T009, T010, then T011; T012) are independent of the Rust ones until T013.
- US2: T014 → T015. T016, T017 and T018 in parallel after T014. T019 needs T015 and T018. T020 needs T019. T021 needs T020. T022 needs T018 and T019. T023 needs T015. T024 needs T017. T026 → T027.
- US4: T033 → T034 → T035. T036 needs T033.

### Parallel Opportunities

- T002 ∥ T003. Then T004.
- US1: the WebUI track (T009 ∥ T010 → T011, T012) runs alongside the Rust track (T005 → T006 → T007, T008).
- US2: T016 ∥ T017 ∥ T018 after T014. T024 alongside T019–T023.
- US3 ∥ US4 once US2 is done.
- T040 ∥ T041.

## Parallel Example: User Story 1

```text
Rust track:   T005 StepRecorder → T006 prompt_with records → T007 chat flushes the rest
              T008 seal on abort / stop / panic
WebUI track:  T009 history_entry frames ┐
              T010 mergeEntries + openTurn ┴→ T011 chat.tsx from history → T012 aborted label
Join:         T013 dummyplug + WebUI check
```

## Implementation Strategy

### MVP First (User Story 1 only)

Phases 1–3 fix the reported bug: the activity list survives navigation, reloads and second tabs,
and aborts leave a clean record. Stop at T013, validate, and ship if desired. Nothing in US1
needs the `session_turn` table.

### Incremental Delivery

1. US1: the reported bug is fixed.
2. US2: running conversations survive restarts and agent updates. This is the core of the
   durability ask.
3. US3: queued messages survive.
4. US4: background jobs and task runs survive.
5. US5: protects the existing schedule catch-up.

Each step is releasable on its own. A restart between steps behaves no worse than today.

## Notes

- [P] tasks touch different files and have no unfinished dependencies.
- Commit after each task or logical group, with conventional commits (`feat:`, `fix:`,
  `refactor:`, `doc:`).
- Don't delete the existing startup sweeps. They still handle work with no backing row
  (pre-upgrade or a failed insert). If T036 adds a column, add the #82 checklist entry.
