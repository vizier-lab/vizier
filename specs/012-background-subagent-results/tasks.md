---

description: "Task list for 012-background-subagent-results"
---

# Tasks: Background Subagent Results

**Input**: Design documents from `/specs/012-background-subagent-results/`

**Prerequisites**: plan.md, spec.md, research.md (decisions D1–D12), data-model.md, contracts/ (agent-tools, background-report, http-api, webui-tray), quickstart.md

**Tests**: the spec does not ask for TDD. The plan names a few unit tests for the pure parts (report rendering, the depth rule, mapping piece frames to states, storage round-trip, sweep, and the cancel/report race guard). They are included as ordinary tasks next to the code they cover. End-to-end verification uses the dummyplug steps in `quickstart.md` (constitution gate).

**Organization**: tasks are grouped by user story. Paths are repository-relative. The backend is `src/` and the frontend is `webui/app/`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US6 from spec.md

---

## Phase 1: Setup

**Purpose**: dependency hygiene and empty module skeletons, so later tasks only fill files in.

- [ ] T001 Add `"sync"` explicitly to the `tokio` feature list in `Cargo.toml`. It is compiled in today only through feature unification, and `broadcast`/`watch` now rely on it (research D6).
- [ ] T002 [P] Create empty modules and register them:
  - `src/schema/background.rs`, with `pub mod background;` and a re-export in `src/schema/mod.rs`, matching how `task` is exported;
  - `src/storage/background_job.rs` (`pub mod background_job;` in `src/storage/mod.rs`);
  - `src/storage/sqlite/background_job.rs` (`mod background_job;` in `src/storage/sqlite/mod.rs`);
  - `src/agents/background/mod.rs` and `src/agents/background/report.rs` (`pub mod background;` in `src/agents/mod.rs`);
  - `src/channels/http/api/v1/agents/jobs.rs` (`pub mod jobs;` in `src/channels/http/api/v1/agents/mod.rs`).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: types, storage, the request and report shapes, and depth plumbing. Every story builds on these.

**⚠️ CRITICAL**: no user story work can begin until this phase is complete.

- [ ] T003 Define the types in `src/schema/background.rs` exactly as in data-model.md "In-process types":
  - `BackgroundJobId`, `JobKind`, `JobState` (Running, Reporting, Reported, Undelivered, Cancelled, Interrupted), `PieceState` (Running, Answered, Failed, TimedOut, Cancelled, Interrupted) and `Canceller` (Agent/Person);
  - the structs `BackgroundJob`, `BackgroundPiece`, `BackgroundReport` and `ReportEntry`;
  - `BackgroundJobSnapshot`/`BackgroundPieceSnapshot` (the wire shape in contracts/http-api.md, with the piece session flattened to `agent_id` + `topic`) and `impl From<&BackgroundJob> for BackgroundJobSnapshot`.

  Derive `Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema` with `#[serde(rename_all = "snake_case")]` on the enums. Add `JobState::as_str`/`FromStr` and `PieceState::as_str`/`FromStr` for the SQLite text columns. Add a `new_job_id()` helper that returns `b-` plus 6 lowercase hex characters taken from a v4 uuid.
- [ ] T004 Add `init_background_job_schema(conn)` to `src/storage/sqlite/mod.rs`, using the DDL in data-model.md for `background_job` and `background_piece` (`ON DELETE CASCADE`, PK `(job_id, ordinal)`). It creates the indexes `(origin_agent, origin_channel, origin_topic, state)`, `(origin_agent, state)` and `(state)`, and is called right after `init_task_run_schema(conn)?` (around line 330). The `origin_channel` and `session_channel` columns hold `serde_json` of `VizierChannelId`.
- [ ] T005 Define the `BackgroundJobStorage` trait in `src/storage/background_job.rs` with the eight methods in data-model.md "Storage trait":
  - `open_background_job` (job plus pieces in one transaction);
  - `close_background_piece`;
  - `transition_background_job(job_id, from, to, cancelled_by, reason, at) -> Result<bool>`, guarded by `WHERE state = from`. When `to == Cancelled`, it also closes all still-`running` pieces as `cancelled` in the same transaction;
  - `get_background_job` (with pieces `ORDER BY ordinal`);
  - `list_running_background_jobs(origin)` and `list_agent_running_background_jobs(agent_id)`, both treating `running` and `reporting` as in flight;
  - `count_running_background_jobs(agent_id, channel)`;
  - `interrupt_open_background_jobs` (`running`/`reporting` jobs and `running` pieces → `interrupted`).

  Use `anyhow::Result` like `src/storage/task_run.rs`.
