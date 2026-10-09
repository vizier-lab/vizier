# Research: Background Subagent Results

**Feature**: `012-background-subagent-results` · **Date**: 2026-10-09

Each decision below resolves one unknown in the plan's Technical Context. Code references point at `master` as of `cc06bb7`.

---

## Decision 1: How a woken turn's reply gets out (FR-010, FR-017)

**Finding**: a reply reaches a person only through the `flume::Sender<VizierResponse>` attached to the request that started the turn (`transport.send_request(session, request, response_tx)`). Every channel creates that sender per inbound message and spawns a loop that renders frames until the turn ends:

- **Discord** (`channels/discord/mod.rs:548-680`): the loop **breaks on the first `Message`**.
- **Telegram** (`channels/telegram/mod.rs:518-…`): the same shape.
- **WebUI WebSocket** (`channels/http/api/v1/agents/channel.rs:428-442`): one forwarder per message the person sends, writing into that socket only.

A turn started with `response_tx: None` still runs and still writes history (`VizierAgent::chat` saves the request and response itself), but none of its frames go anywhere.

**Scope (decided 2026-10-09)**: this version delivers live only to the WebUI. Discord and Telegram conversations are woken and get history, but nothing is posted to them.

**Decision**: every report is sent with **one kind of sender**, whatever the conversation. `BackgroundJobs` creates it and spawns a forwarder that republishes each frame onto the transport's `session_events` broadcast, tagged with the session (Decision 6):

```rust
fn broadcast_sender(&self, session: VizierSession) -> flume::Sender<VizierResponse> {
    let (tx, rx) = flume::unbounded();
    let events = self.transport.session_events.clone();
    tokio::spawn(async move {
        while let Ok(frame) = rx.recv_async().await {
            let _ = events.send(SessionEvent { session: session.clone(), frame: SessionFrame::Response(frame) });
        }
    });
    tx
}
```

Only WebSocket connections subscribe, each filtering on its own session. So a WebUI-originated turn reaches every open tab on that topic. A Discord, Telegram, task, dream, inter-agent or subagent turn has frames that nobody reads, so its reply is in history and nowhere else. That is FR-010 and FR-011 with no per-channel code and no branching on `VizierChannelId`.

**Alternatives considered**:

- **A `ReplySink` trait with Discord, Telegram and HTTP implementations** (the original plan). Deferred: with Discord and Telegram rendering out of scope, it would have one real implementation, which Principle I rejects. It remains the shape to use when channel rendering is added: Discord and Telegram would subscribe to `session_events` the same way the WebSocket does, or register a sink.
- **Hold a clone of the originating request's `response_tx` and reuse it.** Rejected. Discord's and Telegram's loops break after the first `Message`, and the WebUI forwarder dies with its socket.
- **Send with `response_tx: None` and have the WebUI poll history.** Rejected: it fails SC-007 (2 seconds) without polling, and the history route is paginated and heavy.

---

## Decision 2: Where background jobs live

**Decision**: a new `src/agents/background/` module with a `BackgroundJobs` service, constructed once in `VizierDependencies` and cloned cheaply like everything else there. It:

1. opens the job and piece rows (Decision 3);
2. dispatches each piece through `transport.send_request` with its own response channel;
3. spawns one runner task per job, which awaits every piece under its time limit (Decision 8);
4. closes the rows and publishes job events (Decision 6);
5. delivers the report (Decision 4).

`SubtasksTool` and `DelegateAgent` become thin. They validate the call, compute depth (Decision 5), and call `deps.background_jobs.launch(…)`, which returns the acknowledgement.

**Rationale**: both tools need exactly the same machinery, and the HTTP API needs to read job state. Putting it in `deps` gives the tools, the HTTP channel and startup (the sweep) one shared home (Principle II), without a new transport message type.

**Alternatives considered**:

- **Keep the logic inside each tool.** Rejected: it would be duplicated twice over, and the HTTP channel cannot reach a tool instance.
- **A new long-lived subsystem task like `VizierScheduler`, driven over a transport channel.** Rejected: one spawned task per job needs no central loop, so this would be an abstraction without a second use (Principle I).

---

## Decision 3: Persist jobs in SQLite instead of in memory only

**Finding**: the spec assumed in-memory bookkeeping. Three requirements need state that outlives the runner task:

- **FR-016**: the current-state read after a reload needs job state while the job runs.
- **FR-018**: the read after completion needs to know, once the job is gone from memory, that a piece's conversation belongs to a job launched from the requester's session. That is the authorization path.
- **FR-020**: the "lost" state needs to tell an interrupted job apart from one that never existed.

