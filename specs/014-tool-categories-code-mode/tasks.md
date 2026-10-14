---
description: "Task list for 014: code mode by default, tool categories, simpler MCP tool names"
---

# Tasks: Code Mode by Default, Tool Categories, Simpler MCP Tool Names

**Input**: Design documents from `specs/014-tool-categories-code-mode/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R10), data-model.md, contracts/ (system-prompt.md, tool-definitions.md, http-api-webui.md), quickstart.md

**Tests**: The spec asks for no TDD. The unit tests below are the ones plan.md lists under Testing (category coverage, catalogue naming, unsupported old names, listing by category, stable `sandbox_md`). Each sits in the story whose behaviour it checks. End-to-end checks are the quickstart's dummyplug steps (constitution gate).

**Organization**: Tasks are grouped by user story, so each story can be built and tested on its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, and no dependency on an unfinished task)
- **[Story]**: US1–US4 from spec.md
- Paths are relative to the repository root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm the starting state. No new crates, modules or config are needed (plan: "No new crates").

- [ ] T001 Confirm the tree builds and passes before any change: run `cargo build`, `cargo test`, `cargo clippy` and `cd webui && npm run typecheck` on branch `014-tool-categories-code-mode`, and note any pre-existing failures so they aren't blamed on this feature

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Categories on every tool, and one router catalogue that names, categorises and dispatches. Every story depends on these. Behaviour stays the same until the story phases: names, including `mcp_<server>__<tool>`, are unchanged here.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T002 Add `pub enum ToolCategory` to `src/agents/tools/mod.rs`. It has the 13 variants `Memory, Identity, Scheduling, Agents, Skills, Files, Web, Audio, Media, Discord, Telegram, Webui, Shell`, in that declaration order, with `#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]`. Add `pub fn name(self) -> &'static str` and `pub fn description(self) -> &'static str`, returning exactly the names and descriptions in the data-model.md ToolCategory table, and `pub const ALL: [ToolCategory; 13]` in declaration order
- [ ] T003 In `src/agents/tools/mod.rs`, add the required method `fn category() -> Option<ToolCategory>;` (no default) to `trait VizierTool`. Add `fn category(&self) -> Option<ToolCategory>` to `trait VizierToolDyn`, and forward it in the blanket `impl<Tool: VizierTool + …> VizierToolDyn for Tool` as `Self::category()` (depends on T002)
- [ ] T004 [P] Implement `fn category() -> Option<ToolCategory> { Some(ToolCategory::Memory) }` on `MemoryList`, `MemorySearch`, `MemoryWrite`, `MemoryRead`, `MemoryFollow`, `MemoryGraphTool`, `MemoryDelete` and `MemoryDeleteBundle` in `src/agents/tools/vector_memory/mod.rs` (depends on T003)
- [ ] T005 [P] Implement `category()` → `Some(ToolCategory::Identity)` on `ReadCore` and `WriteCore` in `src/agents/tools/workspace/mod.rs` (depends on T003)
- [ ] T006 [P] Implement `category()` → `Some(ToolCategory::Scheduling)` on `ScheduleOneTimeTask`, `ScheduleCronTask`, `ListTask`, `DeleteTask`, `GetTaskDetail`, `ListTaskRuns` and `GetTaskRunDetail` in `src/agents/tools/scheduler/mod.rs` (depends on T003)
- [ ] T007 [P] Implement `category()` → `Some(ToolCategory::Agents)` on `ConsultAgent` and `DelegateAgent` in `src/agents/tools/consult/mod.rs`, `SubtasksTool` in `src/agents/tools/subtasks/mod.rs`, and `ListBackgroundJobs` and `CancelBackgroundJob` in `src/agents/tools/background_jobs.rs` (depends on T003)
- [ ] T008 [P] Implement `category()` → `Some(ToolCategory::Skills)` on `CreateSkill` (`src/agents/tools/skill/mod.rs`), `UpdateSkill` (`skill/update_skill.rs`), `DeleteSkill` (`skill/delete_skill.rs`), `ListSkills` (`skill/list_skills.rs`), `GetSkillDetails` (`skill/get_skill_details.rs`), `UseSkill` (`skill/use_skill.rs`), `ReadSkillResource` (`skill/read_skill_resource.rs`) and `ExecuteSkillResource` (`skill/execute_skill_resource.rs`), all under `src/agents/tools/` (depends on T003)
- [ ] T009 [P] Implement `category()` → `Some(ToolCategory::Files)` on `ListSessionFiles` and `ReadDocumentFile` in `src/agents/tools/session_files/mod.rs`, `SendAttachment` in `src/agents/tools/session_files/send_attachment.rs`, and `ReadImageFile` in `src/agents/tools/read_image/mod.rs` (depends on T003)
- [ ] T010 [P] Implement `category()` → `Some(ToolCategory::Web)` on `BraveSearch<T>` in `src/agents/tools/brave_search/mod.rs` (covers `web_search` and `news_search`), `FetchWebpage` in `src/agents/tools/fetch/mod.rs`, and `HttpClient` in `src/agents/tools/http_client/mod.rs` (depends on T003)
- [ ] T011 [P] Implement `category()` → `Some(ToolCategory::Audio)` on `TtsGenerate` in `src/agents/tools/tts/mod.rs` and `SttTranscribe` in `src/agents/tools/stt/mod.rs`, and → `Some(ToolCategory::Media)` on `ImageGenerate` in `src/agents/tools/image_gen/mod.rs` (depends on T003)
- [ ] T012 [P] Implement `category()` → `Some(ToolCategory::Discord)` on the six tools in `src/agents/tools/discord/mod.rs`, → `Some(ToolCategory::Telegram)` on the three tools in `src/agents/tools/telegram/mod.rs`, and → `Some(ToolCategory::Webui)` on `SendWebuiMessage` and `ListWebuiTopics` in `src/agents/tools/webui/mod.rs` (depends on T003)
- [ ] T013 [P] Implement `category()` → `Some(ToolCategory::Shell)` on `ShellExec` in `src/agents/tools/shell/mod.rs`, and → `None` on `ThinkTool` (`src/agents/tools/think/mod.rs`), `ExecutePython` (`src/agents/tools/python/mod.rs`), `ListToolFunctions` and `DescribeToolFunction` (`src/agents/tools/python/docs_tools.rs`), and `WriteDreamJournal` and `ReadDreamJournal` (`src/agents/tools/dream_journal/mod.rs`) (depends on T003)
- [ ] T014 Add the catalogue types to `src/agents/tools/mod.rs`:
  - `pub enum ToolTarget { Native(String), Mcp { server: String, tool: String } }`
  - `pub struct CatalogueEntry { pub name: String, pub category: String, pub target: ToolTarget, pub definition: ToolDefinition }`
  - `pub struct CategorySummary { pub name: String, pub description: String }`

  Then add `ToolRouter::catalogue(&self) -> Vec<CatalogueEntry>`:
  - Native entries come from `default_toolset` and `user_toolset`, named `sandbox::docs::python_identifier(tool_name)`, with category `tool.category().map(ToolCategory::name)`.
  - MCP entries keep, for now, the current name `python_identifier(format!("mcp_{server}__{tool}"))`, with category = the server key. The final naming rule comes in T034.
  - Native names are reserved first. MCP entries follow, sorted by `(server, tool)`, and a taken name gets `_2`, `_3`, … appended.
  - The result is sorted by (category rank, name), where rank is `ToolCategory` order for built-ins and MCP categories come after the built-ins in name order.
  - MCP servers are listed concurrently with `join_all`, as in today's `definitions()`.

  (Depends on T004–T013; R4.)