- [ ] T006 Implement `BackgroundJobStorage for SqliteStorage` in `src/storage/sqlite/background_job.rs`, following the shape of `src/storage/sqlite/task_run.rs`: the same connection locking and timestamps as Unix milliseconds. Add `#[cfg(test)]` tests:
  - open, then get, round-trips the pieces in order;
  - two concurrent `transition_background_job(Running → Reporting)` and `(Running → Cancelled)` calls produce exactly one `true`;
  - cancelling closes running pieces only;
  - the sweep interrupts `running` and `reporting` jobs and their running pieces.
- [ ] T007 Add `BackgroundJobStorage` to the `VizierStorageProvider` supertrait and hand-forward all eight methods on `VizierStorage` in `src/storage/mod.rs`, in the same way as `TaskRunStorage`.
- [ ] T008 Make deleting a topic delete the jobs launched from it. In `delete_session` in `src/storage/sqlite/session.rs`, also `DELETE FROM background_job WHERE origin_agent = ? AND origin_channel = ? AND origin_topic IS ?`. Pieces go with it through the cascade. `PRAGMA foreign_keys` must be on for that connection; if it is not, delete the pieces explicitly (data-model.md "no orphaned rows").
- [ ] T009 In `src/dependencies.rs`, next to the existing `interrupt_open_task_runs` sweep (around line 895), call `storage.interrupt_open_background_jobs()`. Log the count with `tracing::info!`, and on error log `warn!` and continue booting (research D9).
- [ ] T010 [P] Extend `VizierRequest` in `src/schema/request.rs`:
  - add `#[serde(default)] pub background_depth: u8`;
  - add the variant `VizierRequestContent::BackgroundReport(BackgroundReport)`;
  - make `Display` for that variant delegate to the report renderer from T011;
  - in `generate_frontmatter`, emit `sender: background`, `job: <id>`, `job_kind: batch|delegation` and `metadata` for a `BackgroundReport` (contracts/background-report.md "Frontmatter").

  Add a unit test, in the style of `a_scheduled_run_is_sent_by_the_scheduler_on_someones_behalf`, asserting that `sender: background` appears and that the request's `user` does not appear as the sender. Update every exhaustive `match` on `VizierRequestContent` that the compiler flags. Each one gets the `Chat`-equivalent behaviour, except the two in T013.
- [ ] T011 [P] Implement the pure renderer in `src/agents/background/report.rs`:
  - `truncate(text, 4000) -> (String, bool)`, which appends ` … [truncated]` when it cuts;
  - `render_report(&BackgroundReport) -> String`, exactly per contracts/background-report.md: the title line by kind (with `to <agent>` for a delegation), the "This is not a message from a person…" paragraph, and one `## <n>. <state> — <prompt first line ≤80 chars>` section per entry with its text;
  - `render_cancel_result(job, entries, reason, nested_ids) -> String`, per contracts/agent-tools.md `cancel_background_job` output, sharing the section renderer.

  Unit tests cover entry order, truncation marker, delegation title, prompt shortening, and a `cancelled` entry with an empty body.
- [ ] T012 Add `pub background_depth: u8` to `ToolContext` in `src/agents/tools/mod.rs`:
  - in `VizierAgent::chat` (`src/agents/agent/mod.rs`, around line 456), set it from `req.background_depth`;
  - at the other construction sites (`src/agents/agent/mod.rs` around line 977, the dream path, and `src/agents/process.rs` around lines 193 and 325), set it to `0`.
- [ ] T013 In `src/agents/process.rs`:
  - add `VizierRequestContent::BackgroundReport(_)` to the `Chat | AudioChat` arm of `handle_request` (around line 969), so a report runs with session history. The prompt text for auto-context is the rendered report, but pass a passage budget of `0` for this variant: a report is a poor retrieval query (agreed in conversation);
  - add the variant to both `matches!(…, Chat(_) | AudioChat(_, _))` checks that send `ThinkingStart` (around lines 466 and 557).
