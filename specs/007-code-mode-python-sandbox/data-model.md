# Data Model: Python Sandbox & Code Mode

**Feature**: `007-code-mode-python-sandbox` | **Date**: 2026-09-16

No new tables. Everything here is either (a) a new field on an existing persisted JSON document, (b) a value type that travels through the existing tool-call / session-history path, or (c) in-memory only.

## Entities

### PythonSandboxConfig (persisted — on `AgentToolsConfig`)

Lives at `AgentConfig.tools.python`. `#[serde(default)]` on the field and `Default` on the struct mean every existing agent record deserialises with both switches off (FR-006) — no migration.

| Field | Type | Default | Constraint |
|---|---|---|---|
| `enabled` | `bool` | `false` | the **sandbox** switch |
| `code_mode` | `bool` | `false` | the **code mode** switch; **invalid unless `enabled`** (FR-002) |

That is the whole struct (user decision: no limit settings). The time bound is the agent's existing `tools.timeout`.

**Validation** (one place — `VizierAgents` on `AgentCommand::Create` / `Update`):
- `code_mode && !enabled` → error `"tools.python.code_mode requires tools.python.enabled"` (HTTP 400 via `AgentCommandResult::Error`). Nothing else to validate.

**Normalisation** (`VizierTools::new`, defensive): a loaded record with `code_mode && !enabled` is treated as `code_mode = false` and logged at `warn!`.

**Derived, in-memory**:

```rust
pub enum ToolExposure { Direct, SandboxAdditive, CodeModeExclusive }
// Direct            ⇐ !enabled
// SandboxAdditive   ⇐ enabled && !code_mode
// CodeModeExclusive ⇐ enabled && code_mode
```

### SandboxLimits (in-memory — `src/sandbox`)

What the engine is handed per run; it never sees `PythonSandboxConfig` or `AgentConfig`.

| Field | From |
|---|---|
| `timeout: Duration` | the agent's `tools.timeout` — becomes Monty `max_duration` (CPU clock); the agent loop's own `tools.timeout` wrapper is the wall-clock bound |
| `tools_enabled: bool` | `code_mode` |

Fixed engine constants (`src/sandbox/mod.rs`, not settings): `SINGLE_ALLOCATION_GUARD = 1 GiB` (Monty `max_memory` per-operation pre-check only, research Decision 6), `MAX_SCRIPT_BYTES = 64 KiB`, Monty defaults for recursion depth (1000), suspensions (1000) and print buffer (10 MiB).

### Code Execution Request (transient — the tool's `Input`)

```rust
pub struct ExecutePythonInput { pub code: String }
```

Persisted only as the `arguments` of the `SessionHistoryContent::ToolCall { name: "execute_python", … }` entry the agent loop already writes. `code` is capped at 64 KiB (a script larger than that is refused with a script-kind error before the interpreter starts).

### ExecutionReport (value type — the tool's output, the `ToolResult.content`, and the live event payload)

```rust
pub struct ExecutionReport {
    pub ok: bool,
    /// JSON value of the script's last expression; `null` if none or on error.
    pub result: serde_json::Value,
    /// Captured `print()` output (engine cap 10 MiB; exceeding it ends the run with MemoryError).
    pub stdout: String,
    pub error: Option<ExecutionError>,
    /// Ordered; empty when code mode is off.
    pub tool_calls: Vec<ToolInvocationRecord>,
    pub duration_ms: u64,
}

pub struct ExecutionError {
    pub kind: ExecutionErrorKind,        // Script | Tool | Limit
    /// e.g. "TypeError: unsupported operand type(s) for +: 'int' and 'str'"
    pub message: String,
    /// CPython-style traceback text with line numbers (empty for Limit kinds raised by the host).
    pub traceback: String,
    /// For Limit: which one — "timeout" | "memory" | "recursion" | "suspensions" | "script_size"
    pub limit: Option<String>,
}

pub enum ExecutionErrorKind { Script, Tool, Limit }
```

**Kind mapping**

