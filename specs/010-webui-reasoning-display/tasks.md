---
description: "Task list for WebUI reasoning & tool activity display"
---

# Tasks: WebUI Reasoning & Tool Activity Display

**Input**: Design documents from `/specs/010-webui-reasoning-display/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Included, but narrowly. The plan names three pure, cheaply-testable pieces — history entry
round-trip, `seq` assignment/ordering, and the WebUI history→turns grouping — and tests are written for
those only. This is not a blanket TDD pass; the rest of the feature is verified end to end through
`quickstart.md`, per the constitution's dummyplug gate.

**Organization**: Grouped by user story. US1 is the MVP and the only story that is not optional.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: `[US1]`, `[US2]`, `[US3]` — user story phases only
- Exact file paths included in every task

## Path Conventions

Single Rust binary with a bundled React WebUI. Backend under `src/`, WebUI under `webui/app/`. No new
top-level directory; one new WebUI module (`webui/app/lib/trail.ts`).

---

## Phase 1: Setup

**Purpose**: Establish a known-good baseline so later failures are attributable. Nothing to scaffold —
this is an existing project and the feature adds no dependency.

- [X] T001 Confirm a clean baseline before touching anything: `just install`, then `cargo build`, `cargo clippy`, `cargo test`, and `cd webui && npm run typecheck`. Record any pre-existing failure so it is not later mistaken for a regression. Note `build.rs` shells out to `npm run build` whenever `webui/node_modules/` exists, so `just install` must come first.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The ordering fix. Everything downstream renders stored history, and building UI against
unordered data makes every rendering bug indistinguishable from an ordering bug.

**⚠️ CRITICAL**: No user story work should begin until this phase is complete. (US3 could technically
proceed without it — it is pure subtraction — but US1 and US2 cannot.)

**Reference**: [contracts/history-api.md](./contracts/history-api.md) §1, research Decisions 1 and 2.

- [X] T002 Add `seq INTEGER` (nullable) to the `session_history` DDL and `CREATE INDEX IF NOT EXISTS idx_sh_seq ON session_history(seq);` in the schema batch in `src/storage/sqlite/mod.rs` (~line 200). Nullable is deliberate — FR-006 forbids a backfill.
- [X] T003 Add an `add_column_if_missing(conn, table, column, decl) -> crate::Result<()>` helper in `src/storage/sqlite/mod.rs` that reads `PRAGMA table_info(<table>)` and issues `ALTER TABLE … ADD COLUMN` only when the column is absent; call it for `session_history.seq` right after the schema batch. This is the project's **first** column addition to an existing table — `grep -rn "ALTER TABLE\|table_info\|user_version" src/` returns nothing today. Return a real `VizierError` on failure rather than swallowing SQLite's "duplicate column name" (Principle V).
- [X] T004 Assign `seq` on insert in `save_session_history` in `src/storage/sqlite/history.rs` (~line 61) as `(SELECT IFNULL(MAX(seq), 0) + 1 FROM session_history)` within the existing `INSERT`. The surrounding `self.conn.lock()` already serializes the read-then-write. Confirm `update_history_reactions` (~line 148) does not touch `seq` (H3).
- [X] T005 Fix the ordering defect in `list_session_history` in `src/storage/sqlite/history.rs`: change `ORDER BY timestamp DESC` (line ~114) to `ORDER BY timestamp DESC, seq DESC` **and** change the following `list.sort_by_key(|a| a.timestamp)` (line ~144) to key on `(timestamp, seq)`. Both halves are required — `sort_by_key` is a **stable** sort, so keying on timestamp alone preserves the descending order it was handed for every tie group, which is the actual bug (H6).
- [X] T006 Apply the same tie-break to the four remaining ordered reads in `src/storage/sqlite/history.rs`: the stats-variant `list` (~line 359 `ORDER BY` + ~line 371 re-sort), the agent-wide range read (~line 383), the checkpoint lookup (~line 442), and `list_session_history_until_checkpoint` (~line 492, which orders `ASC` so the tie-break is `seq ASC`).
- [X] T007 Add `before_seq: Option<i64>` to `HistoryQuery` in `src/channels/http/api/v1/agents/channel.rs` (~line 54) and extend the predicate to `timestamp < :before OR (timestamp = :before AND seq < :before_seq)` when both are supplied, leaving today's behaviour when only `before` is given (H9, H10). Note this path is not exercised by the WebUI, which requests history with no paging parameters — it is API correctness, not a visible bug.
- [X] T008 [P] Add the nullable `seq` field to the session-history entry type in `webui/app/interfaces/types.ts` so the WebUI can read it (H13).
- [X] T009 [P] Unit test in `src/storage/sqlite/history.rs` against an in-memory connection, following the test pattern in `src/storage/memory.rs`: inserting N entries in a tight loop yields strictly increasing `seq` with no duplicates, and reading them back returns them in insertion order even though they share a `timestamp` (H1, H5, SC-002).
- [X] T010 [P] Unit test in `src/storage/sqlite/history.rs`: rows with `seq IS NULL` (simulating pre-existing history) are returned without error, and a read over a mix of `NULL` and non-`NULL` `seq` orders the non-`NULL` ones correctly among themselves (H7, H8, FR-007, FR-008, SC-004).

**Checkpoint**: History is reliably ordered. User story work can begin.

---

## Phase 3: User Story 1 - Reasoning and tool activity survive the response (Priority: P1) 🎯 MVP

**Goal**: The trail stays attached to its turn after the answer arrives, folded shut, and is still there
after a reload.

**Independent Test**: Send a message that makes the agent think and call a tool. When the answer lands,
the trail is still present above it, folded; expanding shows what was visible live; reloading the page
keeps it. Delivers the whole point of the feature with no python-specific work done.

**Reference**: [contracts/trail-model.md](./contracts/trail-model.md), [contracts/history-api.md](./contracts/history-api.md) §3, [data-model.md](./data-model.md) replay state transition.

### Tests for User Story 1

> Write these first. T012 in particular should fail against the current code in a way that names the
> wrong message shape — that failure is the whole reason narration was filtered out originally.

- [X] T011 [P] [US1] Test in `src/schema/history.rs` that `messages_to_history_entries` emits the `AssistantMessage` entry **before** the `ToolCall` entries from the same assistant message (FR-011, H14 precondition).
- [X] T012 [P] [US1] Test in `src/schema/history.rs` for the replay shape: history containing `AssistantMessage` → `ToolCall` → `ToolResult` must replay as exactly **one** `Message::Assistant` whose content is `[Text, ToolCall]`, followed by one `Message::User` of `ToolResult`s. Assert explicitly that no two consecutive `Message::Assistant` are emitted and that every `ToolResult` is immediately preceded by the assistant message carrying its `ToolCall` (H14, H15, H16).
- [X] T013 [P] [US1] Regression test in `src/schema/history.rs`: history containing **no** `AssistantMessage` replays to a message sequence identical to today's. This is the guard that the change is invisible for every conversation recorded before it (H17).
- [X] T014 [P] [US1] Unit tests for the pure grouping function in `webui/app/lib/trail.test.ts`: entry order in equals event order out with no reordering (T1); an empty trail yields no trail element (T2); adjacent thoughts merge (T3); an entry with no open turn yields a turn with `request: undefined` (T11); an unrecognised entry kind is skipped without throwing (T8).

### Implementation for User Story 1

> **T015 and T016 must land in one commit.** T015 alone produces history that a provider will reject on
> the next turn: it fixes adjacency but emits two consecutive assistant messages. Neither is separately
> shippable.

- [X] T015 [US1] In `messages_to_history_entries` in `src/schema/history.rs` (~lines 70-94), restructure the `Message::Assistant` arm to collect text and tool calls first, then push the `AssistantMessage` entry, then the `ToolCall` entries — reversing today's order, where `ToolCall`s are pushed inside the content loop and the `AssistantMessage` only after it. Keep the existing `has_tool_calls` branch that decides `AssistantMessage` (intermediate) versus `Response` (final).
- [X] T016 [US1] In `history_entries_to_messages` in `src/schema/history.rs` (~line 117), add a `pending_text: Option<String>` accumulator; make the `AssistantMessage` arm (~line 155) **set it instead of flushing and pushing its own message**; and extend `flush_pending_tool_calls` (~line 201) to prepend `AssistantContent::Text(pending_text)` to the content vector so narration and its tool calls emit as one assistant message. Land with T015.
- [X] T017 [US1] Remove the two `tool_entries.retain(|e| !matches!(e, SessionHistoryContent::AssistantMessage(_)))` filters on the write path in `src/agents/agent/mod.rs` (~line 459 on the error path, ~line 495 on the success path) so narration is recorded (FR-010).
- [X] T018 [US1] Remove the `AssistantMessage` filter on the read path in `get_topic_history` in `src/channels/http/api/v1/agents/channel.rs` (~line 137) so narration reaches the WebUI (H11). Confirm no other entry kind is newly dropped (H12).
- [X] T019 [P] [US1] Add the `TrailEvent` union and `Turn` interface to `webui/app/interfaces/types.ts` exactly as specified in [data-model.md](./data-model.md). Note there is deliberately **no** variant for a nested tool call — T10 is structural, not a filter.
- [X] T020 [US1] Create `webui/app/lib/trail.ts` with the two producers from [contracts/trail-model.md](./contracts/trail-model.md) §2: pure `groupHistory(entries) -> Turn[]` (it must not sort — the storage layer already ordered the input) and `liveEvent(content) -> TrailEvent | null`. Route `execute_python` tool calls to the `python` kind with `intent` read from the arguments, falling back to `null` when absent — which is the case until US2 lands, so the fallback label is exercised from day one. Pair reports by `call_id` from history and positionally from the live stream.
- [X] T021 [US1] Create `webui/app/components/ActivityTrail.tsx` — one renderer for both producers (T12). A native `<details>`/`<summary>`, as `ExecutionReportView` already uses: folded when `live: false`, expanded when `live: true` (T4), each turn's state independent (T5), summary line bounded to two lines regardless of event count (T6, FR-018) showing counts by kind plus the turn duration from `VizierResponseStats.duration` on the closing `Response` (omitted when absent).
- [X] T022 [US1] Rewrite `webui/app/components/ThinkingIndicator.tsx` to build `TrailEvent[]` via `liveEvent` and render them through `ActivityTrail` instead of its own inline `InlineEvent` switch, keeping the thinking-word animation and the Abort button. Delete the now-redundant local `InlineEvent` type.
- [X] T023 [US1] In `webui/app/routes/chat.tsx`, stop discarding the trail when a turn closes: on the `message` and `checkpoint` branches (~lines 665, 688) attach the collected events to the turn being closed instead of calling `clearInlineEvents()`. Keep `clearInlineEvents()` on the `abort` branch (~line 633) — an aborted turn has no answer for a trail to belong to (FR-022).
- [X] T024 [US1] In `webui/app/routes/chat.tsx`, render stored trails on load: pass the fetched history through `groupHistory` and render each turn's trail via `ActivityTrail`. Note `historyVersion` (~line 266) is `useState(0)` with no setter, so history never refetches after mount — the post-answer trail must come from the events already collected in T023, not from a refetch (research Decision 5).
- [X] T025 [US1] Remove the now-dead `execute_python` case from `formatToolChoice` in `webui/app/routes/chat.tsx` (~line 110), which today interpolates `args.code` into the live stream. Python runs render through the `python` TrailEvent from T020 and no longer pass through this function.

**Checkpoint**: The trail persists, folds, survives a reload, and includes narration. US1 is independently shippable. Python entries render with a fallback label and today's report view — US2 improves that.

---

## Phase 4: User Story 2 - A python run reads as an intent (Priority: P2)

**Goal**: The agent states why it ran a script, and that sentence is what the person reads; the code is
one disclosure deeper.

**Independent Test**: Give an agent with the python tool enabled a task needing computation. The python
entry leads with a plain-language sentence; the script and output are reachable but not shown by
default.

**Reference**: [contracts/execute-python-input.md](./contracts/execute-python-input.md).

- [X] T026 [P] [US2] Add a required `intent: String` field to `ExecutePythonInput` in `src/agents/tools/python/mod.rs` (~line 40) with a `#[schemars(description = …)]` telling the agent it is one short sentence for the person watching, in place of the code, with an example. `String` not `Option<String>` so an omitting call fails deserialization and returns an error naming the field (FR-025, C1, C3). Do **not** add `intent` to `ExecutionReport` — it is already persisted in the tool call's arguments, and duplicating it spends tokens echoing the model's own input (C5, research Decision 6).
- [X] T027 [P] [US2] Confirm `intent` is never passed to the sandbox in `src/agents/tools/python/mod.rs::call` — only `args.code` reaches `sandbox::execute`, and `intent` must not become a script variable or affect limits (C4).
- [X] T028 [US2] Rework `webui/app/components/ExecutionReportView.tsx` so the stated intent is the always-visible `<summary>` text and the script source moves into the disclosure alongside Output and Result/Error (P1, P4, FR-027 to FR-029). Keep success/failure evident while collapsed (P2, FR-030), truncate an over-long intent in the collapsed form with the full text available expanded (P3, FR-031), and render a neutral fallback label when `intent` is `null` (T9, FR-032).

