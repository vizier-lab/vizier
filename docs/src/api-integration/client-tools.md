# Client tools

A WebSocket client can give the agent tools of its own: drawing something on the person's screen, asking them to pick a file, reading the browser's location. The client declares each tool on the request it sends; the agent can call it from its scripts during that turn; the call comes back over the same socket, and the client answers it. vizier only checks the declarations' shape, names them, and carries calls and answers.

This is protocol version 1. It extends the [WebSocket chat protocol](./rest-api.md#websocket-chat) and changes nothing for clients that ignore it. The WebUI uses it for its own interactive pages (`render_interactive_ui`), registered through the same mechanism a WebUI extension uses (`registerClientTool` in `webui/app/lib/clientTools.ts`).

```
ws://<host>/api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/chat?token=<jwt>
```

All frames are JSON text frames. Each one is an object with exactly one top-level key naming the frame, except a request, which keeps its usual shape. **A client must ignore frame kinds it does not recognise**, because new ones may be added within version 1.

## 1. Declaring tools on a request

Any request (`chat`, `audio_chat`, `client_event`) may carry:

```json
{
  "timestamp": "2026-10-17T12:00:00Z",
  "user": "alice",
  "content": { "chat": "Pick a colour for the header" },
  "metadata": {},
  "client_tools": [
    {
      "name": "pick_color",
      "category": "palette",
      "description": "Ask the person to pick a colour. Returns {\"hex\": \"#rrggbb\"}.",
      "input_schema": {
        "type": "object",
        "properties": { "prompt": { "type": "string", "description": "What the colour is for" } },
        "required": ["prompt"]
      },
      "output_schema": { "type": "object", "properties": { "hex": { "type": "string" } } },
      "timeout_secs": 120
    }
  ],
  "client_categories": [
    { "name": "palette", "description": "Colour pickers shown to the person" }
  ]
}
```

| Field | Required | Rules |
|-------|----------|-------|
| `client_tools[].name` | yes | `^[a-z][a-z0-9_]*$`, at most 64 bytes; unique within its category |
| `client_tools[].category` | no | same pattern; default `client` |
| `client_tools[].description` | yes | 1 byte to 2 KB. This is what the agent reads to decide when and how to call the tool. |
| `client_tools[].input_schema` | yes | a JSON Schema object with `"type": "object"`, at most 16 KB |
| `client_tools[].output_schema` | no | a JSON Schema object, at most 16 KB |
| `client_tools[].timeout_secs` | no | 1–600, default 60 |
| `client_tools[].deferrable` | no | `true` to let the agent call the tool while no client is connected; see §4a. Default `false` |
| `client_categories[].name` | yes | a category used by at least one tool (others are ignored) |
| `client_categories[].description` | no | at most 512 bytes |

At most 32 tools and 16 categories per request.

**Scope**: the tools exist for the one turn this request starts, and only while this connection stays open. They are not remembered. The next request must declare them again; the WebUI declares them on every request. Requests over REST cannot declare tools. `prompt` and `silent_read` requests ignore declarations in version 1.

**Caching advice**: the agent's system prompt lists your categories (not your tools). Sending the same declaration on every request keeps that prompt identical from turn to turn, which keeps provider prompt caching effective. Changing your declared set between requests works, but costs cache hits.

**Validation failure**: the request is refused before any turn starts and nothing is saved:

```json
{ "client_error": { "code": "invalid_client_tools", "tool": "palette/pick_color", "rule": "description_too_long", "message": "description is 3104 bytes; the limit is 2048" } }
```

`rule` is one of `name_pattern`, `duplicate_name`, `too_many_tools`, `too_many_categories`, `description_missing`, `description_too_long`, `category_description_too_long`, `schema_not_object`, `schema_too_large`, `timeout_out_of_range`.

## 2. Server → client: resolved names

When the agent starts the turn, before the model runs, the declaring connection receives:

```json
{
  "client_tools_resolved": {
    "protocol": 1,
    "tools": [
      { "category": "palette", "tool": "pick_color", "category_name": "palette", "name": "palette_pick_color" }
    ]
  }
}
```

- `name` is what the agent calls the tool: `<category_name>_<tool>`.
- `category_name` equals `category` unless that name is reserved (a built-in category such as `memory`, an addon id, or one of the agent's MCP server names), in which case it is `<category>_client`.
- A name that collides with another tool gets `_2`, `_3`, …. A client tool never replaces another tool.

If the request was queued behind another turn, this frame arrives when its own turn starts. If the connection closed before then, the turn runs without client tools and nothing is sent.

## 3. Server → client: a call

```json
{
  "client_tool_call": {
    "call_id": "6f1c0d6e-…",
    "name": "palette_pick_color",
    "category": "palette",
    "tool": "pick_color",
    "arguments": { "prompt": "Header background" },
    "answer": true
  }
}
```

- `answer: true` only on the connection that declared the tool. Only that connection may answer.
- Other connections watching the same conversation receive the same frame with `answer: false`, so they can display it. They MUST NOT answer.
- `call_id` is stable. It is the id of the call's entry in the conversation history (§7), and the id used for saved state (§6) and client events (§5).
- The arguments are not validated against your `input_schema`. Validate them, and answer with an error if they are wrong.
- `deferred: true` marks a deferred call being delivered now (§4a). Answer it like any other.

## 4. Client → server: the answer

```json
{ "client_tool_result": { "call_id": "6f1c0d6e-…", "ok": { "hex": "#3366ff" } } }
```

```json
{ "client_tool_result": { "call_id": "6f1c0d6e-…", "error": "The person closed the picker" } }
```

Exactly one of `ok` (any JSON value, serialised at most 1 MB) or `error` (string). The agent's script receives `ok` as the function's return value, or `error` as a catchable `RuntimeError`.

An answer for a call that is not pending on this connection (unknown, already answered, timed out, cancelled, or sent to another connection) is ignored, and the connection receives:

```json
{ "client_error": { "code": "not_pending", "call_id": "6f1c0d6e-…", "message": "this call is not waiting for an answer on this connection" } }
```

An oversized answer gets `code: "result_too_large"`, and the call stays pending.

### Ways a call ends without your answer

| Situation | What the agent's script sees | What the client receives |
|-----------|------------------------------|--------------------------|
| The declaring connection closes | `client_disconnected` error, or for a `deferrable` tool `{"status": "deferred"}` (§4a) | — |
| No answer within the limit | `timeout` error | `client_tool_cancelled` |
| The turn is aborted, or the script's own time runs out | (script ended) | `client_tool_cancelled` |

```json
{ "client_tool_cancelled": { "call_id": "6f1c0d6e-…" } }
```

**Effective time limit**: `min(timeout_secs, 600, the agent's tool timeout)`. The whole script that calls your tool must finish within the agent's tool timeout, which an operator configures per agent.

After every call ends, all connections on the conversation receive:

```json
{ "client_tool_outcome": { "call_id": "6f1c0d6e-…", "outcome": { "status": "ok", "result": { "hex": "#3366ff" } } } }
```

`status` is one of `ok`, `error`, `disconnected`, `timeout`, `cancelled`, `interrupted`, `deferred`.

## 4a. Deferred calls

A tool declared with `"deferrable": true` can still be called when the connection that declared it is gone: the person closed the tab while the turn was running, or the connection dropped before answering. Instead of failing, the call is **deferred**:

- The agent's script receives `{"status": "deferred", "call_id": "…", "message": "…"}`: the call will run the next time a client opens this conversation, and its result will not be reported back to the agent.
- Every connection on the conversation receives `client_tool_outcome` with `status: "deferred"`.
- The call is delivered, as a `client_tool_call` with `"answer": true, "deferred": true`, to the first connection on that conversation: one already open (another tab), or the next one to connect. Only one connection receives it at a time. A connection that does not provide the tool should answer with an error.
- If that connection closes before answering, the call stays deferred for the next one; deferred calls also survive a server restart. Delivery is therefore at-least-once: use the `call_id` to recognise a repeat.
- A delivered call is answered within 60 s or recorded as `timeout`. Its outcome is broadcast and saved in history as usual.

Deferral applies only within the turn whose request declared the tool; no later turn gets the tool.

## 5. Client → server: a client event

A client event is a message into the conversation from the client rather than typed by the person. It starts an agent turn exactly like a `chat` request (queued behind a running turn, same history, sent as the authenticated person):

```json
{
  "timestamp": "2026-10-17T12:01:00Z",
  "user": "alice",
  "content": { "client_event": { "text": "Submitted filters", "data": { "quarter": 3 }, "source_call_id": "6f1c0d6e-…" } },
  "metadata": {},
  "client_tools": [ … ]
}
```

| Field | Required | Rules |
|-------|----------|-------|
| `text` | yes | 1 byte to 4 KB. Shown in the conversation as the person's message. |
| `data` | no | any JSON value, at most 64 KB. Seen by the agent, never shown in the bubble. |
| `source_call_id` | no | the call the event came from. Dropped (the event is still delivered) if it is not a client tool call in this conversation. |

The agent receives the text, the data and the source call, and knows the message came from the client. Events beyond 10 per minute per conversation are refused:

```json
{ "client_error": { "code": "rate_limited", "message": "too many client events in this conversation; try again shortly" } }
```

## 6. Saved state

Any client may save one JSON value per client tool call in a conversation the person owns. It is meant for what a drawn call should look like when drawn again: a checklist's ticks, a filter's selection.

```json
{ "client_call_state": { "call_id": "6f1c0d6e-…", "state": { "checked": [1, 3] } } }
```

- The latest value replaces the previous one. There is no history.
- Every connection on the conversation, the saving one included, receives the same frame back. The echo to the saver is its confirmation.
- Saving never starts a turn and is never shown to the agent. To tell the agent something, send a client event.
- Limits: at most 64 KB serialised, at most 2 saves per second per call. A refused save leaves the stored value unchanged and gets `client_error` with `code` `state_too_large`, `rate_limited`, or `not_found` (not a client tool call in this conversation).
- The value is deleted with the conversation and with the agent.

## 7. History

`GET /api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/history` returns each call as an entry in conversation order. It appears after the request that started its turn and before that turn's reply:

```json
{
  "uid": "6f1c0d6e-…",
  "content": {
    "ClientToolCall": {
      "name": "palette_pick_color",
      "category": "palette",
      "tool": "pick_color",
      "arguments": { "prompt": "Header background" },
      "outcome": { "status": "ok", "result": { "hex": "#3366ff" } },
      "state": { "chosen": "#3366ff" }
    }
  },
  "timestamp": "…",
  "seq": 1234
}
```

`outcome` is `{ "status": "pending" }` while the call is in flight. `state` is absent if never saved. A client SHOULD draw the calls of tools it recognises from these entries (with their `state`), and show any other client tool call as a collapsed item naming the tool.

A client event appears as a `Request` entry whose `content` is `{ "client_event": { … } }`.

## 8. Frame index

| Direction | Frame | Section |
|-----------|-------|---------|
| C → S | request with `client_tools` / `client_categories` | §1 |
| S → C | `client_error` | §1, §4, §5, §6 |
| S → C | `client_tools_resolved` | §2 |
| S → C | `client_tool_call` | §3 |
| C → S | `client_tool_result` | §4 |
| S → C | `client_tool_cancelled` | §4 |
| S → C | `client_tool_call` with `deferred: true` | §4a |
| S → C | `client_tool_outcome` | §4 |
| C → S | request with `content.client_event` | §5 |
| C ↔ S | `client_call_state` | §6 |

Frames that are not part of the client tools protocol (for example `session_file`, sent when a file is added to the conversation) are listed in the [REST API](rest-api.md#server--client-a-file-was-added-session_file); a client ignores them like any other unknown frame.

### `client_error` codes

Every refusal goes to the sending connection only.

| `code` | Sent when |
|--------|-----------|
| `invalid_client_tools` | A declaration breaks a rule (§1); carries `tool` and `rule` |
| `not_pending` | An answer for a call not waiting on this connection (§4) |
| `result_too_large` | An answer over 1 MB (§4) |
| `invalid_client_event` | A client event's `text` is empty or over 4 KB, or its `data` over 64 KB (§5) |
| `rate_limited` | Too many client events in the conversation (§5), or state saves for a call (§6) |
| `not_found` | State for a call that is not a client tool call in this conversation (§6) |
| `state_too_large` | State over 64 KB (§6) |
| `state_not_saved` | The server could not store the state; try again (§6) |
| `invalid_frame` | A `client_tool_result` or `client_call_state` without a `call_id`, or an answer with neither `ok` nor `error` |

## 9. Versioning

This is version 1. `client_tools_resolved.protocol` carries it. Within version 1, the server may add optional fields and new frame kinds; clients ignore what they do not recognise. Any other change is a new version, announced in the changelog.
