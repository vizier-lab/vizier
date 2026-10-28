# Data Model: Standalone Dummyplug

Dummyplug is a stateless HTTP model. It keeps only temporary data needed to complete a request; it has no test-session database, scenario queue, retry counter, or request log. No entity below changes Vizier's durable agent schema.

| Entity | Owner | Fields and rules |
|---|---|---|
| Chat request | Client / dummyplug for the request lifetime | `model`, ordered `messages`, `tools`, `tool_choice`, token limit, `stream`, `stream_options`. Accept text string or text parts and any model string. Malformed wire payload returns a client error; malformed legacy JSON command returns assistant text per §3. |
| Normalized message | Dummyplug for the request lifetime | Role, text parts, optional tool-call ID/name/arguments, tool result, context part. Normalize frontmatter/trailer/fences for legacy matching. Resolve a tool result name from the supplied prior assistant call, else use its call ID. No previous request is consulted. |
| Canonical completion | Dummyplug for the response lifetime | Generated ID, requested model, assistant content or tool calls, finish reason, usage. One generation pass feeds whole and SSE serializers. Default legacy usage is zero. |
| Test scenario | Consuming test harness | Ordered replies or failures, next attempt, and isolation key. The harness chooses an action per request, including whether to forward it to dummyplug. Dummyplug never receives or stores the plan. |
| Request record | Consuming test harness | Timestamp, test ID, request messages/tools/tool choice/token limit, non-secret headers, outcome. The harness controls retention, redaction, and cleanup. Dummyplug exposes no record API. |
| Provider configuration | Vizier | Provider ID `dummyplug`, `base_url`, optional env override. Default `http://127.0.0.1:9998/v1`; user-configured URL and `DUMMYPLUG_URL` follow ordinary provider precedence. Existing agent provider/model fields need no migration. |

## Request lifecycle

```text
HTTP request → normalize supplied messages → choose legacy reply from request
             → whole JSON or SSE chunks → discard request-local data
```

The test harness may delay, hold, or replace a response before Vizier receives it; those operations occur outside dummyplug. Disconnect cancels pending proxy work and dummyplug streaming work. A complete stream ends with a final finish chunk and `[DONE]`; a truncated or disconnected stream does not.

## Invariants

- Deterministic legacy commands produce the same observable answer regardless of ordering, concurrent clients, or process restart. Random lorem and generated identifiers retain their documented shape.
- No dummyplug test control or stored scenario changes a request.
- Legacy command precedence remains trailing tool results, `tools`, `context`, exact tool name, JSON tool request, then fallback text.
- A failed HTTP attempt and a cancelled stream never appear as a successful completion.
- Test logs and scenario state belong only to the harness; dummyplug stores neither.
