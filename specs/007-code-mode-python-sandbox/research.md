# Research: Python Sandbox & Code Mode

**Feature**: `007-code-mode-python-sandbox` | **Date**: 2026-09-16

All findings below were verified against `monty` **0.0.23** by compiling and running a scratch probe (`cargo build` of a 100-line program against the crate, then executing it) — not from documentation alone. Where a claim comes from the crate's source in the cargo registry the file is named.

## Decision 1: Sandbox engine — `monty` 0.0.23 (in-process), not `monty-pool`

**Decision**: Depend on `monty = "0.0.23"` and `monty-types = "0.0.23"` and drive the interpreter in-process with `MontyRun::start` / `RunProgress` resume loop. Do **not** use `monty-pool` (subprocess workers) and do **not** use `monty-alloc`.

**Rationale**:
- Pure Rust: the whole dependency tree (ruff parser, `jiter`, `fancy-regex`, `num-bigint`, `postcard`, …) has no C dependencies, so it needs no new `Cross.toml` `pre-build` steps and builds for `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu`, and both macOS targets. MSRV is 1.95; the project toolchain is 1.97. License MIT.
- Compile cost is ~20 s cold on the dev machine (measured); no build-script surprises.
- Satisfies FR-024 (nothing to install, ships in the binary) — `monty-pool` needs a worker binary on disk, which would either break the single-binary contract or require embedding and extracting an executable at runtime (fragile on `noexec` mounts, adds a second cross-compiled artifact). Rejected.
- `monty-alloc` is a **process-wide** global allocator that calls `process::exit` when its hard limit is crossed (`monty-alloc-0.0.23/src/lib.rs::charge` → `out_of_memory`). That is exactly what FR-022 forbids (a script must not affect the hosting process). Rejected; see Decision 6 for what replaces it.

