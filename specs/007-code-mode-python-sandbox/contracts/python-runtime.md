# Contract: Python Runtime (what the script sees)

The behavioural contract between a script and the host, implemented in `src/sandbox/runtime.rs` + `src/agents/tools/python/bridge.rs`. Everything here is testable without an LLM by driving `sandbox::execute` with a fake `SandboxBridge`.

## Engine-facing interfaces (`src/sandbox/mod.rs`)

```rust
pub struct SandboxLimits { timeout: Duration /* = agent tools.timeout */, tools_enabled: bool }
// module constants: SINGLE_ALLOCATION_GUARD = 1 << 30 (Monty max_memory), MAX_SCRIPT_BYTES = 64 KiB;
// Monty defaults kept for recursion depth (1000), max_suspensions (1000), print buffer (10 MiB)

/// Implemented by the agent layer; the engine never sees VizierTools.
#[async_trait]
pub trait SandboxBridge: Send + Sync {
    /// Catalogue for `list_tools()`; also used to resolve names.
    async fn catalogue(&self) -> Vec<ToolFunctionDoc>;
    async fn describe(&self, function: &str) -> Option<ToolFunctionDoc>;
    /// Invoke a tool. `arguments` is the JSON object built from the call (see Argument mapping).
    async fn call(&self, function: &str, arguments: serde_json::Value) -> Result<serde_json::Value, String>;
}

pub async fn execute(code: &str, limits: SandboxLimits, bridge: Arc<dyn SandboxBridge>) -> ExecutionReport;
```

`execute` never returns `Err` — every failure is an `ExecutionReport` with `error` set, because the *agent* must see it (SC-010). Only programmer errors (e.g. a poisoned counter) are `tracing::error!`ed and reported as `Limit/timeout`-style host failures.

## Name resolution (inside a script)

| Script does | Host answers | Script sees |
|---|---|---|
| `list_tools()` | catalogue → list of `{"function", "summary"}` dicts | `list[dict]` |
| `describe_tool("x")` | describe → dict (`ToolFunctionDoc`) or `{"available": False, …}` | `dict` |
| `describe_tool()` / wrong arity | `TypeError` | catchable |
| `execute_python(...)` | `RuntimeError("nested execute_python is not allowed")` | catchable (FR-011) |
| `<tool>(...)` and code mode **off** | `NotFound` | `NameError: name '<tool>' is not defined` (FR-019) |
| `<tool>(...)` and code mode **on** | tool result (JSON → Python) | plain data |
| `<tool>(...)` and the tool returned an error | `RuntimeError("<tool>: <message>")` | catchable (FR-017); record has `ok=false` |
| `<unknown>(...)` | `NotFound` | `NameError` |
| bare `<unknown>` (not called) | `NameLookupResult::Undefined` | `NameError` |
| `open(...)`, `os.environ`, `os.getenv`, path I/O | `OsCall` → `abort(RuntimeError("…not available in the sandbox"))` | **uncatchable**; run ends `Script` kind |
| `import socket` / `subprocess` / `time` / … | interpreter `ModuleNotFoundError` | catchable |

Exception type for tool failures is `RuntimeError` so that `except RuntimeError` / bare `except Exception` both work and the script cannot confuse it with its own `ValueError`s. The message always starts with the tool name.

## Argument mapping (script call → tool JSON)

- **Keyword arguments** (the documented style): `kwargs` → JSON object verbatim (`memory_read(query="x", limit=3)` → `{"query":"x","limit":3}`).
- **Positional arguments**: mapped onto the tool's input-schema `required` list in declared order; if the tool has no `required` list, or more positionals than required params are given → `TypeError("<tool>() takes keyword arguments only; see describe_tool('<tool>')")`.
- Keys must be `str`; a non-string kwarg key cannot occur in Python syntax.
- Values converted per **Value mapping** below. A value that cannot be converted (e.g. passing a function object) → `TypeError` naming the parameter.

