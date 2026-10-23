# Research: Durable Turns

## What happens today (traced)

| Concern | Where | Today |
|---|---|---|
| Turn steps | `VizierAgent::chat` (`agents/agent/mod.rs`) | Request saved at start (`save_history_or_log`). Steps (`AssistantMessage`, `ToolCall`, `ToolResult`) are saved **after** `prompt_with` returns, from `messages_to_history_entries(delta)`, and on the error path from the partial history. |
| Round loop | `prompt_with` (~811) | Sequential: one completion, then each tool call in message order (`on_tool_call` hook → `traced_tool_call`), results appended in that order. |
| Live updates | `hook/thinking.rs`, `hook/tool_calls.rs`, `process.rs::start_session_job` | `ThinkingStart`/`Thinking`/`ToolChoice`/`ToolResponse` frames on the request's `response_tx`. They carry no history uid. |
| WebUI | `layout.tsx`, `routes/chat.tsx` | The socket closes when the person leaves the chat route and opens again on return. History is read on mount. The running trail is `liveTrail` state, lost on unmount. |
| Queue | `process.rs::agent_process` | `session_queues: HashMap<VizierSession, SessionQueue>` in memory. Requests in the flume channel are also memory only. |
| Shutdown | `cli/run.rs` | `vizier shutdown` → `std::process::exit(0)`. A crash ends the process. Neither stops turns gracefully, so durable rows would be left exactly as they were. |
| Instance stop | `process.rs` end of loop | Update/delete: `turns.stop_all` aborts running turns (#52), their senders get `AgentUnavailable`, and `hand_on_queued` re-sends the queue on the agent channel. |
| Startup sweeps | `dependencies.rs` | Open task runs, background jobs (`running`/`reporting`) and pending client calls → `interrupted`. **All scratch rows cleared.** |
| Schedules | `scheduler/mod.rs::tick_tasks`, `scheduler/dream` | Cron: next occurrence after `last_executed_at`, so a missed one fires once at startup. One-time: fires if still active and due. Dream: fires when `last_dream_time + interval ≤ now`. `is_running(task)` (a `running` task_run row) prevents overlap. |
| Request serialisation | `schema/request.rs` | `VizierRequest` serialises everything except `trace`, `client`, and the client tool declarations (`skip_serializing`). `background_depth` and `scheduled_task` are kept. |

## Decision 1: Record steps in history as they happen, rather than an in-memory buffer

**Decision**: `prompt_with` takes an optional step recorder, which `chat` passes. After each
completion that has tool calls, it saves `messages_to_history_entries([assistant])`
(narration, then one `ToolCall` per call). After each tool call returns, it saves that
`ToolResult`. `chat` stops saving the delta at the end and saves only the final `Response`. On
the error path it saves only what the recorder hasn't saved yet (the sealing results and the
`Error`).

**Rationale**: Calls run one at a time in message order, so saving each entry when it happens
produces the same entries, in the same order, as saving the delta at the end (FR-002). History
then holds the running turn, which is both what a reopened page needs (US1) and what a restart
resumes from (US2). A single record serves both, replacing the earlier plan's memory-only
snapshot buffer, which couldn't survive a restart.

**Alternatives considered**: the in-memory `turn_snapshot` buffer from the previous plan
revision. It solved US1 only, and a restart would still lose everything. A separate
"turn journal" table holding the model messages would duplicate what history already stores in
the same form.

## Decision 2: The WebUI's trail is read from history, with live steps delivered as history entries

**Decision**: Each step the recorder saves is also published on the session broadcast as
`SessionFrame::HistoryEntry(entry with uid)`, and `handle_socket` sends it to the client as
`{"history_entry": <ChatMessage>}`. The WebUI merges entries into `messages` by uid, and
`groupHistory` already turns them into turns and trails. It reads history again each time the
socket opens, after it is open. `liveTrail`/`closedTrails` go: the running turn's trail is the
open turn of `groupHistory`, drawn by `ThinkingIndicator`.

**Rationale**: Reading history *after* subscribing means a step is either in the read, or
arrives as a frame, or both. Merging by uid makes "both" harmless, so nothing is lost or
duplicated (FR-005), however the read and the frames interleave. The in-progress list and the
finished list come from the same data through the same grouping (FR-006). The old trail frames
stay on the wire for API clients, which the docs describe. The WebUI simply stops using them
for the trail.

**Alternatives considered**: putting uids on the existing `ToolChoice`/`Thinking` frames. Hooks
see only `(name, args)`, so every hook's signature would change, and the client would still
need two ways of building a trail.

## Decision 3: A durable turn table, written at `send_request`