| Source | `kind` | `limit` |
|---|---|---|
| `MontyRun::new` `SyntaxError`, unsupported feature, runtime exception in script, non-serialisable result | `Script` | — |
| A tool invocation returned `Err` and the script did **not** catch it | `Tool` | — |
| Monty `TimeoutError` (`max_duration`) or outer wall-clock | `Limit` | `timeout` |
| Monty `MemoryError` (single allocation over the 1 GiB guard) | `Limit` | `memory` |
| Monty `RecursionError` | `Limit` | `recursion` |
| Monty `max_suspensions` (1000 host round-trips) hit | `Limit` | `suspensions` |
| Script >64 KiB | `Limit` | `script_size` |

`ok == error.is_none()`. A caught tool error (script used `try/except`) does not set `error`; it is visible in `tool_calls[i].ok == false`.

### ToolInvocationRecord (value type — element of `ExecutionReport.tool_calls`)

```rust
pub struct ToolInvocationRecord {
    pub seq: u32,                    // 1-based order within the script
    pub name: String,                // the tool name as dispatched (e.g. "memory_read", "mcp_github__create_issue")
    pub arguments: serde_json::Value,// the JSON object handed to the tool
    pub ok: bool,
    pub error: Option<String>,       // tool error message when !ok
    pub duration_ms: u64,
}
```

Tool *results* are intentionally **not** stored in the record (they would defeat the context savings and could be large); they were handed to the script and are its business. The live `tool_choice` event for each nested call is emitted through the existing hook (research Decision 10), so operators watching a session still see them one by one.

### ToolFunctionDoc (derived, never stored — `src/sandbox/docs.rs`)

```rust
pub struct ToolFunctionDoc {
    pub function: String,            // sanitised Python identifier (== tool name for all native tools)
    pub tool: String,                // original tool name (differs only if sanitised)
    pub summary: String,             // first sentence/line of the tool description
    pub description: String,         // full description
    pub parameters: Vec<ParamDoc>,   // from input schema `properties` (+ `required`)
    pub returns: String,             // rendered output schema, or "any JSON value" (MCP)
    pub example: String,             // generated: `result = name(required_a="…", required_b=0)`
}
pub struct ParamDoc { pub name: String, pub r#type: String, pub required: bool, pub description: String }
```

**Catalogue entry** = `{ function, summary }` only (US3-S1, "large catalogues" edge case). Both the model-facing `list_tool_functions`/`describe_tool_function` tools and the in-script `list_tools()`/`describe_tool()` call the same `docs::catalogue(&[ToolDefinition])` / `docs::describe(&ToolDefinition, Option<output_schema>)`.

**Identifier sanitisation**: `[^A-Za-z0-9_]` → `_`; leading digit → prefix `_`; collisions resolved deterministically by sorting original names and suffixing `_2`, `_3`, …. Reserved names (`list_tools`, `describe_tool`, `execute_python`, Python keywords/builtins such as `print`, `len`, `list`) get the suffix `_tool`.

## Relationships

```text
AgentConfig ─1─ AgentToolsConfig ─1─ PythonSandboxConfig ──derives──▶ ToolExposure, SandboxLimits
                                                                              │
SessionHistory ─*─ ToolCall{execute_python, {code}} ─1─ ToolResult{ExecutionReport}
                                                            └─*─ ToolInvocationRecord
ToolDefinition (existing, from VizierTools) ──derives──▶ ToolFunctionDoc (on demand)
```

## State transitions

**Config** (per agent): `off` → (`enabled`) → `sandbox` → (`code_mode`) → `code mode`; `code mode` → (`!enabled`) → `off` (cascade — WebUI does it client-side, API rejects the inconsistent state rather than cascading, so the operator sees what happened).

**Execution** (per call): `created` → `running` ⇄ `suspended(tool call | docs lookup)` → `completed | failed(kind)`; any state → `aborted(timeout)` when the outer deadline fires. Executions are stateless with respect to each other (FR-009): nothing carries over.
