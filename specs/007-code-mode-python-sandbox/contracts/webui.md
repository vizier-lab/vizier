# Contract: WebUI

## Types — `webui/app/interfaces/types.ts`

```ts
export interface PythonSandboxConfig {
  enabled: boolean
  code_mode: boolean
}

// AgentToolsConfig (GET shape) and AgentSummary gain:
python: PythonSandboxConfig

// CreateAgentRequest['tools'] gains:
python?: Partial<PythonSandboxConfig>

// Execution report (payload of a `tool_response` event for execute_python)
export interface ToolInvocationRecord { seq: number; name: string; arguments: Record<string, unknown>; ok: boolean; error?: string; duration_ms: number }
export interface ExecutionError { kind: 'script' | 'tool' | 'limit'; message: string; traceback: string; limit?: string }
export interface ExecutionReport { ok: boolean; result: unknown; stdout: string; error?: ExecutionError; tool_calls: ToolInvocationRecord[]; duration_ms: number }

// VizierResponseContent gains the variant already defined server-side:
| { tool_response: { response: unknown } }
```

Default constant (used by `AgentForm` for new agents and when the server omits the block):

```ts
export const DEFAULT_PYTHON_SANDBOX: PythonSandboxConfig =
  { enabled: false, code_mode: false }
```

## `components/AgentForm.tsx` — new "Python" section (inside the existing Tools card)

| Control | Type | Behaviour |
|---|---|---|
| **Python sandbox** | toggle | `python.enabled`. Help text: "Lets the agent run Python scripts in an isolated sandbox for exact computation. No filesystem, network or tool access." |
| **Code mode (programmatic tool calling)** | toggle | `python.code_mode`. **Disabled** (with tooltip "Turn on the Python sandbox first") while `!enabled`. When `enabled` is switched **off**, `code_mode` is set to `false` in the same state update (US4-S5). |
| Warning (shown when `code_mode`) | callout | "While code mode is on, this agent's other tools are hidden from the model and reachable only from scripts. The model sees just `execute_python`, `think`, and two documentation tools." (US4-S7) |
| Help text under the sandbox toggle | text | "Scripts are bounded by this agent's tool timeout (above). There is no separate memory or output limit." |

Form state: `form.tools.python` initialised from `agent.python ?? DEFAULT_PYTHON_SANDBOX` (edit) or `DEFAULT_PYTHON_SANDBOX` (create). Submit sends the whole object under `tools.python`. The "no tools enabled" empty-state condition (currently `!telegram && !fetch && !http_client && …`) also checks `!python.enabled`.

## `routes/chat.tsx`

### `formatToolChoice` — new cases

```ts
case 'execute_python':
  return `🐍 Running Python\n\`\`\`python\n${args.code as string}\n\`\`\``
case 'list_tool_functions':
  return `📖 Listing tool functions`
case 'describe_tool_function':
  return `📖 Describing \`${args.name as string}\``
```

Nested tool calls made by a script arrive as ordinary `tool_choice` events and render through the existing cases — no change needed for them.

### New inline event: execution report

- `InlineEvent['type']` gains `'execution'`.
- WebSocket handler: `if ('tool_response' in content)` → attempt to parse `content.tool_response.response` as `ExecutionReport` (duck-typed: has `ok`, `stdout`, `tool_calls`, `duration_ms`); on success `addInlineEvent('execution', …)` carrying the parsed report; otherwise ignore (other tools never send this today).
- Rendering (new small component `ExecutionReportView`, in `components/`): a collapsible block headed `✅ Python finished in 1.2s` / `❌ Python failed (timeout) in 30.0s`, containing — in order — **Tool calls** (`seq. name(args) ✓/✗ 120ms`, list hidden when empty), **Output** (`stdout` in a `pre`, scroll-boxed), **Result** (pretty JSON, scroll-boxed), **Error** (`message` + `traceback` in a `pre`, only when present). Collapsed by default when `ok`, expanded when `!ok`.
- Inline events are transient today (cleared when the final message arrives) — this event follows the same lifecycle. Stored history rendering of past tool calls is **out of scope** here, consistent with how every other tool is (not) shown from history today (US5 is P5; the persisted report exists in `ToolResult.content` for API consumers, FR-025).

## Typecheck

`cd webui && npm run typecheck` must pass; no new npm dependencies.
