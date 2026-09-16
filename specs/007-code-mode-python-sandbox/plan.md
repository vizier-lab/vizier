# Implementation Plan: Python Sandbox & Code Mode (Programmatic Tool Calling)

**Branch**: `007-code-mode-python-sandbox` | **Date**: 2026-09-16 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/007-code-mode-python-sandbox/spec.md`

## Summary

Give each agent an opt-in `execute_python` tool backed by the `monty` sandboxed Python interpreter (pure Rust, in-process, no filesystem/network/env, VM-enforced time/recursion limits) — the **sandbox** switch. On top of it, a second **code mode** switch makes the agent's regular tools callable *from inside* a script as plain functions and hides them from the model's direct tool list, leaving only `execute_python`, `think`, and two documentation tools (`list_tool_functions`, `describe_tool_function`). Nested tool calls are routed through the exact same `ToolRouter` dispatch, hooks, and per-tool timeout as direct calls, so code mode adds orchestration power without adding capability. There is no per-script memory ceiling in v1 (user decision — every allocation is released when the run ends, verified); a fixed 1 GiB guard on single allocations is the only memory-related check. Results come back as a structured Execution Report that doubles as the persisted `ToolResult` and as a live WebUI event.

## Technical Context

**Language/Version**: Rust 2024 edition, toolchain 1.97 (monty MSRV 1.95); TypeScript / React 19 for the WebUI

**Primary Dependencies**: `monty = "0.0.23"`, `monty-types = "0.0.23"` (new; pure Rust, MIT); existing `tokio`, `serde_json`, `schemars`, `rig-core` (`ToolDefinition`), `flume`, `tracing`, `async-trait`. No new WebUI packages.

**Storage**: none new. `PythonSandboxConfig` is a `#[serde(default)]` field on the existing `AgentToolsConfig` JSON persisted in SQLite `agent` storage; execution reports live in the existing session history (`ToolCall`/`ToolResult` entries).

