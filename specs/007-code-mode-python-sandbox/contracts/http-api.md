# Contract: HTTP API

No new endpoints. The existing agent endpoints gain one nested object. All paths under `/api/v1`.

## `POST /agents`, `PUT /agents/{id}` — request (`CreateAgentRequest.tools`)

`CreateAgentTools` gains:

```jsonc
{
  "tools": {
    // …existing fields unchanged…
    "python": {                       // optional; omitted ⇒ both switches off, defaults for limits
      "enabled": true,                // sandbox switch                     (default false)
      "code_mode": true               // programmatic tool calling switch   (default false)
    }
  }
}
```

Rust: `CreateAgentTools.python: Option<PythonSandboxConfig>` (the schema type itself, `#[serde(default)]` on both fields, so `{"enabled": true}` is valid). `into_config()` maps `None` → `PythonSandboxConfig::default()`. There are no limit fields; the existing `tools.timeout` bounds scripts.

## Validation → `400 Bad Request`

Performed in `VizierAgents` when handling `AgentCommand::Create` / `Update` (single choke point for both endpoints), returned as `AgentCommandResult::Error(msg)` which the handlers already map to 400:

| Condition | `error` |
|---|---|
| `code_mode && !enabled` | `tools.python.code_mode requires tools.python.enabled` |

The API does **not** cascade `code_mode` off when `enabled` is false — it rejects, so a client can never silently lose a setting. The WebUI performs the cascade client-side before sending.

## `GET /agents`, `GET /agents/{id}` — response (`AgentSummary`)

`AgentSummary` gains `python: PythonSandboxConfig` (always present, defaults filled), placed alongside the other tool flags (`fetch`, `http_client`, …). Existing agents show `{"enabled": false, "code_mode": false}`.

## OpenAPI

`PythonSandboxConfig` derives `utoipa::ToSchema` like `TtsToolSettings`; it appears in the generated spec under the existing agent request/response components. No path changes.

## WebSocket (`/agents/{id}/channel`)

No protocol change. Two existing `VizierResponse` shapes gain new *content*:

- `{"content": {"tool_choice": {"name": "execute_python", "args": {"code": "…"}}}}` — already emitted for every tool by `ToolCallsHook`; new only in that the name is new. Nested tool calls made by a script produce their own `tool_choice` events in order (via `ToolContext.hooks`).
- `{"content": {"tool_response": {"response": <ExecutionReport>}}}` — **new emission**: `ToolCallsHook::on_tool_response` forwards the response for `execute_python` only. Clients that do not recognise `tool_response` ignore it (Discord/Telegram channels already ignore unknown variants).
