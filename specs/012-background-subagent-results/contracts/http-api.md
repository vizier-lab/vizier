# Contract: HTTP API and WebSocket Frames

**Feature**: `012-background-subagent-results`

All routes sit under `/api/v1` and require the existing JWT auth. Authorization follows the existing topic routes:

- `user_can_view_agent(user, agent)` must pass.
- The session is always built as `(agent_id, HTTP(user.username, channel_id), Some(topic_id))`, so a person can only address their own topics.
- A job is visible only if its `origin` equals that session (FR-023). Any other job id returns `404`, so a caller cannot tell whether a job exists.

## `BackgroundJobSnapshot`

```json
{
  "id": "b-7f3a9c",
  "kind": "batch",
  "delegated_to": null,
  "state": "running",
  "created_at": "2026-10-09T10:00:00Z",
  "finished_at": null,
  "timeout_secs": 600,
  "pieces": [
    {
      "ordinal": 0,
      "prompt": "Research topic A",
      "state": "answered",
      "reason": null,
      "agent_id": "vizier",
      "topic": "3e1c…",
      "started_at": "2026-10-09T10:00:00Z",
      "finished_at": "2026-10-09T10:00:41Z"
    }
  ]
}
```

- `kind`: `batch` | `delegation`.
- `delegated_to`: the target agent's id for a delegation, otherwise `null`.
- `state`: `running` | `reporting` | `reported` | `undelivered` | `cancelled` | `interrupted`. The tray treats `running` and `reporting` as in flight.
- `cancelled_by`: `{"agent": "<id>"}` or `{"person": "<username>"}` when the job was cancelled, otherwise `null`.
- `reason`: why it was cancelled, or why delivery failed, otherwise `null`.
- `pieces[].state`: `running` | `answered` | `failed` | `timed_out` | `cancelled` | `interrupted`.
- `pieces[].agent_id`: the agent running the piece.

The client computes elapsed time from `started_at` and `finished_at`, or from the current time while a piece is running.

## `GET /agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/jobs`

Returns the in-flight jobs (`running` or `reporting`) launched from this topic, oldest first. The tray uses it on load and on reconnect.

**200**: `APIResponse<BackgroundJobSnapshot[]>`.

## `GET /agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/jobs/{job_id}`

Returns one job in any state. The client uses it to tell `reported` from `interrupted`/`undelivered` ("lost") after a reconnect.

**200**: `APIResponse<BackgroundJobSnapshot>` · **404**: no job with that id was launched from this topic.

## `POST /agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/jobs/{job_id}/cancel`

Cancels a job from the tray (FR-028).

- **Body**: `{ "reason": "string (optional)" }`.
- **Canceller**: `person:<username>`, taken from the JWT.
- **Behaviour**: the same as the agent tool (research D12): pieces are aborted, nested jobs are cascaded, and no report is sent. It also appends `Command("cancelled background job <id>")` to the topic's history.

**Responses**:

- **200**: `APIResponse<BackgroundJobSnapshot>` in state `cancelled`. A `background_job` frame with that snapshot is also pushed to the topic.
- **409**: the job already finished. The body is the snapshot in its final state, so the tray can show it.
- **404**: the job was not launched from this topic.

## `GET /agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/jobs/{job_id}/pieces/{ordinal}/history`

Returns a piece's own conversation (FR-018).

- **Query parameters**: the same as `…/topic/{topic_id}/history` (`before`, `before_seq`, `limit`).
- **200**: the same body shape as topic history, so the WebUI renders it with `groupHistory` and `ActivityTrail` unchanged.
- **404**: unknown job or ordinal.

The piece's session comes from the stored piece row and is never taken from the caller, so a caller cannot use this route to read an arbitrary session.

## `GET /agents/{agent_id}/channel/{channel_id}/topics` (changed)

Each `TopicEntry` gains `"running_jobs": <number>`: the count of jobs in `running` or `reporting`, 0 when there are none.

## WebSocket `…/topic/{topic_id}/chat` (changed)

Two kinds of frame are added, from the session-event broadcast filtered to this socket's session:

1. **A background job changed.**

   ```json
   { "background_job": <BackgroundJobSnapshot> }
   ```

   A frame is sent when a job is created, when any piece reaches a final state, and when the job finishes. The client tells it apart by the top-level `background_job` key: every existing frame is a bare `VizierResponse` with `timestamp` and `content` keys.

2. **A woken turn's responses.** These are bare `VizierResponse` frames, exactly as for a turn the person started (`thinking_start`, `thinking`, `tool_choice`, `tool_response`, `message`, `error`, …). They arrive without the person having sent anything.

Every report is sent with a sender that republishes onto `session_events` (research D1), so frames reach every socket subscribed to the originating session. Sessions that no WebSocket can subscribe to (Discord, Telegram, task, dream, inter-agent, subagent) have no subscribers, and their woken replies exist in history only.

Ordering guarantee: for one job, the `background_job` frame with `state: "reporting"` is published **before** the report is handed to the agent. Its `thinking_start` frame therefore always follows it.

## Pieces in history

A woken turn's `Request` entry in topic history has:

```json
{ "content": { "background_report": { "job_id": "…", "kind": "batch", "delegated_to": null,
  "entries": [ { "ordinal": 0, "prompt": "…", "state": "answered", "text": "…", "truncated": false } ] } } }
```
