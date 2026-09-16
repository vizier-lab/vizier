# Tasks: Python Sandbox & Code Mode (Programmatic Tool Calling)

**Input**: Design documents from `/specs/007-code-mode-python-sandbox/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (tool-definitions, python-runtime, http-api, webui), quickstart.md

**Tests**: plan.md's Technical Context calls for `cargo test` unit tests on `convert.rs`, `docs.rs` and `runtime.rs` (all runnable without an LLM because the bridge is a trait). Those are included as tasks inside the story that introduces each module. No WebUI tests exist in this repo; the WebUI gate is `npm run typecheck`.

**Organization**: Tasks are grouped by user story. US1 (pure sandbox) is the MVP; US2–US5 layer on it in priority order.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1 … US5 as numbered in spec.md
- Every task names the exact file(s) it touches

## Path Conventions

Single Rust binary with an embedded WebUI: backend under `src/`, WebUI under `webui/app/`. Module tree is declared in `src/main.rs` (`mod …;` list, lines 12–33). New engine code goes in `src/sandbox/`; the tool adapters in `src/agents/tools/python/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependencies, module skeleton, and the persisted config field every story reads.

- [ ] T001 Add `monty = "0.0.23"` and `monty-types = "0.0.23"` to `[dependencies]` in `Cargo.toml` (pin exact 0.0.x — the crate is pre-1.0 and its API moves between minor releases) and run `cargo fetch`; confirm `cargo build` still succeeds with `webui/node_modules` present
- [ ] T002 Create the engine module skeleton: `src/sandbox/mod.rs` (empty `pub mod convert; pub mod report; pub mod runtime; pub mod docs;` plus `pub use` of the public types listed in contracts/python-runtime.md), empty `src/sandbox/convert.rs`, `src/sandbox/report.rs`, `src/sandbox/runtime.rs`, `src/sandbox/docs.rs`, and add `mod sandbox;` to the module list in `src/main.rs`
- [ ] T003 [P] Add `PythonSandboxConfig { enabled: bool, code_mode: bool }` (derives `Debug, Serialize, Deserialize, Clone, Default, JsonSchema, utoipa::ToSchema`, `#[serde(default)]` on both fields) to `src/schema/agent.rs` and add `#[serde(default)] pub python: PythonSandboxConfig` to `AgentToolsConfig` (after `image_gen`, ~line 89); verify an existing agent JSON without the field still deserialises (FR-006)
- [ ] T004 [P] Create the tool-adapter module skeleton: `src/agents/tools/python/mod.rs` (declares `mod bridge; mod docs_tools;`), empty `src/agents/tools/python/bridge.rs` and `src/agents/tools/python/docs_tools.rs`, and add `mod python;` to the module list in `src/agents/tools/mod.rs` (~line 49–70)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The value bridge, the report type, the dispatch refactor and the API plumbing that every story needs before it can be exercised at all.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [ ] T005 [P] Implement `src/sandbox/convert.rs`: `json_to_monty(serde_json::Value) -> MontyObject` (total) and `monty_to_json(&MontyObject) -> Result<serde_json::Value, ConversionError>` exactly per the two *Value mapping* tables in `contracts/python-runtime.md` (Dict via `DictPairs::from(Vec<(MontyObject, MontyObject)>)`; `Int(i64)`; BigInt → i64 or decimal string; NaN/inf → strings; date/time → ISO-8601; bytes → base64; `ClassInstance`/`Function`/`Type`/`BuiltinFunction`/`FileHandle`/`Exception`/`Repr`/`Cycle` → `ConversionError { python_type }`)
- [ ] T006 [P] Implement `src/sandbox/report.rs`: `ExecutionReport`, `ExecutionError`, `ExecutionErrorKind { Script, Tool, Limit }` (serde `rename_all = "snake_case"`), `ToolInvocationRecord` — field-for-field as in data-model.md (no `truncated` field), all `Serialize + Deserialize + Clone + Debug`, plus `ExecutionReport::failed(kind, message, traceback, limit)` and `::from_monty_exception(&MontyException, duration)` helpers that map `ExcType::TimeoutError/MemoryError/RecursionError` → `Limit` with `limit = "timeout"/"memory"/"recursion"` and everything else → `Script` with `traceback = exc.to_string()`
- [ ] T007 [P] Define the engine interface in `src/sandbox/mod.rs`: `pub struct SandboxLimits { pub timeout: Duration, pub tools_enabled: bool }`, constants `SINGLE_ALLOCATION_GUARD: usize = 1 << 30` and `MAX_SCRIPT_BYTES: usize = 64 * 1024`, and `#[async_trait] pub trait SandboxBridge: Send + Sync { async fn catalogue(&self) -> Vec<ToolFunctionDoc>; async fn describe(&self, function: &str) -> Option<ToolFunctionDoc>; async fn call(&self, function: &str, arguments: serde_json::Value) -> Result<serde_json::Value, String>; }` plus a `pub struct NoToolsBridge;` impl returning empty catalogue / `None` / `Err("tool access from scripts is not enabled")` (used by US1 and by unit tests); declare a placeholder `pub struct ToolFunctionDoc` in `src/sandbox/docs.rs` with the fields from data-model.md (filled in by US3)
- [ ] T008 Extract `ToolRouter` in `src/agents/tools/mod.rs`: new `#[derive(Clone)] pub struct ToolRouter { pub default_toolset: VizierToolSet, pub user_toolset: VizierToolSet, pub mcp: HashMap<String, Arc<VizierMcp>> }` with `pub async fn definitions(&self) -> Result<Vec<ToolDefinition>>` (move the body of today's `VizierTools::tools`, lines ~199–222) and `pub async fn call(&self, function_name, params, ctx) -> Result<VizierResponse>` (move the body of today's `VizierTools::call`, lines ~224–277, including the `mcp_` prefix split and the default→user fallback); `VizierTools` keeps `router: ToolRouter` and its `tools()`/`call()` delegate to it unchanged in behaviour; update `dream_tools`/`dream_call` and `VizierTools::new` to build the router first and reference `self.router.default_toolset` — `cargo build` + `cargo clippy` must pass with zero behaviour change
- [ ] T009 Add `pub exposure: ToolExposure` (`#[derive(Clone, Copy, Debug, PartialEq)] pub enum ToolExposure { Direct, SandboxAdditive, CodeModeExclusive }`) and `pub sandbox_toolset: VizierToolSet` to `VizierTools` in `src/agents/tools/mod.rs`; compute `exposure` in `VizierTools::new` from `agent_config.tools.python` with the defensive normalisation (`code_mode && !enabled` ⇒ `SandboxAdditive` + `tracing::warn!`); leave `sandbox_toolset` empty for now and `tools()`/`call()` still `Direct`-only (the gating itself lands in US1/US2)
- [ ] T010 Add `pub hooks: Option<Arc<crate::agents::hook::VizierSessionHooks>>` to `ToolContext` in `src/agents/tools/mod.rs` (~line 70) and thread it through every construction site: `src/agents/agent/mod.rs` ~line 466 and ~line 984 (`hooks: hooks.clone()` from the enclosing `hooks: Option<Arc<VizierSessionHooks>>` parameter), `src/agents/process.rs` ~line 193 and ~line 325 (`hooks: None`); no behaviour change yet
- [ ] T011 API plumbing for the switches in `src/channels/http/api/v1/agents/mod.rs`: add `#[serde(default)] pub python: Option<PythonSandboxConfig>` to `CreateAgentTools` (~line 335), map it in `into_config()` (`python: tools.python.unwrap_or_default()` next to the `http_client` mapping ~line 428) and in the `CreateAgentTools` default literal (~line 374), and add `pub python: PythonSandboxConfig` to `AgentSummary` (~line 158) filled from `config.tools.python.clone()` (~line 230) — per contracts/http-api.md

