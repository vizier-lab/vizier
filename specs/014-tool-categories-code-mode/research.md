# Research: Code Mode by Default, Tool Categories, Simpler MCP Tool Names

**Feature**: `014-tool-categories-code-mode` | **Date**: 2026-10-14

The Technical Context had no open unknowns. Every decision below resolves a design question that came up while reading the current code: `src/agents/tools/mod.rs`, `src/agents/tools/python/`, `src/sandbox/`, `src/agents/mcp/mod.rs`, `src/agents/agent/{mod.rs,system_prompt/sandbox.rs}`, `src/utils/mod.rs`, `webui/app/routes/chat.tsx` and `webui/app/components/{ExecutionReportView,PythonSandboxSection}.tsx`.

---

## R1. Removing the Python switches: no migration needed

**Decision**: Delete `PythonSandboxConfig`, `AgentToolsConfig.python`, the HTTP DTO fields (`src/channels/http/api/v1/agents/mod.rs:162,375`), the validation at `src/agents/mod.rs:436`, the `ToolExposure` enum and `ToolExposure::from_config`. Add no migration.

**Rationale**: No struct in the crate uses `#[serde(deny_unknown_fields)]` (checked with `grep`), so an agent stored with `tools.python: {...}` deserialises with the key silently dropped (FR-003). The same goes for old clients that send `python` in a create or update body (FR-004). The next time the agent is saved, the key disappears from storage.

**Alternatives considered**:
- *Keep the struct and only change its defaults*: stored agents have `code_mode: false` written out explicitly, so they would never change. The user rejected this.
- *Keep the field as a deprecated, ignored `Option`*: this adds code with no use (Principle I). Serde already ignores the field.

---

## R2. Which tools stay direct: move `think` out of the router

**Decision**: The tools the model calls directly are exactly `sandbox_toolset`: `think`, `execute_python`, `list_tool_functions` and `describe_tool_function`. `ThinkTool` moves from `default_toolset` into `sandbox_toolset`. `VizierTools::tools()` returns only `sandbox_toolset`'s definitions, and `VizierTools::call()` serves only `sandbox_toolset`. Any other name is refused with the existing "call it from a script via execute_python" error. `CODE_MODE_DIRECT_TOOLS` and `exposed_directly` are deleted.

**Rationale**: Today `think` sits in the router, so it appears in the script catalogue. It would be the only catalogue entry without a category, which breaks FR-005. A script has no use for `think` anyway, since nested calls are not streamed (`hook/thinking.rs:48`). After the move:
- the router holds exactly the categorised, script-callable tools;
- the direct set is one toolset;
- the code-mode gating disappears instead of being kept as a special case.

The dream cycle is unaffected. `DREAM_TOOL_NAMES` never included `think`, and `dream_call` still goes through `router.call`.

**Alternatives considered**:
- *Keep `think` in the router with category `None`*: this breaks FR-005, and every listing would need a special case.
- *Keep `ToolExposure` with one variant*: dead abstraction (Principle I).

---

## R3. Where a built-in tool's category is declared

**Decision**: Add a required method to `VizierTool`, `fn category() -> Option<ToolCategory>`, with no default. It is forwarded through `VizierToolDyn::category(&self)`. Only the four direct tools and the two dream-journal tools return `None`. `ToolCategory` is a plain enum with 13 variants, in the spec's FR-006 order, with `name() -> &'static str` and `description() -> &'static str`.

**Rationale**:
- A required method means a new tool that doesn't declare a category won't compile. It keeps the category next to the tool's name and description, so nothing goes out of date.
- An enum means category names can't be misspelled. Its declaration order is the stable display order (FR-012), which also keeps the system-prompt prefix cacheable.
- A unit test asserts that every tool in the router has `Some` category, which covers FR-005 and FR-006.
- This is one method on an existing trait, not a new abstraction (Principles I and II).

**Alternatives considered**:
- *A central `name → category` table*: it silently drifts when a tool is added or renamed. This is the reason given to the user.
- *Declaring the category at registration (`.tool(ToolCategory::Memory, X)`)*: it is checked at compile time too, but it separates the category from the tool, and memory tools are registered inside `init_vector_memory` rather than in `VizierTools::new`.
- *A required `fn category() -> ToolCategory` with an extra "direct" variant*: this puts a category that isn't really a category into the type.

