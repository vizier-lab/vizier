# Contract: `turn_snapshot` WebSocket frame

**Endpoint**: `ws://…/api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/chat`
(unchanged). **Direction**: server → client. **Compatibility**: additive. A client that doesn't
know the key ignores the frame, as the WebUI already does for unknown frames.

## Shape

```json
{
  "turn_snapshot": {
    "frames": [
      { "timestamp": "2026-10-23T10:00:00.000Z", "content": "thinking_start", "attachments": [] },
      { "timestamp": "2026-10-23T10:00:00.420Z", "content": { "thinking": "…" }, "attachments": [] },
      { "timestamp": "2026-10-23T10:00:01.100Z", "content": { "tool_choice": { "name": "execute_python", "args": { … } } }, "attachments": [] },
      { "timestamp": "2026-10-23T10:00:03.900Z", "content": { "tool_response": { "response": { … } } }, "attachments": [] }
    ]
  }
}
```

`frames` are ordinary response frames, exactly as they were published live, in publication
order. They only ever have the content keys `thinking_start`, `thinking`, `tool_choice` and
`tool_response`. The list is never empty.

## Rules

1. **When**: sent at most once per socket, as the **first** frame after the upgrade, and only
   when the session has a running turn (it has published trail frames since its last
   terminal frame, and storage reports `is_thinking`). No running turn means no frame.
2. **Exactly once**: every trail frame of the running turn is either in `frames` or arrives
   afterwards as a live frame on this socket, never both and never neither.
3. **Running turn boundaries**: `thinking_start` begins a new running turn and discards any
   earlier one. `thinking`, `tool_choice` and `tool_response` extend it. A frame with any other
   content key (`message`, `audio_reply`, `error`, `empty`, `abort`, `checkpoint`) ends it.
4. **Scope**: only the socket's own session. A turn woken by a background report is included
   the same way as a person-started one.
5. **Lifetime**: in memory only, lost on a server restart (the turn is interrupted then anyway).

## Client obligations (WebUI)

- On receipt: treat the agent as working, and replace the in-progress activity list with the
  result of applying `frames` in order, the same way live frames are applied.
- Frames that arrive afterwards are applied on top as usual.

## Documentation

Add a row to the frame table in `docs/src/api-integration/rest-api.md` (§ WebSocket chat):

| `content` key | Payload | When |
|---|---|---|
| `turn_snapshot` *(top-level key, not under `content`)* | `{ "frames": [frame…] }` | First frame on connect while a turn is running: its activity so far |
