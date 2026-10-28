# Contract: OpenAI-compatible Test Server

**Base URL**: `http://127.0.0.1:9998/v1` locally. The server is an independent process. Public chat/model endpoints accept a missing or arbitrary `Authorization` header.

## Endpoints

| Method and path | Result |
|---|---|
| `GET /healthz` | `200` JSON readiness response; no Vizier connection required. |
| `GET /v1/models` | `200` object list containing `{"id":"dummyplug","object":"model"}`. |
| `POST /v1/chat/completions` | OpenAI Chat Completions request; `stream: false` returns one JSON completion; `stream: true` returns `text/event-stream`. |

The server accepts `model` (including arbitrary legacy model strings), ordered `messages`, `tools`, `tool_choice`, `max_tokens`, and `stream_options.include_usage`. User content may be a string or an array of text parts. Assistant messages may contain `tool_calls`; tool messages contain `tool_call_id` and text content. Unsupported media parts are out of scope, but the Vizier HTTP path must preserve the old dummyplug behavior for its existing attachment markers.

## Whole completion

```json
{
  "id": "chatcmpl-dummyplug-example",
  "object": "chat.completion",
  "created": 0,
  "model": "dummyplug",
  "choices": [{"index": 0, "message": {"role": "assistant", "content": "Hello"}, "finish_reason": "stop"}],
  "usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
}
```

A valid legacy JSON tool command returns one assistant `tool_calls` entry with `type: "function"`, an `id` consistent within that completion, `function.name`, and `function.arguments` encoded as a JSON string; `finish_reason` is `tool_calls`. Legacy malformed/unknown tool commands return a successful assistant text response with the exact §3 error wording. Dummyplug reports default zero usage; tests needing nonzero or cached usage supply a replacement response from the harness proxy.

## Streaming

SSE uses `data: {chat.completion.chunk JSON}\n\n` records followed by `data: [DONE]\n\n`. The first choice delta identifies `role: "assistant"`; subsequent deltas carry content or indexed tool-call fragments; the final choice chunk supplies `finish_reason`. If `stream_options.include_usage` is true, one final JSON chunk has `choices: []` and `usage` before `[DONE]`. The server builds whole and streamed forms from the same canonical completion. Deterministic legacy content reassembles to the same whole answer; random content and generated identifiers are checked by shape across separate requests. A harness-truncated stream lacks a normal finish and `[DONE]`.

## Error boundary

- Invalid HTTP/Chat payload: a structured non-2xx API error.
- Legacy command parse or validation error: `200` assistant text, no tool call.
- Test-injected failure: a harness proxy, not dummyplug, returns the selected non-2xx status and optional `Retry-After` header and counts retry attempts.
- Missing server: Vizier reports an ordinary provider connection error; no in-process fallback.

See [legacy protocol](../../008-dummyplug-provider/contracts/dummyplug-protocol.md) for precise user-visible command output. The wire shape follows the [official Chat Completions API](https://platform.openai.com/docs/api-reference/chat).
