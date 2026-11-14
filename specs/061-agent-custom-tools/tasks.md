---
description: "Tasks for 061: agent custom tools"
---

# Tasks: Agent custom tools

**Input**: `specs/061-agent-custom-tools/` (plan.md, spec.md, research.md, data-model.md,
contracts/, quickstart.md)

**Tests**: included where a contract or rule needs pinning (storage contract, validation, the
prompt-stability guarantee, the refactor's unchanged behaviour, dream toolset lists, conformance
vectors for every new route), plus the dummyplug e2e the constitution requires (`quickstart.md`).

**Paths** are relative to the repository root. `core/` means `crates/vizier-core/src/`, `http/`
means `crates/vizier-http/src/`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 save · US2 discover and call · US3 runs in the calling turn · US4 versions ·
  US5 people and the WebUI · US6 composition · US7 dreams · US8 categories (spec.md)

---

## Phase 1: Setup

- [X] T001 Check out the branch everywhere: run `just checkout 061-agent-custom-tools` (workspace, `design/`, `webui/`, `landing/`, `dummyplug/`), then `cargo build -p vizier-core` and `cargo test -p vizier-core --lib agents::tools` and note any test already failing so it is not blamed on this feature
- [X] T002 Add `ruff_python_parser` and `ruff_python_ast` at `=0.0.14` (the versions `monty` already pulls in; confirm in `Cargo.lock`) to `[workspace.dependencies]` in `Cargo.toml` and as `.workspace = true` dependencies in `crates/vizier-core/Cargo.toml`; check `Cargo.lock` gains no new package (research R4)

---

## Phase 2: Foundational (blocks every story)

### 2a. The per-turn tool source seam (research R1, contracts/turn-tool-source.md)