**Decision**: A new table, `session_turn`, holds every request that hasn't finished: state
`queued` or `running`. A row is inserted in `VizierTransport::send_request` before the
envelope is queued, so a request waiting for an agent that is still starting is covered too
(FR-003). The envelope carries the row id. The process loop marks a row `running` when it
starts the job and **deletes** it when the turn ends, whether done, failed or aborted. History
keeps the outcome, so finished rows have no further use.

Not recorded: `abort` and `dream` commands (handled on arrival, never queued) and dream
sessions (`VizierChannelId::Dream`, FR-020). The test is one helper, `VizierRequest::is_durable(&session)`,
next to `is_handover_command`. If the insert fails, the error is logged and the request is sent
anyway. Durability never blocks a request (Principle V).

**Rationale**: `send_request` is the single entry point for every channel, the scheduler,
background jobs and consultations (19 call sites, one function). Storage is already built
before the transport in `dependencies.rs`, so the transport can hold it. Deleting rows on
completion keeps the table at "work not yet finished", so startup recovery is a scan of
whatever rows remain.

**Alternatives considered**:
- Recording in the process loop on receipt. This loses requests sitting in the flume channel
  while an agent is down or starting.
- Keeping finished rows. That duplicates history and needs a retention policy.

## Decision 4: Startup recovery re-sends rows. Resumption is a variant of `chat`.

**Decision**:
- `VizierAgents` boot, after registering channels and before supervisors build: for each agent,
  `storage.pending_turns(agent_id)` is read, ordered by session and then id. The rows are
  re-sent with `transport.resend(envelope)`, which skips the insert. Running rows go first and
  are marked `resumed`. The response sender is the session broadcast sender, which is
  `BackgroundJobs::broadcast_sender` moved onto the transport so there's one copy (DRY). Because
  the rows are re-sent in id order through the loop's existing queue rules, a queued handover
  ends up in the same place as before (FR-010).
- `chat` records the uid of the saved `Request` entry on the row (`set_turn_request_uid`). A
  resumed row with a uid runs `VizierAgent::resume`. Without one, the process died before the
  request was saved, so the row runs as a fresh turn.
- `resume`:
  1. Split the session history at the request uid: before → prior messages, the entry → the
     request message (automatic context computed again), after → progress.
  2. `seal_tool_calls(progress, [], StopReason::Restart)`, and save each sealing result as a
     `ToolResult` (FR-009).
  3. Call `prompt_with` with the progress already in history, so the next completion follows
     the last recorded step.
  4. Mark the final `Response` as `resumed`.
- Crash-loop guard (FR-016): at resume, `note_resume(id, steps_after_request)` stores the step
  count. If it equals the stored count, `resume_count += 1`. Otherwise `resume_count` is reset
  to 1. When it reaches 4 (three resumptions without progress), the turn ends with an `Error`
  ("stopped vizier 3 times without progress") and the queue continues.
