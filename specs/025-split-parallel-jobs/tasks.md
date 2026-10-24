---

description: "Task list for 025-split-parallel-jobs"
---

# Tasks: Blocking and Background Parallel Subagents

**Input**: Design documents from `specs/025-split-parallel-jobs/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md) (R1–R13), [data-model.md](data-model.md), [contracts/agent-tools.md](contracts/agent-tools.md), [quickstart.md](quickstart.md)

**Tests**: The plan's Testing line asks for unit tests: shared validation, result ordering, deadline arithmetic, `is_durable` for transient requests, the retired-name hint and the category table. They are included next to the code they cover, in the crate's existing `#[cfg(test)] mod tests` style. The end-to-end gate is quickstart.md on a dummyplug agent.

**Organization**: by user story from spec.md. US1 and US2 are both P1. US2 is mostly a rename, so it can land first or alongside US1.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US4 from spec.md

---

## Phase 1: Setup

- [X] T001 Confirm a clean baseline on branch `025-split-parallel-jobs`: `cargo clippy` and `cargo test` pass, and `cd webui && npm run typecheck` passes, before any change (record any pre-existing failures in this file's Notes).

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: the shared piece primitives and the shared argument type both tools build on (research R1, R5, R9).

- [X] T002 In `src/agents/background/mod.rs`, extract from `launch` a `async fn dispatch_piece(&self, session: &VizierSession, prompt: &str, origin: &AgentId, depth: u8) -> crate::Result<flume::Receiver<VizierResponse>>`: the `send_request` of `VizierRequestContent::Prompt` with `background_depth: depth`. Make `launch` call it. T012 adds a `transient` parameter later. Keep the `.instrument(job_span.clone())` at the call site.
- [X] T003 In `src/agents/background/mod.rs`, extract from `run_job`'s per-piece future a `async fn await_piece(&self, rx: &flume::Receiver<VizierResponse>, limit: Duration, session: &VizierSession, origin: &AgentId) -> (PieceState, String)`. It runs `first_terminal` under `tokio::time::timeout(limit, …)`, and on elapse calls `self.abort_piece(session, origin)` and returns `(PieceState::TimedOut, format!("No answer within {}s", limit.as_secs()))`. Make `run_job`'s `PieceSource::Dispatched(Some(rx))` arm use it. The recovered `PieceSource::Session` arm keeps its own timeout path, with the same message.
- [X] T004 In `src/agents/background/mod.rs`, move the depth refusal out of `launch` into `pub fn check_depth(ctx: &ToolContext) -> crate::Result<()>`, with the message `nesting limit ({MAX_BACKGROUND_DEPTH}) reached: this turn was itself started by subagent or background work`, and call it from `launch`. Update the module doc comment to name `background_subagents`/`parallel_subagents` instead of `paralel_subtasks`.
- [X] T005 Create `src/agents/tools/subagents.rs` with `SubagentsArgs { tasks: Vec<SubagentTask { prompt: String }>, timeout_secs: Option<u64> }` (schemars descriptions as in `src/agents/tools/subtasks/mod.rs`) and `fn validate(args: &SubagentsArgs) -> crate::Result<u64>`, which returns `tasks must not be empty` for an empty list or a blank prompt, and otherwise `validate_timeout(args.timeout_secs)`. Add unit tests: an empty list, a whitespace prompt, out-of-range and default timeouts. Add `mod subagents;` in `src/agents/tools/mod.rs`.

**Checkpoint**: `cargo test` passes. Behaviour is unchanged: `paralel_subtasks` still works through the extracted primitives.

---

## Phase 3: User Story 2 — Keep background fan-out for long work (Priority: P1)

**Goal**: `background_subagents` is today's tool under its new name, and `paralel_subtasks` is gone but explains itself (FR-002, FR-010, research R8, R9).

**Independent test**: quickstart §9 (spec 012's batch steps with the new name) and §10 (retired name).

- [X] T006 [US2] In `src/agents/tools/subagents.rs`, add `BackgroundSubagentsTool { deps }`: `name()` is `background_subagents`, category `Agents`, `Input = SubagentsArgs`, and `call` uses `validate` and then the body of `SubtasksTool::call` (`launch(ctx, JobKind::Batch, …)`, same acknowledgement text). Keep the description from `subtasks/mod.rs` word for word. Delete `src/agents/tools/subtasks/`.
- [X] T007 [US2] In `src/agents/tools/mod.rs`: replace `subtasks::SubtasksTool` with `subagents::BackgroundSubagentsTool` in the imports, in `.tool(…)` on `default_toolset` (`BaseTools::new`) and in the `entry::<…>()` list. In `every_tool_is_in_its_category`, replace `("paralel_subtasks", Agents)` with `("background_subagents", Agents)`.
- [X] T008 [US2] In `src/agents/tools/mod.rs`, add `pub const RETIRED_TOOL_NAMES: &[(&str, &str)] = &[("paralel_subtasks", "paralel_subtasks was replaced by parallel_subagents (waits for the results) and background_subagents (results arrive later as a background report)")];` and `pub fn retired_hint(name: &str) -> Option<&'static str>`, with a doc comment pointing at #82. Add a unit test that the hint exists and that neither new name is retired.
- [X] T009 [US2] In `src/agents/tools/python/bridge.rs` `SandboxBridge::call`: when `self.entry(function)` is `None` and `retired_hint(function)` is `Some(hint)`, return `Err(BridgeError::Tool(hint.into()))` instead of `UnknownFunction`.
- [X] T010 [P] [US2] In `src/agents/tools/python/docs_tools.rs`, make the `describe_tool_function` not-found answer use the retired hint as its `message` when `retired_hint(name)` is `Some`, keeping `available: false`.
- [X] T011 [P] [US2] Update the model-facing and doc texts that name the old tool: `src/agents/tools/background_jobs.rs` (`list_background_jobs` description → `background_subagents`), `src/schema/request.rs` (`BackgroundReport` doc comment), and `src/utils/mod.rs` (replace the `"paralel_subtasks"` status arm with `"background_subagents" => "⚡ Starting background subagents"`).

**Checkpoint**: quickstart §9 and §10 pass. `cargo test` passes.

---

## Phase 4: User Story 1 — Fan out and answer in the same turn (Priority: P1) 🎯 MVP

**Goal**: `parallel_subagents` runs the tasks concurrently, waits, and returns ordered results within the tool deadline. It creates no job and stops with the turn (FR-004–FR-009, FR-011, research R2–R7).

**Independent test**: quickstart §2–§8.

- [X] T012 [US1] In `src/schema/request.rs`, add `VizierRequest.transient: bool` with `#[serde(skip)]`, `#[schemars(skip)]`, `#[schema(ignore)]` and a doc comment (never journaled, set only for blocking pieces), and make `is_durable` return `false` when it is set. Add a unit test. Then add a `transient: bool` parameter to `dispatch_piece` (T002) that sets the field, and pass `false` from `launch`.
- [X] T013 [US1] In `src/agents/tools/mod.rs`, add `pub deadline: Option<tokio::time::Instant>` to `ToolContext` with a doc comment (when the agent loop's `tools.timeout` cuts the current direct call off; inherited by script calls). Fix every `ToolContext { … }` literal (grep `ToolContext {` across `src/`) to set `deadline: None`.
- [X] T014 [US1] In `src/agents/agent/mod.rs`, at the chat tool-call site (where `ctx_clone` is made before `traced_tool_call`, near line 1116), set `ctx_clone.deadline = Some(tokio::time::Instant::now() + *self.config.tools.timeout)`. Leave the dream call site (near line 1680) as `None`.
- [X] T015 [US1] Create `src/agents/background/wait.rs` (`pub mod wait;` in `background/mod.rs`) with `pub const GRACE: Duration = Duration::from_secs(2);`, a pure `fn wait_limit(per_task: Duration, deadline: Option<Instant>, now: Instant) -> Result<Duration, Duration>` (Ok(limit) = `min(per_task, deadline − now − GRACE)`; Err(remaining) when under `GRACE + 1s` is left), and unit tests for no deadline, deadline first, per-task first and too little time.
- [X] T016 [US1] In `src/agents/background/wait.rs`, implement `impl BackgroundJobs { pub async fn run_and_wait(&self, ctx: &ToolContext, prompts: Vec<String>, per_task: Duration) -> crate::Result<Vec<SubagentResult>> }`: `check_depth(ctx)`, then `wait_limit` (refusal text `not enough time left in this tool call ({n}s); call it earlier in the script or from a new execute_python`). For each prompt, make a `VizierSession(ctx.session.0, VizierChannelId::Subagent, Some(uuid))` and call `dispatch_piece(…, depth = ctx.background_depth + 1, transient = true)`; a dispatch error becomes `failed` with its reason. Then `join_all` of `await_piece` per piece, cut each text with `report::truncate(…, REPORT_TEXT_LIMIT)`, and return `SubagentResult { task, state, text, truncated, topic }` in input order (define `SubagentResult` here: `Serialize`, `JsonSchema`, fields per data-model.md, `state: PieceState`). No storage writes and no `publish`.
- [X] T017 [US1] In `src/agents/background/wait.rs`, add the drop guard: a struct holding `BackgroundJobs` + origin agent + the sessions of unfinished pieces (shared `Arc<Mutex<Vec<…>>>`, each piece removing itself when final). Its `Drop` spawns `abort_piece` for whatever is left (`tokio::spawn`, guarded by `tokio::runtime::Handle::try_current`). `run_and_wait` holds it for the wait.
- [X] T018 [US1] Unit-test the ordering and alignment in `src/agents/background/wait.rs` (factor the outcome→`SubagentResult` mapping into a pure helper if needed): mixed answered, failed and timed-out outcomes in input order give one entry each with the right `task`.
- [X] T019 [US1] In `src/agents/tools/subagents.rs`, add `ParallelSubagentsTool { deps }`: `name()` is `parallel_subagents`, category `Agents`, the description from `contracts/agent-tools.md` word for word, and `Output = serde_json::Value`. `call` runs `validate`, then `deps.background_jobs.run_and_wait(ctx, prompts, Duration::from_secs(timeout_secs))`, and returns `json!({"results": results})`.
- [X] T020 [US1] In `src/agents/tools/mod.rs`, register `ParallelSubagentsTool::new(deps.clone())` on `default_toolset` next to `BackgroundSubagentsTool`, add it to the `entry::<…>()` list, add `("parallel_subagents", Agents)` to `every_tool_is_in_its_category`, and check that neither name is in `VizierTools::DREAM_TOOL_NAMES`.
- [X] T021 [P] [US1] In `src/utils/mod.rs`, add the status arm `"parallel_subagents" => "⚡ Running subagents"`.

**Checkpoint**: quickstart §2–§8 pass on a dummyplug agent. `cargo test` passes.

---

## Phase 5: User Story 3 — The agent picks the right one (Priority: P2)

**Goal**: the guidance and descriptions say which tool waits and when to use each (FR-013).

**Independent test**: quickstart §11 (descriptions and boot prompt).

- [X] T022 [US3] In `src/agents/agent/system_prompt/boot.rs`, replace the "Large tasks" paragraph with the text in `contracts/agent-tools.md` § Boot guidance.
- [X] T023 [US3] Check that `ParallelSubagentsTool::description` and `BackgroundSubagentsTool::description` match `contracts/agent-tools.md`. Add a unit test in `src/agents/tools/subagents.rs` that the blocking description mentions `background_subagents` and "wait", and that the background one contains "returns immediately".

**Checkpoint**: quickstart §11 (first two bullets) passes.

---

## Phase 6: User Story 4 — Old conversations still read correctly (Priority: P3)

**Goal**: the WebUI keeps showing old `paralel_subtasks` calls as job launches, treats `background_subagents` as one and shows `parallel_subagents` as an ordinary call (FR-014, FR-015, research R11). This work is in the `webui/` submodule (`vizier-lab/webui`), committed there before vizier's pin bump.

**Independent test**: quickstart §12.

- [X] T024 [P] [US4] In `webui/app/lib/trail.ts`, set `JOB_LAUNCHING_TOOLS = new Set(['paralel_subtasks', 'background_subagents', 'delegate_agent'])`, with a comment that the first is kept for saved history. Leave `parallel_subagents` out.
- [X] T025 [P] [US4] In `webui/app/lib/trail.test.ts`, next to the existing `paralel_subtasks` case (around line 298), add cases: `background_subagents` is a job launch, `paralel_subtasks` still is, `parallel_subagents` is an ordinary tool call.
- [X] T026 [P] [US4] In `webui/app/screens/chat/BackgroundJobsPanel.tsx` (line 15) and `webui/app/screens/chat/MessageList.tsx` (line 165), change the batch label `'paralel_subtasks'` to `'background_subagents'`. In `webui/app/api/frames.ts` (line 264), update the comment.
- [X] T027 [US4] Regenerate `webui/app/api/schema.d.ts` with `scripts/openapi.mjs` against a server built after T011, and check that the `BackgroundReport` doc comment changed and nothing else unexpected did.
- [X] T028 [US4] Run `cd webui && npm run typecheck && npx vitest run`, commit in the submodule (`fix: name background_subagents and parallel_subagents`), then stage the new `webui` pin in vizier.

**Checkpoint**: quickstart §12 passes.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T029 [P] In `src/usage/mod.rs` (around line 795), change the `SUBAGENT` session sublabel from `Background job` to `Subagent` (research R10).
- [X] T030 [P] Remove the unused `Subtask` and `SubtaskArgs` structs from `src/agents/agent/mod.rs` (lines ~69–79), and their now-unused imports.
- [X] T031 [P] Docs: `docs/src/configuration/tools-embedding.md` (replace the `paralel_subtasks` row with rows for both tools), `docs/src/configuration/agents.md` (usage "Background" row), and `docs/src/configuration/observability.md` (the `execute_tool` example → `parallel_subagents`, with its piece `invoke_agent` spans as children; the `background_job` paragraph → `background_subagents`).
- [X] T032 Update `CLAUDE.md`'s "Background jobs" paragraph (tools section) to name `background_subagents`, add one sentence on `parallel_subagents` (blocking, no job, `ToolContext.deadline`, transient pieces), and drop the "misspelt, kept" note.
- [X] T033 Add a checklist entry to issue #82 for `RETIRED_TOOL_NAMES`/`retired_hint` (and its two call sites), noting the WebUI's `paralel_subtasks` entry in `JOB_LAUNCHING_TOOLS` (`gh issue comment 82 --body …` after confirming with the user, since it is outward-facing).
- [X] T034 `grep -rn paralel_subtasks src docs CLAUDE.md` finds only `RETIRED_TOOL_NAMES`, its test and the "replaced" hint text (SC-004).
- [X] T035 `cargo clippy` and `cargo test` pass, as does `cd webui && npm run typecheck`.
- [X] T036 End-to-end check with a `dummyplug` agent on a running binary (constitution e2e gate): walk quickstart.md §1–§12 and record any deviation here.
- [X] T037 Commit as `feat: split paralel_subtasks into parallel_subagents and background_subagents [**breaking**]`, with the webui pin bump in the same commit or a following `chore: pin webui`.

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T005)** → user stories.
- **US2 (T006–T011)** depends only on Foundational. T010 and T011 are in different files and can run alongside T008/T009.
- **US1 (T012–T021)** depends on Foundational. T007 and T020 both edit `src/agents/tools/mod.rs`, so do US2's T007 before T020 (or merge them). T012 → T016 (transient), T013 → T014 → T016 (deadline), T015 → T016 → T017/T018 → T019 → T020.
- **US3 (T022–T023)** depends on T006 and T019, since it checks their descriptions.
- **US4 (T024–T028)** depends on nothing server-side except T027 (needs T011). T024–T026 can start any time.
- **Polish** after all stories. T036 needs everything, and T037 is last.

### Parallel opportunities

- T010 ∥ T011 ∥ T021 (different files).
- T024 ∥ T025 ∥ T026 (WebUI, independent of the Rust work).
- T029 ∥ T030 ∥ T031.
- US4's WebUI tasks can run alongside Phases 3–5.

### Parallel example: US4 while US1 is in progress

```text
Agent A: T015 → T016 → T017 → T018 (src/agents/background/wait.rs)
Agent B: T024, T025, T026 (webui/app/…)
```

## Implementation Strategy

1. **Foundational + US2 first**: a pure refactor and rename, verified by re-running spec 012's quickstart under the new name. This is shippable on its own, since only the name changes.
2. **US1 (MVP of the new capability)**: the blocking tool, verified by quickstart §2–§8.
3. **US3**: guidance, so models use the new tool well.
4. **US4 + Polish**: WebUI labels, docs, usage label, #82, the e2e walk, then one breaking commit (plus the webui pin).

## Notes

- Baseline (T001): `cargo clippy` passed with pre-existing warnings, `cargo test` passed (589 tests), and `cd webui && npm run typecheck` passed. No baseline failures.
- Dummyplug e2e (T036), isolated on port 10025 with a scratch workspace: verified the agents-category listing; ordered blocking results; concurrent execution; task-aligned timeout; no background job or report; retained piece history; the 15s tool-deadline refusal; `/abort` stopping the parent and piece; restart resuming the parent but not the transient piece; the shared depth-3 refusal; the renamed background tool's immediate acknowledgement, ordered report and cancellation; and both retired-name answers. The WebUI trail/labels were covered by 125 passing Vitest tests and typecheck. The explicitly advisory live-model check was not run.
- E2E clarification: nested `execute_python` requests need the required `intent` field, so quickstart.md's helper now includes it. The sandbox's standard catchable-tool-error prefix is visible on the retired-name `RuntimeError`; the contract now states this explicitly while keeping the hint text itself unchanged.
- Cancellation support: the sandbox bridge now cancels an in-flight `parallel_subagents` call when the outer script is dropped, so the blocking run's drop guard aborts unfinished pieces immediately on `/abort` or agent stop.
- Tool names are dispatch keys: never add `paralel_subtasks` back as an alias.
- Keep every `VizierRequest` literal on `..Default::default()` so `transient` defaults to `false` everywhere except blocking pieces.