**Checkpoint**: `cargo build`, `cargo clippy`, `cargo test` pass; the binary behaves exactly as before; `PUT /agents/{id}` with `{"tools":{"python":{"enabled":true}}}` round-trips through `GET /agents/{id}`.

---

## Phase 3: User Story 1 - Agent Runs Pure Python in a Sandbox (Priority: P1) 🎯 MVP

**Goal**: With only the sandbox switch on, the agent gets `execute_python` alongside its regular tools and can run isolated, time-bounded pure-computation scripts; tool calls from scripts fail with `NameError`.

**Independent Test**: Enable `python.enabled` on an agent via the API, ask it "What is the 40th Fibonacci number, and how many weekdays between 2026-01-01 and 2026-09-16?" — expect one `execute_python` call and a correct answer while `memory_read` etc. still work directly. Then run the quickstart safety table rows for `while True: pass`, recursion, `open(...)`, `import socket`.

### Implementation for User Story 1

- [ ] T012 [US1] Implement `src/sandbox/runtime.rs` `pub async fn execute(code: &str, limits: SandboxLimits, bridge: Arc<dyn SandboxBridge>) -> ExecutionReport` per the *Runtime loop* in plan.md: reject `code.len() > MAX_SCRIPT_BYTES` (`Limit/script_size`); capture `tokio::runtime::Handle::current()`; inside `tokio::task::spawn_blocking` build `MontyRun::new(code, "main.py", vec![], CompileOptions::default())` (compile error → `Script` report with `traceback = exc.to_string()`), `ResourceTracker::new(ResourceLimits::default().max_duration(limits.timeout).max_memory(SINGLE_ALLOCATION_GUARD))`, a `String` stdout buffer with `PrintWriter::CollectString(&mut buf, Some(monty_types::DEFAULT_MAX_PRINT_COLLECT_BYTES))`, then loop on `RunProgress`: `Complete(v)` → `monty_to_json` (conversion error → `Script`), `NameLookup(n)` → `resume(NameLookupResult::Undefined)`, `OsCall(o)` → `o.abort(MontyException::new(ExcType::RuntimeError, Some("filesystem/OS access is not available in the sandbox")))`, `ResolveFutures(_)` → abort likewise, `Err(exc)` → `ExecutionReport::from_monty_exception`; `FunctionCall` handling is a stub for this task: every name → `ExtFunctionResult::NotFound(name)` (FR-019) except `"execute_python"` → `ExtFunctionResult::Error(RuntimeError "nested execute_python is not allowed")` (FR-011); drop the `MontyRun` and every `MontyObject` inside the blocking closure before returning the plain report (FR-021a); map `JoinError` (panic) → `Script` report `"sandbox panicked: …"`; wrap the whole run in `tracing::info_span!("python_exec")` and record `duration_ms`
- [ ] T013 [US1] Add a cancellation flag to `src/sandbox/runtime.rs`: `execute` creates an `Arc<AtomicBool>` deadline and returns a guard whose `Drop` sets it (so when the agent loop's `tools.timeout` drops the future, the flag flips); the blocking loop checks the flag before servicing every `FunctionCall`/`NameLookup`/`OsCall` and, if set, `abort`s with `TimeoutError` — no new timeout setting (research Decision 3)
- [ ] T014 [US1] Implement `ExecutePython` in `src/agents/tools/python/mod.rs`: `struct ExecutePython { router: ToolRouter, limits: SandboxLimits, description: String }`, `impl VizierTool` with `name() = "execute_python"`, `Input = ExecutePythonInput { code: String }` (schema/description per contracts/tool-definitions.md), `Output = VizierResponse`; `call()` runs `sandbox::execute(&args.code, self.limits, Arc::new(NoToolsBridge))` and returns `VizierResponse { content: ToolResponse { response: serde_json::to_value(report)? }, attachments: vec![] }`; `limits.timeout` comes from `agent_config.tools.timeout` and `tools_enabled = false` in this story
- [ ] T015 [P] [US1] Add the two description templates as `pub fn execute_python_description(code_mode: bool, tools_timeout: &str) -> String` in `src/agents/tools/python/mod.rs`, text verbatim from contracts/tool-definitions.md (*sandbox only* and *code mode on* variants, `{tools_timeout}` interpolated); `ExecutePython::description()` returns the stored string
- [ ] T016 [US1] Wire `SandboxAdditive` exposure in `src/agents/tools/mod.rs`: in `VizierTools::new`, when `python.enabled`, build `sandbox_toolset = VizierToolSet::new().tool(ExecutePython::new(router.clone(), limits, code_mode=false, description))`; in `tools()` return `router.definitions()` **plus** `sandbox_toolset` defs when `exposure == SandboxAdditive`; in `call()` try `sandbox_toolset.get_tool(name)` first (its `tool_call` output already deserialises as `VizierResponse`) then fall back to `router.call`; confirm `dream_tools()` never includes the sandbox toolset (FR-007)
- [ ] T017 [P] [US1] Create `src/agents/agent/system_prompt/sandbox.rs` with `pub fn sandbox_md(exposure: &ToolExposure, tools_timeout: &str) -> Option<String>` returning `None` for `Direct` and the two `# SANDBOX.md` texts verbatim from the *System prompt briefing* section of contracts/tool-definitions.md; add `pub mod sandbox;` to `src/agents/agent/system_prompt/mod.rs`
- [ ] T018 [US1] Inject the briefing in `src/agents/agent/mod.rs` `prepare_system_prompts` (~line 275): after `Message::system(boot)` push `Message::system(md)` when `sandbox_md(&self.tools.exposure, &self.config.tools.timeout.to_string())` is `Some`
- [ ] T019 [P] [US1] Unit tests `#[cfg(test)] mod tests` in `src/sandbox/convert.rs`: JSON→Monty→JSON round-trip for null/bool/int/float/string/nested list/nested object; i64 overflow BigInt → string; NaN → `"NaN"`; non-string dict keys → repr keys; `MontyObject::Function{..}` → `ConversionError` naming the type
- [ ] T020 [P] [US1] Unit tests `#[cfg(test)] mod tests` in `src/sandbox/runtime.rs` using `NoToolsBridge` and `#[tokio::test]`: last-expression result (`1 + 1` → `2`); print capture; empty script → `ok, result: null`; syntax error → `Script` with `SyntaxError` in message; `while True: pass` with `timeout = 50ms` → `Limit/timeout`; `def f(n): return f(n+1)\nf(0)` → `Limit/recursion`; `open('/etc/passwd')` → `Script` error mentioning "not available"; `import socket` → `Script` with `ModuleNotFoundError`; `memory_read(query='x')` → `Script` with `NameError`; `execute_python(code='1')` → `Script` with "nested"; `'a' * (2**31)` → `Limit/memory`; a class instance as last expression → `Script` "cannot return"; script > 64 KiB → `Limit/script_size`
- [ ] T021 [US1] Manual verification per quickstart.md *Try it — sandbox only* and the safety table (`just run`, enable via API, run the Fibonacci/weekday prompt and each safety row); confirm the agent's regular tools still work directly and that a timed-out script yields the ordinary `Tool 'execute_python' timed out` turn error without affecting the next message

**Checkpoint**: US1 is a shippable MVP — pure Python sandbox, opt-in, isolated, time-bounded, with the briefing and description in place.

---

## Phase 4: User Story 2 - Agent Orchestrates Multiple Tool Calls in One Script (Priority: P2)

**Goal**: With code mode on, scripts call the agent's tools as functions through the same dispatch/hook/timeout path as direct calls; the model's tool list becomes exclusive (`think`, `execute_python`, and — after US3 — the two docs tools).

**Independent Test**: Enable `python.code_mode` on an agent with web-search + fetch, ask for "the three latest Rust releases summarised from their release pages" — expect one `execute_python` call whose report lists ≥4 nested invocations, a correct answer, and the intermediate page contents absent from the model context (check the stored `ToolResult`). Asking the agent to call `memory_read` directly is refused with the "not exposed directly" message.

### Implementation for User Story 2

- [ ] T022 [US2] Implement `RouterBridge` in `src/agents/tools/python/bridge.rs`: `struct RouterBridge { router: ToolRouter, ctx: ToolContext, tool_timeout: Duration, invocations: Mutex<Vec<ToolInvocationRecord>>, attachments: Mutex<Vec<VizierAttachment>> }` implementing `SandboxBridge::call`: `hooks.on_tool_call(name, args_json)` if `ctx.hooks` is `Some` → `tokio::time::timeout(tool_timeout, router.call(name, args, &ctx))` (elapsed → `Err("<name>: timed out after …")`) → `hooks.on_tool_response(resp)` → push `resp.attachments` into `attachments` → record a `ToolInvocationRecord { seq, name, arguments, ok, error, duration_ms }` → return the `ToolResponse { response }` JSON (or the `Message` text as a JSON string for tools that answer with a `Message` variant), `Err(e.to_string())` on failure with `ok=false` recorded; `catalogue`/`describe` return empty/`None` until US3; expose `into_parts(self) -> (Vec<ToolInvocationRecord>, Vec<VizierAttachment>)`
- [ ] T023 [US2] Route `FunctionCall` to the bridge in `src/sandbox/runtime.rs`: when `limits.tools_enabled`, for any name other than `execute_python`/`list_tools`/`describe_tool` build the argument object per *Argument mapping* in contracts/python-runtime.md (kwargs verbatim via `monty_to_json`; positionals only onto the schema-declared `required` order, else `TypeError("<tool>() takes keyword arguments only; see describe_tool('<tool>')")` — the required-order list comes from `bridge.describe(name)` and is `None`-tolerant), then `handle.block_on(bridge.call(name, args))`: `Ok(v)` → `resume(json_to_monty(v))`, `Err(msg)` → `resume(MontyException::new(ExcType::RuntimeError, Some(format!("{name}: {msg}"))))` (catchable, FR-017); when `!tools_enabled` keep `NotFound`; an uncaught `RuntimeError` that originated from a tool call must produce `ExecutionErrorKind::Tool` (track "last error came from the bridge" so `from_monty_exception` can classify it)
- [ ] T024 [US2] Finish `ExecutePython::call` in `src/agents/tools/python/mod.rs` for code mode: construct `RouterBridge` from `self.router`, the `ctx` passed to `call`, and `tool_timeout = limits.timeout`; after `sandbox::execute` returns, `into_parts()` the bridge and set `report.tool_calls` and `VizierResponse.attachments` from it (FR-018, FR-025); log one `tracing::info!` per nested invocation with agent/session ids (FR-027)
- [ ] T025 [US2] Wire `CodeModeExclusive` exposure in `src/agents/tools/mod.rs`: when `python.code_mode`, build `ExecutePython` with `tools_enabled = true` and the code-mode description; `tools()` returns exactly `[think (from router.default_toolset), execute_python]` (docs tools appended in US3); `call()` accepts those names and, for any other name, returns `Err(VizierError(format!("{name} is not exposed directly while code mode is on; call it from a script via execute_python (see list_tool_functions)")))` **without** dispatching it — per the exposure table in contracts/tool-definitions.md
- [ ] T026 [P] [US2] Unit tests in `src/sandbox/runtime.rs` with a `FakeBridge` (records calls, returns canned JSON, errors on a chosen name, exposes a `required` list for one tool): kwargs arrive verbatim; positionals map onto `required` order; positionals on a tool without `required` → `TypeError`; tool `Err` → catchable `RuntimeError` whose message starts with the tool name; uncaught tool error → `ExecutionErrorKind::Tool`; three calls in a loop → three invocation records in order; `tools_enabled = false` → `NameError` even with the fake bridge attached
- [ ] T027 [US2] Manual verification per quickstart.md *Try it — code mode* (Rust-releases prompt): nested `tool_choice` events appear one by one, the stored `ToolResult` for `execute_python` contains the report with every invocation, page bodies never appear as separate `ToolResult` entries, and a direct `memory_read` call is refused with the "not exposed directly" message

**Checkpoint**: Code mode works end-to-end for an agent whose operator already knows the tool names; discovery (US3) makes it self-serve.

---

## Phase 5: User Story 3 - Agent Discovers Available Tool Functions (Priority: P3)

**Goal**: In code mode the agent can list every script-callable function and get any function's parameters, return shape and example — from the model side (`list_tool_functions`, `describe_tool_function`) and from inside a script (`list_tools()`, `describe_tool()`), all derived from `ToolDefinition`.

**Independent Test**: With code mode on and at least one MCP server configured, ask "what can you do?" — the answer is drawn from `list_tool_functions` and includes the MCP tools; then ask for a task using one tool and verify the agent calls `describe_tool_function` and writes a correct call first time. `describe_tool_function("memory_reed")` returns `available: false` with `did_you_mean: ["memory_read", …]`.

### Implementation for User Story 3

- [ ] T028 [US3] Implement `src/sandbox/docs.rs`: `ToolFunctionDoc`/`ParamDoc` (fields per data-model.md, `Serialize + Clone`), `pub fn python_identifier(tool_name: &str) -> String` (sanitise `[^A-Za-z0-9_]`→`_`, leading digit → `_` prefix, reserved names `list_tools`/`describe_tool`/`execute_python`/Python keywords & common builtins → `_tool` suffix), `pub fn catalogue(defs: &[ToolDefinition]) -> Vec<ToolFunctionDoc>` (deterministic collision suffixes `_2`, `_3` in sorted-name order; `summary` = first sentence/line of the description; sorted by `function`), `pub fn describe(def: &ToolDefinition, output_schema: Option<&serde_json::Value>) -> ToolFunctionDoc` (walk top-level `properties`/`required` of the input schema into `ParamDoc`s with `type` from `type`/`enum`/`anyOf`; `returns` = compact rendering of `output_schema` or `"any JSON value"`; `example` = `result = <function>(<required params with placeholders by type>)`), and `pub fn did_you_mean(name: &str, known: &[String]) -> Vec<String>` (≤3 by edit distance ≤3 or prefix)
- [ ] T029 [US3] Implement `catalogue`/`describe` on `RouterBridge` in `src/agents/tools/python/bridge.rs`: `router.definitions().await` → `docs::catalogue`; `describe(function)` finds the matching `ToolDefinition` (by sanitised name) and passes `router.default_toolset/user_toolset .get_tool(name).map(|t| t.output_schema())` as the output schema (MCP tools → `None`); computed on every call, never cached (FR-012)
- [ ] T030 [US3] Handle the in-script helpers in `src/sandbox/runtime.rs` `FunctionCall` routing: `list_tools()` → `bridge.catalogue()` mapped to a `MontyObject::List` of `{"function", "summary"}` dicts; `describe_tool(name)` (one positional or `name=` kwarg, else `TypeError`) → `bridge.describe(name)` as a dict, or the `{"available": False, "name", "message", "did_you_mean"}` dict when `None`; both work regardless of `tools_enabled` but return an empty catalogue when tools are off (FR-013)
- [ ] T031 [P] [US3] Implement `ListToolFunctions` and `DescribeToolFunction` in `src/agents/tools/python/docs_tools.rs` as `VizierTool`s holding a `ToolRouter`: names `list_tool_functions` / `describe_tool_function`, input/output shapes and descriptions verbatim from contracts/tool-definitions.md (found → `ToolFunctionDoc`; not found → `{available:false, name, message, did_you_mean}` as a normal output, not an `Err`, FR-014); they share the same `docs::catalogue`/`describe` calls as the bridge (one code path)
- [ ] T032 [US3] Register the docs tools in `src/agents/tools/mod.rs`: in `VizierTools::new`, when `code_mode`, `.tool(ListToolFunctions::new(router.clone())).tool(DescribeToolFunction::new(router.clone()))` onto `sandbox_toolset`; `tools()` in `CodeModeExclusive` now returns `think`, `execute_python`, `list_tool_functions`, `describe_tool_function`; they are **not** added in `SandboxAdditive` (US3-S6)
- [ ] T033 [P] [US3] Unit tests `#[cfg(test)] mod tests` in `src/sandbox/docs.rs`: identifier sanitisation (`my-tool`→`my_tool`, `3d`→`_3d`, `print`→`print_tool`), deterministic collision suffixes, catalogue sorted and summaries truncated at the first sentence, `describe` on a schema with required+optional params yields correct `ParamDoc`s and an `example` using only required params, `returns = "any JSON value"` when no output schema, `did_you_mean("memory_reed")` includes `memory_read`
- [ ] T034 [US3] Manual verification per `specs/007-code-mode-python-sandbox/spec.md` US3 acceptance scenarios: with an MCP server configured, `list_tool_functions` lists its tools with sanitised names; `describe_tool_function` on a native tool shows its output schema; `describe_tool("nope")` inside a script returns the not-available dict without ending the run; adding a tool to the agent and asking again shows it (US3-S5)

**Checkpoint**: Code mode is self-serve — the agent can discover and correctly call any tool without operator hints.

---

## Phase 6: User Story 4 - Operator Configures the Two Switches (Priority: P4)

**Goal**: Operators flip the two switches in the WebUI or API with the `code_mode ⇒ enabled` invariant enforced server-side and cascaded client-side, and are warned what code mode does.

**Independent Test**: In the WebUI, agent A: sandbox only; agent B: sandbox + code mode; agent C: nothing — each agent's live tool list matches the exposure table. `PUT` with `{"enabled":false,"code_mode":true}` returns 400 with the documented message. Turning A's sandbox off in the form also clears code mode. Existing agents load with both off.

### Implementation for User Story 4

- [ ] T035 [US4] Validate the invariant in `src/agents/mod.rs`: add `fn validate_config(config: &AgentConfig) -> Result<(), String>` returning `"tools.python.code_mode requires tools.python.enabled"` when violated, and call it at the top of `handle_create` (~line 205) and `handle_update` (~line 260) returning `AgentCommandResult::Error(msg)` (the HTTP handlers already map that to 400 — contracts/http-api.md)
- [ ] T036 [P] [US4] WebUI types in `webui/app/interfaces/types.ts`: add `PythonSandboxConfig { enabled: boolean; code_mode: boolean }`, `DEFAULT_PYTHON_SANDBOX`, add `python: PythonSandboxConfig` to `AgentToolsConfig` (~line 352) and to the agent summary type (~line 448), and `python?: Partial<PythonSandboxConfig>` to the create/update request `tools` (~line 405) — per contracts/webui.md
- [ ] T037 [US4] WebUI form in `webui/app/components/AgentForm.tsx`: initialise `form.tools.python` from `agent.python ?? DEFAULT_PYTHON_SANDBOX` (defaults block ~line 90 and edit-load mapping ~line 166); add a **Python** section inside the Tools card (next to the `fetch`/`http_client` toggles ~line 1173) with the *Python sandbox* toggle, the *Code mode (programmatic tool calling)* toggle (disabled with tooltip while `!enabled`; setting `enabled=false` also sets `code_mode=false` in the same state update), the help text and the code-mode warning callout verbatim from contracts/webui.md; include `python` in the submit payload; extend the "no tools enabled" empty-state condition (~line 3237) with `!form.tools?.python?.enabled`
- [ ] T038 [US4] Run `cd webui && npm run typecheck` and `npm run build`, then manual verification per the US4 Independent Test in `specs/007-code-mode-python-sandbox/spec.md` (three agents, 400 on the invalid PUT, cascade in the form, an agent record saved before this feature loads with both switches off)

**Checkpoint**: Operators can fully manage the feature from the UI; invalid states are impossible to persist.

---

## Phase 7: User Story 5 - Operator and User Can See What a Script Did (Priority: P5)

**Goal**: The script source, nested invocations, stdout, result/error and duration are visible live in the WebUI and persisted in session history.

**Independent Test**: With code mode on, run a script that calls two tools and prints one line; in the WebUI see `🐍 Running Python` with the source, two nested `tool_choice` events, then a collapsible execution report showing both invocations with timings, the printed line, the result and the duration; `GET` the session history and find the report as the `execute_python` `ToolResult.content`.

### Implementation for User Story 5

- [ ] T039 [US5] Forward the execution report live in `src/agents/hook/tool_calls.rs`: `ToolCallsHook` needs to know the tool name at `on_tool_response` time — record the last `function_name` seen in `on_tool_call` (a `Mutex<Option<String>>` on the hook, or match on the response shape: a `ToolResponse` whose `response` has `ok`, `stdout`, `tool_calls`, `duration_ms`) and `send_async` the `VizierResponse` unchanged to `response_tx` **only** for `execute_python`; all other tools keep today's behaviour (contracts/http-api.md *WebSocket*)
- [ ] T040 [P] [US5] WebUI types in `webui/app/interfaces/types.ts`: add `ToolInvocationRecord`, `ExecutionError`, `ExecutionReport` (no `truncated`) and the `{ tool_response: { response: unknown } }` variant to `VizierResponseContent` — per contracts/webui.md
- [ ] T041 [P] [US5] Create `webui/app/components/ExecutionReportView.tsx`: collapsible block (collapsed when `ok`, expanded when `!ok`) headed `✅ Python finished in {s}s` / `❌ Python failed ({limit ?? kind}) in {s}s`, sections in order **Tool calls** (`seq. name(args) ✓/✗ {ms}ms`, hidden when empty), **Output** (`pre`, scroll-boxed), **Result** (pretty JSON, scroll-boxed), **Error** (`message` + `traceback` in a `pre`, only when present); use the repo's existing highlight.js setup for the JSON block
- [ ] T042 [US5] Wire `webui/app/routes/chat.tsx`: extend `InlineEvent['type']` (~line 54) with `'execution'` and an optional `report?: ExecutionReport`; in `formatToolChoice` (~line 99) add the `execute_python` (fenced `python` block of `args.code`), `list_tool_functions`, `describe_tool_function` cases from contracts/webui.md; in the WebSocket handler (~line 630) add `if ('tool_response' in content)` → duck-type parse as `ExecutionReport` → `addInlineEvent('execution', …)` with the report, else ignore; render `'execution'` events with `ExecutionReportView`
- [ ] T043 [US5] Run `cd webui && npm run typecheck && npm run build`; manual verification per the US5 Independent Test in `specs/007-code-mode-python-sandbox/spec.md` (live events in order, report contents, `ToolResult.content` in the session history API)

**Checkpoint**: Every script run is auditable live and after the fact.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Gates the constitution requires, docs, and a final consistency pass.

- [ ] T044 [P] Cross-compilation gate: `cross build --release --target x86_64-unknown-linux-musl` and `cross build --release --target aarch64-unknown-linux-gnu` must pass with **no** `Cross.toml` change (monty is pure Rust — research Decision 1); if either fails, fix it in this feature, not as a follow-up
- [ ] T045 [P] Update `CLAUDE.md`: in the *Tools* section note the `ToolRouter`/`ToolExposure` split, the `python/` tool module and `src/sandbox/`, and that `execute_python` + docs tools are never in `DREAM_TOOL_NAMES`; in *Config layering* mention `tools.python.{enabled,code_mode}`
- [ ] T046 [P] Update user-facing docs (`README.md` tools/agent-settings section, and `docs/` if the feature list lives there) with a short "Python sandbox & code mode" entry: the two switches, what each does, that the agent's tool timeout is the only bound, and the deferred items (memory ceiling, tool-call cap, output truncation)
- [ ] T047 Full gate run at the repo root (`Cargo.toml` crate + `webui/`): `cargo clippy` (zero warnings in new modules), `cargo test`, `cd webui && npm run typecheck`, `just run` smoke test with an agent in each exposure mode; confirm `dream` cycle tool list is unchanged for an agent with code mode on (FR-007)
- [ ] T048 Walk quickstart.md end to end (build, both *Try it* flows, the limits table statements, every row of the safety table including two concurrent `while True: pass` sessions) and fix any drift between the docs and the shipped behaviour

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies; T003/T004 parallel with T001→T002
- **Foundational (Phase 2)**: depends on Phase 1 — **blocks all stories**. T005/T006/T007 parallel; T008 → T009 → (T010, T011 parallel)
- **US1 (Phase 3)**: depends on Phase 2. T012 → T013 → T014; T015/T017 parallel with T012; T016 after T014; T018 after T017; T019/T020 after T012/T013; T021 last
- **US2 (Phase 4)**: depends on US1 (extends `runtime.rs`, `ExecutePython`, exposure). T022 → T023 → T024 → T025; T026 after T023; T027 last
- **US3 (Phase 5)**: depends on US2 (bridge + code-mode exposure). T028 → T029 → T030; T031 parallel with T029/T030; T032 after T031; T033 after T028; T034 last
- **US4 (Phase 6)**: depends on Phase 2 only (config + API plumbing); can run in parallel with US1–US3. T035 ∥ T036 → T037 → T038
- **US5 (Phase 7)**: depends on US2 (nested invocations to show). T039 ∥ T040 ∥ T041 → T042 → T043
- **Polish (Phase 8)**: after all desired stories

### Within-file serialisation (avoid conflicts)

`src/agents/tools/mod.rs` is touched by T008, T009, T010, T016, T025, T032 — do these strictly in ID order. `src/sandbox/runtime.rs`: T012 → T013 → T023 → T030. `src/agents/tools/python/mod.rs`: T014 → T015 → T024. `webui/app/interfaces/types.ts`: T036 then T040. `webui/app/routes/chat.tsx`: T042 only.

### Parallel Opportunities

- Phase 1: T003, T004 alongside T001/T002
- Phase 2: T005, T006, T007 together; then T010 and T011 together after T009
- US1: T015, T017, T019, T020 alongside the runtime/tool work
- US4 can be built by a second person while US1–US3 are in progress (only Phase 2 needed)
- US5: T040, T041 while T039 is done

---

## Parallel Example: User Story 1

```bash
# After Phase 2, launch in parallel:
Task: "T015 execute_python description templates in src/agents/tools/python/mod.rs"
Task: "T017 sandbox_md briefing in src/agents/agent/system_prompt/sandbox.rs"
Task: "T012 sandbox::execute loop in src/sandbox/runtime.rs"

# Once T012/T013 land:
Task: "T019 convert.rs unit tests"
Task: "T020 runtime.rs unit tests (NoToolsBridge)"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 → Phase 2 (T001–T011): dependency, config field, `ToolRouter` refactor with zero behaviour change, API plumbing.
2. Phase 3 (T012–T021): pure sandbox. **Stop and validate**: enable on one agent, run the quickstart safety table. This alone is a useful, shippable feature (exact computation without a shell).

### Incremental Delivery

3. US2 → code mode for operators who already know tool names (validate with the Rust-releases prompt).
4. US3 → discovery; code mode becomes self-serve.
5. US4 → WebUI switches + server-side invariant (can be done any time after Phase 2).
6. US5 → live report + history visibility.
7. Phase 8 gates: cross build, clippy/test/typecheck, docs, quickstart walk.

### Notes

- `monty` is pre-1.0: keep the exact pin from T001; bumping it is its own change with the probe from research.md re-run.
- Nothing in this feature adds settings beyond `enabled`/`code_mode`, a global allocator, or a memory ceiling — those were explicit user decisions (spec Assumptions, research Decision 6); do not reintroduce them "while you're there".
- Every failure a script can cause must come back as an `ExecutionReport` (data), never as `Err` from `sandbox::execute` — the model has to read it to self-correct.
