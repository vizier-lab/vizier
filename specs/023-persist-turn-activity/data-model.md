# Data Model: Durable Turns

## New table: `session_turn` (`TurnStorage`, `storage/turn.rs` + `storage/sqlite/turn.rs`)

Work that has been accepted and is not finished yet. A row is deleted when its turn ends.

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK AUTOINCREMENT | Also the order requests were accepted in |
| `agent_id` | TEXT NOT NULL | Deleted with the agent |
| `channel` | TEXT NOT NULL | `VizierChannelId::to_slug()`, used to delete with a session |
| `topic` | TEXT NOT NULL | `''` when none (same convention as `scratch_file`) |
| `session` | TEXT NOT NULL | JSON `VizierSession`, rebuilt on recovery |
| `request` | TEXT NOT NULL | JSON `VizierRequest` (client bindings and trace are not serialised) |
| `state` | TEXT NOT NULL | `queued` \| `running` |
| `request_uid` | TEXT NULL | History uid of this turn's saved `Request` entry, set by `chat` |
| `accepted_at` | TEXT NOT NULL | RFC 3339 |
| `started_at` | TEXT NULL | Set when it becomes `running` |
| `resume_count` | INTEGER NOT NULL DEFAULT 0 | Crash-loop guard |
| `steps_at_resume` | INTEGER NOT NULL DEFAULT 0 | Steps after `request_uid` at the last resume |

Index: `(agent_id, id)`.

**Lifecycle**

```text
send_request ──insert──▶ queued ──start_session_job──▶ running ──turn ends (any outcome)──▶ (deleted)
                           │                              │
                           └──── /abort, agent delete ────┴──────────────────────────────▶ (deleted)
process exits (crash / shutdown): rows stay → startup re-sends them (running rows as resumed)
instance stop for update: running → deleted (aborted, #52); queued → re-sent with the same id
```

**Trait** (`TurnStorage`, added to the `VizierStorageProvider` supertrait and forwarded by
`VizierStorage`):

- `accept_turn(session, request) -> i64`
- `start_turn(id)`
- `set_turn_request_uid(id, uid)`
- `note_resume(id, steps) -> u32`: returns the new `resume_count`
- `finish_turn(id)`: deletes the row
- `cancel_session_turns(session)`: deletes them (abort)
- `pending_turns(agent_id) -> Vec<PendingTurn>`
- `sessions_with_pending_turns() -> Vec<VizierSession>`: used by the startup clean-up

`delete_session` and `delete_agent` delete their rows.

## Changed types

| Type | Change |
|---|---|
| `VizierRequestEnvelope` | `+ turn_id: Option<i64>`, `+ resumed: bool` |
| `VizierResponse` | `+ resumed: bool` (`serde(default, skip_serializing_if)`), stored with the final reply |
| `SessionFrame` | `+ HistoryEntry(SessionHistory)`: a saved step, uid included |
| `VizierTransport` | holds `Arc<VizierStorage>`. `send_request` journals the request. Adds `resend(envelope)`, `broadcast_sender(session)` (moved from `BackgroundJobs`) and `await_session_outcome(session, since)`. |

## Unchanged, used differently

- **Session history**: the same entry kinds and the same order. Steps are written during the
  turn instead of at its end (research Decision 1).
- **`task_run`, `background_job`/`background_piece`**: rows backed by a pending turn stay
  `running`/`reporting` across a restart and are closed by recovery. Others are swept as today.
- **`scratch_file`**: kept at startup for sessions with a `session_turn` row.
- **`session_detail.is_thinking`**: reset at startup for sessions with no running row.

## Constants

- `StopReason` (`agents/process.rs`): the result recorded for a tool call that never finished.
  Each text ends with "…so its outcome is unknown. It was not run again."
  - `Restart`: "Interrupted: vizier restarted before this call finished, …"
  - `Aborted`: "Stopped: the person aborted the turn before this call finished, …"
  - `AgentRestarted`: "Stopped: the agent was restarted before this call finished, …"
  - `InternalError`: "Stopped: the turn failed with an internal error before this call finished, …"
- `MAX_RESUMES_WITHOUT_PROGRESS = 3`.