- [ ] T014 [P] Add the session-event broadcast to `VizierTransport` in `src/transport.rs`:
  - `pub struct SessionEvent { pub session: VizierSession, pub frame: SessionFrame }` and `pub enum SessionFrame { Response(VizierResponse), Job(BackgroundJobSnapshot) }`;
  - a field `session_events: tokio::sync::broadcast::Sender<SessionEvent>` (capacity 256), created in `VizierTransport::new()`;
  - `pub fn publish_session_event(&self, ev: SessionEvent)`, which ignores the no-receivers error;
  - `pub fn subscribe_session_events(&self) -> broadcast::Receiver<SessionEvent>`.

**Checkpoint**: the binary builds, `cargo test` passes, existing behaviour is unchanged, and the new tables exist on startup.

---

## Phase 3: User Story 1 — Parallel subtasks no longer freeze the conversation (P1) 🎯 MVP

**Goal**: `paralel_subtasks` returns immediately. When every task has finished, one ordered report wakes the originating conversation.

**Independent Test**: quickstart steps 1, 2 and 6. A dummyplug batch returns an acknowledgement within 1 second, the person can keep chatting, and one `background_report` request followed by a woken reply appears in history. The nesting chain stops at depth 3.

- [ ] T015 [US1] Create `BackgroundJobs` in `src/agents/background/mod.rs`, a cheaply cloneable struct holding `Arc<VizierStorage>`, `VizierTransport` and `cancels: Arc<Mutex<HashMap<BackgroundJobId, watch::Sender<bool>>>>`. Add `pub background_jobs: BackgroundJobs` to `VizierDependencies` in `src/dependencies.rs` and construct it where `transport` and `storage` are built.
- [ ] T016 [US1] Implement `BackgroundJobs::launch(&self, ctx: &ToolContext, kind: JobKind, pieces: Vec<PieceSpec { executor_agent, prompt }>, timeout_secs: u64) -> crate::Result<BackgroundJob>` in `src/agents/background/mod.rs`. It:
  1. refuses with `background nesting limit (3) reached: this turn was itself started by a background result` when `ctx.background_depth >= 3` (const `MAX_BACKGROUND_DEPTH`);
  2. builds the job with `origin = ctx.session`, `depth = ctx.background_depth`, and each piece's session as `(executor, Subagent, Some(uuid))` for a batch or `(target, InterAgent([origin_agent, target]), Some(uuid))` for a delegation (research D7);
  3. calls `open_background_job`;
  4. for each piece, creates a `flume` response channel and calls `transport.send_request(piece.session, VizierRequest { user: origin agent id, content: Prompt(prompt), background_depth: job.depth + 1, ..Default::default() }, Some(tx))`. A send error marks that piece `failed` right away with the error text (FR-014);
  5. inserts a `watch` cancel sender into `cancels`;
  6. publishes a `Job` session event with the snapshot;
  7. spawns `run_job`;
  8. returns the job.
- [ ] T017 [US1] Implement `run_job` in `src/agents/background/mod.rs`. It awaits all pieces concurrently (`futures::future::join_all`, already a dependency through the tree, or `JoinSet`), using `first_terminal(rx)`, which reads frames until the first terminal one:
  - `Message`, or `AudioReply` with text → `Answered(text)`;
  - `Error` → `Failed(message)`;
  - `Abort`, `Empty`, or the channel closing → `Failed("the piece ended without an answer")`.

  As each piece finishes, `close_background_piece` runs and a `Job` event is published. When all pieces are final, `transition_background_job(Running → Reporting)` runs. If that returns `false`, the job was cancelled: return without delivering. Otherwise, publish the `reporting` snapshot, build the `BackgroundReport` from the piece states and texts in ordinal order using `report::truncate`, then call `deliver`. Always remove the job's `cancels` entry on exit.
