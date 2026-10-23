# Contract: WebSocket frames and resumed replies

Endpoint unchanged: `ws://…/api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/chat`.
All changes are additive. Clients that ignore unknown keys are unaffected.

## 1. `history_entry` frame (server → client)

```json
{ "history_entry": { "uid": "…", "vizier_session": { … }, "timestamp": "…", "content": { "ToolCall": { "call_id": "…", "name": "execute_python", "arguments": { … } } } } }
```

- The payload has exactly the shape of one element of `GET …/topic/{topic_id}/history`.
- It is sent for every step saved during a turn on this session: `AssistantMessage`, `ToolCall`
  and `ToolResult`, including the results that seal a cut-off call on resumption. It is sent
  after the entry is saved.
- **Exactly-once recipe for clients**: open the socket, then read history, and merge the two by
  `uid`. Every step is then in the read, in a frame, or in both, and never missing.
- The existing `thinking_start`, `thinking`, `tool_choice` and `tool_response` frames keep being
  sent unchanged.

## 2. `resumed` on replies

A final response frame (and its stored history entry) may carry `"resumed": true`. This means
the turn was interrupted by a restart and finished afterwards. The field is absent otherwise.

```json
{ "timestamp": "…", "content": { "message": { "content": "…", "stats": { … } } }, "history_uid": "…", "resumed": true }
```

Rendering:
- **WebUI**: a "continued after a restart" label on the reply.
- **Discord/Telegram**: a short italic line before the reply text.

## 3. Restart notice (agent-facing)

A tool call with no recorded result when its turn resumes gets this as its result, saved as a
`ToolResult` like any other:

```text
Interrupted: vizier restarted before this call finished, so its outcome is unknown. It was not run again.
```

## 4. Delivery of resumed turns

Every frame of a resumed turn is published on the session broadcast, the same path as a turn
woken by a background report. Open WebUI sockets on the session receive it, and the
Discord/Telegram readers post it to their channel. A blocking `POST …/chat` that was waiting
before the restart gets nothing; the reply is in history.

## Documentation

`docs/src/api-integration/rest-api.md` § WebSocket chat:
- add `history_entry` to the frame list, with the exactly-once recipe
- add `resumed` to the response frame description
- state that requests are durable across restarts and resumed automatically
