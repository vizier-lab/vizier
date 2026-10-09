# Contract: WebUI Reaction Protocol

**Feature**: `013-reaction-awareness` · Covers FR-013 to FR-016, Story 2

The socket is the existing chat WebSocket for `/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}`. Frames are JSON text. Only the frames below are new or changed.

## Server → client: response frames carry their history uid

The final frame of a turn, `content.message` or `content.audio_reply`, now includes `history_uid`. That applies both to frames from the person's own turn and to frames republished on the session-event broadcast (a woken background-report turn).

```json
{
  "timestamp": "2026-10-10T14:02:11.123Z",
  "content": { "message": { "content": "Here's the plan…", "stats": { … } } },
  "attachments": [],
  "history_uid": "6f1c…-uuid"
}
```

- `history_uid` is the `uid` the same entry has in `GET …/topic/{topic}/history`.
- It's absent on every non-final frame (thinking, tool choice, tool response, and so on) and on a final frame from an older server. **The client keys an agent message by `history_uid` when present.** A message without one gets no react control.

## Client → server: change a reaction

This is unchanged in shape. `action` is now meaningful.

```json
{ "reaction": { "message_uid": "6f1c…-uuid", "emoji": "👍", "action": "added" } }
```

| Field | Rule |
|---|---|
| `message_uid` | A `history_uid` the client received or loaded from history |
| `emoji` | 1 to 64 bytes |
| `action` | `"added"` or `"removed"`. The client sends `removed` iff the person's own `(user, emoji)` is in the set it currently shows |

The server checks that the uid is an agent `Response` entry **in this socket's session**, then applies it. An add of an existing reaction or a remove of an absent one is a no-op that still answers with the current set.

## Server → every socket on the session: the new set

This goes out after any change from any source on the session (this socket, another tab, or a moderator clear), via `SessionFrame::Reactions` on the session-event broadcast:

```json
{ "reactions": { "message_uid": "6f1c…-uuid", "reactions": [ { "user_id": "alice", "emoji": "👍" } ] } }
```

- It's the **full** current set for that message, never a delta. The client replaces its copy.
- The sender's own socket receives it too. That's the sender's acknowledgement (FR-014).
- `user_name` may appear on entries, and the WebUI ignores it.

## Server → sending socket only: failure

```json
{ "reaction_error": { "message_uid": "6f1c…-uuid", "message": "message not found in this conversation" } }
```

This is sent when the uid is unknown, belongs to another session, isn't an agent reply, the emoji is invalid, or the save failed.

## Client behaviour

| # | Behaviour |
|---|---|
| W1 | Clicking an emoji, a badge or a picker choice marks that `(message, emoji)` **pending** (a dimmed badge with a spinner) and sends the frame. The displayed set doesn't change until a `reactions` frame arrives. |
| W2 | A `reactions` frame replaces the message's set and clears any pending marker on it. |
| W3 | A `reaction_error`, or no `reactions` frame within **5 s**, clears the pending marker and shows a toast. |
| W4 | When disconnected, the click shows a toast ("not connected") and sends nothing. |
| W5 | The react handler's identity doesn't depend on connection state (it reads it through a ref), and `MessageItem`'s memo comparator includes `onReact`. A message rendered while offline reacts normally after a reconnect (FR-016). |
| W6 | Only agent messages with a server uid render the react control (FR-012). A person's own messages render existing badges (from migrated data) read-only. |
| W7 | On load, the set comes from `SessionHistory.reactions` in the history API response. Its shape is unchanged. |