- [ ] T018 [US1] Implement `deliver(&self, job, report)` in `src/agents/background/mod.rs`. It sends `VizierRequest { user: origin agent id, content: BackgroundReport(report), background_depth: job.depth + 1, timestamp: now, ..Default::default() }` to `job.origin` with `Some(self.broadcast_sender(job.origin.clone()))`. `broadcast_sender` is the forwarder from research D1: it republishes every frame as `SessionFrame::Response` through `transport.publish_session_event`. On success, `transition_background_job(Reporting → Reported)` runs and the final snapshot is published. Retries are added in T032.
- [ ] T019 [US1] Rewrite `SubtasksTool` in `src/agents/tools/subtasks/mod.rs`:
  - the input is `tasks: Vec<Task { prompt }>` plus `#[serde(default)] timeout_secs: Option<u64>`, and the output is `String`;
  - validate `tasks` as non-empty with non-empty prompts, and `timeout_secs` as 1..=3600 (default 600);
  - call `deps.background_jobs.launch(ctx, JobKind::Batch, …)`;
  - return `Started background batch <id> with <n> tasks. Results will arrive as a background report in this conversation.`;
  - replace the description with the verbatim text in contracts/agent-tools.md;
  - hold `VizierDependencies` instead of `transport` (the constructor at `src/agents/tools/mod.rs` around line 532 already passes `deps.clone()`).
- [ ] T020 [US1] Verify with the dummyplug agent: run quickstart steps 1, 2 and 6 against `just run`, and record any deviation in this file before you continue.

**Checkpoint**: US1 works. Woken replies are in history (and live in the WebUI only after US3).

---

## Phase 4: User Story 2 — Delegated work reports back (P1)

**Goal**: `delegate_agent` stays non-blocking. The target's answer comes back as a report that names the target agent.

**Independent Test**: quickstart step 4. The acknowledgement names the job and the target. A report titled `delegation … to <B>` arrives. Delegating to `nobody` is a tool error, and no job is created.

- [ ] T021 [US2] Rewrite `DelegateAgent` in `src/agents/tools/consult/mod.rs`:
  - change its constructor to `new(agent_id, agents, deps: VizierDependencies)`, and update the call at `src/agents/tools/mod.rs` around line 527;
  - add `#[serde(default)] timeout_secs: Option<u64>` to `DelegateAgentArgs`, with the same validation as T019;
  - before launching, check that the target exists in `agents` and is registered with the transport (add `pub async fn is_agent_registered(&self, &AgentId) -> bool` to `src/transport.rs`). If not, return `agent '<id>' not found or not running`;
  - launch `JobKind::Delegation` with one piece whose executor is the target, and return `Delegated to agent '<id>' as background job <job>. Its answer will arrive as a background report in this conversation.`;
  - replace the description's first paragraph with the verbatim text in contracts/agent-tools.md, keeping the agent list.

  Leave `ConsultAgent` unchanged.
- [ ] T022 [US2] Make the delegation report carry the target. In `run_job` (`src/agents/background/mod.rs`), set `BackgroundReport.delegated_to = Some(piece.session.0)` when `kind == Delegation`. The renderer from T011 already shows it in the title. Snapshots expose it as `delegated_to`.
- [ ] T023 [US2] Verify with the dummyplug agents: run quickstart steps 4 and 5 (two agents) and record any deviation.

**Checkpoint**: US1 and US2 both report back.

---

## Phase 5: User Story 3 — The person hears about the result in the WebUI (P1)

**Goal**: a woken reply streams live into an open WebUI topic and is in history when the topic is reopened. Discord, Telegram and conversations with no person are woken into history only.

**Independent Test**: quickstart steps 1 and 9. With the topic open, the woken reply streams in with no new message from the person. A Discord-originated batch posts nothing after the acknowledgement, and its history holds the report and the reply.

- [ ] T024 [US3] In `handle_socket` in `src/channels/http/api/v1/agents/channel.rs`, call `transport.subscribe_session_events()` once per connection and add a `select!` branch:
  - for each event where `ev.session == curr_session`, write a `SessionFrame::Response(r)` as `serde_json::to_string(&r)` (the existing bare frame), and a `SessionFrame::Job(s)` as `{"background_job": s}`;
  - on `RecvError::Lagged(n)`, log `warn!` and continue;
  - on `Closed`, stop the branch.

  The existing per-message forwarder must not be changed (no duplicates; research D6).