**Decision**: add two tables, `background_job` and `background_piece` (see `data-model.md`), behind a new `BackgroundJobStorage` trait. It is added to the `VizierStorageProvider` supertrait, implemented for `SqliteStorage`, and hand-forwarded by `VizierStorage`, the same way spec 011 added `TaskRunStorage`. `dependencies.rs` gets a startup sweep that marks `running` jobs and pieces `interrupted`, copied in shape from `interrupt_open_task_runs`. Piece *results* are not stored: like a task run's response, a piece's answer is the last message in its own session's history. The report carries a truncated copy.

**Alternatives considered**:

- **In-memory `HashMap` plus the broadcast.** Rejected: after a restart a reloaded tray could not tell "lost" from "never existed", and the drill-down from a finished report would have nothing to authorize against short of scanning the originating session's history for a matching report.
- **Store the full result text in the piece row.** Rejected: it duplicates history (the task-run precedent explicitly avoided this), and it can be large.

---

## Decision 4: The report is a new request content kind

**Decision**: add `VizierRequestContent::BackgroundReport(BackgroundReport)`, serialized as `{"background_report": {…}}`.

- **What the model sees**: its `Display` renders the Markdown in `contracts/background-report.md`.
- **Frontmatter**: `VizierRequest::generate_frontmatter` emits `sender: background`, `job: <id>` and `job_kind`. This follows the spec 011 precedent of attributing machine-written turns honestly instead of naming a person.
- **Request user**: set to the originating agent's own id.
- **Processing**: `handle_request` handles it in the **same arm as `Chat`**, which loads the session history, so the agent knows what it asked for and why.
- **Typing indicator**: `ThinkingStart` is also sent for it. Both `matches!(…, Chat | AudioChat)` sites in `process.rs` gain the variant, so the WebUI and Discord show "thinking" for a woken turn.

**Rationale**: FR-009 requires the report to be distinguishable in history and in the WebUI. A distinct content kind is distinguishable without parsing text, and the WebUI renders it from `content.Request.content.background_report`.

**Alternatives considered**:

- **`Chat(text)` with a metadata flag.** Rejected: history and the WebUI would have to sniff metadata, and a person's message and a report would look alike in every other consumer.
- **`Unattended(text)`.** Rejected: that arm runs with an empty history (`process.rs`, `_ => agent.chat(… vec![] …)`), so the woken agent would not know what the results answer. Its "nobody is waiting" meaning is also wrong here.
- **A `background_job` field on `VizierRequest` beside `Chat`**, mirroring `scheduled_task`. Rejected: a scheduled run reuses `Unattended` content, but a report has structured content of its own, which belongs in the content kind.

---

## Decision 5: Nesting depth travels on the request, not the session

**Finding**: FR-012 counts a turn *woken by* a background result as one level deeper. Tracking depth per session cannot express that. Agent B's woken turn runs in the same session as the turn that delegated, so a delegate → report → delegate ping-pong would stay at a constant depth forever.

**Decision**: add `VizierRequest.background_depth: u8`, with `#[serde(default)]` so that 0 covers every existing construction site. Piece requests and report requests both carry `job.depth + 1`. `VizierAgent::chat` copies it into a new `ToolContext.background_depth`, and `launch` refuses when `ctx.background_depth >= MAX_BACKGROUND_DEPTH`, with `MAX_BACKGROUND_DEPTH = 3`.

The model of each turn is:

```
person's message (depth 0) ──launch──► job d=0 ─► pieces run at 1, report wakes at 1
woken turn (depth 1) ──launch──► job d=1 ─► pieces/report at 2
woken turn (depth 2) ──launch──► job d=2 ─► pieces/report at 3
woken turn (depth 3) ──launch──► refused: "background nesting limit (3) reached"
```

**Alternatives considered**:

- **Track depth per session.** Rejected, per the ping-pong above.
- **Store depth in `metadata` JSON.** Rejected: it is stringly typed, and `metadata` is echoed into the model's frontmatter.

---

## Decision 6: One broadcast for everything pushed to the WebUI

**Decision**: add `session_events: tokio::sync::broadcast::Sender<SessionEvent>` to `VizierTransport`:

```rust
pub struct SessionEvent { pub session: VizierSession, pub frame: SessionFrame }
pub enum SessionFrame { Response(VizierResponse), Job(BackgroundJobSnapshot) }
```