---

## R4. One catalogue that names, categorises and dispatches

**Decision**: `ToolRouter` gets one method, `catalogue()`, that builds `Vec<CatalogueEntry>`. `CatalogueEntry` holds the script name, the category label, the target (native tool or MCP `(server, tool)`) and the `ToolDefinition`. Names are assigned in this order:

1. **Native tools first**: each name is `python_identifier(tool_name)`. Native names are unique and are reserved before any MCP name.
2. **MCP tools next**, sorted by `(server, tool)`. Each gets `python_identifier(format!("{server}_{tool}"))`. If that name is already taken, `_2`, `_3`, … is appended until it is free.

`ToolRouter::call` resolves names in this order:
1. native toolsets;
2. a catalogue lookup.

`RouterBridge` keeps snapshotting the catalogue once per script run. `ToolRouter::definitions()` is replaced by `catalogue()`.

**Rationale**:
- Today `sandbox::docs::named` resolves collisions by sorted original name. With `<server>_<tool>` names, a native tool could therefore lose its name to an MCP tool that sorts first. FR-014 requires native tools to keep their names, so priority has to be decided where both native and MCP tools are known, which is the agent layer.
- Without the `mcp_` prefix, dispatch can no longer tell an MCP tool apart by its name, so it has to use the same table that named the tools. Using one table for naming, listing and dispatch means they can't disagree.
- The result depends only on the current set of tools, not on the order servers connect in (FR-014).

`sandbox::docs` stays agent-agnostic. It keeps `python_identifier` and `describe_as`. `catalogue`, `named` and `find` move out of it, or get a version that takes the names already resolved by the router.

**Alternatives considered**:
- *Keep the naming in `sandbox::docs` and pass it a priority flag*: this leaks agent concepts (native vs MCP) into the engine.
- *Always suffix MCP tools whose name matches a native tool's*: this is the same thing as the chosen approach.

---

## R5. MCP categories, and an MCP server named like a built-in category

**Decision**: An MCP tool's category label is its server's config key. If that key equals a built-in category name (such as `memory` or `discord`), the label becomes `<server>_mcp`. The description is always "Tools from the `<server>` MCP server".

**Rationale**:
- Category names must be unique for `list_tool_functions(category=…)` to mean one thing.
- Server keys are already unique per agent, so the built-in names are the only possible clash. A readable suffix tells the reader the source, where a numeric suffix wouldn't.
- The server's self-description (MCP `initialize` → `instructions` / `serverInfo`) is not used. It only becomes known after the server connects, so the system prompt would change between the first turn and later ones and after every restart, which breaks FR-010b. The spec's FR-010 has been updated to match.

**Alternatives considered**:
- *Merge the server's tools into the built-in category*: forbidden by the spec's edge case.
- *An operator-set description in the MCP config*: this adds config surface nobody has asked for yet (Principle I). It can be added later as an optional field.

---

## R6. Old names are not supported

**Decision**: `mcp_<server>__<tool>`, `READ_CORE` and `WRITE_CORE` resolve to nothing. They are not in the catalogue, so a script calling one gets `NameError`, and a direct model call gets the usual "not called directly" refusal. No alias table or resolution code is written.

**Rationale**: This is the user's decision. It also removes the only code whose sole purpose is backward compatibility. Recovery is the normal discovery path: the agent lists the category and finds the current name.

**Alternatives considered**:
- *An alias function consulted by `ToolRouter::call` and `RouterBridge::resolve`*: this was the earlier design. It was dropped on request.

---

## R7. Rendering tool calls (FR-016)

**Decision**: Delete both `mcp_` prefix branches:
- `src/utils/mod.rs:115,264` (Discord/Telegram, through `format_thinking`)
- `webui/app/routes/chat.tsx:251`

Rename the `READ_CORE`/`WRITE_CORE` label arms to `read_core`/`write_core`. Calls already in history under old names fall through to the generic display ("🔧 Using <name>" plus arguments), which still shows the full name.

