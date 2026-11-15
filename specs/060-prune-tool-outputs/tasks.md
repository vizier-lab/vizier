---
description: "Tasks for 060: prune old tool outputs, recall them on demand"
---

# Tasks: Prune old tool outputs, recall them on demand

**Input**: `specs/060-prune-tool-outputs/` (plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md)

**Tests**: included. The rule contract (`contracts/prune-rule.md`) requires a unit test per
guarantee, and the constitution requires the dummyplug e2e in `quickstart.md`.

**Paths** are relative to the repository root. `core/` below means `crates/vizier-core/src/`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 pruning, US2 recall, US3 measurement (spec.md)

---

## Phase 1: Setup

- [X] T001 Check the branch is `060-prune-tool-outputs` and the base builds: run `cargo build -p vizier-core` and `cargo test -p vizier-core --lib schema::history` from the repository root, and note any test already failing so it is not blamed on this feature

---

## Phase 2: Foundational (the record id, needed by US1 and US2)

**Purpose**: every tool-result message knows the history uid it was saved under (research R1).

- [X] T002 Add `record: Option<String>` to `Message::ToolResult` in `crates/vizier-core/src/llm/types.rs`, with `#[serde(default, skip_serializing_if = "Option::is_none")]` and a doc comment ("the `session_history.uid` of this result once saved; never sent to a provider"); make `Message::tool_result` set it to `None`
- [X] T003 Make every exhaustive `Message::ToolResult { call_id, content }` pattern compile by adding `..` (or `record: _`), and carry `record` through where a ToolResult is rebuilt, in `crates/vizier-core/src/llm/dialect/{anthropic,bedrock,gemini,ollama,openai_chat,openai_responses}.rs`, `crates/vizier-core/src/llm/prepare.rs`, `crates/vizier-core/src/llm/telemetry.rs`, `crates/vizier-core/src/schema/history.rs` (`message_for_model` must keep `record`), `crates/vizier-core/src/schema/response.rs`, `crates/vizier-core/src/agents/agent/mod.rs` (around lines 2347 and 2540) and `crates/vizier-core/src/agents/agent/model/dummyplug.rs`. No dialect may encode `record`; the golden fixtures in `core/llm/dialect/fixtures/golden/` must still pass unchanged
- [X] T004 In `history_entries_to_messages` (`crates/vizier-core/src/schema/history.rs`), set `record: Some(entry.uid.clone())` on the `Message::ToolResult` built from each `SessionHistoryContent::ToolResult`; leave the `NO_RESULT_RECORDED` results built by `repair_tool_pairing` at `None`. Add a unit test in the same file's `tests` module asserting a replayed result carries its entry's uid
- [X] T005 Make `StepRecorder::record` in `crates/vizier-core/src/agents/steps.rs` return the uid(s) `record_step` gave back (`Vec<String>`, in entry order; a failed save contributes nothing), keeping `recorded` counting as now; update its callers to ignore or use the value
- [X] T006 In `prompt_with` (`crates/vizier-core/src/agents/agent/mod.rs`, the tool loop around line 1456), take the uid returned by `recorder.record(…, &response_content)` and set it as the `record` of `response_content` before it is pushed into `tool_responses`, so the live `history` carries the same ids the replay will

**Checkpoint**: `cargo test -p vizier-core` passes; nothing observable has changed yet.

---

## Phase 3: User Story 1 - Long turns stop re-sending old outputs (P1) 🎯 MVP

**Goal**: old, large tool results are replaced by placeholders in the model's view, prefix-stable across rounds and turns.

**Independent test**: quickstart steps 1–3 and 5; `cargo test -p vizier-core prune`.

### Tests for US1

- [X] T007 [P] [US1] Write the unit tests for `contracts/prune-rule.md` in a `#[cfg(test)] mod tests` in `crates/vizier-core/src/schema/prune.rs`, one per guarantee: identity below the mark (SC-002); prefix stability (the pruned set of `a ++ b` contains that of `a`, and the prefix is equal when no prune fires walking `b`); the newest round is never pruned; hysteresis (no second prune before `HIGH_TOKENS − LOW_TOKENS` new tokens); ineligible results (under `MIN_BYTES`, an image part, `record: None`, an `execute_python` report with `ok: false`, a report whose call ended the turn) pass through and are not counted; the placeholder text is exactly `[output pruned (<size>) - use recall_history("<record>") to recall]` and keeps `call_id` and `record`; size formatting (`812 B`, `74.2 KB`, `1.2 MB`). Build the reports with `serde_json::json!` like the tests in `core/schema/history.rs`

### Implementation for US1