- `BackgroundJobs` publishes `Job` frames on every state change (job created, piece finished, job finished).
- The report's broadcast sender (Decision 1) forwards a woken turn's responses as `Response` frames.
- `handle_socket` subscribes once per connection and adds a `select!` branch that writes frames whose `session` equals the socket's `curr_session`:
  - a `Response` frame is written as the bare `VizierResponse` JSON the client already understands, so a woken turn streams through the existing chat code unchanged;
  - a `Job` frame is written as `{"background_job": <snapshot>}`.
- A lagged receiver (`RecvError::Lagged`) is logged and skipped. The client reconciles by re-reading the job list on reconnect (contract `http-api.md`), so a dropped frame never leaves the tray wrong for good.

Frames of a turn the person started themselves still go through the existing per-message forwarder and are **not** republished, so nothing is delivered twice.

**Rationale**: a single fan-out, filtered by session, is the smallest thing that reaches every open tab on a topic, which is FR-017. `tokio::sync::broadcast` is already in the tree. The `sync` feature is enabled today only through feature unification, so the change adds it to `Cargo.toml` explicitly. No new crate is needed.

**Alternatives considered**:

- **A per-session hub map** (`HashMap<VizierSession, broadcast::Sender>`). Rejected: entries have to be created and garbage-collected, and the per-event filtering cost it saves is negligible at WebUI scale.
- **Route the person's own turns through the broadcast as well**, unifying the paths. Rejected as scope creep: it changes multi-tab behaviour that nobody asked to change.

---

## Decision 7: Each delegation gets its own topic

**Finding**: `DelegateAgent` sends to `(target, InterAgent([caller, target]), None)`, so every delegation between a pair of agents lands in one session. It sends `Prompt`, and the `Prompt` arm runs the target with **no history**, so the shared session gives the target no continuity anyway.

**Decision**: each delegation uses `Some(<uuid>)` as its topic. A piece's drill-down (FR-018) then shows that delegation alone. Nothing is lost, because `Prompt` never read the shared history.

---

## Decision 8: Collecting piece results, time limits, late answers

**Decision**: the runner awaits each piece's response channel inside `tokio::time::timeout(limit, …)`:

| First terminal frame | Piece state | Report entry |
|---|---|---|
| `Message { content }` / `AudioReply(_, Some(text), _)` | `answered` | the text, truncated to 4,000 characters with `… [truncated]` |
| `Error { message }` | `failed` | the message |
| `Abort` / `Empty` / channel closed before any of the above | `failed` | "the piece ended without an answer" or "the agent was restarted" |
| time limit elapses | `timed_out` | "no answer within Ns" |

Other details:

- **Abort on time-out**: after a time-out the runner sends `Command("abort")` to the piece's session (the existing abort path, `process.rs:222`), so a runaway piece stops spending tokens.
- **Late answers**: the receiver is dropped, so any late answer is discarded (FR-013).
- **Hand-off failure**: a failed `send_request` at dispatch marks that piece `failed` immediately (FR-014). An unknown target agent is checked up front, so `delegate_agent` returns a tool error and no job is created (acceptance 2.2).
- **Time limit**: the default is 600 seconds. Both tools accept an optional `timeout_secs` from 1 to 3,600, which lets the agent shorten or lengthen it and lets the quickstart exercise time-outs in seconds.

---

## Decision 9: Report delivery when the originating agent is respawning

**Finding**: a job outlives an agent *respawn*, because `BackgroundJobs` lives in `deps`, not in the agent process. When an agent is updated, its process shuts down, the pieces' response senders are dropped, and the runner sees those pieces end. That is better than the spec's edge case, which said the work would be lost.

**Decision**:

- **Pieces cut off by a respawn** are reported as `failed` with "the agent was restarted", and the report is delivered to the new process.
- **Delivery retries**: `send_request` fails with "agent not registered" while the new process is coming up, so delivery retries with backoff for up to 30 seconds. After that the job is closed `undelivered` and a warning is logged.
- **Server restart** still loses everything in flight, as the spec says. The startup sweep marks such jobs `interrupted`, and no wake is sent.

**Spec follow-up**: the spec's first edge case should be narrowed to server restarts, with respawned agents covered as above. This plan applies that one-line amendment to `spec.md`.

---

## Decision 10: Topic-list badge and the "lost" state