**Testing**: `cargo test` unit tests for `convert.rs` (JSON⇄Monty round-trips), `docs.rs` (identifier sanitisation, example generation from schemas), `runtime.rs` (pure script, limits, `NotFound`, catchable tool error, `OsCall` refusal, deadline abort — all runnable without an LLM since the bridge is a trait); `cargo clippy`; `cd webui && npm run typecheck`; manual `just run` verification of the three exposure modes against a live agent (constitution's manual-verification gate for runtime-affecting changes).

**Target Platform**: Linux (gnu + musl, x86_64 + aarch64), macOS (x86_64 + aarch64) — every target in `Cross.toml` / `release.yml`; monty's tree is pure Rust so no `pre-build` changes.

**Project Type**: single binary (Rust backend + embedded React WebUI)

**Performance Goals**: sandbox start + trivial script ≤ 50 ms overhead (SC-004; probe measured sub-millisecond for interpreter setup); no runtime-worker starvation — interpreter runs on the blocking pool.

**Constraints**: FR-020/022/024 — no host access, a crashing/limit-hitting script must never affect the process, nothing to install. No memory ceiling in v1; memory is released per run (research Decision 6). No new limit settings: the agent loop's existing `tools.timeout` bounds the whole `execute_python` call like any other tool, and Monty's CPU clock is set to the same value so an orphaned spinning thread stops on its own (Decision 3).

**Scale/Scope**: one execution at a time per session, concurrent across sessions/agents; catalogues of up to a few hundred tool functions (MCP-heavy agents); scripts of a few KB; no output truncation (engine's own 10 MiB print buffer is the only cap).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Lean by Default**: PASS with two justified dependencies. `monty` + `monty-types` (one logical dependency; the second is the shared value-type crate) — a sandboxed Python interpreter with pause/resume host calls and VM-enforced limits is not something a few hand-rolled lines provide, and it is the entire point of the feature (research Decision 1). **No new trait**: `execute_python` and the two documentation tools are ordinary `VizierTool` impls registered with `.tool()`. The one new internal struct, `ToolRouter`, is not an abstraction over a hypothetical second implementation — it is the *existing* dispatch body of `VizierTools::call` moved so that two real callers (direct calls and the sandbox bridge) share it (Decision 8). No global allocator: the memory ceiling was deliberately dropped from v1 (Decision 6), which removes the only piece that would have touched `main.rs`. Config surface: one new `python` block on `AgentToolsConfig` with exactly two booleans, both defaulting to `false` — no limit settings, no defaults to tune.
- **II. DRY via Trait-Based Extensibility**: PASS. The three new tools implement `VizierTool`; the exposure decision (`Direct` / `SandboxAdditive` / `CodeModeExclusive`) is a single `enum` consulted in exactly two places (`VizierTools::tools`, `VizierTools::call`), not a type-tag `match` spread across call sites. Nested tool calls do not re-implement dispatch, timeouts, hooks, or attachment handling — they go through `ToolRouter::call` + `ToolContext.hooks` + the existing `VizierResponse` attachment path. Tool documentation is derived from `ToolDefinition` (the same thing the model already receives) so there is no second source of truth (Decision 9). The in-script `list_tools()`/`describe_tool()` and the model-facing documentation tools call one `docs::catalogue()` / `docs::describe()` pair. The system-prompt briefing follows the existing `boot_md`/`owner_md` pattern (a `format!` function in `system_prompt/`), not a new mechanism.
- **III. Self-Contained, Zero-Dependency Runtime**: PASS. In-process interpreter; no worker binary, no system Python, no network. Both switches default off; config-less mode is untouched. `monty-pool`/`monty-alloc` rejected precisely on this principle (Decision 1).
- **IV. Portability by Default**: PASS. Pure-Rust dependency tree — `cross build` for every `Cross.toml` target needs no new system packages. `web_time` inside `monty-types` is only active on wasm.
- **V. Unified Errors & Observability**: PASS. Tool `call()` returns `Result<_, VizierError>`; Monty errors are mapped with `throw_vizier_error`/`VizierError(format!(…))` at the boundary and never leak `MontyException` beyond `runtime.rs`. Script-level failures are *data* (the report's `error` field), not `Err` — the agent must see them to self-correct (SC-010). `tracing` spans/events only; no `println!`.

**Post-design re-check (after Phase 1)**: unchanged. The data model adds no tables; contracts add one config block, three tool definitions, one WebUI inline event. The `ToolContext.hooks` addition is an `Option<…>` field defaulting to `None` — the dream cycle and existing tests are unaffected.

## Project Structure

### Documentation (this feature)

```text
specs/007-code-mode-python-sandbox/
├── plan.md              # This file
├── research.md          # Phase 0 — monty verification, all design decisions
├── data-model.md        # Phase 1 — config, report, doc entry, exposure
├── quickstart.md        # Phase 1 — enabling, trying, verifying, limits semantics
├── contracts/
│   ├── tool-definitions.md   # What the model sees: execute_python, list_tool_functions, describe_tool_function
│   ├── python-runtime.md     # What the script sees: in-script API, value mapping, errors, limits
│   ├── http-api.md           # agent create/update request/response changes + validation
│   └── webui.md              # AgentForm fields, chat.tsx events, types.ts
└── tasks.md             # Phase 2 — /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
src/
├── schema/agent.rs                  # + PythonSandboxConfig on AgentToolsConfig (serde(default))
├── sandbox/                         # NEW — engine-facing code, no knowledge of VizierTools
│   ├── mod.rs                       # pub use; SandboxLimits; SandboxBridge trait
│   ├── runtime.rs                   # drive MontyRun: start/resume loop, FunctionCall routing,
│   │                                #   OsCall/NameLookup refusal, deadline flag, report assembly
│   ├── convert.rs                   # serde_json::Value ⇄ MontyObject (+ unit tests)
│   ├── report.rs                    # ExecutionReport, ToolInvocationRecord, ErrorKind
│   └── docs.rs                      # ToolFunctionDoc from ToolDefinition; identifier sanitisation;
│                                    #   example generation (+ unit tests)
├── agents/
│   ├── agent/system_prompt/sandbox.rs  # NEW — sandbox_md(exposure, tools_timeout): mode-specific briefing
│   ├── tools/
│   │   ├── mod.rs                   # ToolRouter (extracted from VizierTools::call); ToolExposure;
│   │   │                            #   sandbox_toolset; tools()/call() gating; ToolContext.hooks
│   │   └── python/                  # NEW — the three VizierTool impls
│   │       ├── mod.rs               # ExecutePython { router, limits, code_mode }
│   │       ├── bridge.rs            # impl SandboxBridge for the router + hooks + attachment collection
│   │       └── docs_tools.rs        # ListToolFunctions, DescribeToolFunction
│   ├── agent/mod.rs                 # ToolContext { hooks: Some(hooks.clone()) }; prepare_system_prompts pushes sandbox_md
│   ├── hook/tool_calls.rs           # on_tool_response: forward ToolResponse for execute_python
│   └── mod.rs                       # validate code_mode ⇒ enabled on Create/Update
├── channels/http/api/v1/agents/mod.rs   # CreateAgentTools.python, AgentSummary tools.python
└── constant.rs (or python/mod.rs)   # EXECUTE_PYTHON_DESCRIPTION template (limits interpolated)

webui/app/
├── interfaces/types.ts              # PythonSandboxConfig on AgentToolConfig/request types
├── components/AgentForm.tsx         # sandbox + code-mode toggles, limits, warning copy
└── routes/chat.tsx                  # formatToolChoice cases; 'tool_response' → execution event
```

**Structure Decision**: `src/sandbox/` is engine-facing and depends only on `monty`, `serde_json`, and `rig-core::ToolDefinition` — it knows nothing about `VizierTools`, storage, or channels, which keeps it unit-testable without an agent and leaves the door open (without any abstraction now) to reuse from the dream cycle later. `src/agents/tools/python/` is the thin adapter that turns it into `VizierTool`s and wires the bridge to `ToolRouter`, mirroring how `shell/` (engine) and `tools/shell.rs` (tool) are already split.

## Design Notes (bridging spec → code)

### Exposure and dispatch (`agents/tools/mod.rs`)

```rust
pub enum ToolExposure { Direct, SandboxAdditive, CodeModeExclusive }

/// Name-based dispatch over the three regular sets. Extracted verbatim from today's
/// `VizierTools::call`; used by `VizierTools::call` and by the sandbox bridge.
pub struct ToolRouter { default_toolset, user_toolset, mcp }
impl ToolRouter {
    pub async fn definitions(&self) -> Result<Vec<ToolDefinition>>;   // today's tools() body
    pub async fn call(&self, name, args_json, ctx) -> Result<VizierResponse>; // today's call() body
}

pub struct VizierTools { router: ToolRouter, sandbox_toolset: VizierToolSet, exposure: ToolExposure, … }
```

`VizierTools::new` builds the router first, then (if `python.enabled`) `sandbox_toolset = ExecutePython::new(router.clone(), limits, code_mode)` and (if `python.code_mode`) the two docs tools. `tools()`/`call()` branch on `exposure` exactly once each (table in research Decision 8). Public fields `default_toolset`/`user_toolset`/`mcp` remain available through the router for the few external readers (`dream_tools`, skills) — grep at implementation time and forward accessors rather than duplicating.

### Runtime loop (`sandbox/runtime.rs`)

```text
execute(script, limits, bridge) -> ExecutionReport
  ├─ MontyRun::new(script, "main.py", [], CompileOptions::default())   // SyntaxError → report.error{kind: script}
  ├─ tracker = ResourceTracker::new(ResourceLimits{ max_duration: limits.timeout /* = tools.timeout */,
  │            max_memory: Some(SINGLE_ALLOCATION_GUARD /* 1 GiB, per-op pre-check only */),
  │            ..default /* recursion 1000, suspensions 1000 */ })
  ├─ loop on RunProgress:
  │    Complete(v)        → report.result = monty_to_json(v)?  (ConversionError → error{kind: script})
  │    FunctionCall(c)    → if deadline.hit  → c.abort(TimeoutError)
  │                         match c.function_name:
  │                           "list_tools" | "describe_tool" → bridge.docs(...)
  │                           "execute_python"               → Error(RuntimeError "nested execution is not allowed")
  │                           name → if !code_mode           → NotFound → NameError in script (FR-019)
  │                                  else handle.block_on(bridge.call(name, args→json, kwargs→json))
  │                                       Ok(resp)  → record invocation ok; Return(json_to_monty(resp.content))
  │                                       Err(e)    → record invocation err; Error(MontyException(RuntimeError, e))
  │    NameLookup(n)      → Undefined
  │    OsCall(o)          → o.abort(RuntimeError "filesystem/OS access is not available in the sandbox")
  │    ResolveFutures(_)  → abort (never uses resume_pending)
  │    Err(exc)           → report.error{kind: limit|script by ExcType(TimeoutError/MemoryError/RecursionError)}
  └─ drop MontyRun + every MontyObject on this thread before returning the plain-data report (FR-021a)
```

Outer: nothing new — the agent loop already wraps every tool call in `tokio::time::timeout(tools.timeout, …)` and returns `Tool 'execute_python' timed out` to the turn. `ExecutePython::call` sets `deadline` on drop (the timeout drops the future) so the orphaned blocking thread `abort`s at its next host call; if it is spinning without host calls, Monty's `max_duration` (same value) ends it within one more `tools.timeout`. Worst-case orphan lifetime ≤ `2 × tools.timeout`.

### Bridge (`agents/tools/python/bridge.rs`)

`impl SandboxBridge for RouterBridge { router, ctx, hooks, attachments: Vec }`: `call()` = `hooks.on_tool_call` → `timeout(tools.timeout, router.call)` → `hooks.on_tool_response` → collect `resp.attachments` into the outer response → return `resp.content` JSON. Nested `execute_python` is refused *before* reaching the router (FR-011).

### System prompt briefing (`agents/agent/system_prompt/sandbox.rs`)

`pub fn sandbox_md(exposure: &ToolExposure, tools_timeout: Duration) -> Option<String>` — `None` for `Direct`; otherwise a `# SANDBOX.md` system message (text in `contracts/tool-definitions.md`, section *System prompt briefing*). Pushed by `prepare_system_prompts` right after `BOOT.md`, before the owner/CORE documents, so it reads as operating doctrine rather than identity. Same shape as `boot_md`/`owner_md`: a plain `format!` function, no template engine. The static BOOT.md directive 7 ("Programmatic Sandbox") stays as-is — it is the generic nudge, `SANDBOX.md` is the per-agent specifics.

### WebUI

`AgentForm.tsx` "Python" card: `Python sandbox` toggle; `Code mode (programmatic tool calling)` toggle — disabled + tooltip when sandbox off, auto-off when sandbox toggled off, with the warning copy from US4-S7. No other inputs — the help text points at the existing tool timeout as the bound. `chat.tsx`: `formatToolChoice` cases for the three tools (`execute_python` renders the script as a fenced `python` block); new `'tool_response' in content` branch that, when the payload parses as an `ExecutionReport`, adds an `execution` inline event rendered by a small collapsible component.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| `ToolRouter` struct | Two real callers (direct dispatch, sandbox bridge) must share one dispatch path (Principle II, FR-016) | Letting `ExecutePython` hold a reference to `VizierTools` is circular (tool inside the set it dispatches through); duplicating the `mcp_`/default/user lookup in the bridge is the copy-paste Principle II forbids. |