- [ ] T025 [P] [US3] In `webui/app/interfaces/types.ts`, add `BackgroundJobSnapshot`, `BackgroundPieceSnapshot`, `BackgroundReport` and `ReportEntry` per contracts/http-api.md, and add `| { background_report: BackgroundReport }` to `VizierRequestContent`. Add an optional `background_job` key to the incoming WebSocket frame union, as `WebSocketJobFrame = { background_job: BackgroundJobSnapshot }`.
- [ ] T026 [US3] In `webui/app/hooks/connectionStore.tsx` `ws.onmessage`, route a parsed frame that has a top-level `background_job` key to `useBackgroundJobStore.getState().applySnapshot(frame.background_job)`. A minimal store stub in `webui/app/hooks/backgroundJobStore.tsx` is enough until US5. Do not set `lastMessage` for such frames.
- [ ] T027 [US3] Create `webui/app/components/BackgroundReportItem.tsx` and use it in `webui/app/routes/chat.tsx`'s message map (around line 1638, before the `Command` branch) for history entries whose `content.Request.content.background_report` is set. It is a divider-styled, collapsed row: `⚙ Background <batch|delegation to X> <id> finished · <n> ✓ · <n> ✕ failed · <n> ⧖ timed out ▸`. Expanded, it lists each entry's prompt, state and text, and it is never rendered as a user bubble (FR-009, FR-021). Also make `webui/app/lib/trail.ts` `groupHistory` treat a `background_report` request as a turn boundary, the same way as a `chat` request.
- [ ] T028 [US3] Live report entry. In `chat.tsx`, when the job store receives a snapshot with `state: 'reporting'` for the open topic, synthesize a `ChatMessage` whose `content.Request.content.background_report` is built from the snapshot. The entries carry prompts and states only; the full texts arrive on the next history reload. If `isThinking`, push it into `queuedMessages`, otherwise append it to `messages` (contracts/webui-tray.md "Report entry"). The woken turn's frames then flow through the existing `lastMessage` handler unchanged.
- [ ] T029 [US3] Check by running that a Discord/Telegram-originated report posts nothing. Code needs no change, since nobody subscribes to those sessions' events. Run quickstart step 9 if a bot token is available; otherwise note in this file that it was skipped and why.

**Checkpoint**: P1 complete. Background work is non-blocking, reports back, and is visible live in the WebUI.

---

## Phase 6: User Story 4 — Failures and stuck work are reported, not lost (P2)

**Goal**: time-outs, failures and respawns become explicit report entries in task order, and late answers are discarded.

**Independent Test**: quickstart steps 3 and 8 (agent respawn). The report shows `answered`, `timed out`, `answered` in order, and no second report arrives. A respawn yields `failed` with "the agent was restarted".

- [ ] T030 [US4] Add the per-piece time limit in `run_job` (`src/agents/background/mod.rs`): wrap each piece's `first_terminal` in `tokio::time::timeout(Duration::from_secs(job.timeout_secs))`. On elapse:
  - send `VizierRequest { content: Command("abort".into()), user: origin agent id, .. }` to the piece session with `None`, which reuses the abort path at `src/agents/process.rs` around line 222;
  - close the piece as `timed_out` with the reason `No answer within <n>s`;
  - drop the receiver so a late answer is discarded (FR-013).
- [ ] T031 [US4] Distinguish a respawn from other failures in `first_terminal` (`src/agents/background/mod.rs`). If the channel closes with no terminal frame and `transport.is_agent_registered(executor)` was false at any point, or the executor's registration generation changed, use the reason `the agent was restarted`; otherwise use `the piece ended without an answer`. If no generation counter exists, the simpler rule "channel closed before any frame → `the agent was restarted`" is acceptable, so state that in a comment (research D9).
- [ ] T032 [US4] Add delivery retry in `deliver` (`src/agents/background/mod.rs`). On `send_request` error, retry with exponential backoff (0.5s, 1s, 2s, 4s, 8s, 14s ≈ 30s total), opening a fresh `broadcast_sender` per attempt. After the last failure, `transition_background_job(Reporting → Undelivered, reason)`, publish the snapshot and `warn!`.
- [ ] T033 [US4] Add a unit test in `src/agents/background/mod.rs` for the frame-to-state mapping, factoring `classify(frame) -> Option<PieceOutcome>` out of `first_terminal` if needed. It covers `Message`, `AudioReply` with and without text, `Error`, `Abort`, `Empty`, and that mid-turn frames (`ThinkingStart`, `ToolChoice`, `ToolResponse`, `Thinking`) are not terminal.
- [ ] T034 [US4] Verify with the dummyplug agent: run quickstart step 3 and step 8's agent-respawn part, and record any deviation.