- **Instance stop for an update or restart (FR-019)**: `stop_all` still aborts the running
  tasks, so nothing continues on the old config, tools or MCP clients (#52's intent). Then, for
  each stopped turn, `seal_open_calls(AgentRestarted)` runs, the row is kept, and the turn is
  handed on ahead of its session's queue: `hand_on_queued` re-sends it with `resend` as
  `resumed`, **keeping its original `response_tx`**. The new instance resumes it like a restart
  resume, and the reply streams to whoever was waiting: the WebUI forwarder, or the
  Discord/Telegram per-message renderer. `notify_turn_stopped` no longer sends
  `AgentUnavailable` in this case. On delete (`AgentCommand::Delete`), rows are deleted and the
  current behaviour is kept. `agent_process` learns which of the two it is from the supervisor's
  stop reason (a field on the shutdown signal), since that's the only difference.
- Because the sealing step counts as progress, a config that makes the turn fail at once is
  handled by the ordinary error path (the turn ends with its `Error`), not by the crash-loop
  guard.

**Rationale**: re-sending through the existing loop reuses queueing, abort, and handover
ordering unchanged. Resumption differs from a new turn in only one place: how the history and
message are built.

## Decision 5: Background jobs and task runs re-attach by session

**Decision**: Piece sessions and task-run sessions are unique to their job piece or firing, so
"the turn in session S ended" identifies them. One helper,
`transport::await_session_outcome(session, since)`, subscribes to the broadcast first, then
checks history for a terminal `Response` after `since`, then waits for a terminal frame
(`background::classify`). It replaces the current startup sweeps where a turn row still backs
the work:
- **Background jobs** `running`: `BackgroundJobs::recover()` restarts `run_job` for each one,
  waiting on unfinished pieces through the helper, with a fresh time limit (spec edge case).
  For a job in `reporting`, the report is delivered again unless a `BackgroundReport` request
  for that job id is already in a turn row or in the launching session's history. Delivering
  the report is itself a journaled request, so this check keeps it exactly-once (FR-012).
- **Task runs** `running`: `VizierScheduler::recover()` spawns the same closing logic `fire`
  uses, fed by the helper. The row stays `running` until it is closed, so `is_running` keeps
  the scheduler from firing that task again (FR-013/FR-014).
- Work with no backing turn row (pre-upgrade, or a failed insert) is swept to `interrupted` as
  today. Pending client calls are still swept, and the resumed turn seals them as interrupted.

**Alternatives considered**: storing an "awaiter" on the turn row (job id, run id). That's a
second link to maintain. The session already identifies the work.

## Decision 6: Delivering and marking resumed replies

**Decision**: `VizierResponse` gains `resumed: bool` (`serde(default, skip_serializing_if = "is_false")`).
It is set on the final response of a resumed turn and stored with it. Replies go out through
the broadcast sender, so the WebUI receives them like a woken turn, and the Discord/Telegram
`woken.rs` readers post them through the person-turn renderer. Each renderer adds a short
"continued after a restart" line when `resumed` is set. In the WebUI it is a small label on the
reply bubble.

**Rationale**: delivering a reply nobody is waiting on is exactly what woken turns already do.
The flag travels on the response, so every channel decides how to show it once, in its
renderer.

## Decision 8: Seal unfinished calls on every stop the turn can't clean up after

**Problem**: once steps are saved as they happen, a turn stopped mid-call leaves a `ToolCall`
with no `ToolResult` in history. This happens with `/abort` (and `/checkpoint`/`/lobotomy`,
which abort through `abort_session_silent`), with an instance stop for an update or delete
(`turns.stop_all`, #52), and with a turn task that panics (`recover_failed_turn`). The task is
cancelled, so it can't seal anything itself. Replay wouldn't break, because
`repair_tool_pairing` already fills such gaps with `NO_RESULT_RECORDED` (`schema/history.rs`).
But the record would be vague, and the WebUI would show the call as running forever.

**Decision**: one helper in `agents/process.rs`:

```text
seal_open_calls(storage, transport, session, request_uid, reason)
```

It reads the session's entries after `request_uid`, finds `ToolCall`s with no `ToolResult`, and
for each one saves a `ToolResult` carrying `reason`'s notice and publishes it as a
`history_entry`. It is called, **after** the task is stopped, from:

| Path | Reason |
|---|---|
| `abort_session_notify` / the handover-command abort | `StopReason::Aborted` |
| end of `agent_process` after `stop_all` (update/restart: then handed on to resume; delete: dropped) | `StopReason::AgentRestarted` |
| `join_next` with a panicked outcome (`recover_failed_turn`) | `StopReason::InternalError` |
| `resume()` (Decision 4) | `StopReason::Restart`, followed by continuing the turn |

Turn timeouts and failures inside the loop are already sealed by `fail_turn`, and unchanged.
The `request_uid` comes from the turn's `session_turn` row, which is read before the row is
deleted. When the row doesn't exist (not durable, or the insert failed), the helper scans back to
the session's last `Request` entry instead.

**Rationale**: one place, one rule: "a turn that didn't seal itself is sealed by whoever stopped
it". The restart path differs only in continuing afterwards. Sealing after the task is stopped
means a sealing result can't race a real one, because the task can no longer write.

**WebUI**: an aborted turn's trail is no longer cleared. `groupHistory` already shows a turn
with no outcome as an unanchored trail, and the `Command(abort)` entry after it marks it as
aborted (FR-023). The `abort` frame now only stops the indicator.

## Decision 7: Startup clean-up respects pending work

- `clear_all_scratch` becomes "clear scratch for sessions with no `session_turn` row", which
  keeps spec 020's rule that files last while a turn is running or queued.
- `is_thinking` is reset to false for sessions with no running row. A resumed turn sets it
  again when it starts.

## Risks

- **`prompt_with` resumption** is the most delicate change: the loop has to start with progress
  already in history and the right message pending. It is covered by a unit test around the
  history split and seal, and by quickstart §4.
- **The per-step write** costs one SQLite insert per step, against model and tool calls that
  take seconds. A save failure is logged and the turn continues, as `save_history_or_log`
  already does. That step is then missing from a reopened page and from resumption, which is
  no worse than today.
- **Automatic context computed again** on resume can differ from the original. This is
  accepted (spec Assumptions).
