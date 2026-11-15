---
description: "Task list for 062 Steer a Busy Turn"
---

# Tasks: Steer a Busy Turn

**Input**: Design documents from `specs/062-steer-busy-turn/` — [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md) (R1–R11), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: Included where the plan's Technical Context names them (pure predicates, the one-transaction storage method, live/replay agreement, `groupHistory`), plus the constitution's dummyplug end-to-end gate (quickstart E1–E9). Silent reads are out of scope (spec Assumptions).

**Organization**: by user story, in priority order: US1, US2 (P1) → US3, US5 (P2) → US4 (P3).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US5 from spec.md

---

## Phase 1: Setup

- [ ] T001 Create the `062-steer-busy-turn` branch in every submodule with `just checkout 062-steer-busy-turn` (first settle the existing uncommitted changes in `design/`, `webui/`, `dummyplug/`); confirm with `just status`
- [ ] T002 Create the empty module `crates/vizier-core/src/agents/steer.rs` with a module doc pointing at `specs/062-steer-busy-turn/contracts/steering.md`, and register `pub mod steer;` in `crates/vizier-core/src/agents/mod.rs`

---

## Phase 2: Foundational (blocks every story)

- [ ] T003 [P] Add `queue: bool` and `mid_turn: bool` to `VizierRequest` in `crates/vizier-core/src/schema/request.rs` (`#[serde(default, skip_serializing_if = "is_false")]`, add an `is_false` helper beside `is_zero`), and `VizierRequest::opens_turn()` = not a `DetachedDelivery` and not `mid_turn` (data-model.md); unit tests: both fields round-trip, an old JSON row without them reads as `false`, `opens_turn` table
- [ ] T004 [P] Add `VizierResponseContent::Steered` (unit variant, serialised `steered`) in `crates/vizier-core/src/schema/response.rs` and include it in `VizierResponse::ends_turn()`; fix every exhaustive `match` on `VizierResponseContent` the compiler reports (Discord/Telegram renderers, `vizier-http` socket, transport wire) so `Steered` ends quietly there (stop typing, post nothing)
- [ ] T005 [P] Clear `mid_turn` from client input in `stamp_on_receipt` in `crates/vizier-http/src/topic.rs` (next to the timestamp stamp), with a unit test that a client-sent `mid_turn: true` arrives as `false`
- [ ] T006 In `crates/vizier-core/src/agents/steer.rs` add `enum Mode { Steer, Queue }`, `struct Queued { request, response_tx, turn, mode }`, `type SessionQueue = Arc<std::sync::Mutex<VecDeque<Queued>>>`, `struct SteerInbox(SessionQueue)` with `take(&self) -> Vec<Queued>` (removes every `Steer` item, keeps the rest in order, never holds the lock across `.await`), and the pure `mode(&VizierRequest, &VizierSession) -> Mode` per research R3 (`Queue` for `queue`, `transient`, non-`Chat` content, skill commands via `skill::invocation::parse`, sessions with no person); unit tests for `take` ordering and the `mode` table
- [ ] T007 Split the body of `save_session_history` in `crates/vizier-core/src/storage/sqlite/history.rs` into `pub(super) fn write_history_entry(tx, session, content) -> Result<String>` (insert, artifact references, Workspace index) and call it from `save_session_history`; no behaviour change
- [ ] T008 Add `steer_turns(&self, session, items: &[(Option<i64>, VizierRequest)]) -> Result<Vec<SteeredTurn>>` and `struct SteeredTurn { index, history_uid }` to `TurnStorage` in `crates/vizier-core/src/storage/turn.rs` (doc: fenced, one transaction, contracts/steering.md S3) and its forwarding impl on `VizierStorage`
- [ ] T009 Implement `steer_turns` in `crates/vizier-core/src/storage/sqlite/turn.rs`: one transaction, `check_fence` on `agent:{id}`, per item `DELETE FROM session_turn WHERE id = ? AND state = 'queued'` and, only when a row went (or the id is `None`), `write_history_entry` with `SessionHistoryContent::Request` carrying `mid_turn = true`; return the saved items (depends on T007, T008)
- [ ] T010 [P] Tests for `steer_turns` in `crates/vizier-core/src/storage/sqlite/turn.rs` tests: saves and deletes together; a row cancelled first (`cancel_session_turns`) is skipped and nothing is saved; a `running` row is never taken; plus the ownership row in `crates/vizier-core/src/storage/sqlite/fence_tests.rs` (refused with `OwnershipLost` under a newer owner, nothing written)
- [ ] T011 Make `SessionQueue` in `crates/vizier-core/src/agents/process.rs` the shared `steer::SessionQueue` of `Queued` items: update `abort_session_silent` (also clear the shared deque, research R9), `queue_handover`, `session_is_busy`, the pop in the `join_next` arm, `hand_on_queued` and the tests at the end of the file; every new item gets its mode from `steer::mode` and `mid_turn` forced `false`; no steering yet, behaviour identical (`cargo test -p vizier-core` green)
- [ ] T012 Replace the turn-start search in `seal_open_calls` in `crates/vizier-core/src/agents/steps.rs` with `VizierRequest::opens_turn()`, and grep `crates/vizier-core/src` for other "last `Request`" searches (`rposition`/`position` over `SessionHistoryContent::Request`) and route them through it

