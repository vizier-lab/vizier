# Contract: Inbox Stream

One live connection per client, pushing notifications and setup status. Frames are JSON objects with a `type`. The server pushes; a client frame is ignored.

## Opening

| Host | How | Refused |
|---|---|---|
| Browser, API client | `GET /api/v1/inbox/stream` as a WebSocket, under `require_auth` (token in the header or query, like the chat socket) | `401` |
| Desktop app | `inbox_open(frames: Channel) -> { id }`, `inbox_close(id)` | `{ status }`, `503` while the engine is starting |

Close and reconnect behaviour:

- The server pings every 30 s. There is no idle timeout.
- The server closes when the person no longer resolves, and at the token's expiry.
- Clients reconnect with the existing backoff: 1 s doubling to 30 s.
- A client treats a dropped stream as "state unknown", never as an empty inbox.

## Frames

### `hello`

First frame on every open. Replaces all setup state and counts.

```json
{ "type": "hello", "counts": { "all": 5, "unread": 3, "read": 2 }, "latest_id": 412, "setups": [ /* SetupState */ ] }
```

`latest_id` is the highest id this person may see, or `0`. If it is above the client's mark, the client fetches `GET /notifications?filter=all` and treats rows above the mark as new.

### `notification`

A row this person may see was created. `read` is always `false`.

```json
{
  "type": "notification",
  "item": {
    "id": 413,
    "at": "2026-11-06T09:12:03Z",
    "agent_id": "atlas",
    "category": "chat_reply",
    "tone": "default",
    "text": "Replied in your conversation.",
    "title": "Q3 pricing review",
    "excerpt": "The middle plan is underpriced by about 12%, and churn is…",
    "href": "/atlas/chat/abc",
    "read": false
  },
  "counts": { "all": 6, "unread": 4, "read": 2 }
}
```

### `read`

This person's read state changed on any client. `id` is absent for mark-all.

```json
{ "type": "read", "id": 413, "read": true, "counts": { "all": 6, "unread": 3, "read": 3 } }
```

### `setup`

One agent's setup changed. Shape is `SetupState` as in `GET /utils/setup-state`. At most one every 250 ms per agent while bytes move; other transitions are immediate.

```json
{ "type": "setup", "state": { "agent_id": "scout", "name": "Scout", "phase": "downloading", "message": "BGE small", "downloads": [ { "kind": "embedding", "label": "BGE small", "completed": 84934656, "total": 136314880, "status": "downloading" } ] } }
```

A client removes a `ready` or `failed` entry 5 s after receiving it.

### `setup_cleared`

```json
{ "type": "setup_cleared", "agent_id": "scout" }
```

### `resync`

What this client holds may be out of date: a conversation or agent was deleted, a title changed, or the server fell behind. The client reloads the first page and counts and treats rows above its mark as new.

```json
{ "type": "resync" }
```

Unknown frame types MUST be ignored, so frames can be added later.

## Delivery rules

- Each frame is checked against the person's current permissions when it is delivered. A person who loses access to an agent gets nothing further about it.
- `notification`: the agent is visible and `audience_user_id` is empty or this person.
- `title` and `excerpt`: present only when the row's source conversation is not another person's.
- `setup`: `can_view_agent`, or for an agent not yet stored, the system role, `all_agents:view` or its owner. `owner_id` is never sent.
- `read`: only to the person who made the change.
- `resync` after a title change: only to that conversation's person.
- Delivery is best-effort. A slow client lags, then gets `resync`. It never delays another client or the work that caused the event.

## Changes to REST (`/api/v1`)

| Route | Change |
|---|---|
| `GET /notifications` | Items gain optional `title` and `excerpt`. `channel_replies` no longer appears. No longer runs a projection before reading |
| `GET /notifications/count` | Same counts; no projection. Still the sign-out probe for a failing stream |
| `PUT /notifications/{id}/read`, `POST /notifications/read-all` | Unchanged responses; also publish a `read` frame |
| `GET /utils/setup-state` | Unchanged. No longer polled by the WebUI; kept for older clients (FR-011), and listed on #82 |

## Bus

| Subject | Trigger | Pattern |
|---|---|---|
| `inbox.events` | A row created, read state changed, setup changed, rows made stale | fan-out |

Add to `docs/src/fleet/bus-contract.md`. `Subject::inbox_events()` lives in `vizier-schema`, which stays a leaf.

## Conformance

The route is a WebSocket upgrade, so it joins `tests/conformance/pending.txt` beside the chat socket and is tested by `crates/vizier-http/tests/inbox_stream.rs` and `scripts/inbox-e2e.mjs`. `pending.txt` "may only shrink", so this one addition is deliberate and noted in the PR.