- [ ] T015 In `src/agents/tools/mod.rs`, make `ToolRouter::call` resolve through the catalogue. A name found in `default_toolset`/`user_toolset` is called natively, as today. Otherwise, look the name up in `self.catalogue().await`: `ToolTarget::Mcp { server, tool }` dispatches to `self.mcp[server].call(tool, args)`. Delete the `starts_with("mcp_")` / `split_once("__")` parsing. Unknown names keep the existing `"{name} not found"` error (depends on T014)
- [ ] T016 Move collision naming out of the engine, in `src/sandbox/docs.rs`:
  - Add `pub category: Option<String>` to `ToolFunctionDoc`, with `#[serde(skip_serializing_if = "Option::is_none")]`.
  - Delete `named`, `catalogue` and `find`, plus their tests that depend on collision ordering.
  - Keep `python_identifier`, `describe_as` (now `pub`, taking the function name, definition, output schema and category), `not_found` and `did_you_mean`.

  (Depends on T014.)
- [ ] T017 Rewire `src/agents/tools/python/bridge.rs`:
  - `RouterBridge` snapshots `router.catalogue()` once per run, in a `OnceCell<Vec<CatalogueEntry>>`, in place of `definitions()` plus `docs::catalogue`.
  - `catalogue()` maps each entry to a `ToolFunctionDoc`, with `category: Some(entry.category)`.
  - `describe()` finds the entry by `name`.
  - `resolve()` returns the entry's `name`, which is the name `ToolRouter::call` accepts.

  (Depends on T015, T016.)