**Checkpoint**: the build is unchanged in behaviour; the queue is shared and every item knows its mode.

---

## Phase 3: User Story 1 — Correct the agent mid-turn (P1) 🎯 MVP

**Goal**: a message sent while a turn runs enters that turn at the next round boundary, saved where it was read, answered in the turn's one reply.

**Independent test**: quickstart E1.

- [ ] T013 [US1] Add `steer: Option<SteerInbox>` to `ToolContext` in `crates/vizier-core/src/agents/tools/mod.rs` (`None` everywhere by default; detached copies and dream contexts keep `None`)
- [ ] T014 [US1] Pass the session's `SessionQueue` from `start_session_job`/`SessionJobs` in `crates/vizier-core/src/agents/process.rs` into `VizierAgent::chat` (new parameter) so the chat turn sets `ToolContext.steer = Some(SteerInbox(queue))` in `crates/vizier-core/src/agents/agent/mod.rs`; only `Chat`/`ClientEvent`/`QuestionAnswer`/delivery turns in a person session get it
- [ ] T015 [US1] Extract the request preparation of `VizierAgent::chat` in `crates/vizier-core/src/agents/agent/mod.rs` (attachments moved into session files + `attachments_note`, project `@` mentions, `to_prompt`) into one helper `prepare_person_request` used by the turn start, with no behaviour change
- [ ] T016 [US1] Add `via: mid_turn` to `generate_frontmatter` for a `mid_turn` request in `crates/vizier-core/src/schema/request.rs` (sender stays the person); unit test against contracts/steering.md S5
- [ ] T017 [US1] Add the fixed sentence of contracts/steering.md S5 to `boot_md` in `crates/vizier-core/src/agents/agent/system_prompt/boot.rs`; keep the prompt byte-identical across turns
- [ ] T018 [US1] At the round boundary in `prompt_with` in `crates/vizier-core/src/agents/agent/mod.rs` (where `deps.deliveries.claim` runs): `ctx.steer.take()` → if non-empty `storage.steer_turns` → `prepare_person_request` for each saved one → claim deliveries → build **one** `Message::User` (delivery text first, then each person message as its own `Part::Text`), record each saved entry through `StepRecorder` as `SessionFrame::HistoryEntry`, send `Steered` on each item's `response_tx` and drop it; items `steer_turns` skipped are dropped with `tracing::debug!`; an empty take does no storage call
- [ ] T019 [US1] Make `history_entries_to_messages` in `crates/vizier-core/src/schema/history.rs` merge adjacent user-side entries after a round (a delivery and/or `mid_turn` requests) into one user message, and add a test that the replayed messages equal the live boundary's message (research R6)
- [ ] T020 [P] [US1] Add `delay_ms` (and `tool` optional when `delay_ms` is present) and the §8 `**Read mid-turn**` echo to `dummyplug/src/protocol.mjs` per contracts/dummyplug-steer.md, with its tests; document §3 addition and §8 in `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`
- [ ] T021 [US1] Write `scripts/steer-e2e.mjs` (shape of `scripts/client-tools-e2e.mjs`) with quickstart E1, and run it against `just run` with a dummyplug agent

