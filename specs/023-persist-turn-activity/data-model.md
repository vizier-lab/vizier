# Data Model: Persist In-Progress Turn Activity

No storage changes: no tables, columns or migrations. The one new piece of state is in memory.

## Running turn buffer (server, in memory)

Held by `VizierTransport`: `Arc<parking_lot::Mutex<HashMap<VizierSession, Vec<VizierResponse>>>>`.
The `Mutex` also serialises `session_events.send` so that snapshot and subscribe can't
interleave (research Decision 2).

| Field | Type | Notes |
|---|---|---|
| key | `VizierSession` | `(agent_id, channel, topic)`, the same key `SessionEvent.session` uses |
| value | `Vec<VizierResponse>` | Trail frames in publication order. Never empty while present. |

**Transitions**, driven only by `publish_session_event` with `SessionFrame::Response(r)`. Other
`SessionFrame` variants don't touch the buffer.

| `r.content` | Effect |
|---|---|
| `ThinkingStart` | `insert(session, vec![r])`, replacing any previous entry |
| `Thinking` / `ToolChoice` / `ToolResponse` | `entry(session).or_default().push(r)` |
| anything else | `remove(session)` |

Taking a snapshot on connect, when `is_thinking` is false, also does `remove(session)` (a stale
entry).

**Bounds**: at most one entry per session with a running turn, and each entry lives for one turn.

## Running trail (WebUI, `routes/chat.tsx`)

Unchanged: `liveTrail: TrailEvent[]`. New input: `connectionStore.turnSnapshot`.

| Field | Type | Notes |
|---|---|---|
| `agentId`, `topicId` | `string` | The socket the snapshot came from. Applied only if it matches the page. |
| `frames` | `WebSocketResponse[]` | Folded by the existing `appendLiveEvent` into `liveTrail`. |

`turnSnapshot` is reset to `null` on `connect()` and `disconnect()`.