- [ ] T018 Rewire `ListToolFunctions` and `DescribeToolFunction` in `src/agents/tools/python/docs_tools.rs` to use `router.catalogue()` in place of `router.definitions()` plus `docs::catalogue`. Keep the output shapes unchanged for now, plus `category` on the describe output. Then delete `ToolRouter::definitions()` from `src/agents/tools/mod.rs` once nothing uses it (`VizierTools::tools()` still calls it in Direct mode until US1, so build `tools()`'s Direct branch from `catalogue()` definitions meanwhile) (depends on T016)
- [ ] T019 Run `cargo build`, `cargo test` and `cargo clippy`. A dummyplug agent with code mode on must still list and call tools exactly as before (spot-check with spec 007's quickstart step "list_tool_functions")

**Checkpoint**: Every tool carries a category, and one catalogue drives naming, listing and dispatch. Behaviour is unchanged.

---

## Phase 3: User Story 1 - Every agent works in code mode (Priority: P1) 🎯 MVP

**Goal**: Remove the Python sandbox and code-mode settings. Every agent, new or old, is offered only `think`, `execute_python`, `list_tool_functions` and `describe_tool_function`, and reaches every other tool from scripts.

**Independent Test**: quickstart §1. `tools` on `dp-old` (created on `master` with `code_mode: false`) and on `dp-new` both list exactly the 4 direct tools. A script `memory_list(limit=1)` succeeds. `GET /api/v1/agents/dp-new` has no `tools.python`. The WebUI shows no sandbox switches.

### Implementation for User Story 1

- [ ] T020 [US1] In `src/agents/tools/mod.rs`:
  - Delete `enum ToolExposure` and its `from_config`, `const CODE_MODE_DIRECT_TOOLS`, `fn exposed_directly`, the `exposure` field on `VizierTools`, and the `code_mode_exposes_only_the_sandbox_tools_directly` test.
  - In `VizierTools::new`, move `.tool(ThinkTool)` out of `default_toolset` into `sandbox_toolset`. Always build `sandbox_toolset` with `ExecutePython` (with `SandboxLimits { timeout, tools_enabled: true }`), `ListToolFunctions` and `DescribeToolFunction`.
  - `VizierTools::tools()` returns only `sandbox_toolset`'s definitions, sorted by name.
  - `VizierTools::call()` serves only `sandbox_toolset`. Any other name returns `VizierError("{name} is not called directly; call it from an execute_python script (see list_tool_functions)")`, per contracts/tool-definitions.md §1. `dream_tools`/`dream_call` are unchanged.

  (R2.)
- [ ] T021 [US1] In `src/agents/tools/python/mod.rs`, drop the `code_mode: bool` parameter of `execute_python_description` and delete its "tools are NOT callable" branch. Keep the callable-tools text, and reword its pointer to say "`list_tool_functions(category=…)`" lists a category's functions. Update `ExecutePython::new` to match
- [ ] T022 [US1] In `src/agents/agent/system_prompt/sandbox.rs`, replace `sandbox_md(exposure, tools_timeout) -> Option<String>` with `sandbox_md(tools_timeout: &str) -> String`, returning the contracts/system-prompt.md text with its `## Tool categories` block left out for now (T030 adds it). In `src/agents/agent/mod.rs::prepare_system_prompts`, always `res.insert(1, Message::system(sandbox_md(...)))`, and remove the `ToolExposure` import
- [ ] T023 [P] [US1] In `src/schema/agent.rs`, delete `PythonSandboxConfig`, the `python` field of `AgentToolsConfig`, and the tests asserting its defaults (around lines 424–427). Add a test showing that an `AgentToolsConfig` JSON containing `"python": {"enabled": false, "code_mode": false}` still deserialises (FR-003, R1). Remove the `PythonSandboxConfig` re-export from `src/schema/mod.rs`
- [ ] T024 [P] [US1] In `src/agents/mod.rs`, delete the `tools.python.code_mode requires tools.python.enabled` validation (around line 436) and any helper that only it used
- [ ] T025 [P] [US1] In `src/channels/http/api/v1/agents/mod.rs`, remove the `python` field from the response DTO (around line 162) and the request DTO (around line 375), the mappings at around lines 234 and 456, and `PythonSandboxConfig` from the imports and the `utoipa` schema list. A body that still sends `tools.python` must be accepted and ignored: confirm there's no `deny_unknown_fields` (FR-004)
- [ ] T026 [P] [US1] In the WebUI:
  - delete `webui/app/components/PythonSandboxSection.tsx`;
  - remove its import, the `python` form state and the `<PythonSandboxSection …>` block from `webui/app/routes/agent-settings.tsx` (around lines 24, 134, 218, 1328);
  - remove the `python` defaults (around lines 105, 182, 1206) and the "Python (code mode)" badge (around line 3221) from `webui/app/components/AgentForm.tsx`;
  - remove `PythonSandboxConfig`, `DEFAULT_PYTHON_SANDBOX` and `tools.python` from `webui/app/interfaces/types.ts` (around lines 380–386).

  Run `cd webui && npm run typecheck`.
- [ ] T027 [US1] Run `cargo build`, `cargo test` and `cargo clippy`, then quickstart §0–§1 on dummyplug agents `dp-old` and `dp-new`

**Checkpoint**: Every agent is in code mode. The MVP is shippable on its own: discovery still works through the existing flat listing.

---

## Phase 4: User Story 2 - Agent discovers tools by category (Priority: P1)

**Goal**: The system prompt lists the agent's categories and discovery steps. `list_tool_functions` and the in-script `list_tools` take an optional `category`.

**Independent Test**: quickstart §2 and §4. `list_tool_functions {"category": "scheduling"}` returns the 7 scheduling functions. Leaving the category out returns every function with its category. `"schedule"` gives `did_you_mean: ["scheduling"]`. In a script, `list_tools(category="identity")` works and an unknown category raises a catchable `ValueError`. The prompt's category block is byte-stable across turns.

### Implementation for User Story 2

- [ ] T028 [US2] In `src/agents/tools/mod.rs`:
  - add `pub categories: Vec<CategorySummary>` to `VizierTools`;
  - add `fn category_summaries(router: &ToolRouter) -> Vec<CategorySummary>`, which returns the built-in categories in `ToolCategory::ALL` order that have at least one tool in `default_toolset`/`user_toolset`, followed by one entry per `router.mcp` key sorted by name, described as "Tools from the `<key>` MCP server". It reads config only, never live MCP listings (FR-010b, FR-011);
  - fill `categories` in `VizierTools::new`;
  - add a unit test that the result is identical across two calls and lists no MCP server twice.
- [ ] T029 [US2] In `src/agents/tools/python/docs_tools.rs`:
  - Change `ListToolFunctionsInput` to `{ #[serde(default)] pub category: Option<String> }`, with a `schemars` description.
  - Change `ListToolFunctions::call` to return the shapes in contracts/tool-definitions.md §2:
    - with no category: `{count, functions: [{name, summary, category}]}`;
    - with a known category: `{category, count, functions: [{name, summary}]}`, plus the "may be unreachable" `message` when a configured MCP category has no entries;
    - with an unknown category: `{available: false, category, message, did_you_mean}`, using `docs::did_you_mean` over `VizierTools`' category names.

  `ListToolFunctions` needs the category names, so pass `Vec<CategorySummary>` (or the names) into `ListToolFunctions::new`. Make `Output` a `serde_json::Value`, as `DescribeToolFunction` does. Update the tool description to mention `category`. (Depends on T028.)
- [ ] T030 [US2] In `src/agents/agent/system_prompt/sandbox.rs`, change the signature to `sandbox_md(categories: &[CategorySummary], tools_timeout: &str) -> String`, and render the full contracts/system-prompt.md text, with one `- {name}: {description}` line per category. Update the call in `src/agents/agent/mod.rs::prepare_system_prompts` to pass `&self.tools.categories`. Add a unit test that renders a fixed category list and checks the order and the absence of counts (depends on T028)
- [ ] T031 [P] [US2] In `src/sandbox/runtime.rs`, change `list_tools` so it accepts an optional `category` keyword (or a single positional argument):
  - no argument returns every catalogue doc as a list of `{name, summary, category}` dicts;
  - a known category returns only that category's entries;
  - a category no doc carries raises `ValueError("no category '<x>'; did you mean: …")`, using `docs::did_you_mean` over the distinct categories.

  Keep `list_tool_functions` as an alias. Add runtime tests using the existing `FakeBridge`, with `category` set on its docs: filtered, unfiltered, and unknown raises a catchable `ValueError`. (Depends on T016; contracts/tool-definitions.md §4.)
- [ ] T032 [US2] Run `cargo test` and `cargo clippy`, then quickstart §2 on a dummyplug agent (use `dp-new`; the MCP rows wait for US3), and §4 via `RUST_LOG=vizier=debug`

**Checkpoint**: Discovery by category works directly and in scripts, and the categories are in the system prompt.

---

## Phase 5: User Story 3 - MCP tools are tagged by server and get simpler names (Priority: P2)

**Goal**: MCP tools are named `<server>_<tool>`. A server whose key clashes with a built-in category gets the category `<key>_mcp`. Collisions get predictable suffixes, old `mcp_` names don't work, and no renderer parses MCP names.

**Independent Test**: quickstart §3 on `dp-mcp`, with the `fixtures/echo_mcp.py` servers `github`, `memory`, `a` and `a_b`:
- `github_create_issue(text="hi")` returns `"create_issue: hi"`;
- `memory_search_2` reaches the MCP server, and the `memory_mcp` category holds it;
- `a_b_c` / `a_b_c_2` reach `a`/`b_c` and `a_b`/`c`;
- `mcp_github__create_issue(...)` raises `NameError`;
- a server that is down still appears and lists `count: 0` with the message.

### Implementation for User Story 3

- [ ] T033 [US3] In `src/agents/tools/mod.rs`, add `fn mcp_category_name(server: &str) -> String`, which returns `format!("{server}_mcp")` when `server` equals any `ToolCategory::ALL[i].name()`, and otherwise `server`. Use it in both `ToolRouter::catalogue` (an MCP entry's `category`) and `category_summaries` (an MCP summary's `name`; the description still names the plain server key) (R5)
- [ ] T034 [US3] In `ToolRouter::catalogue` in `src/agents/tools/mod.rs`, change the MCP entry name to `python_identifier(&format!("{server}_{tool}"))`, keeping the native-first reservation and the `(server, tool)`-ordered `_2`, `_3` suffixes from T014. Add unit tests on a pure naming helper, e.g. `fn assign_names(native: &[&str], mcp: &[(&str, &str)]) -> Vec<String>`, extracted from `catalogue` so it can be tested without live servers:
  - `github`/`create_issue` → `github_create_issue`;
  - native `memory_search` plus `memory`/`search` → `memory_search` and `memory_search_2`;
  - `a`/`b_c` and `a_b`/`c` → `a_b_c` and `a_b_c_2`;
  - `my-srv`/`do-thing` → `my_srv_do_thing`;
  - the same names come out whatever order the input is in;
  - no name starts with `mcp_`, and `mcp_github__create_issue` does not resolve.

  (FR-013, FR-014, FR-015.)
- [ ] T035 [P] [US3] In `src/utils/mod.rs::format_thinking`, delete both `_ if name.starts_with("mcp_")` arms (around lines 115 and 264), so old calls fall through to the generic display (FR-016, R7)
- [ ] T036 [P] [US3] In `webui/app/routes/chat.tsx`, delete the `if (name.startsWith('mcp_')) { … }` branch in the tool-call label's `default` case (around line 251), so old calls fall through to `🔧 Using <name>` with their arguments. Run `cd webui && npm run typecheck`
- [ ] T037 [US3] Run `cargo test` and `cargo clippy`, then quickstart §3 in full (including the server-down step) and the MCP rows of §2, with `fixtures/echo_mcp.py`

**Checkpoint**: MCP tools carry their server category and simple names, and collisions and old names behave as the contract says.

---

## Phase 6: User Story 4 - Built-in tools have meaningful categories (Priority: P2)

**Goal**: Every built-in tool is in exactly its FR-006 category, and categories with no enabled tools disappear. `READ_CORE`/`WRITE_CORE` become `read_core`/`write_core` everywhere, with no support for the old names.

**Independent Test**: The unit test from T038 passes. Quickstart §5–§6:
- `read_core()` works in a script and `READ_CORE()` raises `NameError`;
- the `identity` category lists `read_core`, `write_core`;
- a dream run completes with `write_core`/`read_core` among its tools.

With speech and image generation off, `audio` and `media` are absent from both the prompt and the listings.

### Implementation for User Story 4

- [ ] T038 [US4] Add a `#[cfg(test)]` test in `src/agents/tools/mod.rs` that pins the exact tool-to-category table in data-model.md, `const EXPECTED: &[(&str, ToolCategory)]`, against each tool's `VizierTool::name()`/`category()`. Call the associated functions directly on each tool type, e.g. `<MemorySearch as VizierTool>::category()`, so no `VizierDependencies` is needed. It must also assert that every built-in variant is used by at least one entry, and that only the 6 direct and dream tools return `None` (FR-005, FR-006)
- [ ] T039 [US4] In `src/agents/tools/mod.rs`, add a unit test for `category_summaries` on a router with only some optional tools registered, e.g. no Discord, Telegram, TTS, STT, image or shell tools. It must assert that `discord`, `telegram`, `audio`, `media` and `shell` are absent and the rest keep enum order (FR-011, FR-012). Build the toolsets from tools that need no dependencies, or test the pure ordering and filtering helper that `category_summaries` delegates to
- [ ] T040 [P] [US4] In `src/agents/tools/workspace/mod.rs`, rename the tool names `"WRITE_CORE"` → `"write_core"` (around line 37) and `"READ_CORE"` → `"read_core"` (around line 81), and update any description text there that names them (FR-018)
- [ ] T041 [P] [US4] In `src/agents/tools/mod.rs`, change `DREAM_TOOL_NAMES` from `"WRITE_CORE"`, `"READ_CORE"` to `"write_core"`, `"read_core"` (around lines 404–405). In `src/scheduler/dream/prompts.rs`, change `` `WRITE_CORE` `` to `` `write_core` `` (around line 48). Then `grep -rn "READ_CORE\|WRITE_CORE" src templates` and fix any remaining live reference (tests and comments included)
- [ ] T042 [P] [US4] In `src/utils/mod.rs::format_thinking`, rename the `"WRITE_CORE"`/`"READ_CORE"` arms (around lines 105–106) to `"write_core"`/`"read_core"`
- [ ] T043 [P] [US4] In `webui/app/routes/chat.tsx`, rename `case 'WRITE_CORE':`/`case 'READ_CORE':` (around lines 230–232) to `'write_core'`/`'read_core'`. In `webui/app/routes/agent-core.tsx` (around line 153), change `<code>WRITE_CORE</code>` to `<code>write_core</code>`. Run `cd webui && npm run typecheck`
- [ ] T044 [US4] Run `cargo test` and `cargo clippy`, then quickstart §5 and §6 on `dp-new`

**Checkpoint**: Built-in categories are complete and pinned by a test, and the CORE tools are snake_case everywhere.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, a final sweep, and the constitution's end-to-end gate.

- [ ] T045 [P] Update `CLAUDE.md`, "Tools" and "Python sandbox / code mode" paragraphs:
  - code mode is always on (no `tools.python`, no `ToolExposure`);
  - the direct set is `sandbox_toolset` (`think`, `execute_python`, `list_tool_functions`, `describe_tool_function`);
  - `VizierTool::category()` is required, and the 13 `ToolCategory` variants are listed;
  - MCP tools are `<server>_<tool>` with category = the server key (or `<key>_mcp`), collisions are suffixed, and old names are unsupported;
  - **Adding a new tool** now also means picking a `ToolCategory`;
  - `ToolRouter::catalogue()` is the single source for naming and dispatch.

  Remove the `tools.python.{enabled, code_mode}` bullet from "Config layering", and replace `mcp_<server>__<tool>` in the Agents section with the new naming.
- [ ] T046 [P] Sweep for anything left over: `grep -rn "mcp_\|code_mode\|ToolExposure\|PythonSandbox\|READ_CORE\|WRITE_CORE" src webui/app`. Every remaining hit must be an `mcp_servers`/`mcp_config` config key or a deliberate test fixture. Update the dummyplug test fixture name in `src/agents/agent/model/dummyplug.rs` (around line 695) from `mcp_fs__read_file` to `fs_read_file`, so the tests reflect current naming
- [ ] T047 Run the full checks: `cargo build`, `cargo test`, `cargo clippy` and `cd webui && npm run typecheck`
- [ ] T048 End-to-end check with `dummyplug` agents on a running binary (constitution e2e gate): walk quickstart.md §0–§7 in full, including SC-001's tool-definition size comparison against `master`, and record any deviations in the PR description
- [ ] T049 Commit as a breaking change: `feat!: code mode by default, tool categories, <server>_<tool> MCP names [**breaking**]` (conventional commits; the body lists the removed `tools.python` config and the renamed tools)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on T001. It blocks every story.
- **US1 (Phase 3)**: depends on Phase 2.
- **US2 (Phase 4)**: depends on Phase 2. T030 also edits `sandbox.rs` and `agent/mod.rs`, which T022 introduces, so run US2 after US1, or merge T022 and T030 if you start US2 first.
- **US3 (Phase 5)**: depends on Phase 2. T033 changes `category_summaries`, which comes from T028, so US3 follows US2's T028.
- **US4 (Phase 6)**: depends on Phase 2. T039 tests `category_summaries` (T028). T040–T043 (the CORE rename) depend only on Phase 2.
- **Polish (Phase 7)**: after every story it covers.

### Within Phase 2

T002 → T003 → T004–T013 (all [P], different files) → T014 → T015 → T016 → T017 and T018 → T019.

### Parallel Opportunities

- T004–T013: ten different files, each adding one `category()` line per tool.
- In US1: T023, T024, T025 and T026 (schema, validation, HTTP DTOs, WebUI).
- In US2: T031 (sandbox runtime) alongside T028–T030 (agent layer).
- In US3: T035 and T036 (Rust and WebUI renderers).
- In US4: T040, T041, T042 and T043 (four different files).
- In Polish: T045 and T046.

---

## Parallel Example: Phase 2 category declarations

```bash
Task: "T004 Memory category on vector_memory tools in src/agents/tools/vector_memory/mod.rs"
Task: "T006 Scheduling category on scheduler tools in src/agents/tools/scheduler/mod.rs"
Task: "T008 Skills category on skill tools in src/agents/tools/skill/*.rs"
Task: "T012 Discord/Telegram/Webui categories in src/agents/tools/{discord,telegram,webui}/mod.rs"
```

## Parallel Example: User Story 1

```bash
Task: "T023 Remove PythonSandboxConfig in src/schema/agent.rs"
Task: "T025 Remove python from HTTP DTOs in src/channels/http/api/v1/agents/mod.rs"
Task: "T026 Remove sandbox UI in webui/app/..."
```

## Parallel Example: User Story 4

```bash
Task: "T040 Rename CORE tools in src/agents/tools/workspace/mod.rs"
Task: "T042 Rename CORE labels in src/utils/mod.rs"
Task: "T043 Rename CORE labels in webui/app/routes/chat.tsx and agent-core.tsx"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1, then Phase 2 (categories plus catalogue, with no behaviour change).
2. Phase 3 (US1): every agent is in code mode, using today's flat listing.
3. **Stop and validate**: run quickstart §1 on `dp-old` and `dp-new`.

### Incremental Delivery

1. US1: code mode always on (MVP).
2. US2: category discovery and the prompt category list. This is what makes the MVP efficient.
3. US3: `<server>_<tool>` names and MCP categories.
4. US4: the pinned category table and the CORE rename.
5. Polish: CLAUDE.md, the sweep, the full e2e run, and the breaking-change commit.

Each increment builds, passes `cargo test`/`clippy`, and leaves the agent usable. Ship order matches the spec's priorities (P1, P1, P2, P2).