**Checkpoint**: Python runs read as intents. US1 and US2 both work independently.

---

## Phase 5: User Story 3 - The script's own tool calls stay out of the transcript (Priority: P3)

**Goal**: A script's nested tool calls do not appear in the WebUI, while the agent's own view of them is
unchanged.

**Independent Test**: Run a script that calls a tool several times in a loop. No per-call list appears
anywhere in the transcript, and none stream live either, while the agent still receives the same
information.

**Reference**: research Decision 4. Note this is **two** fixes — dropping the report's list is not
sufficient, because `RouterBridge::dispatch` runs every nested call through `on_tool_call` and
`ToolCallsHook` emits a `ToolChoice` frame for each, so a twelve-iteration loop streams twelve frames
live today.

- [X] T029 [US3] Add `async fn on_nested_tool_call(&self, function_name: String, args: String) -> Result<(String, String)>` to the `VizierSessionHook` trait in `src/agents/hook/mod.rs`, with a default body that delegates to `self.on_tool_call(function_name, args)` so every existing hook keeps behaving exactly as it does now. Add the matching forwarding impl on `VizierSessionHooks` that chains it across hooks, mirroring the existing `on_tool_call` impl.
- [X] T030 [P] [US3] Override `on_nested_tool_call` in `src/agents/hook/tool_calls.rs` to return `(function_name, args)` unchanged **without** sending a `ToolChoice` frame (FR-034).
- [X] T031 [P] [US3] Override `on_nested_tool_call` in `src/agents/hook/thinking.rs` to return its arguments unchanged without sending a `Thinking` frame — a script calling `think` is equally script-internal.
- [X] T032 [US3] Change `RouterBridge::dispatch` in `src/agents/tools/python/bridge.rs` (~lines 78-84) to call `hooks.on_nested_tool_call(…)` instead of `hooks.on_tool_call(…)`. Leave the `on_tool_response` call as is, and leave the `python script called a tool` tracing line intact — the agent's own view must not change (FR-035, C2).
- [X] T033 [US3] Delete the `Tool calls` section from `webui/app/components/ExecutionReportView.tsx` (the `report.tool_calls.length > 0` block rendering an `<ol>` above Output), along with the now-unused `formatArgs` helper (P5, FR-032).