**Checkpoint**: E1 passes; steering works end to end over the WebSocket.

---

## Phase 4: User Story 2 — Fall back to the queue when there is no next step (P1)

**Goal**: a waiting message the turn never reaches runs as its own turn; never both, never neither — across abort, restart and crash.

**Independent test**: quickstart E4, E5, E6.

- [ ] T022 [US2] Verify and, where needed, fix in `crates/vizier-core/src/agents/process.rs` that a turn-ending round, a handover/checkpoint job and a side chat never take from the inbox, and that the `join_next` pop runs leftover `Steer` items in arrival order (unit test with a fake queue: steer item left after the turn ends is popped once)
- [ ] T023 [US2] Check the restart path in `crates/vizier-core/src/agents/process.rs` (`hand_on_queued` re-sends steer items behind the resumed turn) and recovery in `crates/vizier-core/src/agents/recovery.rs` (re-sent queued rows land behind the resumed `running` row and are steered at its next boundary); add a test that a steered message replays inside the resumed turn via `split_at_request`
- [ ] T024 [US2] Add quickstart E4 (fallback), E5 (abort while waiting) and E6 (restart mid-turn, both the `PUT /agents` and the shutdown/restart path) to `scripts/steer-e2e.mjs` and run them

**Checkpoint**: SC-002 holds in E5/E6: each message in history once, answered once.

---

## Phase 5: User Story 3 — Several messages while busy (P2)

**Goal**: every message that arrived during a round is delivered together, in order, alongside background work.

**Independent test**: quickstart E2.

- [ ] T025 [US3] Unit test in `crates/vizier-core/src/agents/agent/mod.rs` tests (or `steer.rs`) that three steer items and a delivery produce one user message in the order delivery, one, two, three, and three `mid_turn` history entries in arrival order
- [ ] T026 [US3] Add quickstart E2 to `scripts/steer-e2e.mjs` and run it

---

## Phase 6: User Story 5 — Line up a separate task instead (P2)

**Goal**: Ctrl/Cmd+Enter in the WebUI and `/queue` on every channel keep today's behaviour.

**Independent test**: quickstart E3; E8 for the platforms.

- [ ] T027 [US5] Add `strip_queue_command(&mut VizierRequest) -> QueueCommand` (`Plain`, `Queued`, `UsageOnly`) to `crates/vizier-core/src/agents/steer.rs` per contracts/queue-command.md Q2 (case-insensitive, Telegram `@bot` suffix, word boundary), with a unit test per table row
- [ ] T028 [US5] Call it on receipt in `agent_process` in `crates/vizier-core/src/agents/process.rs` before `steer::mode` (idle and busy); answer `UsageOnly` directly with the usage line, saving nothing and starting no turn
- [ ] T029 [P] [US5] Add `"queue"` to `RESERVED` in `crates/vizier-core/src/skill/invocation.rs` and check the `channels/commands.rs` tests that list reserved/builtin names
- [ ] T030 [P] [US5] Add the `queue` builtin to `builtin_commands()` in `crates/vizier-core/src/channels/discord/mod.rs` (required `message` option) and make the slash-command handler turn it into `Chat("/queue <message>")` as skill commands are; add it to the Telegram `builtins` in `crates/vizier-core/src/channels/telegram/mod.rs`
- [ ] T031 [P] [US5] Parse `/queue @<agent-id> <message>` in `crates/vizier-core/src/workspaces/commands.rs` into the text `/queue <message>` for the agent, people only, with parser tests beside the existing ones
- [ ] T032 [US5] Add quickstart E3 (including `"queue": true` on the frame and `/queue` alone) to `scripts/steer-e2e.mjs` and run it

---

## Phase 7: User Story 4 — Knowing where the message went (P3)

**Goal**: the WebUI shows a waiting message, then splits the turn's trail at it — live, after reload, in every tab (contracts/webui.md, approved).

**Independent test**: quickstart E9.

Commit order design → webui → vizier pins.