**Checkpoint**: no background outcome is silent.

---

## Phase 7: User Story 5 — See running background work in the WebUI (P2)

**Goal**: a tray above the input shows in-flight jobs and their pieces, with a piece drill-down, jump, phone pill, topic-list badge, and the lost state after a restart.

**Independent Test**: quickstart step 7 and step 8's server-restart part.

- [ ] T035 [P] [US5] Create `src/channels/http/api/v1/agents/jobs.rs` with three handlers, registered in the agents router in `src/channels/http/api/v1/agents/mod.rs` next to the existing `channel/{channel_id}/topic/{topic_id}/…` routes:
  - `GET …/jobs`: `list_running_background_jobs` → snapshots;
  - `GET …/jobs/{job_id}`: 404 unless `job.origin == session`;
  - `GET …/jobs/{job_id}/pieces/{ordinal}/history`: the piece session is taken from the stored row, then `list_session_history(piece_session, before, before_seq, limit)`, with the same query struct and body as `get_topic_history`.

  All three use the same authorization as `get_topic_history`: `user_can_view_agent`, with the session built from `user.username` (FR-023). Add `utoipa::path` annotations in the style of `channel.rs`.
- [ ] T036 [P] [US5] Add `pub running_jobs: usize` to `TopicEntry` in `src/channels/http/api/v1/agents/channel.rs`. Fill it in `list_topics` from one `count_running_background_jobs(agent_id, channel)` call, mapped by topic, defaulting to 0. Add `running_jobs?: number` to `Topic` in `webui/app/interfaces/types.ts`.
- [ ] T037 [P] [US5] Add API functions to `webui/app/services/vizier.tsx`, `listBackgroundJobs(agentId, topicId)`, `getBackgroundJob(agentId, topicId, jobId)` and `getPieceHistory(agentId, topicId, jobId, ordinal, params)`, following the existing topic-history function's URL building and auth.
- [ ] T038 [US5] Complete `webui/app/hooks/backgroundJobStore.tsx` per contracts/webui-tray.md "Store":
  - state: `jobs: Map`, `finishing: Map<id,'done'|'cancelled'|'lost'>`, plus `agentId`/`topicId`;
  - `applySnapshot`, which applies the state table, removing finished jobs after 3 seconds;
  - `load(agentId, topicId)`, which replaces `jobs` from `listBackgroundJobs`. Any previously held job that is no longer listed is resolved with `getBackgroundJob`: `reported` or `cancelled` jobs are dropped silently, and `interrupted` or `undelivered` jobs are marked `lost`.

  Call `load` on topic open and from `connectionStore`'s `ws.onopen`, including reconnects.
- [ ] T039 [US5] Create `webui/app/components/BackgroundJobTray.tsx` per contracts/webui-tray.md "Tray":
  - a collapsed row per job, and expanded piece rows with a state icon, an ellipsized prompt, state, elapsed time and `›`;
  - `↥ jump`, a 40vh maximum height with internal scrolling, and the finishing states (`✓ …`, `⊘ cancelled`, `⚠ lost`);
  - one shared 1-second timer, active only while jobs exist;
  - below 640px, a pill `⟳ N running` that opens a bottom sheet.

  Use Tailwind and the existing theme tokens; look at `ActivityTrail.tsx` and `ThinkingIndicator.tsx` for the visual language. Leave a slot for the ✕ (US6).
- [ ] T040 [US5] Mount the tray in `webui/app/routes/chat.tsx` directly above the message `<form>` (around line 1926), inside the same sticky container so it moves with the input. Implement `↥ jump` by scrolling to the trail row of the `paralel_subtasks`/`delegate_agent` tool call whose tool response contains the job id: give `ActivityTrail` tool rows a `data-job-id` attribute when the response text matches `/b-[0-9a-f]{6}/`.
- [ ] T041 [US5] Add the piece panel in `webui/app/routes/chat.tsx` using `SlideOver` (as in `routes/agent-core.tsx`). It is opened from the tray's `›` and from `BackgroundReportItem`'s per-entry `›`. It loads `getPieceHistory`, groups the entries with `groupHistory` from `lib/trail.ts`, and renders them with `ActivityTrail` and the final message read-only. While the piece is `running`, it re-fetches whenever the store applies a snapshot for that job.
- [ ] T042 [US5] Show a `⟳N` badge on topic-list entries with `running_jobs > 0` in `webui/app/routes/chat.tsx`'s topic list (near line 1413), styled like the existing `is_thinking` indicator.
- [ ] T043 [US5] Run `cd webui && npm run typecheck`, then verify with the dummyplug agent: quickstart step 7 and step 8's server-restart part (the tray shows `⚠ lost`). Record any deviation.