**Checkpoint**: All three stories independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T034 Run `cargo clippy` and resolve every new warning. Confirm no new `unwrap()`/`expect()` outside tests and no `println!` was introduced (Principle V).
- [X] T035 [P] Run `cargo test` — the five new unit tests from T009, T010, T011, T012, T013 plus the existing suite.
- [X] T036 [P] Run `cd webui && npm run typecheck` and the `webui/app/lib/trail.test.ts` suite from T014.
- [X] T037 Walk the upgrade path in [quickstart.md](./quickstart.md) step 1 on a database created by the **previous** build: old rows keep `seq IS NULL`, new rows increment, startup does not panic on the missing column, and a second restart adds no duplicate column.
- [X] T038 End-to-end check with a `dummyplug` agent on a running binary (constitution e2e gate): walk [quickstart.md](./quickstart.md) steps 2 through 7 — same-millisecond ordering with 20 repeated reads (SC-002), trail persists folded and survives reload, `intent` required and rejected when absent, intent-led python entry with no nested-call list streamed or shown (SC-011), pre-existing conversations still load, and paging without skip or duplicate (SC-003). Enable `tools.python.enabled` and `tools.python.code_mode` on the agent — step 5 needs a script that calls tools.
- [ ] T039 **[LIVE]** Verify narration against a provider that emits assistant text alongside tool calls — [quickstart.md](./quickstart.md) step 8. Dummyplug cannot cover this: it "never starts another tool call in the same turn" and emits no assistant text beside one, so it never writes an `AssistantMessage`. Confirm the `AssistantMessage` row's `seq` is **lower** than the `ToolCall` rows from the same turn, and that narration renders as the trail's first entry.
- [ ] T040 **[LIVE]** Verify SC-005 — [quickstart.md](./quickstart.md) step 9. Send at least 20 multi-tool turns on an Anthropic-family provider, each replaying all prior history including narration, with **zero** rejections for tool_use/tool_result mismatch or consecutive same-role messages. Also send a turn in a conversation recorded *before* this change to confirm H17. A single failure here means one of the two forbidden shapes in [data-model.md](./data-model.md) is being emitted; dummyplug enforces neither rule and would pass regardless.
- [X] T041 Write the commit/changelog entry flagged `[**breaking**]` naming both breaking changes from the spec's Breaking Changes section: pre-existing history ordering is not repaired, and `intent` becomes required on `execute_python` (FR-009).
- [ ] T042 [P] SC-010 judgement pass: read a sample of 20 intents written by a real model and confirm each describes its run in plain language to someone who has not seen the script. Not scriptable — dummyplug intents come from an operator pasting JSON and prove nothing about what a model writes.
- [X] T043 [P] SC-012 check: scroll a conversation of ~100 turns each with a folded trail and confirm smoothness is unchanged from today. Verified by hand, not scripted.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (T001)**: no dependencies.
- **Foundational (T002-T010)**: depends on T001. **Blocks US1 and US2.** US3 does not strictly need it.
- **US1 (T011-T025)**: depends on Foundational.
- **US2 (T026-T028)**: depends on Foundational; T028 depends on T020 (the `python` TrailEvent must exist).
- **US3 (T029-T033)**: independent of US1 and US2. T033 touches the same file as T028, so sequence them.
- **Polish (T034-T043)**: depends on whichever stories are being shipped.