**Rationale**: With code mode always on, MCP tools are reachable only from scripts. Script calls are not streamed as `ToolChoice` (`hook/tool_calls.rs:50`), and by design the WebUI's `ExecutionReportView` doesn't list them. So no view displays a new MCP call by name, and nothing ever has to split a `<server>_<tool>` name. With old names unsupported (R6), the prefix branches would only ever decorate history, and the generic fallback already shows the full name. The spec's FR-016 and US3 have been updated to match. Adding a category to `ToolInvocationRecord` would give data to a view that deliberately shows none (Principle I).

**Alternatives considered**:
- *Add `category` to `ToolInvocationRecord`*: there is no consumer for it today.

---

## R8. System prompt: category list and discovery instructions

**Decision**: `system_prompt/sandbox.rs::sandbox_md` keeps only its code-mode branch. Its signature becomes `sandbox_md(categories: &[CategorySummary], tools_timeout: &str) -> String`, and it renders:
- the discovery steps (FR-010a);
- one `- name: description` line per category (FR-010).

It is always inserted at index 1 of `prepare_system_prompts`, which is where it sits today. `VizierTools::new` computes `categories` once and stores them on `VizierTools`:
- built-in categories with at least one registered tool, in enum order;
- then MCP categories, sorted by label.

**Rationale**:
- `VizierTools` is rebuilt whenever the agent's configuration changes (an update restarts the agent), so the list is fixed for the agent instance's lifetime. FR-010b is met by construction.
- MCP categories come from configured server keys, not live listings, so a server that is down stays in the list.
- Function counts are left out because they would vary with live listings.
- The text sits before CORE, as today, so the cacheable prefix stays byte-identical across turns.

**Alternatives considered**:
- *Putting the list in the per-request `context_md`*: it is stable, so it belongs in the cached system prefix, not in the per-turn context.

---

## R9. Listing: one tool, optional `category` (FR-008, FR-009)

**Decision**: `list_tool_functions` takes `{ category?: string }` and returns:

- **Without a category**: `{ count, functions: [{ name, summary, category }] }`, covering every function, sorted by category order and then name.
- **With a known category**: `{ category, count, functions: [{ name, summary }] }`. When an MCP category's server has no tools right now, it returns `count: 0` with a note that the server may be unreachable (edge case).
- **With an unknown category**: `{ available: false, category, message, did_you_mean }`. This reuses `docs::did_you_mean` over the category labels.

In a script, `list_tools(category=None)` returns the same `functions` list, with each entry carrying `category`. An unknown category raises `ValueError("no category '<x>'; did you mean …")`, which the script can catch. That is the Python equivalent of the direct tool's non-fatal answer, and it keeps `list_tools()` returning a list, as documented in spec 007.

**Rationale**:
- This is the user's choice: no wildcard, and leaving the category out means everything.
- In a script, an exception is the natural "not found" signal. The direct tool can't raise, so it returns data, as `describe_tool_function`'s `not_found` already does.

**Alternatives considered**:
- *Making `list_tools()` return the direct tool's dict*: this needlessly breaks every script that iterates the list.
- *A separate `list_categories` tool*: the system prompt already carries the categories.

---

## R10. End-to-end testing without network access

**Decision**: The quickstart ships a stdio MCP server of about 60 lines, `specs/014-tool-categories-code-mode/fixtures/echo_mcp.py`, written in standard-library Python. It answers `initialize`, `tools/list` and `tools/call`, and its tool names and server key come from command-line arguments. The dummyplug agents use it to cover:
- a plain server (`echo`);
- a server whose name clashes with a built-in category, and whose tool name clashes with a built-in tool (`memory` with tool `search` → `memory_search` clash);
- a two-server clash (`a_b`/`c` vs `a`/`b_c`).

The dummyplug protocol itself does not change. Its `tools` reply already shows exactly the four direct tools, and every other check goes through `list_tool_functions` and `execute_python` JSON requests (FR-017).

**Rationale**: The constitution requires dummyplug e2e checks with no network. `npx`-based reference servers need a download. `python3` is only needed to run the quickstart, not by vizier, so Principle III isn't affected.

**Alternatives considered**:
- *A Rust test MCP server inside the crate*: it would add test-only code and dependencies to the binary's tree for a manual check.