**Checkpoint**: background work is visible and inspectable.

---

## Phase 8: User Story 6 — Stop background work that is no longer wanted (P2)

**Goal**: the agent can list and cancel its jobs, and the person can cancel from the tray. A cancel cascades to nested jobs and never sends a report, and the race with completion has exactly one winner.

**Independent Test**: quickstart step 10.

- [ ] T044 [US6] Implement `BackgroundJobs::cancel(&self, job_id, by: Canceller, reason: Option<String>, scope: CancelScope) -> crate::Result<CancelOutcome>` in `src/agents/background/mod.rs`, per research D12. `CancelScope` is either `Agent(AgentId)` (requires `origin.0 == agent`) or `Origin(VizierSession)` (requires `origin == session`). It:
  1. loads the job. If it is missing or out of scope, return `NotFound`;
  2. calls `transition_background_job(Running → Cancelled, by, reason)`. If that returns `false`, return `AlreadyFinished(state)`;
  3. signals the `watch` sender in `cancels`;
  4. sends `Command("abort")` to every piece session that was still `running`;
  5. cascades: for each piece session, `list_running_background_jobs(piece_session)` → `cancel(child, same by, Some("parent job <id> cancelled"), Origin(piece_session))`, collecting the cancelled child ids;
  6. reads each answered piece's text, which is the last `Response` message in its session's history, the same way `storage::task_run::task_run_response` reads a run's response, and truncates it;
  7. publishes the `cancelled` snapshot;
  8. returns `Cancelled { job, entries, nested_ids }`.
- [ ] T045 [US6] Make `run_job` observe cancellation (`src/agents/background/mod.rs`): `select!` the pieces' join against `cancel_rx.changed()`. On cancel, drop all receivers and return without touching rows, since `cancel` already closed them. The guard in T017 still covers the race where the cancel lands after all pieces finish.
- [ ] T046 [P] [US6] Create `src/agents/tools/background_jobs.rs` with `ListBackgroundJobs` and `CancelBackgroundJob`, implementing `VizierTool`. Names, descriptions, inputs and outputs are verbatim from contracts/agent-tools.md:
  - `list_background_jobs` renders `list_agent_running_background_jobs(ctx.session.0)`. The origin label is the WebUI topic title from the session detail when available, otherwise `<channel slug>`;
  - `cancel_background_job` maps `Cancelled` through `report::render_cancel_result`, `AlreadyFinished(s)` to `Background job <id> already finished (<s>).`, and `NotFound` to an error `no running job <id> launched by you`.

  Register both with `.tool(...)` on `default_toolset` in `src/agents/tools/mod.rs`, next to `SubtasksTool`. Do not add them to `DREAM_TOOL_NAMES`.
- [ ] T047 [US6] Add `POST …/jobs/{job_id}/cancel` to `src/channels/http/api/v1/agents/jobs.rs` with the body `{ reason?: string }`. It uses the same authorization as T035 and calls `cancel(job_id, Canceller::Person(user.username), reason, CancelScope::Origin(session))`. Responses:
  - `Cancelled`: append `SessionHistoryContent::Command("cancelled background job <id>")` to the topic's history and return 200 with the snapshot;
  - `AlreadyFinished`: 409 with the snapshot;
  - `NotFound`: 404.
- [ ] T048 [US6] Tray cancel in `webui/app/components/BackgroundJobTray.tsx` and `webui/app/services/vizier.tsx` (`cancelBackgroundJob`):
  - `✕` on each `running` job opens an inline confirmation `Cancel this job? [Keep] [Cancel job]`. It must not be a browser `confirm()`;
  - while the request is in flight the row shows `cancelling…`;
  - a 200 or 409 response applies the returned snapshot;
  - the `✕` is hidden once the job is `reporting`.

  In `webui/app/routes/chat.tsx`'s `Command` branch (around line 1638), render a command text that starts with `cancelled background job ` as `⊘ You cancelled background job <id>` instead of `/<command>`.
