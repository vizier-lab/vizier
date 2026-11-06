# Contract: Workspace Topic Socket

`GET /api/v1/workspaces/{id}/topics/{tid}/stream`, a WebSocket. The desktop app drives the same `WorkspaceConnection` through its IPC, as for `InboxConnection`. A socket is opened only by a member (`404` otherwise).

## Client to server

| Frame | Effect |
|---|---|
| `{ "post": { "text": "...", "files": [] } }` | Same as the POST route. People's messages go through this path |

Stop and answers are REST routes, so they work without a socket.

## Server to client

| Frame | When |
|---|---|
| `hello` | First. `{ latest_seq, working: [{agent, phase}] }`. The client reads missed entries by `seq` |
| `workspace_entry` | A new entry: same shape as the timeline, so it also carries step rows |
| `workspace_mentions` | `{ seq, mentions }`: states changed, drives the "woke" line |
| `agent_state` | `{ agent, phase: "thinking" \| "idle", queued }`, from the existing thinking frames tagged with the agent |
| `member_changed` | Membership changed. The client reloads the member list. A removed person gets `closed` instead |
| `closed` | `{ reason: "removed" \| "deleted" \| "token_expired" }`, then the socket closes |
| `resync` | The client lagged. It re-reads from its last `seq` |

## Rules

- **Membership is re-checked per delivery** (FR-004, SC-006), in `service/workspaces.rs`. Removal closes open sockets within 1 second.
- **Filter**: session events are selected by `(channel = Workspace(id), topic = tid)`, and tagged with `ev.session.0`.
- **Bus**: `workspace.{id}.events` carries `workspace_entry`, `workspace_mentions` and `member_changed`, as a new arm of the pump in `transport/events.rs`. Over the 1 MiB limit a frame is cut as for messages.
- **Idle**: like the inbox stream, no idle deadline, a ping every 30 s, and a close at token expiry. A Workspace is watched while quiet.
- **Recovery**: no replay buffer. `seq` is the cursor.

## What it does not carry

No per-person client tools and no `client_tool_call` frames (spec Assumptions). No `Response` frames: a turn's text arrives as a `workspace_entry` once saved, so all agents' turns render through one path.