**Alternatives considered**: RustPython (much larger, no host-pause/resume model, no built-in limits); `pyo3` + CPython (requires system Python — violates FR-024/Principle III; the repo's `Cross.toml` already carries `python3-dev` for another reason, but that is a build-time artefact, not a runtime one); WASM Python (heavy runtime, slow startup, no first-class host function bridge); a home-grown expression evaluator (not Python, defeats the point).

## Decision 2: Host↔script bridge — `FunctionCall` pause/resume, lazy resolution, no pre-injection

**Verified behaviour** (probe run):
- `MontyRun::new(code, "main.py", vec![] /* no input names */, CompileOptions::default())` then `runner.start(vec![], tracker, print)` returns `RunProgress`. **Any call to an unresolved name arrives as `RunProgress::FunctionCall(call)`** with `call.function_name`, `call.args: Vec<MontyObject>`, `call.kwargs: Vec<(MontyObject, MontyObject)>`. The host resumes with `call.resume(result, print)` where `result: impl Into<ExtFunctionResult>`:
  - `MontyObject` → `ExtFunctionResult::Return` (the value the script receives),
  - `MontyException::new(ExcType::ValueError, Some(msg))` → `ExtFunctionResult::Error` → **catchable** by `try/except` in the script (verified),
  - `ExtFunctionResult::NotFound(name)` → `NameError` in the script (catchable) — this is how "tool not available" is reported (FR-019, US2-S4).
  - `call.abort(exc, print)` → uncatchable; used for host-side hard failures (limits, cancellation).
- Repeated calls of the same name do **not** trigger `NameLookup` (0 lookups, 3 calls for a 3-iteration loop). `RunProgress::NameLookup` only fires for a bare unresolved name that is *not* called; the host answers `NameLookupResult::Undefined` → `NameError`.
- `RunProgress::OsCall` fires for `open(...)`, `os.environ`, etc. The host answers `abort`/error → nothing touches the filesystem. `import socket` / `import subprocess` fail with `ModuleNotFoundError` inside the interpreter. (FR-020, US1-S5.)
- `RunProgress` is `Send` (a `fn assert_send<T: Send>` compiled against it) and it is fine to `.await` arbitrary futures between `start`/`resume` — execution is suspended, the interpreter's clock is paused.

**Decision**: Do not pre-declare tool names as inputs. Every `FunctionCall` is routed by name to: (a) the two in-script helpers `list_tools()` / `describe_tool(name)` (FR-013), (b) a tool function when code mode is on, (c) `NotFound` otherwise. This scales to hundreds of MCP tools with zero per-run setup cost.

**Alternatives considered**: pre-injecting one `MontyObject::Function` per tool (works, but is O(tools) per run and needs the list up front); a `tools.<name>` namespace object via `object_id`-based method calls (more moving parts, no benefit for a flat tool namespace).

## Decision 3: Threading — `spawn_blocking` interpreter thread, tool calls bridged back to the runtime

**Decision**: `ExecutePython::call` (async) captures `tokio::runtime::Handle::current()`, then runs the whole `start`/`resume` loop inside `tokio::task::spawn_blocking`. When the loop hits `FunctionCall`, it calls `handle.block_on(bridge.call(name, args))` — legal from a blocking-pool thread — so the tool executes on the normal async runtime with the same `ToolContext`, hooks and per-tool `tools.timeout` as a direct call (FR-016).

**Rationale**: Monty is synchronous and CPU-bound; running it on a runtime worker would starve other agents. `spawn_blocking` is the project's existing pattern for CPU work and needs no new dependency.

**Cancellation / time bound (user decision: no new timeout setting)**: `execute_python` is itself a tool call, so the agent loop's existing `tokio::time::timeout(tools.timeout, …)` already bounds it wall-clock and reports `Tool 'execute_python' timed out` like any other tool — nothing to add. What Monty additionally needs is a value for its own CPU clock, because a `while True: pass` cannot be interrupted from outside: `max_duration` is set to the same `tools.timeout` (verified it fires cleanly: `TimeoutError: time limit exceeded: 50.01ms > 50ms`). Since `max_duration` is **paused while the host services a call** (`monty-types-0.0.23/src/resource.rs`, `ResourceTracker` doc comment), an `AtomicBool` deadline flag — set when the tool future is dropped by the outer timeout — is checked before every host call so the orphaned interpreter thread `abort`s at its next suspension. Worst-case orphan lifetime ≤ `2 × tools.timeout`. Trade-off accepted: a script's nested slow tool calls all count against the one outer `tools.timeout`; operators raise it if needed.

## Decision 4: Value conversion — one `serde_json::Value ⇄ MontyObject` module, no serde round-trip

**Verified**: `MontyObject`'s derived `Serialize` is an *externally tagged* enum (`{"Int": 42}`) meant for snapshots (`object.rs` doc comment), so it cannot be handed to tools as JSON. `MontyObject` variants (`object.rs`): `None, Bool, Int(i64), BigInt, Float, String, Bytes, List, Tuple, NamedTuple, Dict(DictPairs), Set, FrozenSet, Date, DateTime, Time, TimeDelta, TimeZone, Exception, Type, BuiltinFunction, Path, FileHandle, ClassInstance, Function, Repr, Cycle, Ellipsis, NotImplemented`. `DictPairs: From<Vec<(MontyObject, MontyObject)>>`.

**Decision**: `convert.rs` with two total functions:
- `json_to_monty(Value) -> MontyObject`: Null→None, Bool, Number→Int(i64)/Float, String, Array→List, Object→Dict with String keys. Total — every JSON value converts.
- `monty_to_json(&MontyObject) -> Result<Value, ConversionError>`: None/Bool/Int/BigInt(→ i64 or decimal string)/Float(NaN/inf → string)/String/List/Tuple/Set/FrozenSet(→ array)/Dict(non-string keys → `py_repr` string keys)/Date/DateTime/Time/TimeDelta (→ ISO-8601 string)/Bytes (→ base64 string)/Path (→ string). `ClassInstance`, `Function`, `Type`, `BuiltinFunction`, `FileHandle`, `Exception`, `Repr`, `Cycle` → `ConversionError` naming the offending type, surfaced to the agent as the "return value must be plain data" error (spec edge case).

Tool arguments: `call.args` (positional) are mapped onto the tool's input schema by parameter order only when the schema has `required` keys in declared order — otherwise positional args are rejected with a `TypeError` in-script telling the agent to use keyword arguments. `kwargs` become the JSON object passed to the tool verbatim.

## Decision 5: Output capture & result shape

**Verified**: `PrintWriter::CollectString(&mut String, Option<usize>)` collects `print()` output into a caller-owned buffer, hard-capped at the given byte count (`"a\nb"` with cap 3 → exactly 3 bytes). Syntax errors come back from `MontyRun::new` as `MontyException` with `SyntaxError: …`; runtime errors carry a full CPython-style traceback with `StackFrame { filename, start: CodeLoc { line, column }, preview_line, … }` (FR-010, US1-S3).

**Decision**: The tool returns an **Execution Report** (see data-model.md) as a `VizierResponse` (so nested tools' attachments propagate, FR-018): `{ ok, result, stdout, error: { kind, message, traceback }, tool_calls: [...], duration_ms }`. `result` is the value of the last expression (Monty semantics — same as the REPL), or `null`. No truncation (user decision); the engine's 10 MiB print cap ends a run with `MemoryError` if `stdout` grows past it.

## Decision 6: Memory — no per-script ceiling in v1 (user decision); memory is released per run

**Verified**:
- Without a process-installed allocator, `ResourceLimits.max_memory` **is silently not enforced** for cumulative growth: `x = []; for i in range(200000): x.append('a' * 1000)` (≈200 MB) completed under a 2 MiB limit. A single oversized allocation *is* caught (`'a' * 10**8` → `MemoryError: memory limit exceeded: 100000000 bytes > 2097152 bytes`) because specific operations pre-check their size against `max_memory`.
- Enforcing cumulative growth requires feeding `monty_types::LIVE_MEMORY` from a `#[global_allocator]`; Monty's own `monty-alloc` does that but calls `process::exit` on its hard limit (unacceptable for a long-lived server), so it would have to be a custom thread-scoped wrapper.
- **Memory is released when a run ends.** Measured RSS on this machine (glibc, debug build): 4.8 MB before any run → 227.7 MB after a 200 MB-allocating script and dropping its result → 231.7 MB after the second run → 231.7 MB after the third (no growth: the first run's memory was reused). On a separate thread: 448.7 MB peak during the run → 232.1 MB after the thread exited (the thread arena was released). glibc keeps freed pages cached in-process rather than returning them to the kernel, but they are reused, not leaked; musl (the static release target) trims more eagerly.

**Decision**: **No configurable memory ceiling and no global allocator in v1.** The user chose not to install a process-wide counting allocator at this stage. What ships instead:
- `ResourceLimits.max_memory` is set to a fixed internal constant `SINGLE_ALLOCATION_GUARD = 1 GiB` (not a setting) so Monty's free per-operation pre-check turns `"a" * 10**10` into a clean `MemoryError` rather than an attempted 10 GB allocation. This is a guard, not a ceiling — it does nothing for cumulative growth.
- FR-021a (all memory released at the end of a run) is guaranteed structurally: one `MontyRun` per execution, created and dropped on the interpreter thread; only the plain-data `ExecutionReport` crosses back.
- Residual risk, accepted and documented in the spec: a single run's peak before `timeout` fires is unbounded; a tight accumulation loop could reach several GB in 30 s on a box with that much headroom, or trip the kernel OOM-killer on one without. Operators who care can lower `timeout`.

**Alternatives considered**: thread-scoped counting `#[global_allocator]` (~60 lines, no crate; would make the ceiling soft and aggregate across concurrent scripts — deferred, easy to add later without changing any contract); `monty-alloc` (exits the process); `monty-pool` subprocess workers (needs a second binary — Decision 1); per-thread cgroup/RLIMIT (not portable, not thread-granular); serialising runs behind a mutex (breaks FR-023).

## Decision 7: Two switches on `AgentToolsConfig.python`, validated in one place

**Decision**: `AgentToolsConfig` gains `#[serde(default)] python: PythonSandboxConfig { enabled, code_mode }` — two booleans and nothing else (user decision: no limit settings; the existing `tools.timeout` is the bound). `#[serde(default)]` makes FR-006 (existing agents load with both off) automatic — no migration. The `code_mode ⇒ enabled` invariant (FR-002) is enforced in `VizierAgents`' `Create`/`Update` command handling (the single choke point both the REST handlers already funnel through — `AgentCommandResult::Error` becomes HTTP 400), *and* defensively normalised in `VizierTools::new` (a legacy/hand-edited record with `code_mode && !enabled` is treated as `code_mode = false`, spec edge case). The WebUI disables the code-mode toggle while the sandbox is off and cascades it off when the sandbox is switched off (US4-S4/S5).

**Rationale**: matches how every other conditional tool is configured (`ToolConfig<Settings>` + `CreateAgentTools` request struct + `AgentForm.tsx`), so operators find it where they expect (FR-001).

## Decision 8: Tool exposure gating lives in `VizierTools`, tools stay ordinary `VizierTool`s

**Decision**: Build the toolsets as today, then construct the sandbox tools *after* `default_toolset`/`user_toolset`/`mcp` exist and hand them a `ToolRouter` (a small struct wrapping clones of those three — `VizierToolSet` is a `HashMap<String, Arc<…>>`, cheap to clone). The existing body of `VizierTools::call` (the `mcp_` prefix split + default→user lookup) moves into `ToolRouter::call`; `VizierTools::call` and the sandbox bridge both use it — one dispatch path (Principle II, FR-016). `VizierTools` gets an `exposure: ToolExposure { Direct, SandboxAdditive, CodeModeExclusive }` computed once from the config, and `tools()` / `call()` consult it:

| Exposure | `tools()` returns | `call()` accepts |
|---|---|---|
| `Direct` | regular tools | regular tools |
| `SandboxAdditive` | regular tools + `execute_python` | regular + `execute_python` |
| `CodeModeExclusive` | `think`, `execute_python`, `list_tool_functions`, `describe_tool_function` | those four; any regular tool name → error "not exposed directly; call it from `execute_python`" |

`think` is the single "housekeeping" tool kept direct in exclusive mode (spec assumption): it acts on nothing but the model's own reasoning and is what the WebUI's live "thinking" indicator is built on.

Dream cycle: `dream_tools()` is untouched (FR-007) — the sandbox tools are never in `DREAM_TOOL_NAMES`.

**Alternatives considered**: making `ExecutePython` hold a back-reference to `VizierTools` (circular, needs `Weak`/`OnceLock`); a separate `VizierTools` variant per mode (duplicates the whole constructor).

## Decision 9: Documentation tools derive everything from `ToolDefinition`

**Decision**: `docs.rs` builds a `ToolFunctionDoc` from `ToolDefinition { name, description, parameters }` (+ `output_schema()` for native `VizierTool`s; MCP tools have no output schema → "any JSON value"). It walks the JSON Schema's top-level `properties`/`required` to produce the parameter table and a generated example: `result = memory_read(query="…", limit=5)` (placeholders by JSON type). Python identifier sanitisation: replace any char outside `[A-Za-z0-9_]` with `_`, prefix `_` if leading digit; on collision append `_2`, `_3`, … deterministically in sorted-name order. The catalogue and the in-script `list_tools()` share the same function.

Native tool names in this repo (`memory_read`, `WRITE_CORE`, `mcp_<server>__<tool>`) are already valid identifiers, so sanitisation only matters for MCP servers with `-` in tool names.

## Decision 10: Observability — history via `ToolResult`, live via the existing hook pipeline

**Verified**: tool calls are already persisted as `SessionHistoryContent::ToolCall { name, arguments }` + `ToolResult { content }` (`schema/history.rs`), and live `tool_choice` events are emitted by `ToolCallsHook::on_tool_call` and forwarded verbatim by the WebSocket channel (`api/v1/agents/channel.rs`). The WebUI renders tool activity only as transient inline events during a turn (`chat.tsx` `formatToolChoice`), not from stored history.

**Decision**:
- **History (FR-025)**: the Execution Report *is* the `ToolResult.content` of the `execute_python` call — script (in the `ToolCall.arguments`), nested invocations, stdout, result, error, duration. No new storage.
- **Live nested calls**: `ToolContext` gains `hooks: Option<VizierSessionHooks>`; the sandbox bridge runs each nested call through `hooks.on_tool_call` / `on_tool_response`, so nested tools produce the same `tool_choice` events (and the same debug logging) as direct calls, for free.
- **Live report (US5-S1)**: `ToolCallsHook::on_tool_response` forwards the `ToolResponse` for `execute_python` only; `chat.tsx` renders it as a collapsible "execution" inline event (script + invocation list + stdout + result/error + duration). Other tools' responses stay un-forwarded (unchanged behaviour).
- **Logging (FR-027)**: `tracing::info_span!("python_exec", agent, session)` around the run; one `info!` per nested tool call; `warn!` on limit hits.

## Decision 11: Defaults

| Setting | Default | Why |
|---|---|---|
| (time) | agent's existing `tools.timeout` | No new setting (user decision); same bound as every other tool call |
| Monty `max_recursion_depth` | `1000` (crate default) | CPython's default; verified error is a clean `RecursionError` |
| Monty `max_memory` | `1 GiB` fixed constant | Single-allocation guard only (Decision 6); not user-configurable |
| Monty `max_suspensions` | `1000` (crate default) | Backstop on host round-trips per run; a script making >1000 tool/docs calls ends with an error |
| print buffer | `10 MiB` (crate default, `DEFAULT_MAX_PRINT_COLLECT_BYTES`) | No output truncation setting (user decision); this is the only hard cap on `stdout` |

## Decision 12: How the agent learns the feature — four layers, one new system message

**Observation**: `BOOT.md` (`system_prompt/boot.rs`) already carries a static directive *"7. Programmatic Sandbox — Use sandbox tools for complex multi-step operations"* for every agent, but nothing tells a specific agent which mode it is in. In exclusive code mode the model wakes up with a four-tool list and no explanation of why its tools vanished; a tool description alone is easy to skim past.

**Decision**: Teach the model through four layers, adding only the last:
1. **Tool description** (`execute_python`, per-agent, limits interpolated) — the primary channel (FR-008; text in `contracts/tool-definitions.md`).
2. **Documentation tools** in code mode (`list_tool_functions`, `describe_tool_function`) and their in-script twins (`list_tools()`, `describe_tool()`), all derived from `ToolDefinition` (Decision 9).
3. **Error feedback as data** — syntax/runtime/limit errors with tracebacks, `NameError` when tools are off, `RuntimeError("<tool>: …")` for tool failures, `did_you_mean` on unknown names (SC-010).
4. **`SANDBOX.md` system message** (new, `system_prompt/sandbox.rs`): a mode-specific briefing injected by `prepare_system_prompts` only when the sandbox is on — sandbox-only variant (what it's for, no tool access, timeout) or code-mode variant (tools are reached via scripts, start with `list_tool_functions`, prefer one aggregating script, whole script within `tools.timeout`, return only what you need). Mirrors the existing `boot_md`/`owner_md` pattern — a `format!` function, no config, no template engine.

**Alternatives considered**: editing the static `BOOT.md` directive to carry the specifics (wrong — it is shared by agents that have the sandbox off); seeding guidance into the agent's `CORE.md` (persists after the operator turns the mode off, and CORE is the agent's own document, not ours to rewrite); relying on the tool description alone (rejected for the exclusive-mode reason above).

## Supported Python subset (from the Monty limitations page, to be surfaced in the tool description)

Supported: functions/closures/lambdas/decorators, simple classes (no inheritance), `@dataclass`, comprehensions, `try/except/finally`, `with`, f-strings, `async/await`, `import` of the bundled modules, starred unpacking. Modules: `json, math, datetime, re, collections, itertools, functools, dataclasses, typing, base64, binascii, copy, random, unicodedata, asyncio, sys, os, pathlib` (the last three are stubs — any real OS access pauses as `OsCall` and is refused).
Not supported: class inheritance, `@property`/`@classmethod`/`@staticmethod`, generators/`yield`, `match`, `del`, `async with/for`, user-defined exception classes, `eval`/`exec`, third-party packages, `enum`, `time`, `hashlib`, `io`, `socket`, `subprocess`, `threading`.