- [X] T003 Create `core/agents/tools/turn_source.rs` with the `TurnToolSource` trait (`id`, `entries`, `categories`, `describe_extra` default `None`, `call`) and `BridgeEnv` (the bridge's `ToolRouter`, tool timeout, and per-call output slots: client call id, nested `ToolInvocationRecord`s); add `mod turn_source;` and re-exports in `core/agents/tools/mod.rs`
- [X] T004 Replace `ToolTarget::Client { .. }` and `ToolTarget::Project` with `ToolTarget::Turn { source: &'static str }` in `core/agents/tools/mod.rs`; update `ToolRouter::catalogue`'s rank closure (one rank for `Turn`, after MCP) and `ToolRouter::dispatch` (one arm answering "`<name>` is only callable from a script in this turn")
- [X] T005 Add `turn_sources: Vec<Arc<dyn TurnToolSource>>` to `ToolContext` in `core/agents/tools/mod.rs` (default empty); implement `TurnToolSource` for client tools in `core/agents/client/` (wrapping `ClientTools::catalogue`/`categories`/`call`, returning the call id through `BridgeEnv`) and for project tools in `core/projects/tools/` (wrapping `offered`/`call`, adding the `coding` line as `turn_categories` does today)
- [X] T006 Rewrite `turn_catalogue` and `turn_categories` in `core/agents/tools/mod.rs` to iterate `ctx.turn_sources` (order client, project, custom) instead of reading `ctx.client`/`ctx.project`; keep the `ctx.detached` turn-ending filter after all sources
- [X] T007 Rewrite `RouterBridge::dispatch_to` in `core/agents/tools/python/bridge.rs` so a `ToolTarget::Turn { source }` entry is dispatched through the matching source's `call` with a `BridgeEnv`; delete the client and project arms; keep the client call-id side channel working through `BridgeEnv`
- [X] T008 Populate `ctx.turn_sources` wherever a turn's `ToolContext` is built today with `client`/`project` (search `client:` and `project:` in `core/agents/agent/mod.rs`, `core/agents/detached/`, `core/agents/subagents.rs`, `core/agents/tools/python/docs_tools.rs`), so detached scripts and workers inherit them as before
- [X] T009 Run the existing client-tool, project-tool, detached and docs-tool tests (`cargo test -p vizier-core --lib agents::tools agents::client projects::tools agents::detached`) and `scripts/client-tools-e2e.mjs` against a dummyplug agent; all must pass unchanged — this is the refactor's acceptance test

### 2b. Storage (data-model.md, research R10)

- [X] T010 Create `core/storage/custom_tool.rs`: value types `CustomTool`, `CustomToolVersion`, `CustomToolCategory`, `CustomToolEvent`, `CustomToolActor` (reusing `RevisionActor` and `dream_cycle_id` from `core/schema/revision.rs`), `NewVersion` (the definition fields), and the `CustomToolStorage` trait: `list_tools(agent, include_disabled)`, `get_tool(agent, id_or_name)`, `get_version(tool_id, seq)`, `list_versions(tool_id, before, limit)`, `list_categories(agent)`, `list_events(agent, before, limit)`, `save_version(agent, tool_id?, expected_seq?, NewVersion, actor, category_description?) -> Result<SaveOutcome>` (one transaction: name free, category exists or created, `current_seq` check, insert version, update tool row, delete emptied categories), `set_enabled(agent, tool_id, enabled, actor)`, `delete_tool(agent, tool_id, actor)`, `update_category(agent, name, description?, new_name?, actor)`, `touch_called(tool_id, at)`, `delete_agent_custom_tools(agent)`; add it to `VizierStorageProvider` in `core/storage/mod.rs`
- [X] T011 Add `init_custom_tool_schema` to `core/storage/sqlite/mod.rs` creating the four tables and indexes of data-model.md, and call it from the SQLite init sequence
- [X] T012 [P] Add `core/storage/sql/postgres/migrations/0010_custom_tools.sql` with the same tables (`JSONB` for JSON columns, `BIGSERIAL` ids)
- [X] T013 Implement `CustomToolStorage` for `SqlStorage` in `core/storage/sqlite/custom_tool.rs` over `SqlConn`/`Dialect` (one implementation for both backends); `save_version` re-checks every stored-state rule inside its transaction and returns `out_of_date`, `name_taken`, `category_description_required` or `category_name_taken` as outcomes, not errors
- [X] T014 Delete an agent's custom tool rows in the agent deletion transaction (`delete_agent` in `core/storage/sqlite/agent.rs` or wherever `delete_agent` lives)
- [X] T015 [P] Storage tests in `core/storage/sqlite/custom_tool_tests.rs` (on SQLite): create → v1; update → v2 keeps v1; `expected_seq` mismatch → `out_of_date`; two concurrent saves never lose a version; new category needs a description; last tool moved away deletes the category; rename keeps the id and history; delete keeps a `deleted` event; agent deletion removes all four tables' rows

### 2c. Domain service skeleton (research R11)

- [X] T016 Create `core/custom_tools/mod.rs` with `CustomTools` (holding `VizierStorage` and the published-view reader) and `Actor` (`Agent { agent_id }`, `Dream { agent_id, cycle_id }`, `Person { user_id, name }`), plus `Outcome` (the `status` shapes of contracts/agent-tools.md §2); add `custom_tools: CustomTools` to `VizierDependencies` in `core/dependencies.rs`
- [X] T017 [P] Create `core/custom_tools/naming.rs`: tool name rule `^[a-z][a-z0-9_]*$`, `call_as(name) = "custom_" + name`, `category_name(requested, reserved)` reusing `reserved_category_name(.., "custom")` from `core/agents/tools/mod.rs` (make it `pub(crate)`), default category `custom` and its fixed description; unit tests for each rule
- [X] T018 Reserve the `custom_` prefix: add a test in `core/agents/tools/mod.rs` that no native, addon or `RETIRED_TOOL_NAMES` name starts with `custom_`, and make `assign_names` (MCP) and `client_category_name`/client tool naming skip names that start with `custom_` by suffixing as for any clash
- [X] T019 Add `ToolCategory::CustomTools` (`name() = "custom_tools"`, description "Make, change and organise your own custom tools") at the **end** of `ToolCategory::ALL` in `core/agents/tools/mod.rs`; update `a_new_category_is_appended_to_the_end_of_the_list` and the category count in comments/CLAUDE.md wording where tests read it
- [X] T020 Add `ToolContext` fields `script_router: Option<ToolRouter>`, `script_router_is_dream: bool`, `custom_chain: Vec<String>`, `custom_categories: Arc<Vec<CategorySummary>>` (all defaulted) in `core/agents/tools/mod.rs`; set `script_router` on the bridge's own context copy in `RouterBridge::new` (`core/agents/tools/python/bridge.rs`) (research R9)

**Checkpoint**: `cargo test -p vizier-core` and `cargo clippy --workspace` pass; client and project tools behave exactly as before; no custom tool is reachable yet.

---

## Phase 3: User Story 1 — An agent saves a workflow as a custom tool (P1) 🎯 MVP

**Goal**: `create_custom_tool` validates a definition, reports every problem at once, and stores
version 1 (with its category); an optional sample input is run for real first.

**Independent Test**: quickstart §1.

- [X] T021 [P] [US1] Create `core/custom_tools/schema.rs`: `check_schema(&Value) -> Vec<Problem>` (must be `type: object`; only the R6 keywords; unknown keyword → `schema_keyword_unsupported` with JSON path) and `validate(&schema, &input) -> Result<Value, Vec<Problem>>` (every mismatch with its path; fills `default`s); unit tests for each keyword, nested objects/arrays, multiple errors at once
- [X] T022 [P] [US1] Create `core/custom_tools/calls.rs`: `parse_calls(script) -> Result<Vec<String>, SyntaxProblem>` using `ruff_python_parser::parse_module` and an AST visitor collecting `Call` nodes whose `func` is a bare `Name`, minus Python builtins and names the script assigns or defines; `SyntaxProblem { line, column, message }`; unit tests (strings/comments ignored, attribute calls ignored, nested calls found, syntax error position)
- [X] T023 [US1] Create `core/custom_tools/validate.rs`: `validate_definition(def, ctx_names: Option<&HashSet<String>>, agent_tools: &[CustomTool]) -> (Vec<Problem>, Vec<Warning>)` running name, category-description, schema, syntax, unknown-tool (with nearest names via the existing `did_you_mean` helper used by `list_tool_functions`) and sample-input checks, all collected; `None` names → warning `tools_unchecked`
- [X] T024 [US1] Implement `CustomTools::create` in `core/custom_tools/mod.rs`: validate, then (if `sample_input` and a non-dream `script_router`) run the sample via `custom_tools::run` (T030) and refuse with `sample_failed` on failure, then `save_version`; return `created` with `call_as`, `category_created`, `category_renamed_to`, `sample_run.took_effect`
- [X] T025 [US1] Create `core/agents/tools/custom_tools.rs` with `CreateCustomTool` (`VizierTool`, category `CustomTools`, input per contracts/agent-tools.md §2; description states the `input` global, last-expression result and the skills-vs-custom-tools sentence of FR-029); register it on `default_toolset` in `BaseTools::new()`; add it to the `every_tool_is_in_its_category` table
- [X] T026 [US1] Name check against the turn: in `CreateCustomTool::call`, build the known-name set from `ctx.script_router` (`turn_catalogue` names) plus the agent's custom `call_as` names; with no router, read the published view (T036)

**Checkpoint**: quickstart §1 passes except the sample run (needs US2's runner) — T030 completes it.

---

## Phase 4: User Story 2 — An agent discovers and calls its custom tools (P1)

**Goal**: enabled custom tools appear in the turn's catalogue and category lines and run when
called, with input validation and failure-as-data.

**Independent Test**: quickstart §2.

- [X] T027 [US2] Add `execute_with_input(code, input: Option<Value>, limits, bridge)` to `core/sandbox/runtime.rs` (pass `vec!["input".into()]` to `MontyRun::new` and the converted value to `runner.start` when present; `execute` delegates with `None`); engine test that `input["a"] + 1` returns the right value
- [X] T028 [US2] Add `input: Option<serde_json::Value>` to `run_script` in `core/agents/tools/python/mod.rs` and pass it through; existing callers pass `None`
- [X] T029 [US2] Add `calls: Vec<ToolInvocationRecord>` (serde default, skip if empty) to `ToolInvocationRecord` in `core/sandbox/report.rs`; make sure the redactor (`secret_redactor.report`) walks nested records
- [X] T030 [US2] Create `core/custom_tools/run.rs`: `call(tool_name, arguments, ctx, env)` — load the current version by name; disabled/deleted → error text naming who disabled it / unknown; `schema::validate` → `invalid input: …` error; chain check against `ctx.custom_chain` → cycle error; then `run_script(env.router, script, limits, env.tool_timeout, &nested_ctx, Some(input))` with `nested_ctx.custom_chain` extended; on `!report.ok` build the failure summary of contracts/agent-tools.md §3 (failing step, calls made, stdout); store the nested records in `env`; write `touch_called` best-effort; open the span fields `vizier.custom_tool.id`/`.version`
- [X] T031 [US2] Create `core/custom_tools/catalogue.rs` with `CustomTurnSource` implementing `TurnToolSource`: `entries` = one `list_tools(agent, false)` query → `CatalogueEntry { name: call_as, category, target: Turn{"custom"}, definition }` with the description suffix of research R3; `categories` = `ctx.custom_categories`; `describe_extra` = `{custom: {version, author, model_backed, needs_attention}}`; `call` → `run::call`
- [X] T032 [US2] Register `CustomTurnSource` in `ctx.turn_sources` (after client and project) wherever T008 builds them, for every turn that can run scripts (conversations, tasks, workers, detached copies), and **not** for dream contexts
- [X] T033 [US2] Snapshot category lines at turn start: in `core/agents/agent/mod.rs`, where the turn's `ToolContext` is assembled, read `list_categories(agent)` filtered to categories with ≥ 1 enabled tool and store them in `ctx.custom_categories`, ordered by name with `custom` last; `turn_categories` gets them through the source (T031)
- [X] T034 [US2] Mark custom entries in the full listing: add `custom: bool` (skip if false) to `ToolFunctionDoc` in `core/sandbox/docs.rs`, set it for `Turn{"custom"}` entries in `RouterBridge::catalogue` and `list_tool_functions`; merge `describe_extra` into `describe_tool_function`'s doc in `core/agents/tools/python/docs_tools.rs`
- [X] T035 [US2] Prompt-stability tests in `core/agents/tools/mod.rs` (or `core/custom_tools/catalogue.rs` tests): no custom tools → no custom lines; a tool saved into an existing category between two turns → identical `sandbox_md`; a new category → exactly one added line; a disabled tool never appears in entries, listing or description
- [X] T036 [US2] Publish `agent.{id}.tool_names` in `core/agents/views.rs` (sorted names of the composed router's native, addon and MCP catalogue, with the standard `expires_at_ms` envelope) on take, on `refresh_integrations` and on the views' refresh; add a reader used by `CustomTools`; add the read to `specs/048-agent-service-seam/contracts/agent-state-audit.md` if `scripts/048-agent-state-read-check.sh` flags it
- [X] T037 [US2] Needs attention and model-backed: in `core/custom_tools/mod.rs`, compute `needs_attention.missing` (current `calls` − published names − enabled custom `call_as` names) and `model_backed` (R7) on every read; add `fn model_backed(&self) -> bool` (default false) to `VizierTool`/`VizierToolDyn` in `core/agents/tools/mod.rs` and return true from `spawn_subagents`, `mention_agent`, image generation, `read_image` (vision) and STT/TTS tools
- [X] T038 [US2] Dummyplug e2e: add `custom_tools` steps for quickstart §0–§2 to the `dummyplug_e2e` test (`crates/vizier-core/tests/dummyplug_e2e.rs` or its module), including calling a tool saved in the same turn and from a second chat

**Checkpoint**: US1 + US2 are the MVP — an agent saves a tool and calls it anywhere.

---

## Phase 5: User Story 3 — A custom tool runs as part of the calling turn (P1)

**Goal**: inner calls behave exactly as direct calls in the calling turn; the caller's time limit
bounds the run.

**Independent Test**: quickstart §3.

- [X] T039 [US3] Test in `core/custom_tools/run.rs`: a custom tool calling a tool whose `available(ctx)` is false in an unattended context gets the same error a direct script call gets (use a probe tool like `bridge.rs`'s tests)
- [X] T040 [US3] Test: a custom tool whose script loops forever is stopped by the caller's tool timeout, and the error names `custom_<name>`; a custom tool called from a detached script runs under the detached script's limit
- [ ] T041 [US3] Trace check: the custom tool's `execute_tool custom_<name>` span is the parent of its nested `execute_tool` spans (assert with the tracing test subscriber pattern already used in `core/agents/tools/mod.rs` tests, or document a manual `just run-otel` check in quickstart if no harness exists)

---

## Phase 6: User Story 4 — Updates, history, diff, rollback (P2)

**Goal**: every change is a version; diffs and rollback work for the agent.

**Independent Test**: quickstart §4.

- [X] T042 [US4] Implement `CustomTools::update` and `CustomTools::rollback` in `core/custom_tools/mod.rs` (rollback = copy the old version's definition and validate it as an update; `restored_from` set; agent actor refused with `disabled_by_person` on a person-disabled tool; rename refused with `has_callers` when other tools call it)
- [X] T043 [US4] Implement `CustomTools::diff(tool, from, to)` using `core/storage/diff.rs::diff_lines` for description, input schema (pretty JSON) and script, plus a `category: {from, to}` field when it changed
- [X] T044 [P] [US4] Add `UpdateCustomTool`, `GetCustomTool`, `ListCustomTools`, `CustomToolHistory`, `DiffCustomTool`, `RollbackCustomTool`, `DeleteCustomTool`, `SetCustomToolEnabled` to `core/agents/tools/custom_tools.rs` per contracts/agent-tools.md §2; register on `default_toolset`; add to `every_tool_is_in_its_category`
- [X] T045 [US4] Implement `CustomTools::delete` (agent: refused with `has_callers`) and `CustomTools::set_enabled` (agent may not re-enable a person-disabled tool) in `core/custom_tools/mod.rs`
- [X] T046 [US4] Dummyplug e2e steps for quickstart §4 in the `dummyplug_e2e` test

---

## Phase 7: User Story 8 — The agent organises its tools into its own categories (P2)

**Goal**: categories are created by the first save, renamed/redescribed by one tool, moved with a
version, cleaned up when empty, and clash-safe.

**Independent Test**: quickstart §6.

- [X] T047 [US8] Add `UpdateCustomToolCategory` to `core/agents/tools/custom_tools.rs` and `CustomTools::update_category` in `core/custom_tools/mod.rs` (rename moves every tool with one `category_renamed` event each and no version; onto an existing name → `category_name_taken`; rename/description applies to the prompt from the next turn)
- [X] T048 [US8] Clash at turn time: in `CustomTurnSource::categories`/`entries`, show a custom category as `<name>_custom` while an addon id or MCP key of the composed router equals its stored name (research R12); test it
- [X] T049 [US8] Tests in `core/custom_tools/` for: built-in clash at save → `_custom` + `category_renamed_to`; moving a tool is a version; last tool deleted removes the category; all tools disabled keeps the row but drops the prompt line
- [X] T050 [US8] Dummyplug e2e steps for quickstart §6

---

## Phase 8: User Story 7 — A dream turns a repeated workflow into a custom tool or a skill (P2)

**Goal**: dreams can author (not delete, toggle or call) custom tools; the dream prompts look for
reusable workflows.

**Independent Test**: quickstart §8 (mechanics with dummyplug; judgment marked live-provider).

- [X] T051 [US7] Add `list_custom_tools`, `get_custom_tool`, `get_custom_tool_history`, `diff_custom_tool`, `create_custom_tool`, `update_custom_tool`, `rollback_custom_tool`, `update_custom_tool_category` to `VizierTools::DREAM_TOOL_NAMES` in `core/agents/tools/mod.rs`; add tests that `delete_custom_tool`, `set_custom_tool_enabled` and every `custom_*` name never reach the dream cycle, like `secrets_tools_never_reach_the_dream_cycle`
- [X] T052 [US7] In `VizierTools::dream_call` (`core/agents/tools/mod.rs`), set `ctx.script_router = Some(self.router.clone())` and `ctx.script_router_is_dream = true` before dispatch, so saves validate against the full toolset and skip the sample run; take the cycle id from the request metadata the way memory revisions do (`core/agents/process.rs`, `dream_cycle_id`) and pass `Actor::Dream`
- [X] T053 [P] [US7] Update `EXTRACTION_PROMPT` and `CONSOLIDATION_PROMPT_TEMPLATE` in `core/scheduler/dream/prompts.rs` exactly as contracts/prompt-and-dream.md §2–§3 (Reusable Workflows section; step 4 "skills and custom tools" with the rule, check-first, category nudge; audit counts)
- [X] T054 [US7] Dummyplug check for quickstart §8 mechanics (a dream session calling `create_custom_tool` directly records a dream author and no sample run; `delete_custom_tool` refused); document the live-provider judgment steps as marked in `quickstart.md`

---

## Phase 9: User Story 5 — People see, edit, disable and delete custom tools (P2)

**Goal**: HTTP routes and a WebUI page per agent, grouped by category, with history, diff,
rollback, enable/disable, delete and an editor.

**Independent Test**: quickstart §7.

### API

- [X] T055 [US5] Create `core/service/custom_tools.rs`: list (grouped), detail, edit (person actor; no sample run; unknown-tool check via the published view, `tools_unchecked` warning when it is missing), delete, set_enabled, versions, diff, rollback, events, update_category — each authorized with `access::can_view_agent` / `can_edit_agent`, mapping outcomes to `ServiceError` kinds (`out_of_date` → conflict, `invalid` → unprocessable)
- [X] T056 [US5] Create `http/api/v1/agents/custom_tools.rs` with the ten routes of contracts/http-api.md, registered in the agents router and in the OpenAPI document (`utoipa`); epoch-ms fields as `i64`
- [X] T057 [US5] Conformance vectors in `crates/vizier-http/tests/conformance/groups/custom_tools.rs` for every new route (three fixture users; vectors idempotent across the harness's two sends), record the baseline with `CONFORMANCE_RECORD=1`, and make sure `tests/conformance/pending.txt` does not grow
- [ ] T058 [US5] Regenerate `webui/app/api/schema.d.ts` with `scripts/openapi.mjs`

### Design first (constitution: no UI code before a reviewed mockup)

- [X] T059 [US5] Draw the Custom tools page in FigCraft (`Vizier.figcraft`, a new `Custom tools` page matching the Skills/Secrets agent pages, both themes): category groups with heading and edit action, tool rows (version, author, time, switch, needs-attention warning), the tool view (script, schema, history, diff), the editor with the category picker and "new category asks for a description", the category dialog, the view-only state and the phone layout. If FigCraft is unreachable, make `specs/061-agent-custom-tools/mockups/custom-tools.html` instead. Get it reviewed before T060

### Implementation

- [ ] T060 [US5] In `design/`: add `CustomToolGroup` and `CustomToolRow` (and any new variant the drawing needs) with Storybook stories; reuse the existing revision list and diff view components from memory history; run the design checks; commit in `design/` first
- [ ] T061 [P] [US5] Create `webui/app/lib/customTools.ts` (+ `customTools.test.ts`): grouping by category (`custom` last), author label, needs-attention text, "changed underneath you" detection from `expected_version` vs a 409
- [ ] T062 [US5] Create `webui/app/routes/custom-tools.tsx` and `webui/app/routes/custom-tool.tsx` plus `webui/app/screens/customTools/` (page, tool view with history and diff, editor using the editor component skill editing uses, category dialog, delete confirmation via design `Dialog`); hide every control for view-only people; register the routes
- [ ] T063 [US5] Add the Custom tools entry to the agent menu beside Skills and a row in `webui/app/lib/agentDetails.ts` (+ its test) for the phone details list
- [ ] T064 [US5] `cd webui && npm run typecheck && npm test`; commit `webui/`, then bump the `design`/`webui` pins in the workspace (order design → webui → vizier)

---

## Phase 10: User Story 6 — Custom tools call other custom tools (P3)

**Goal**: composition works with no depth limit; cycles are refused at save and at call.

**Independent Test**: quickstart §5.

- [X] T065 [US6] Save-time cycle check in `core/custom_tools/validate.rs`: graph of current `calls` restricted to `custom_*` names plus the candidate; a path back to the candidate → `cycle` problem with the path (`a → b → a`); unit tests including a three-tool cycle and a rename
- [X] T066 [US6] Test the call-time chain check in `core/custom_tools/run.rs` with a cycle introduced indirectly (`f = custom_b; f()`) that the save check cannot see
- [X] T067 [US6] Make `model_backed` and `needs_attention` transitive through called custom tools (T037) and test it; dummyplug e2e steps for quickstart §5

---

## Phase 11: Polish & cross-cutting

- [X] T068 [P] Write `docs/src/configuration/custom-tools.md` (what a custom tool is, writing one, categories, versions, the WebUI page, dreams, and "skill or custom tool?" — FR-029) and add it to `docs/src/SUMMARY.md`
- [X] T069 [P] Update `CLAUDE.md`: a **Custom tools** paragraph under Tools (service, storage, turn source, naming, `custom_tools` category as the 22nd, dream tool list), the refactored `ToolTarget::Turn` seam in place of the client/project wording, and "Adding a new tool" unchanged
- [ ] T070 Run the full quickstart by hand on a dummyplug agent (including §7 in the WebUI and §9 needs-attention with an MCP server), then `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, `cd webui && npm run typecheck`; fix anything found
- [ ] T071 Re-read spec.md's functional requirements and success criteria against the result and note any gap in the PR description (e.g. SC-008 depends on a live model)

---

## Dependencies

- **Phase 1 → Phase 2 → stories.** 2a (seam) and 2b (storage) are independent of each other; 2c needs both.
- **US1 (Phase 3)** needs Phase 2. Its sample run (T024) needs T030 from US2; build T027–T030 alongside US1 or accept the checkpoint note.
- **US2 (Phase 4)** needs US1's storage writes to have something to list; together they are the MVP.
- **US3** needs US2's runner (tests only).
- **US4, US8** need US1 + US2; independent of each other.
- **US7** needs US1 (+ US4's update tool and US8's category tool for its full toolset list).
- **US5** needs US1, US2 (reads), US4 (versions/diff/rollback) and US8 (category edit) for the full page; its design task T059 can start any time.
- **US6** needs US2; its save check can land with US1's validator.
- **Polish** last.

```
Setup ─▶ Foundational ─▶ US1 ─┬─▶ US2 ─┬─▶ US3
                              │        ├─▶ US4 ─┐
                              │        ├─▶ US8 ─┼─▶ US5 (API + UI)
                              │        └─▶ US6  │
                              └────────────────▶ US7
T059 (design) ── any time ──▶ T060 ─▶ T061–T064
```

## Parallel opportunities

- **Phase 2**: T003–T009 (seam) in parallel with T010–T015 (storage); T012 and T015 in parallel with T013.
- **US1**: T021 (schema) and T022 (parser) in parallel, then T023.
- **US2**: T027–T029 (sandbox/report) in parallel with T036 (published view) and T034 (docs).
- **US4/US8/US6** can proceed in parallel once US2 lands (different functions in the same files — coordinate edits to `core/agents/tools/custom_tools.rs` and `core/custom_tools/mod.rs`).
- **US5**: T059 design from day one; T055–T058 API in parallel with T060 design implementation; T061 in parallel with T062.
- **US7**: T053 (prompts) any time.

## Implementation strategy

1. **MVP = Phase 1–4 (US1 + US2)**: an agent saves a custom tool and calls it in any later
   script or conversation. Verify with quickstart §0–§2 and stop to review.
2. **US3** adds confidence tests, no new behaviour.
3. **US4 + US8**: maintainable toolbox (versions, categories) — still agent-only.
4. **US7**: dreams start producing tools; review prompt quality with a live provider.
5. **US5**: people can see and govern it (design first).
6. **US6**: composition polish.
7. Polish and docs; open the PR with a conventional title such as
   `feat(agents): let agents save scripts as custom tools`.

**Totals**: 71 tasks — Setup 2, Foundational 18, US1 6, US2 12, US3 3, US4 5, US8 4, US7 4,
US5 10, US6 3, Polish 4.