- **Badge (FR-022)**: `TopicEntry` (`channel.rs:64`) gains `running_jobs: usize`, counted from `background_job` rows whose state is `running` for that session. It refreshes whenever the topic list does, alongside `is_thinking`. It is not pushed live, because the WebSocket is per topic.
- **Lost (FR-020)**: on WebSocket reconnect the client re-reads `GET …/jobs`. A job that was in the tray but is no longer running is fetched by id. If its state is `interrupted` or `undelivered`, it shows a short "lost" state and leaves the tray. If it is `reported`, the report is already in history.

---

## Decision 11: Tool surface changes

| Tool | Input change | Output change | Description change |
|---|---|---|---|
| `paralel_subtasks` | `tasks: [{prompt}]` (min 1), plus optional `timeout_secs` | `String` acknowledgement (was `Vec<String>` results) | says it returns immediately and that results arrive as a later message |
| `delegate_agent` | plus optional `timeout_secs` | the acknowledgement now names the job | same, plus the existing agent list |
| `consult_agent` | — | — | — |

The name `paralel_subtasks` keeps its misspelling. Tool names are the dispatch key, and renaming it would break every stored conversation and prompt that uses it, which is not worth doing inside this feature. Neither tool joins `DREAM_TOOL_NAMES`: a dream turn has no sink and no person to report to.

---

## Decision 12: Cancellation (User Story 6, FR-024–FR-029)

**Decision**: a `cancel(job_id, by: Canceller, reason)` method on `BackgroundJobs`, called by the new `cancel_background_job` tool and by a new HTTP route. `Canceller` is either `Agent(AgentId)` or `Person(username)`.

**Making each job end exactly once (FR-029)**: the runner and `cancel` race to close the job row. Both use `UPDATE background_job SET state = ? WHERE id = ? AND state = 'running'`, and whichever affects one row wins.

- **The runner closes the row *before* it delivers the report.** The state machine becomes `running → reporting → reported | undelivered`, and only `running` can be cancelled. If cancel wins, the runner sees zero rows affected and delivers nothing.
- **If the runner wins**, cancel sees zero rows and returns `already finished (<state>)`.

**Stopping pieces**: each runner holds a `CancellationToken`-like `tokio::sync::watch<bool>` in an in-memory `HashMap<JobId, watch::Sender<bool>>` on `BackgroundJobs`. This is the one piece of in-memory state, and it is only valid while the runner is alive. On cancel:

1. Signal the watch. The runner's `select!` over `{every piece, the time limit, cancelled}` takes the `cancelled` branch.
2. For every still-running piece, send the existing `Command("abort")` to its session and close its row as `cancelled`. The runner then drops its receivers, so a late answer is discarded, as with a time-out.
3. **Cascade (FR-026)**: query running jobs whose `origin` is one of this job's piece sessions, and cancel each one recursively, with the same canceller and the reason `parent job <id> cancelled`. Depth is at most 3, so the recursion is bounded.

**The tool's result (FR-027)**: cancel reads the answered pieces' texts from their sessions' history, in the same way as the report (Decision 8: last message, truncated), and renders them in report-entry format with `cancelled` entries. No report request is sent.

**Who can cancel what**:

- **Agent tool**: a job is visible to `cancel_background_job` and `list_background_jobs` only when `job.origin.0 == ctx.session.0`, meaning the agent that launched it, from any of its sessions. Otherwise the tool returns `no running job b-… launched by you`, the same error as for an unknown id (FR-025).
- **HTTP route**: the same session-ownership rule as the read routes (FR-023), since the route is built from the caller's own topic.

**A person's cancel is recorded in the conversation (FR-028)** as `SessionHistoryContent::Command("cancelled background job b-…")`. `history_entries_to_messages` already skips `Command` entries (`schema/history.rs`), so the agent does not see it and is not woken. The WebUI renders `Command` entries in the timeline already.

**Alternatives considered**:

- **Deliver a report on cancel.** Rejected (decided 2026-10-09): the agent asked for the cancel and gets the partial results in the tool's answer, so an extra turn adds nothing.
- **Inject the person's cancel into the agent's context.** Rejected for now: it would need either a wake or an unanswered stored `Request`, and providers reject or mishandle consecutive user messages. Recorded as an assumption in the spec.
- **Per-piece cancel.** Out of scope (spec Assumptions). The cascade and the row guard work the same way per piece if it is added later.
- **Cancel through `AbortHandle`s on the piece turns.** Rejected: piece turns run inside other agent processes. The `abort` command already crosses that boundary through the transport.