## Value mapping

**Host → script** (`json_to_monty`, total):

| JSON | Python |
|---|---|
| `null` | `None` |
| `true`/`false` | `bool` |
| integer (fits i64) | `int` |
| other number | `float` |
| string | `str` |
| array | `list` |
| object | `dict` (str keys, insertion order preserved) |

**Script → host** (`monty_to_json`, for `result` and for tool arguments):

| Python | JSON |
|---|---|
| `None`, `bool`, `int` (i64 range), `float` (finite), `str` | native |
| `int` outside i64 | decimal string |
| `nan`/`±inf` | `"NaN"`, `"Infinity"`, `"-Infinity"` |
| `list`, `tuple`, `set`, `frozenset` | array (`set` ordered by `py_repr`) |
| `dict` | object; non-`str` keys → their `repr` |
| `datetime.date/datetime/time/timedelta` | ISO-8601 string (`timedelta` → `"PT…"`) |
| `bytes` | base64 string |
| `pathlib.Path` | string |
| class instance, function, type, builtin, file handle, exception object | **error**: `"cannot return <type>; return plain data (str, int, float, bool, None, list, dict)"` → `ExecutionErrorKind::Script` |

## Result semantics

- `result` = value of the **last expression statement** of the script (REPL semantics — `x = 1` as the last line yields `null`; `x` as the last line yields `1`). This is exactly what `MontyRun::run` returns and matches the Pydantic code-mode convention.
- `stdout` = everything `print()`ed (`PrintWriter::CollectString(&mut buf, Some(DEFAULT_MAX_PRINT_COLLECT_BYTES))`, the engine's 10 MiB cap; exceeding it ends the run with `MemoryError`). No truncation of `stdout` or `result` (user decision).
- An empty script, or one whose last statement is not an expression, is a **successful** run with `result: null`.

## Limits and their errors

| Limit | Enforced by | Script sees | Report |
|---|---|---|---|
| Wall-clock (`tools.timeout`) | the agent loop's existing per-tool `tokio::time::timeout` — nothing new | the turn gets `Tool 'execute_python' timed out`; the dropped future sets the deadline flag so the next host call is `abort`ed | (turn error, as for any tool) |
| CPU time (`tools.timeout`) | Monty `max_duration` set to the same value (paused during host calls) — ends an orphaned spinning thread | `TimeoutError` — **uncatchable** in practice (raised at VM checkpoint, run ends) | `Limit/timeout` |
| Single allocation > 1 GiB | Monty's per-operation size pre-check (fixed constant, no allocator) | `MemoryError` | `Limit/memory` |
| Cumulative memory | **not enforced in v1** (user decision) — peak is bounded by `timeout`; everything is released when the run ends | — | — |
| Recursion (1000) | Monty | `RecursionError` | `Limit/recursion` |
| Host round-trips (1000) | Monty `max_suspensions` (crate default) | run ends | `Limit/suspensions` |
| Script size (64 KiB) | host, before compile | — | `Limit/script_size` |
| Per-tool time (`tools.timeout`) | bridge (`tokio::time::timeout` around `router.call`) | `RuntimeError("<tool>: timed out after …")` (catchable) | record `ok=false` |

## Threading & isolation guarantees

- The interpreter runs on a `spawn_blocking` thread; tool futures run on the tokio runtime via `Handle::block_on` while the interpreter is suspended. `RunProgress` is `Send` (verified), so no `unsafe`.
- One `MontyRun` per execution; nothing is shared between executions (FR-009). Concurrent executions share nothing.
- All `MontyObject`s are created and dropped on the interpreter thread before it returns; only `ExecutionReport` (plain Rust/JSON) crosses back — this is what guarantees FR-021a (memory released per run; verified: three consecutive 200 MB runs did not grow the process).
- Panics inside the blocking task are caught by `JoinError` and reported as a `Script`-kind host failure with `message = "sandbox panicked: …"` — the agent and process continue (FR-022).