- [ ] T033 [P] [US4] In `design/src/components/Composer/Composer.tsx` give `onSend` a `{ queue: boolean }` argument (Ctrl+Enter, or Cmd+Enter on macOS via `metaKey`, sends with `queue: true`; neither fires while the mention/suggestion menu is open) and add a `hint?: ReactNode` prop drawn as one muted line below the composer; add busy-with-hint and idle stories in `design/src/components/Composer/Composer.stories.tsx`
- [ ] T034 [P] [US4] Add a split-trail story in `design/` (trail · bubble · trail · reply, both parts "Worked") reusing the existing trail and message components, per contracts/webui.md W3
- [ ] T035 [US4] Pass the busy hint ("Enter adds to the current turn · Ctrl+Enter queues it", `⌘↵` on macOS; none below `lg`) and forward `queue` from `webui/app/screens/chat/Composer.tsx` to `send` in `webui/app/stores/turns.ts`, which puts `queue: true` on the request frame
- [ ] T036 [US4] In `webui/app/lib/frames.ts` keep `queue` on `QueuedMessage`, draw "Waiting for the agent" vs "Queued · runs after this turn", and match a stored request to its waiting row on the text **after** a `/queue` prefix is stripped; vitest for both matches
- [ ] T037 [US4] In `groupHistory` in `webui/app/lib/trail.ts`, push a `{ kind: 'steer', entry }` block into the open turn for a `Request` with `mid_turn: true` and `chat` content instead of opening a turn; regenerate `webui/app/api/schema.d.ts` (`scripts/openapi.mjs`) for the new fields; vitest: one and two steer blocks, a delivery stays a trail step, a fallback message opens its own turn
- [ ] T038 [US4] Cut the trail at `steer` blocks in `webui/app/screens/chat/MessageList.tsx` / `webui/app/screens/chat/Trail.tsx`: one "Worked · N steps · duration" part per segment (from that segment's steps), the person's bubble between parts, the reply after the last part
- [ ] T039 [US4] Run `cd webui && npm run typecheck` and the vitest suite; walk quickstart E9 in the browser (busy hint, waiting row, split trail, reload, second tab, Ctrl/Cmd+Enter, phone width with `/queue`)

---

## Phase 8: Polish & cross-cutting

- [ ] T040 [P] Document steering and `/queue` in `docs/src/configuration/channels.md` (per channel) and in the chat section of `docs/src/configuration/agents.md`
- [ ] T041 [P] Update the CLAUDE.md architecture notes (durable turns / delivery paragraphs) with the shared queue, `steer_turns`, `mid_turn`, `queue` and the `Steered` frame
- [ ] T042 Run quickstart E7 (excluded kinds) and E8 (a Discord or Telegram test bot: reply posted once, no "agent unavailable", `/queue` in the command list)
- [ ] T043 `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, and the full `scripts/steer-e2e.mjs`; commit with `just commit "feat(agents): steer a busy turn with a person's message"` (design → webui → dummyplug → vizier)

---

## Dependencies

```text
Setup (T001–T002)
  └─► Foundational (T003–T012)
        ├─► US1 (T013–T021)  ──► US2 (T022–T024) ──► US3 (T025–T026)
        ├─► US5 (T027–T032)        (needs T011 only; can run beside US1)
        └─► US4 (T033–T039)        (design T033–T034 can start at once; T035–T038 need T003 for the fields and US1 for live checks)
Polish (T040–T043) after the stories it documents
```

- US2 and US3 verify and extend the US1 boundary; they cannot ship without it.
- US5 is independent of US1 (with US1 absent, `queue` simply changes nothing).
- Within US1: T013 → T014 → T015 → T018; T016, T017, T019 alongside; T020 before T021.

## Parallel examples

- Foundational: T003, T004, T005 together (three files); then T006 and T007 together.
- US1: T016, T017, T019 and T020 (dummyplug) while T018 is written.
- US5: T029, T030, T031 together after T027.
- US4: T033 and T034 (design) at any time after the approved mockups.

## Implementation strategy

1. **MVP**: Setup + Foundational + US1 → quickstart E1 passes; steering works over the WebUI socket (shown with today's queued row and an unsplit trail until US4).
2. **Safe to ship**: + US2 (fallback and exactly-once proven by E4–E6).
3. **Increments**: US3 (bursts), US5 (`/queue`, Ctrl/Cmd+Enter), US4 (WebUI split trail), then Polish.
