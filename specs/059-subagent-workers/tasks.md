---
description: "Task list for 059-subagent-workers"
---

# Tasks: Subagents as temporary workers

**Input**: Design documents from `/specs/059-subagent-workers/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: included. The constitution requires `cargo test` to pass, CLAUDE.md requires one `fence_tests.rs` row per fenced storage method, and the plan names the unit tests each story needs. End-to-end checks are the dummyplug steps in [quickstart.md](quickstart.md).

**Organization**: tasks are grouped by user story. Story numbers follow [spec.md](spec.md); phases follow priority (US1, US2, US6 are P1; US3, US4 are P2; US5 is P3). All paths are relative to the repository root; `core/` below means `crates/vizier-core/src/`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: the user story the task belongs to (US1–US6)

---

## Phase 1: Setup

- [X] T001 Confirm the branch is `059-subagent-workers` and the workspace builds and tests green before changes: `cargo build` and `cargo test -p vizier-core subagent` (records the baseline the retired tool's tests start from)

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the shared types, storage, config and delivery seam every story uses.

- [X] T002 [P] Create `crates/vizier-core/src/schema/subagent.rs`: `SubagentId` (`s_` + 12 base32 chars, generated like `DetachedScriptId`), `SubagentState` (`running|asking|answered|failed|timed_out|stopped`, `is_final`, serde snake_case), `SubagentStopReason` (`by_agent|conversation_deleted|agent_deleted`), `Subagent` (every column of the `subagent` table in data-model.md), `SubagentSnapshot` (the tool summary plus `group`, `hiring_turn`, `parent_detached_id`, `updated_at`; `task` cut to 200 chars, `result` to 500), and `Subagent::snapshot()`; export from `crates/vizier-core/src/schema/mod.rs`
- [X] T003 [P] Add `SubagentsConfig { max_depth: u8 }` (serde default `1`, `Default` impl) and the field `subagents: SubagentsConfig` with `#[serde(default)]` on `VizierConfig`, beside `lsp`, in `crates/vizier-core/src/config/mod.rs`; add a unit test that a config without the key yields `max_depth == 1`
- [X] T004 Add `subagent_depth: u8` (`#[serde(default)]`) to `VizierRequest` in `crates/vizier-core/src/schema/request.rs`, carry it in `crates/vizier-core/src/transport/wire.rs` exactly as `background_depth` is, and add `subagent_depth: u8` to `ToolContext` in `crates/vizier-core/src/agents/tools/mod.rs`, set from the request where `background_depth` is set in `crates/vizier-core/src/agents/agent/mod.rs` (~595, ~617, ~771); fill `0` at every other `ToolContext` literal the compiler lists
- [X] T005 Add `VizierRequestContent::SubagentTask(SubagentBrief { subagent_id, position, group_size, text })` and `VizierRequestContent::SubagentReply { subagent_id, text }` in `crates/vizier-core/src/schema/request.rs`: `kind()` = `subagent_task` / `subagent_reply`, `Display` = the text, both durable and both counted by `records_session_activity` (`crates/vizier-core/src/agents/process.rs`); extend the `records_session_activity` test list
- [X] T006 Add `SessionFrame::Subagent(SubagentSnapshot)` (socket frame name `subagent`) in `crates/vizier-core/src/schema/session.rs` and make sure the WebSocket topic handler in `crates/vizier-http` forwards it like `DetachedScript` (filtering on its own session)
- [X] T007 Create the `SubagentStorage` trait in `crates/vizier-core/src/storage/subagent.rs` with every method in the data-model.md table (doc comment on each saying fenced or not), the `VizierStorage` delegation impl, and add it to `VizierStorageProvider` in `crates/vizier-core/src/storage/mod.rs`
- [X] T008 Create the SQLite schema init for the `subagent` table and its three indexes in `crates/vizier-core/src/storage/sqlite/subagent.rs` (idempotent `CREATE TABLE IF NOT EXISTS`, called from the schema init in `crates/vizier-core/src/storage/sqlite/mod.rs`) and the PostgreSQL migration `crates/vizier-core/src/storage/sql/postgres/migrations/0009_subagents.sql` with the same columns
- [X] T009 Implement every `SubagentStorage` method on `SqlStorage` in `crates/vizier-core/src/storage/sqlite/subagent.rs` over `SqlConn`/`Dialect` (shared by both backends): fenced methods call `check_fence` on `agent:{agent_id}` in their own transaction; every transition is one guarded `UPDATE … WHERE state IN (…)`; `settle` and `ask` increment `outcome_seq`; `mark_seen` and `claim_subagent_deliveries` set `seen_seq`/`delivered_seq = outcome_seq` only where still deliverable (data-model.md "Deliverability")
- [X] T010 [P] Unit tests for the storage in `crates/vizier-core/src/storage/sqlite/subagent.rs` (`#[cfg(test)]`): every allowed transition, every refused one (reply to non-asking, settle of a stopped row, stop of a final row returns `AlreadyFinished`), `outcome_seq` after two asks, a claim and `mark_seen` racing on one outcome hand it out once, `stopped` never deliverable, list order (running and asking first, then the last N settled)
- [X] T011 [P] Add one row per fenced `SubagentStorage` method to `crates/vizier-core/src/storage/sqlite/fence_tests.rs` (refused with `OwnershipLost` under a newer owner; allowed with no fence)
- [X] T012 Create `crates/vizier-core/src/agents/delivery.rs`: trait `DeliverySource` (`async fn claim(&self, session) -> Vec<String>` rendered sections; `async fn sessions_with_deliveries(&self, agent) -> Vec<VizierSession>`), struct `Deliveries` holding `Vec<Arc<dyn DeliverySource>>` with `claim(session) -> Option<String>` (sections joined, `None` when empty) and `poke(session, depth)` (moved from `DetachedScripts`' private `poke`, same retry-while-unregistered behaviour); add `deliveries: Deliveries` to `VizierDependencies` in `crates/vizier-core/src/dependencies.rs`
- [X] T013 Make `DetachedScripts` implement `DeliverySource` in `crates/vizier-core/src/agents/detached/mod.rs` (claim → `render_delivery`, `deliverable_script_sessions`) and route its pokes through `deps.deliveries.poke`; switch the three claim sites to `deps.deliveries.claim(&session)`: the round boundary in `crates/vizier-core/src/agents/agent/mod.rs` (~1555), the idle poke (~187) and the queued poke (~1500) in `crates/vizier-core/src/agents/process.rs`; widen the doc comment on `VizierRequestContent::DetachedDelivery` to "background delivery (detached scripts and subagents)"; `cargo test -p vizier-core detached` must still pass unchanged
- [X] T014 Make `recover_agent` in `crates/vizier-core/src/recovery.rs` poke every session returned by any delivery source through `deps.deliveries` (detached's own interrupt step stays where it is)

**Checkpoint**: the workspace builds, spec 058's tests pass through the new seam, the table exists on both backends.

---

## Phase 3: User Story 1 - A worker knows it is a worker (Priority: P1) 🎯 MVP

**Goal**: a worker's prompt and request tell it who it works for, who reads its answer and what shape the answer takes (FR-003–FR-007).

**Independent Test**: quickstart step 2 — spawn one worker (with the existing `parallel_subagents`, which still exists at this point) whose task is `context`; its frontmatter says `via: subagent`, `task: 1 of 1`, and its prompt opens with the worker framing and carries the soul under "The agent you work for".

- [X] T015 [P] [US1] Create `crates/vizier-core/src/agents/agent/system_prompt/worker.rs` with `worker_md(name, description)` (contracts/worker-turn.md item 1, including the report shape and the `ask_main_agent` sentence), `employer_identity_md(identity_block, name)` (`## The agent you work for` + the block) and `served_owner_md(owner_text, name)` (`## The person <name> serves` + the text); unit tests that each is byte-stable and that the identity block is embedded unchanged; register the module in `crates/vizier-core/src/agents/agent/system_prompt/mod.rs`
- [X] T016 [US1] Branch `prepare_system_prompts` in `crates/vizier-core/src/agents/agent/mod.rs` on `session.1 == VizierChannelId::Subagent`: worker turns get `worker_md` instead of `boot_md`, no configured `system_prompt`, `served_owner_md` instead of `owner_md`, `employer_identity_md` around `identity_block()`, then project and documents as today (contracts/worker-turn.md order); main turns are byte-identical to before; unit test both layouts
- [X] T017 [US1] Add the frontmatter branch for `SubagentTask` and `SubagentReply` to `generate_frontmatter` in `crates/vizier-core/src/schema/request.rs` (`sender`, `via: subagent|subagent_reply`, `subagent`, `task: "N of M"` for a task, `sent_at`, `metadata`); unit tests for both
- [X] T018 [US1] Route `SubagentTask` and `SubagentReply` into the `Chat` arm of `handle_request` and `chat_turn_prompt` in `crates/vizier-core/src/agents/process.rs` (same history, hooks, steps and resume; auto-context passages from the task text; no `chat_replied` notification)
- [X] T019 [US1] Change `Subagents::dispatch_piece` in `crates/vizier-core/src/agents/subagents.rs` to send `SubagentTask` (position and group size from the call) instead of `Prompt`, so the existing tool exercises the new framing; keep everything else of the old path for now
- [X] T020 [US1] Add `vizier.subagent.id` and `vizier.subagent.position` attributes to the `invoke_agent` span when the request is a `SubagentTask`/`SubagentReply` (`VizierAgent::invoke_agent_span` in `crates/vizier-core/src/agents/agent/mod.rs`)

**Checkpoint**: quickstart step 2 passes; main-turn prompts unchanged.

---

## Phase 4: User Story 2 - The main agent hires, waits for and stops workers (Priority: P1)

**Goal**: handles instead of a blocking fan-out (FR-008–FR-013, FR-015, FR-016).

**Independent Test**: quickstart steps 3 and 4.

- [X] T021 [US2] Add `ToolCategory::Subagents` (name `subagents`, description "Hire temporary workers for parts of a task, wait for them, answer their questions and stop them") to `ToolCategory` and `ALL` (now 20) in `crates/vizier-core/src/agents/tools/mod.rs`; change the `Agents` description to "See and stop your background work"
- [X] T022 [US2] Rewrite the service in `crates/vizier-core/src/agents/subagents.rs`: delete `run_and_wait`, `WaitGuard`, `first_terminal`, `wait_limit`; keep `validate_timeout`, `truncate`, `classify`, `project_scope`; add `spawn(ctx, tasks, timeout) -> group + rows` (open rows in one transaction, then send one durable `SubagentTask` per row with `transient: false`, no reply sender, `subagent_depth = ctx.subagent_depth + 1`, `background_depth + 1`, `project_scope`, `parent_detached_id`), `settle(session, Settled)`, `stop(ctx, id)` (guarded stop then `Command("abort")` to the worker session), `get`/`list` scoped to `ctx.session` (hiring session), and `publish(row)` sending `SessionFrame::Subagent` on the hiring session after every committed change
- [X] T023 [US2] Add the per-worker timer to `Subagents` in `crates/vizier-core/src/agents/subagents.rs`: a task per running row sleeping until `deadline_at`; on expiry `settle(timed_out)` guarded from `running`, then abort the worker session; cancelled when the row leaves `running`
- [X] T024 [US2] Record outcomes agent-side in `crates/vizier-core/src/agents/process.rs`: after `agent.chat` returns in a `Subagent` session call `deps.subagents.settle(&session, Answered(text, truncated))` (guarded from `running`, so an `asking` row is untouched); in the `join_next` arm, for a `Subagent` session whose turn ended `Panicked` or whose `handle_request` returned `Err`, call `settle(Failed(reason))`
- [X] T025 [US2] Implement `wait(ctx, ids, until)` on `Subagents` in `crates/vizier-core/src/agents/subagents.rs`: subscribe to session events first, read the rows, return when met; otherwise re-read on every `SessionFrame::Subagent` for the hiring session until `ctx.deadline`; mark seen every settled or asking row it returns (`mark_seen`); never changes a worker
- [X] T026 [US2] Rewrite `crates/vizier-core/src/agents/tools/subagents.rs` with `spawn_subagents`, `wait_subagents`, `stop_subagent`, `list_subagents`, `get_subagent` per contracts/subagent-tools.md (inputs with `schemars` descriptions, the summary shape, `status` refusals `invalid_input`/`unknown_subagent`/`already_finished`), category `Subagents`, and `available(ctx)` = `ctx.subagent_depth < deps.config.subagents.max_depth` and not a dream session; delete `ParallelSubagentsTool`
- [X] T027 [US2] Register the five tools on `default_toolset` in `BaseTools::new()` in `crates/vizier-core/src/agents/tools/mod.rs` (replacing `ParallelSubagentsTool`), update `every_tool_is_in_its_category` and the catalogue `entry::<…>()` list, and keep them out of `DREAM_TOOL_NAMES`
- [X] T028 [P] [US2] Unit tests in `crates/vizier-core/src/agents/subagents.rs` (or a `tests` submodule): spawn returns at once in order; wait `any`/`all` met and not met (deadline returns current states, workers untouched); stop one leaves the others running; stop of a settled worker; an unknown or another conversation's id is `unknown_subagent`; timeout moves `running → timed_out` and aborts the session
- [ ] T029 [US2] Run quickstart steps 3 and 4 against a dummyplug agent and record the results in the PR description

**Checkpoint**: the main agent manages workers; `parallel_subagents` no longer exists (its guidance lands in US5).

---

## Phase 5: User Story 6 - Work survives the turn that started it (Priority: P1)

**Goal**: workers live as long as their conversation; unseen outcomes are delivered once (FR-017–FR-017c, FR-024–FR-026).

**Independent Test**: quickstart steps 6, 7, 8 and 11.

- [X] T030 [US6] Implement `DeliverySource` for `Subagents` in `crates/vizier-core/src/agents/subagents.rs` (`claim` → `claim_subagent_deliveries` → one `[Subagent update]` section per row per contracts/worker-turn.md, published as frames; `sessions_with_deliveries` → `subagent_delivery_sessions`), register it after detached in `Deliveries` (`crates/vizier-core/src/dependencies.rs`), and poke the hiring session through `deps.deliveries.poke` after every settle and ask
- [X] T031 [P] [US6] Create the renderer `render_subagent_update(row)` with unit tests (answered with a cut result, failed/timed_out with `error:`, asking with the reply hint, task title cut to 120 chars) in `crates/vizier-core/src/agents/subagents.rs` or a `delivery` submodule beside it
- [X] T032 [US6] Add `recover_agent(agent, below)` to `Subagents` and call it from `crates/vizier-core/src/recovery.rs` as step 7 (after turns and detached): re-own and re-arm timers for `running` rows under an older fence (their `session_turn` is resumed by step 5); `asking` rows need nothing
- [X] T033 [US6] Stop workers on deletion: in the conversation-deletion path (`SessionFiles::delete_conversation` / `delete_session`, wherever detached's `delete_session` hook is called) call `Subagents::delete_session(hiring session)` → `stop_session_subagents(conversation_deleted)`, abort each worker session, delete each worker topic's history through the existing session deletion, then `delete_session_subagents`; in `delete_agent` call `delete_agent_subagents`; files in `crates/vizier-core/src/agents/subagents.rs`, `crates/vizier-core/src/session_files/` and the agent-deletion path in `crates/vizier-core/src/storage/sqlite/agent.rs` (or its service caller)
- [X] T034 [US6] Verify nothing aborts workers any more: the parent's `/abort` (`abort_session_notify` in `crates/vizier-core/src/agents/process.rs`), a cancelled or timed-out detached script (`crates/vizier-core/src/agents/detached/mod.rs`), and an instance stop (update/restart) must not touch `Subagent` sessions other than resuming them; add a test that aborting the hiring session leaves a worker `running`
- [X] T035 [P] [US6] Tests: a worker settling while the hiring session is idle produces exactly one poke turn with the section; settling mid-turn is claimed at the round boundary and the queued poke finds nothing; an outcome already returned by `wait_subagents` is not delivered; a stopped worker is never delivered (in `crates/vizier-core/src/agents/subagents.rs` tests, using the existing transport test helpers)
- [ ] T036 [US6] Run quickstart steps 6, 7, 8 and 11 and record the results

**Checkpoint**: interrupting or restarting loses no worker outcome.

---

## Phase 6: User Story 3 - A worker asks, the main agent replies (Priority: P2)

**Goal**: FR-011, FR-014, FR-017b.

**Independent Test**: quickstart step 5.

- [X] T037 [US3] Create `AskMainAgentTool` (`ask_main_agent {question}`, ≤ 4 KiB, category `Subagents`, `ends_turn() = true`, `available(ctx)` = `ctx.session.1 == VizierChannelId::Subagent`) in `crates/vizier-core/src/agents/tools/subagents.rs`; its call writes `ask_subagent(id, question, remaining_ms)` for the row of `ctx.session` (`subagent_by_worker`), publishes, pokes the hiring session, and returns `{"status": "asked"}`; register it on `default_toolset` and add it to `every_tool_is_in_its_category`
- [X] T038 [US3] Add `reply(ctx, id, text)` to `Subagents` (`reply_subagent` storage call with `deadline_at = now + remaining_ms`, re-arm the timer, send a durable `SubagentReply` to the worker session) and the `reply_subagent` tool (`not_asking` refusal naming the state) in `crates/vizier-core/src/agents/subagents.rs` and `crates/vizier-core/src/agents/tools/subagents.rs`; register it with the other management tools
- [X] T039 [P] [US3] Tests: ask → `asking` with the question and no running timer; reply → `running` with the remaining time; a second ask increments `outcome_seq` and is deliverable again; reply to a non-asking worker is refused; `ask_question`/`ask_sensitive_input` are unavailable in a worker turn (extend the existing availability tests in `crates/vizier-core/src/questions/`)
- [ ] T040 [US3] Run quickstart step 5 and record the result

---

## Phase 7: User Story 4 - Only the main agent hires (Priority: P2)

**Goal**: FR-018, FR-023.

**Independent Test**: quickstart step 9.

- [X] T041 [US4] Test in `crates/vizier-core/src/agents/tools/mod.rs` (category/catalogue tests): with the default config a `ToolContext` with `subagent_depth = 1` sees only `ask_main_agent` in category `subagents`; depth 0 sees the six management tools and not `ask_main_agent`; with `max_depth = 2`, depth 1 sees the six as well; a detached script launched from a worker inherits `subagent_depth` (check `crates/vizier-core/src/agents/detached/mod.rs` copies it onto the task's `ToolContext`; add the copy if missing)
- [X] T042 [US4] Make `subagents` a reserved category name: confirm the MCP/client naming (`ToolRouter::catalogue()`, `client_category_name`) treats every `ToolCategory::ALL` name as reserved, and add a test that an MCP server keyed `subagents` is named `subagents_mcp`, in `crates/vizier-core/src/agents/tools/mod.rs`
- [ ] T043 [US4] Run quickstart step 9 (default and `max_depth: 2` in `dev.vizier.yaml`) and record the result

---

## Phase 8: User Story 5 - The old fan-out tool is retired with guidance (Priority: P3)

**Goal**: FR-020–FR-022.

**Independent Test**: quickstart step 10.

- [X] T044 [US5] In `crates/vizier-core/src/agents/tools/mod.rs` add `parallel_subagents` to `RETIRED_TOOL_NAMES`, rewrite the `paralel_subtasks` hint and `BACKGROUND_REPLACED` to the texts in contracts/subagent-tools.md "Retired names", and add a test that no hint names a retired tool as the replacement
- [X] T045 [US5] Rewrite the "Large tasks" section of `crates/vizier-core/src/agents/agent/system_prompt/boot.rs` per FR-022 (workers are temporary helpers who know only their task, act with your tools, cannot talk to the person, can ask you with `ask_main_agent`, cannot hire workers; give each the goal, the context and the answer's shape; `spawn_subagents` then `wait_subagents`; results you did not wait for arrive as `[Subagent update]`), and fix the "Background work" paragraph that mentions a "subagent fan-out" to name `spawn_subagents`
- [ ] T046 [US5] Add checklist entries to #82 (https://github.com/vizier-lab/vizier/issues/82) for the new `parallel_subagents` row in `RETIRED_TOOL_NAMES` and the rewritten `paralel_subtasks`/`BACKGROUND_REPLACED` hints — draft the text in the PR description and ask before posting
- [ ] T047 [US5] Run quickstart step 10 and record the result

---

## Phase 9: Polish and cross-cutting

- [X] T048 [P] Update `CLAUDE.md`: replace every `parallel_subagents` mention (Detached scripts, Tools, Telemetry, project subagent scope) with the worker model, add a short "Subagents" paragraph under Agents (worker framing, the seven tools, the `subagent` row, durable worker turns, delivery through `Deliveries`, `subagents.max_depth`), and list `subagents` among the built-in categories
- [X] T049 [P] Update the docs in `docs/src/` that mention subagents or `parallel_subagents` (find them with a search) and document `subagents.max_depth` beside `lsp` in the configuration docs
- [X] T050 [P] Update `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md` only if a quickstart step needed a protocol change (expected: none)
- [ ] T051 Run quickstart step 12 (**live**, real model) and record whether worker reports avoid addressing a person (SC-001)
- [ ] T052 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, and the PostgreSQL storage tests with `VIZIER_TEST_DATABASE_URL`; fix anything they report
- [ ] T053 Open the PR titled `feat(agents)!: subagents as temporary workers`, with the quickstart results and the #82 draft in the description

---

## Dependencies and execution order

- **Phase 1 → Phase 2** → every story.
- **US1 (Phase 3)** needs only Phase 2 and can ship alone on the old tool (MVP).
- **US2 (Phase 4)** needs Phase 2; it replaces the tool US1 used (T019 is superseded by T022's spawn).
- **US6 (Phase 5)** needs US2 (rows, settle, spawn).
- **US3 (Phase 6)** needs US2 (and US6 for delivering a question when nobody waits).
- **US4 (Phase 7)** needs US2 and US3 (both tool sets exist).
- **US5 (Phase 8)** needs US2 (the replacement tools exist).
- **Polish** last.

Within Phase 2: T002, T003 in parallel; T004 → T005 → T006 (same files); T007 → T008 → T009 → T010, T011 in parallel; T012 → T013 → T014.

## Parallel examples

- Phase 2: T002 (schema/subagent.rs) with T003 (config) with T012 (delivery.rs).
- After T009: T010 (storage tests) with T011 (fence rows).
- US1: T015 (worker.rs) while T017 (frontmatter) is written.
- US6: T031 (renderer) with T032 (recovery).
- Polish: T048, T049, T050 together.

## Implementation strategy

1. **MVP**: Phases 1–3. Workers understand they are workers, on the existing fan-out tool. Ship-ready on its own if the rest slips.
2. **Handles**: Phase 4. The main agent spawns, waits and stops; the old tool is gone (guidance follows in Phase 8, so land Phases 4 and 8 in the same PR).
3. **Survival**: Phase 5. Interrupts and restarts lose nothing.
4. **Conversation and limits**: Phases 6–7.
5. **Retirement, docs, PR**: Phases 8–9. One PR, breaking.
