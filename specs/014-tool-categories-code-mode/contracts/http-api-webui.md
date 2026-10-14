# Contract: HTTP API and WebUI changes

**Feature**: `014-tool-categories-code-mode` | FR-002, FR-003, FR-004, FR-016, FR-018

## HTTP API (`/api/v1/agents`)

| Endpoint | Change |
|----------|--------|
| `GET /agents/{id}` and list endpoints | the response's `tools` object no longer has a `python` key |
| `POST /agents`, `PUT /agents/{id}` | a `tools.python` key in the body is accepted and ignored (no 400). The validation "`tools.python.code_mode` requires `tools.python.enabled`" is removed |
| OpenAPI (`utoipa`) | the `PythonSandboxConfig` schema is removed |

Stored agents that have a `tools.python` key load without error, and the key is dropped the next time they are saved. No startup migration runs.

## WebUI

| Place | Change |
|-------|--------|
| `components/PythonSandboxSection.tsx` | deleted |
| `routes/agent-settings.tsx`, `components/AgentForm.tsx` | the sandbox section, the `python` form state and the "Python (code mode)" badge are removed |
| `interfaces/types.ts` | `PythonSandboxConfig`, `DEFAULT_PYTHON_SANDBOX` and `tools.python` are removed |
| `routes/agent-core.tsx:153` | the text names `write_core` |
| `routes/chat.tsx` tool-call labels | the `READ_CORE`/`WRITE_CORE` cases are renamed to `read_core`/`write_core`; the `mcp_` prefix branch is deleted, so old calls in history use the generic display |

## Discord / Telegram

`utils::format_thinking`: the `READ_CORE`/`WRITE_CORE` arms are renamed to `read_core`/`write_core`, and both `mcp_` arms are deleted. No other change.