### Within Foundational

- T002 → T003 (the helper adds the column the DDL declares) → T004 (assignment needs the column).
- T004 → T005, T006 (reads need `seq` populated to be meaningful).
- T005, T006 → T009, T010 (tests need the ordering in place).
- T007, T008 are independent of the above ordering chain.

### Within US1

- T011-T014 (tests) before T015-T024.
- **T015 + T016 are one commit** — see the note in Phase 3.
- T017, T018 after T015/T016: recording narration before replay can handle it produces history that breaks the next turn.
- T019 → T020 → T021 → T022, T023, T024.
- T025 after T020.

### Within US3

- T029 before T030, T031, T032 (the trait method must exist).

### Parallel Opportunities

- T008, T009, T010 in Foundational, once T004-T006 land.
- All four US1 tests (T011-T014) together.
- T019 alongside T015-T018 — different files, no overlap.
- T026 and T027 together.
- T030 and T031 together.
- T035, T036, T042, T043 in Polish.
- With multiple people: US3 can run fully parallel to US1 from the start, since it shares only `ExecutionReportView.tsx` with US2.

---

## Parallel Example: User Story 1 tests

```bash
# All four are different files or different concerns — launch together:
Task: "T011 Test AssistantMessage recorded before ToolCall in src/schema/history.rs"
Task: "T012 Test replay shape: one assistant message, adjacency preserved, in src/schema/history.rs"
Task: "T013 Test narration-free history replays identically, in src/schema/history.rs"
Task: "T014 Test groupHistory purity and grouping rules, in webui/app/lib/trail.test.ts"
```