- [X] T008 [US1] Create `crates/vizier-core/src/schema/prune.rs` with `pub const HIGH_TOKENS: u64 = 60_000`, `LOW_TOKENS: u64 = 30_000`, `MIN_BYTES: usize = 2_048`, `pub struct PruneStats { pub pruned: usize, pub tokens_removed: u64 }` and `pub fn prune_for_model(messages: &[Message]) -> (Vec<Message>, PruneStats)`, implementing the walk in `contracts/prune-rule.md` exactly (rounds counted at each `Message::Assistant`; tokens `(bytes + 3) / 4` of the text; eligibility per research R4, using `ExecutionReport::looks_like`, the report's `ok` and `ExecutionReport::ending_call` from `core/sandbox/report.rs`); register it in `crates/vizier-core/src/schema/mod.rs` and re-export `prune_for_model` and `PruneStats` beside `messages_for_model`
- [X] T009 [US1] *(Done as a sibling `pruned_completion_request` returning `(CompletionRequest, PruneStats)`, leaving `completion_request` for the dream cycle and its tests, instead of a `prune: bool`.)* Give `completion_request` in `crates/vizier-core/src/agents/agent/mod.rs` (line ~2269) a `prune: bool` parameter: when true, `messages = prune_for_model(&messages_for_model(history))`, then push the narrowed current `message` (never walked); return the `PruneStats` alongside the request (a tuple or a small struct). Pass `true` from `prompt_with` and the tests around line 2583, `false` from `dream_prompt` (line ~2021)
- [X] T010 [US1] Pass `true` from the side chat's call in `crates/vizier-core/src/agents/agent/side_chat.rs` (line ~85), so an aside shares the host turn's pruned prefix (research R2)
- [X] T011 [US1] Make `generate_handover_with_model` (`crates/vizier-core/src/agents/agent/mod.rs`, line ~2433) summarise the pruned view: replace `messages_for_model(history)` with the `prune_for_model` output (research R2: compaction triggers on the pruned size, so the full view may not fit)
- [X] T012 [US1] *(Done in the `dummyplug` submodule's `src/protocol.mjs`, with a test in `test/legacy-protocol.test.mjs`: the live dummyplug is that Node server; `model/dummyplug.rs` is declared by no `mod` and never compiled. The record is read from the placeholder text, since `record` is never sent.)* Add §7 `pruned` to the dummyplug model in `crates/vizier-core/src/agents/agent/model/dummyplug.rs`, per `specs/060-prune-tool-outputs/contracts/dummyplug-pruned.md`: checked after `tools` and `context`, it lists every tool result in the received request whose text starts with `[output pruned (` as `- <call_id> <record> <size>`, under `<n> pruned:`, or replies `(nothing pruned)`; add unit tests beside the existing `context` tests in that file
- [X] T013 [P] [US1] Append §7 to `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`, copied from `specs/060-prune-tool-outputs/contracts/dummyplug-pruned.md`, and add `pruned` to the precedence notes of §1 and §6

**Checkpoint**: `cargo test -p vizier-core prune` and `dummyplug` pass; quickstart steps 1–3 and 5 behave as written (recall comes in US2).

---

## Phase 4: User Story 2 - The agent recalls what it needs (P1)

**Goal**: `recall_history` in the new `history` category reads a result back by id from its own conversation, uncapped.

**Independent test**: quickstart steps 4 and 6; `cargo test -p vizier-core recall_history`.

### Tests for US2

- [X] T014 [P] [US2] Write unit tests in a `#[cfg(test)] mod tests` in `crates/vizier-core/src/agents/tools/history.rs`, with an in-memory SQLite storage built as in `test_storage()` of `crates/vizier-core/src/agents/client/mod.rs`: save a `ToolResult` holding a JSON report and one holding plain text in session A, then assert: found + parsed JSON for the first, found + string for the second, `not_found` for an unknown uid, `not_found` for a uid saved in session B or another agent, `not_found` for the uid of a non-ToolResult entry (a `Request`), and that a 1 MB output comes back whole

### Implementation for US2

- [X] T015 [US2] Add `History` to `ToolCategory` in `crates/vizier-core/src/agents/tools/mod.rs`: the variant appended last (with a doc comment naming spec 060), appended to the end of `ALL` (size 21; `a_new_category_is_appended_to_the_end_of_the_list` pins the order), `name()` → `"history"`, `description()` → "Recall tool outputs pruned from this conversation's view; recall them in a script and return only what you need" (research R7)
- [X] T016 [US2] Create `crates/vizier-core/src/agents/tools/history.rs` with `RecallHistoryArgs { id: String }` (doc comment: "The id from a pruned output's placeholder.") and `RecallHistory { storage: Arc<VizierStorage> }` implementing `VizierTool`: `name()` `"recall_history"`, `category()` `Some(ToolCategory::History)`, the description from `contracts/recall-history.md`, and `call` reading `get_session_history_entry(&ctx.session, &args.id)` and answering `{status: "found", id, output}` or `{status: "not_found", id}` (output: the result's text parts joined, parsed with `serde_json::from_str` when it parses, else the string); storage errors are `Err` via `throw_vizier_error`, a miss never is. Model the result type on `ScratchResult` in `core/agents/tools/scratchpad.rs`
- [X] T017 [US2] Register the tool: `mod history;` in `crates/vizier-core/src/agents/tools/mod.rs`, `.tool(RecallHistory { storage: … })` on `default_toolset` inside `BaseTools::new()`, a `recall_history` → `History` row in `every_tool_is_in_its_category`, and confirm it is **not** in `VizierTools::DREAM_TOOL_NAMES`
- [X] T018 [US2] Check that `history` is reserved: add a case to the existing reserved-name test for `reserved_category_name` in `crates/vizier-core/src/agents/tools/mod.rs` (an MCP server keyed `history` becomes `history_mcp`); if no such test exists, add one

**Checkpoint**: quickstart steps 1–6 pass.

---

## Phase 5: User Story 3 - Operators can see whether it helps (P2)

**Goal**: each turn reports how much was pruned.

**Independent test**: quickstart step 7.

- [X] T019 [US3] Declare `vizier.prune.outputs = tracing::field::Empty` and `vizier.prune.tokens_removed = tracing::field::Empty` in `invoke_agent_span` (`crates/vizier-core/src/agents/agent/mod.rs`, around line 555)
- [X] T020 [US3] In `prompt_with`, after each completion, record the `PruneStats` returned by `completion_request` on the turn's `invoke_agent` span (`tracing::Span::current().record(…)`, the way `vizier.turn.ended_by_tool` is recorded near line 536), so the span ends with the last request's numbers (research R8)
- [X] T021 [P] [US3] Document the two span fields in `docs/src/configuration/observability.md`, next to the other `vizier.*` attributes

---

## Phase 6: Polish & cross-cutting

- [X] T022 [P] Document pruning and `recall_history` in `docs/src/configuration/tools-embedding.md` (or the page that lists tool categories): the marks, what is never pruned, the placeholder, recall's scope, no cap yet
- [X] T023 [P] Add a paragraph **Pruned tool outputs** (`specs/060-prune-tool-outputs/`) to the Tools section of `CLAUDE.md`: `prune_for_model` after `messages_for_model`, `Message::ToolResult.record`, the marks, `completion_request`'s `prune` flag (dreams off), the handover uses the pruned view, `recall_history` in category `history` (the 21st), not in `DREAM_TOOL_NAMES`; and update the "15 variants"/category count mentions there
- [X] T024 *(On Windows the server binary does not build (Unix-only daemonize), so clippy and tests ran for `vizier-core` and `vizier-http`: no new clippy warnings (103 before and after); `vizier-core` 1861 lib tests pass, 2 fail on master too (`projects::path`, `skill::scopes`, Windows paths); every integration test and `vizier-http` pass.)* Run `cargo clippy --workspace` and `cargo test --workspace`; then `cargo test -p vizier-core --no-default-features` (the engine with no flags, as CI does)
- [X] T025 *(Steps 1–6 are scripted as `old_tool_outputs_are_pruned_from_the_model_and_recalled_by_id` in `crates/vizier-core/tests/dummyplug_e2e.rs`, which drives the runtime against the Node dummyplug, and pass. It caught a cross-topic recall, fixed by checking the entry's topic. Step 7 (Jaeger) was not run.)* Run `specs/060-prune-tool-outputs/quickstart.md` steps 1–7 against `just run` with a dummyplug agent and record the outcome of each step in the summary (step 7 needs `just jaeger` + `just run-otel`)

---

## Dependencies

- Phase 1 → Phase 2 → US1 and US2. T002 blocks T003–T006; T005 blocks T006.
- **US1** needs Phase 2. **US2** needs Phase 2 only (T015–T018 do not touch US1's files except `agents/tools/mod.rs`) but **ships with US1**: a placeholder names `recall_history`, so neither story is released alone.
- **US3** needs T009 (the stats come from `completion_request`).
- Polish after all stories.

## Parallel opportunities

- After Phase 2: T007 (tests, `schema/prune.rs`) ∥ T014 (tests, `agents/tools/history.rs`) ∥ T013 (protocol doc).
- T015–T018 (US2) can proceed alongside T008–T012 (US1); they share only `agents/tools/mod.rs` with nothing in US1.
- T021, T022, T023 are docs-only and parallel to each other.

## Implementation strategy

1. Phase 2, then US1 to its checkpoint: the saving is measurable with `pruned` even before recall exists.
2. US2 before any release, since the placeholder points at it.
3. US3, polish, then the quickstart run (T025) as the done gate.