- [ ] T049 [US6] Verify with the dummyplug agents: run quickstart step 10 in full (by agent, repeat cancel, cascade, foreign job, from the tray, race) and record any deviation.

**Checkpoint**: all six stories work.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T050 [P] Update `CLAUDE.md`:
  - in the Tools section, a paragraph on background jobs: `paralel_subtasks` is now non-blocking, `delegate_agent` reports back, the new `list_background_jobs`/`cancel_background_job` tools, the `BackgroundReport` request kind, and `background_depth`/`MAX_BACKGROUND_DEPTH`;
  - in the Storage list, `BackgroundJobStorage` and its two tables;
  - in the Architecture section, the `session_events` broadcast on `VizierTransport`;
  - the rule that a report's woken reply reaches only WebUI sockets, while Discord and Telegram get history only, by decision.
- [ ] T051 Run `cargo clippy` and `cargo test` and fix all warnings and failures in touched files.
- [ ] T052 Run `cd webui && npm run typecheck` and fix any errors.
- [ ] T053 Constitution e2e gate: on a fresh `just run`, walk all of `quickstart.md` (steps 1–10) with dummyplug agents and mark each step pass or fail in this file. Run the "Live model only" check if a provider key is at hand, marked as such.
- [ ] T054 Commit with a conventional message `feat: [**breaking**] background subagent jobs with report-back and cancel`. The breaking part is that `paralel_subtasks` now returns an acknowledgement instead of results.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)** → **Foundational (Phase 2)** → user stories → **Polish (Phase 9)**.
- Inside Foundational, T003 comes first. Then T004→T005→T006→T007→T008 run in sequence, because they share the storage files. T009 depends on T007. T010, T011 and T014 are [P], since they touch different files (T010 needs T011's renderer signature, so stub it first or do T011 before T010). T012 and T013 depend on T010.

### User Story Dependencies

- **US1** depends on Foundational only. It creates `BackgroundJobs` (T015–T018), which every later story uses.
- **US2** depends on US1 (T016–T018 launch, run and deliver).
- **US3** depends on US1, for there to be something to deliver. Its WebUI tasks (T025–T028) do not depend on US2.
- **US4** depends on US1 (it modifies `run_job` and `deliver`).
- **US5** depends on US3 (WebSocket fan-out, types, store stub) and benefits from US4 (time-out states to display).
- **US6** depends on US1 (runner, `cancels`) and, for the tray ✕, on US5 (T039).

### Within Each User Story

Types → storage → service (`BackgroundJobs`) → tool or route → WebUI → dummyplug verification.

### Parallel Opportunities

- Phase 2: T010 ∥ T011 ∥ T014 once T003 is done.
- US3: T025 (types) ∥ T024 (WebSocket fan-out).
- US5: T035 ∥ T036 ∥ T037 (backend routes, the badge field, and the frontend service).
- US6: T046 (tools) ∥ T047 (route) once T044 is done.
- US4 and US5 can be developed in parallel after US3; they touch different files apart from `run_job`, which belongs only to US4.

---

## Parallel Example: User Story 5

```bash
Task: "T035 jobs.rs GET routes in src/channels/http/api/v1/agents/jobs.rs"
Task: "T036 running_jobs on TopicEntry in src/channels/http/api/v1/agents/channel.rs"
Task: "T037 job API functions in webui/app/services/vizier.tsx"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Phases 1–2.
2. Phase 3 (US1). **Stop and validate** with quickstart steps 1, 2 and 6. Reports already wake the agent into history, which is useful on its own.

### Incremental Delivery

1. US1 → US2 (delegation reports back) → US3 (live in the WebUI): the P1 slice. This is the first point worth demoing.
2. US4 (robustness) → US5 (tray) → US6 (cancel).
3. Polish and the full quickstart.

---

## Notes

- Tool names are dispatch keys: `paralel_subtasks` keeps its spelling.
- Never branch on `VizierChannelId` in `src/agents/background/`. Delivery is the same for every conversation (research D1).
- A report never interrupts a turn. If it seems to, the request bypassed `session_queues`, which is a bug.
- Record verification deviations inline under the task that found them.