---

## Implementation Strategy

### MVP First (User Story 1 only)

1. T001 — baseline.
2. T002-T010 — ordering. **Do not skip**; the UI is unreadable to debug without it.
3. T011-T025 — the trail.
4. **STOP and VALIDATE**: quickstart steps 1-3, 6, plus the two `[LIVE]` steps. The live steps are not
   optional for US1 — narration is part of US1, and T040 is what proves it has not broken the agent loop.
5. Ship.

### Incremental Delivery

1. Foundational → history is ordered, nothing user-visible yet.
2. US1 → trail persists and folds (MVP). Python entries show a fallback label.
3. US2 → python runs read as intents.
4. US3 → nested calls disappear.

US3 is the cheapest and most visibly satisfying on a loop-heavy script; if US2 slips, US3 still lands
independently.

### Risk Note

T015/T016 is the only step here that can break the product rather than merely look wrong. A wrong replay
shape means an agent cannot take another turn in an affected conversation at all. T012 is written to fail
loudly before the fix, and T040 is the gate after it. Do not merge the pair on a green dummyplug run
alone — dummyplug enforces neither of the two message-shape rules at issue.

---

## Notes

- `[P]` tasks touch different files and have no dependency on incomplete work.
- `[LIVE]` marks the two tasks the dummyplug gate cannot cover, with the reason stated inline so a future
  reader does not mistake it for a shortcut.
- Commit after each task or logical group, except T015+T016 which must be one commit.
- **Known defect, deliberately out of scope**: `formatToolChoice` in `webui/app/routes/chat.tsx` was never
  updated for the spec 009 memory tool rename — it labels `memory_read` with the old search semantics and
  reads `args.query`, which no longer exists, so it renders `🔍 Searching memory for 'undefined'`; there is
  no `memory_search` case, and `memory_detail` is still handled though retired. T025 removes only the
  `execute_python` case. This feature raises the cost of the defect — today the wrong label flashes past in
  an ephemeral indicator, afterwards it is permanent text in every stored transcript on the most frequently
  called tool family — so it is worth its own change soon. It is not folded in here to keep an unrelated
  repair out of this diff.
