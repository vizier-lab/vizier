# Tasks: Asking the Person — Structured Questions and Sensitive Input

**Input**: Design documents from `/specs/042-agent-asking-question/`

**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅, quickstart.md ✅

**Tests**: Included. The constitution requires `cargo test` and a `dummyplug` end-to-end run for every
agent-observable change, so each story carries the tests that prove it. Write the test task before its
implementation task and watch it fail.

**Organization**: Tasks are grouped by user story so each can be implemented, tested and delivered on its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different file, no dependency on an incomplete task)
- **[Story]**: which user story the task belongs to (US1…US10), from spec.md
- Every task names an exact file path
- Paths are relative to the repository root of this worktree; `webui/` and `design/` are submodules

## Path Conventions

Rust workspace: `crates/vizier-core/src/`, `crates/vizier-http/src/`. Submodules: `webui/app/`,
`design/src/`. Docs: `docs/src/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: The module skeleton and the one table of limits every later task reads.

- [X] T001 Run `just checkout 042-agent-asking-question` so `design/` and `webui/` are initialised on this branch (plan R15, constitution "matching submodule branches"); confirm with `git submodule status` that no line starts with `-`
- [X] T002 Create `crates/vizier-core/src/questions/mod.rs` declaring `model`, `limits`, `validate`, `surface`, `submit`, `text_form`, `sensitive`, `tool_question`, `tool_sensitive`, and register `pub mod questions;` in `crates/vizier-core/src/lib.rs`; every declared file exists and holds only a doc comment, so the crate compiles
- [X] T003 [P] Add the limits of research R12 as `pub const`s with their meaning documented in `crates/vizier-core/src/questions/limits.rs` (questions per set 10, options 25, prompt/help 500, label 100, option value 100, text answer 4 000, set/answer payload 64 KiB, ask wait 120 s, one open ask per conversation, one set per turn)
- [X] T004 Add a test in `crates/vizier-core/src/questions/limits.rs` asserting each constant's value and the invariants (text answer ≤ modal limit, option value ≤ smallest platform control budget), so a later edit cannot silently drift from plan R12

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The category, the per-turn availability rule, the shared types and the surface registry.
Nothing below can start before this phase completes.

**⚠️ CRITICAL**: every user story depends on this phase.

- [X] T005 Add `ToolCategory::Questions` as the 18th variant in `crates/vizier-core/src/agents/tools/mod.rs`: the variant, `ALL: [ToolCategory; 18]`, `name() -> "questions"`, and `description()` ("Ask the person one or more structured questions; the answer arrives as their next message")
- [X] T006 Add `fn available(&self, ctx: &ToolContext) -> bool { true }` to `VizierToolDyn` and `VizierTool` in `crates/vizier-core/src/agents/tools/mod.rs`, forwarding through the blanket `impl<Tool: VizierTool> VizierToolDyn for Tool`
- [X] T007 [P] Add `pub interactive: bool` with its doc comment to `ToolContext` in `crates/vizier-core/src/agents/tools/mod.rs` and set it at every construction site (`crates/vizier-core/src/agents/agent/mod.rs`, `agents/process.rs`, `agents/background/mod.rs`, and the test helpers in `agents/tools/mod.rs`, `agents/tools/secrets.rs`, `agents/tools/artifacts/mod.rs`, `projects/mentions.rs`, `projects/shell.rs`)
- [X] T008 [P] Create `crates/vizier-core/src/questions/model.rs`: `QuestionSet`, `Question`, `QuestionType`, `Option`, `Answer`, `AnswerState`, `QuestionSetRecord`, `QuestionState`, `SensitiveAskRecord`, `AskState`, `Controls`, `SensitiveOutcome` — exactly the fields of data-model.md §1–§8, `Serialize`/`Deserialize`/`JsonSchema` where they cross the wire
- [X] T009 [P] Create `crates/vizier-core/src/questions/surface.rs`: the `QuestionSurface` trait and `Refusal` of `contracts/channel-controls.md` §1, plus a `PlatformSurfaces` registry keyed by `(agent_id, ChannelPlatform)` that a reader inserts into on start and removes from on stop
- [X] T010 Add the `Questions` service to `crates/vizier-core/src/questions/mod.rs` (surface lookup, per-turn set bookkeeping, the open-ask registry of research R9) and its field to `VizierDependencies` in `crates/vizier-core/src/dependencies.rs`
- [X] T011 Filter the turn's offer in `crates/vizier-core/src/agents/tools/mod.rs`: `ToolRouter::catalogue` takes `&ToolContext` and drops native entries whose `available(ctx)` is false (updating its three call sites), and `turn_categories` drops a native category that has no available tool this turn
- [X] T012 Compute `interactive` in the agent loop in `crates/vizier-core/src/agents/agent/mod.rs` per research R2 (Chat/AudioChat/ClientEvent/QuestionAnswer on a web or channel conversation; never Unattended/BackgroundReport/Command/Btw/SilentRead), with unit tests next to it in the same file
- [X] T013 Add availability tests in `crates/vizier-core/src/agents/tools/mod.rs`: a dream/`task__`/`SUBAGENT` context lists neither tool and no `questions` category line, a `Btw`/`Unattended` request lists neither, and a Discord/Telegram context lists `ask_question` but not `ask_sensitive_input` (FR-007, FR-028, FR-063)

**Checkpoint**: the category exists, the offer is per-turn, and both tools can be declared `available` — user stories can begin.

---
## Phase 3: User Story 1 — One structured question, one structured answer (Priority: P1) 🎯 MVP

**Goal**: the agent asks one typed question, the turn ends, the WebUI draws a card, the person answers, and
the agent's next turn carries the answer tied to the question it asked.

**Independent Test**: ask an agent to make an ambiguous choice; confirm it calls `ask_question`, the turn ends
without another model round, the card appears, a choice can be submitted, the card becomes read-only, and the
next turn carries the answer (quickstart.md §2–§3).

### Tests for User Story 1

- [X] T014 [P] [US1] In `crates/vizier-core/src/questions/validate.rs`, tests that each FR-005 rule is reported and that several problems come back together: no questions, too many, duplicate id, empty prompt, choice with no options, duplicate option value, default outside the options, inconsistent selection bounds, oversized payload
- [X] T015 [P] [US1] In `crates/vizier-core/src/storage/sqlite/question.rs`, tests for the atomic claim: two submissions of one set, exactly one wins and the loser sees the already-answered reason; a submission against another conversation's set is refused; an unknown set id is refused

### Implementation for User Story 1

- [X] T016 [US1] Add `init_question_schema` to `crates/vizier-core/src/storage/sqlite/mod.rs`: the `question_set` table and its two indexes exactly as data-model.md §9, called from `init_schema` next to the other split-out schemas; no migration and no `add_column_if_missing`, because the table is new
- [X] T017 [P] [US1] Create `crates/vizier-core/src/storage/question.rs`: the `question_set` half of data-model.md §10 (`create_question_set`, `claim_question_set`, `decline_question_set`, `withdraw_question_set`, `close_question_set`, `get_question_set`) as a trait on `VizierStorage` delegating to the backend, mirroring `storage/client_call.rs`
- [X] T018 [US1] Implement the trait in `crates/vizier-core/src/storage/sqlite/question.rs`, with `claim_question_set` as the single `UPDATE … WHERE set_id = ? AND state = 'pending'` that decides the winner, and add `fill_question_sets` next to `fill_client_calls` in `crates/vizier-core/src/storage/sqlite/history.rs` (chunked `SELECT … IN (…)`, wired into every history read and into `content_type_discriminant`); a pending set never expires and a later set never invalidates an earlier one (FR-019)
- [X] T019 [US1] Implement `crates/vizier-core/src/questions/validate.rs`: `validate_set(&QuestionSet) -> Result<(), Vec<Problem>>` naming every FR-005 rule with a stable `rule` key and the offending question id, plus `validate_answer(&QuestionSet, &[Answer]) -> Result<(), Vec<Problem>>` for FR-012; every refusal states the limit it broke (FR-060)
- [X] T020 [US1] Add `SessionHistoryContent::QuestionSet { set_id, title, description, questions, submit_label, dismiss_label, state, answers, answerer }` to `crates/vizier-core/src/schema/history.rs`, with the last three filled at read time and never written into the stored blob
- [X] T021 [US1] Add `VizierRequestContent::QuestionAnswer(QuestionAnswer)` to `crates/vizier-core/src/schema/request.rs` with its struct, `kind() == "question_answer"`, and the `Display` that renders the summary text then the answers as a fenced JSON block (contracts/questions-wire.md §7)
- [X] T022 [US1] Add `SessionFrame::QuestionSet { set_id, state, answers, answerer }` to `crates/vizier-core/src/transport.rs` and its mapping in `TopicOutbound::run` in `crates/vizier-http/src/topic.rs` as the `question_set` frame, changing nothing about `ui_render` or any other turn-ending tool (FR-065)
- [X] T023 [US1] Implement `crates/vizier-core/src/questions/submit.rs`: one function every surface calls — validate against the stored set, claim atomically, build the `QuestionAnswer` carrying the set id, each question id, the values and a readable summary (FR-013), `record_step` the `Request`, and `transport.send_request` on the session stored with the set; refusal returns a typed reason the surface turns into a `client_error` or an ephemeral notice; a dismissal produces the declined outcome with no answers (FR-014)
- [X] T024 [P] [US1] Implement `crates/vizier-core/src/questions/tool_question.rs`: `AskQuestion` as a built-in available in every conversation a person can answer, independent of client (FR-001), with the input schema of contracts/agent-tools.md §1 — title, description, ordered questions, submit and dismiss labels (FR-002), the four types with option value/label/description (FR-003) — `category() == Some(ToolCategory::Questions)`, `ends_turn() == true` with the agent's own step text kept as the reply (FR-006), `available()` per FR-007, the description of FR-009, validation before anything is shown, the one-set-per-turn guard (FR-008), and a present-then-withdraw sequence that withdraws the set if the script fails (FR-010)
- [X] T025 [US1] Register `AskQuestion` in `BaseTools::new` in `crates/vizier-core/src/agents/tools/mod.rs` and add both new tools to `dummyplug`'s sample-request path if a schema shape is unsupported (`agents/agent/model/dummyplug.rs`)
- [X] T026 [US1] Accept the answer in `crates/vizier-http/src/topic.rs` `handle_inbound` (route `content.question_answer` through `questions::submit`, like `check_client_event` does for a client event) so the answer takes the ordinary queue, history and attribution path with no bypass (FR-015) and starts the next turn within one second on the same machine (SC-009), and add the `client_error` codes of contracts/questions-wire.md §2 in `crates/vizier-http/src/api/v1/agents/channel.rs` (`question_not_open`, `invalid_answer`, `answer_too_large`, rate limit)
- [X] T027 [US1] Run `specs/042-agent-asking-question/quickstart.md` §2 and §3 against a `dummyplug` agent, confirming that 100% of successful question calls end the turn with no further model round (SC-003), and record the observed `result`, `stats.ended_by_tool`, the `QuestionSet` history entry and the answer turn in `specs/042-agent-asking-question/tasks.md`'s Notes

**Checkpoint**: the whole question loop works in the WebUI with one question — the MVP.

---

## Phase 4: User Story 2 — Several questions in one card (Priority: P1)

**Goal**: one card holds several questions in order, one submission answers them all.

**Independent Test**: ask for a task needing three answers; one card holds all three, one submission sends all
three, a required question left blank blocks the whole submission with an inline message (quickstart.md §3, §8).

- [X] T028 [P] [US2] Extend `crates/vizier-core/src/questions/validate.rs` with the multi-question rules and tests: choose-several distinct values, `min_selections`/`max_selections` bounds, per-question dimension limits, and a whole-set payload over 64 KiB
- [X] T029 [US2] Implement the multi-question card in `webui/app/components/questions/QuestionCard.tsx`: up to the documented limit of questions (FR-004), every question in the agent's order with its prompt, help text, options, default and required state (FR-011), one submit and one dismiss action, drawn as the WebUI's custom card in the active theme (FR-021)
- [X] T030 [US2] Implement per-type fields in `webui/app/components/questions/` (`TextField.tsx`, `ChoiceField.tsx`, `MultiChoiceField.tsx`, `ConfirmField.tsx`), each rendering its own error and keeping the answers already given when a submission is refused (FR-012), so at least 95% of valid submissions are accepted on the first attempt and 99% within two (SC-002)
- [X] T031 [US2] Add the reducer cases for the `question_set` frame and the `QuestionSet` history entry in `webui/app/lib/frames.ts` with vitest tests, so a pending card and its later state survive a page reload (FR-016, FR-020)
- [X] T032 [US2] Render every answer and every explicitly skipped question on the closed card in `webui/app/components/questions/QuestionCard.tsx` (FR-020, US2 #6)
- [X] T033 [US2] Run `specs/042-agent-asking-question/quickstart.md` §3 with a three-question set and its §8 (one invalid answer blocks the whole submission, the other answers are kept), confirming a three-question, five-option set is presented in one reply rather than one turn per question (SC-012), and record the outcome in `specs/042-agent-asking-question/tasks.md`

**Checkpoint**: US1 and US2 both work independently.

---
## Phase 5: User Story 3 — Questions work on every channel, with that channel's own controls (Priority: P1)

**Goal**: Discord and Telegram present the same set with native controls; the answer carries the same shape
everywhere.

**Independent Test**: ask the same two-question set from each supported channel and answer it with the native
controls; the answers reach the agent with the same ids and values, the message becomes a record, a second
press is refused, and a restart leaves pending controls working (quickstart.md §7, §10).

- [X] T034 [P] [US3] Add `short_id` generation (8 chars, unique) and `question_set_by_short_id` to `crates/vizier-core/src/storage/question.rs` and `crates/vizier-core/src/storage/sqlite/question.rs`, with a retry on the unique-index conflict and a test for the collision path; a control therefore resolves from storage only, so it keeps working across a reconnect and a restart (FR-025)
- [X] T035 [P] [US3] Implement `crates/vizier-core/src/questions/text_form.rs`: the renderer and parser of `contracts/channel-controls.md` §4 (one `key: value` line per question, `*` for required) with unit tests for a valid reply, an unknown key, a missing required key, a value outside the options and an over-limit text
- [X] T036 [US3] Implement `QuestionSurface` for Discord in `crates/vizier-core/src/channels/discord/mod.rs`: buttons for a small choose-one, a string select above five options, a `min`/`max` select for choose-several, a two-button row for confirm, and a modal with one `TextInput` per free-text question (FR-022), all built with `twilight_util::builder`, carrying only `<short>.<question>.<action>` custom ids, and needing no extra bot permission or privileged intent (FR-031)
- [X] T037 [US3] Branch `Event::InteractionCreate` in `crates/vizier-core/src/channels/discord/mod.rs` on component and modal interactions (not only application commands), acknowledging with `DeferredUpdateMessage` before any storage read (FR-023, SC-013)
- [X] T038 [US3] Implement `QuestionSurface` for Telegram in `crates/vizier-core/src/channels/telegram/mod.rs`: an inline keyboard per question with the four types covered (FR-022), toggle rows plus submit for choose-several, and one message per question when a set has more than one, each carrying the same short id (FR-029)
- [X] T039 [US3] Add `AllowedUpdate::CallbackQuery` to the poller's `allowed_updates` and an `UpdateKind::CallbackQuery` branch to `handle_update` in `crates/vizier-core/src/channels/telegram/mod.rs`, answering with `answer_callback_query` before any storage read
- [X] T040 [US3] Register both surfaces in their readers' constructors and remove them on stop in `crates/vizier-core/src/channels/discord/mod.rs` and `crates/vizier-core/src/channels/telegram/mod.rs`, so a running reader is discoverable through `PlatformSurfaces` and a stopped one is not
- [X] T041 [US3] Route every press through `questions::submit` and every stale press through `QuestionSurface::refuse` (`crates/vizier-core/src/questions/submit.rs`), so a control-answer is validated and delivered with the same structure as a WebUI answer and records the answerer as the platform identifies them (FR-026), and a closed set answers with a short notice and starts no turn (FR-024, FR-017)
- [X] T042 [US3] Edit the question message into a record of the answers and remove its components on close in `crates/vizier-core/src/channels/discord/mod.rs` and `crates/vizier-core/src/channels/telegram/mod.rs` (FR-024)
- [X] T043 [US3] Handle a reply to a question message as the text-form answer in `crates/vizier-core/src/channels/discord/mod.rs` and `crates/vizier-core/src/channels/telegram/mod.rs`, falling back to `crates/vizier-core/src/questions/text_form.rs`'s parser when native controls cannot express the set (FR-027)
- [ ] T044 [US3] Run the **LIVE** channel checks of `specs/042-agent-asking-question/quickstart.md` §10 with a real bot on each platform, confirming 100% of sets in a fixed sample are presented with native controls and answerable through them while unsupported channels never offer the tool (SC-005), and record in `specs/042-agent-asking-question/tasks.md` what was verified and what was not

**Checkpoint**: the same set is answerable on WebUI, Discord and Telegram with identical answer structure (FR-030).

---

## Phase 6: User Story 4 — The agent gets a credential without ever seeing it (Priority: P1)

**Goal**: the script receives the value, and everything downstream shows `[redacted]`.

**Independent Test**: have an agent ask for a token, supply one, and use it in a request to a local echo endpoint;
confirm the request carried the real value and that the value appears nowhere else (quickstart.md §9).

- [X] T045 [P] [US4] Add the `sensitive_ask` table to `crates/vizier-core/src/storage/sqlite/mod.rs` (data-model.md §9, no value column) and its trait methods of data-model.md §10 in `crates/vizier-core/src/storage/question.rs` plus the implementation in `crates/vizier-core/src/storage/sqlite/question.rs`
- [X] T046 [US4] Add `SessionFrame::SensitiveAsk { ask_id, label, purpose, protected, min_bytes, max_bytes, expires_at }` and `SessionFrame::SensitiveAskClosed { ask_id, outcome }` to `crates/vizier-core/src/transport.rs`, map them in `TopicOutbound::run`, and replay every open ask of the session in `TopicConnection::open` in `crates/vizier-http/src/topic.rs` (FR-048)
- [X] T047 [US4] Implement `crates/vizier-core/src/questions/sensitive.rs`: the per-conversation slot (FR-053), publish-then-register, the `oneshot` wait capped by `min(ASK_WAIT, ctx.deadline)` so the script sees a branchable result rather than the bridge's timeout, `arm()` of the turn's redactor at receipt, delivery of the value byte for byte with no trimming or newline conversion (FR-037), the 8–65 536-byte bounds taken from the redaction minimum and maximum (FR-056), the `provided`/`declined`/`timeout` outcomes, and no retention of the value anywhere (FR-044)
- [X] T048 [P] [US4] Implement `crates/vizier-core/src/questions/tool_sensitive.rs`: `AskSensitiveInput` with the input of contracts/agent-tools.md §2 — a stated purpose the person can judge (FR-034) — `category() == Some(ToolCategory::Secrets)`, `ends_turn() == false` so the turn keeps running for a provided, declined or timed-out ask alike (FR-033), `available()` restricted to `VizierChannelId::web_user()` on an interactive turn and never where the value would be visible to someone else (FR-046, FR-055), the FR-040 description stating it does not store and that `set_secret` is the storing tool (FR-045), that it is not an approval gate (FR-067), and the empty/whitespace refusal of FR-038
- [X] T049 [US4] Register `AskSensitiveInput` in `BaseTools::new` in `crates/vizier-core/src/agents/tools/mod.rs` beside `SetSecret`/`GetSecret`
- [X] T050 [US4] Accept `sensitive_input_answer` in `crates/vizier-http/src/topic.rs` `handle_inbound` and add its `client_error` codes (`sensitive_ask_not_open`, `sensitive_value_empty`, `sensitive_value_too_large`) in `crates/vizier-http/src/api/v1/agents/channel.rs`; the ask is answerable only from the conversation that asked and records who supplied it and when (FR-039), and the frame is never echoed and never logged
- [X] T051 [US4] Implement `webui/app/components/questions/SensitiveAskDialog.tsx` on the design `Dialog`: a masked field with an explicit reveal, no autocomplete or autofill, not offered to the password manager, no echo after submit, submit plus reject, where reject/Escape/outside press/close all decline (FR-035, FR-036), and a person can supply a value in under 20 seconds from seeing it (SC-018)
- [X] T052 [US4] Wire the dialog into `webui/app/lib/frames.ts` and `webui/app/stores/turns.ts`, so the ask opens on every socket on the conversation and the first submission or dismissal closes it everywhere (FR-048)
- [X] T053 [US4] Abandon open asks when a turn ends abnormally: call `Questions::abandon(&session)` next to `seal_open_calls` in `crates/vizier-core/src/agents/process.rs`, and sweep every open ask at startup in `crates/vizier-core/src/dependencies.rs` beside `interrupt_orphaned_client_calls` (FR-049)
- [X] T054 [US4] Run `specs/042-agent-asking-question/quickstart.md` §9 (the offline half proves the contract; the dialog itself is marked LIVE) and record what was verified in `specs/042-agent-asking-question/tasks.md`

**Checkpoint**: a credential reaches the script and nothing else, in the WebUI.

---
## Phase 7: User Story 5 — A supplied value stays out of every copy the system keeps (Priority: P1)

**Goal**: history, step records, model copy, logs, telemetry and notifications carry the marker, never the value.

**Independent Test**: supply a distinctive value, then grep the conversation history, the step records, the log
output and the exported telemetry for it: zero occurrences, and the marker where the value was used.

- [X] T055 [P] [US5] Add `SessionHistoryContent::SensitiveAsk { ask_id, purpose, label, state }` to `crates/vizier-core/src/schema/history.rs`, filled at read time by the fill pass in `crates/vizier-core/src/storage/sqlite/history.rs`, and recorded when the ask opens (FR-054, FR-061)
- [X] T056 [P] [US5] Add an integration test in `crates/vizier-core/src/questions/sensitive.rs` asserting the value is absent from the serialised `ExecutionReport` (result, stdout, error, nested call arguments) after a script prints it and returns it, while the script itself received it byte for byte (SC-014, SC-015)
- [X] T057 [US5] Add an integration test in `crates/vizier-core/src/agents/tools/python/bridge.rs` proving a supplied value passed to a nested tool call is scrubbed from that call's recorded arguments and error, not only from the top-level report (FR-042)
- [X] T058 [US5] Add a test in `crates/vizier-core/src/agents/tools/mod.rs` proving the `execute_tool` span records the marker with content capture on, and nothing at all with it off (FR-042, US5 #2)
- [X] T059 [US5] Assert the value is absent from the next turn's built context in `crates/vizier-core/src/agents/agent/mod.rs` (a test over `history_entries_to_messages`) — the marker is present where the value was used (FR-041, US5 #3)
- [X] T060 [US5] Assert neither a notification nor the reply's `VizierResponseStats` carries the value in `crates/vizier-core/src/notifications/` and `crates/vizier-core/src/schema/response.rs` (US5 #4)
- [X] T061 [US5] Add the automated grep to `specs/042-agent-asking-question/quickstart.md` §9 as a scripted check (`grep -c` over `/tmp/vizier-ask.log` and the `session_history` blob) and run it end to end with a `dummyplug` agent, confirming a supplied value is unrecoverable after its turn ends — no surface in the product returns it (SC-022) — and record the counts

**Checkpoint**: the guarantee is measured, not asserted.

---

## Phase 8: User Story 6 — The agent knows when and how to ask a question (Priority: P2)

**Goal**: the tool's description is the whole interface the agent has, and it is complete and correct.

**Independent Test**: with no skill installed, give the agent a prompt with an obvious ambiguity; it discovers the
tool, asks a valid set, and the answer arrives with the expected shape. A scheduled run or dream does not offer it.

- [X] T062 [P] [US6] Add a test in `crates/vizier-core/src/questions/tool_question.rs` asserting the description states the purpose, all four types, the ten-question limit, the turn-ending behaviour, the answer shape, the limits, that it is unavailable in unattended turns, and the pointer to `ask_sensitive_input` (FR-009, FR-066)
- [X] T063 [US6] Assert the `ENDS_TURN_NOTICE` is appended exactly once and does not count against the author's description budget in `crates/vizier-core/src/questions/tool_question.rs` (FR-009, US6 #2)
- [X] T064 [US6] Return the clear unattended refusal from `AskQuestion::call` (a direct call in a dream, task run, background piece, inter-agent turn or side chat fails, and the turn continues) in `crates/vizier-core/src/questions/tool_question.rs` (FR-007, FR-063)
- [X] T065 [US6] Keep the text the agent wrote in the same step and show it with the card, in `crates/vizier-core/src/agents/agent/mod.rs`'s ended-turn branch and `webui/app/components/questions/QuestionCard.tsx` (US6 #4)
- [X] T066 [US6] Add a test that the next turn sees the set id, each question, the answer or skipped/declined state, the option labels and values, and who answered, distinct from an ordinary typed message — in `crates/vizier-core/src/schema/request.rs` (FR-062, US6 #5)
- [ ] T067 [US6] Run `specs/042-agent-asking-question/quickstart.md` §1, §4 and §6 with a `dummyplug` agent (tool listing, invalid set refused, unattended absence), checking that an agent with no skill installed asks a valid set on the first or second attempt in at least 95% of 20 sample prompts and picks the right tool of the two in the same proportion (SC-008, SC-025), and record the outcome in `specs/042-agent-asking-question/tasks.md`

**Checkpoint**: an agent that has never seen this feature can use it from the listing alone.

---

## Phase 9: User Story 7 — The agent knows when it may ask for a sensitive value (Priority: P2)

**Goal**: the sensitive tool's description is honest about the guarantee and where it exists.

**Independent Test**: with no skill installed, give the agent a task needing a credential; it discovers the tool,
asks with a clear purpose and uses the value through another tool. Run the same task from Discord and from a
scheduled run: the tool is absent.

- [X] T068 [P] [US7] Add a test in `crates/vizier-core/src/questions/tool_sensitive.rs` asserting the description states that the value goes to the script only, that the agent will not see it, that the call does not end the turn, that the person may reject or let it time out, the length limits, that redaction is exact-string only, and where the tool is unavailable (FR-040), and that an agent with no skill installed uses it correctly — a stated purpose, the value passed to another tool rather than printed — in at least 95% of 20 sample prompts (SC-020)
- [X] T069 [US7] Assert the tool is listed under `secrets` beside `set_secret`/`get_secret` and absent on Discord and Telegram in `crates/vizier-core/src/agents/tools/mod.rs` (FR-032, FR-046, SC-017)
- [X] T070 [US7] Allow a second ask in the same turn once the first has closed, with each ask closing on its own outcome, in `crates/vizier-core/src/questions/sensitive.rs` (FR-052)
- [X] T071 [US7] Return the naming error for a direct call where the tool is not offered (channel conversation or unattended turn) without failing the turn, in `crates/vizier-core/src/questions/tool_sensitive.rs` (FR-047)

**Checkpoint**: the agent knows the contract before it uses it.

---
## Phase 10: User Story 8 — The person can decline a sensitive-input ask, and the agent copes (Priority: P2)

**Goal**: reject, dismiss, timeout and abort are all branchable results; nothing hangs and nothing fails the turn.

**Independent Test**: decline an ask; the script receives a declined result, the turn finishes normally, and the
transcript records the refusal (quickstart.md §9 LIVE, plus the unit tests below).

- [X] T072 [P] [US8] Add tests in `crates/vizier-core/src/questions/sensitive.rs` that reject, Escape-equivalent dismissal, the wait limit and an abort each resolve the waiter with `declined`/`timeout` respectively, that the tool returns a value the script can branch on rather than an exception (FR-050, FR-051, SC-016), and that the turn is still running when the ask closes for every outcome (SC-023)
- [X] T073 [US8] Refuse a submission that arrives after the ask closed (answered, declined, timed out, turn ended or aborted) with a short notice, delivering nothing, in `crates/vizier-core/src/questions/sensitive.rs` and `crates/vizier-http/src/topic.rs` (FR-049, SC-021)
- [X] T074 [US8] Record a declined or timed-out ask in the conversation without any value, in `crates/vizier-core/src/questions/sensitive.rs` (FR-054, US8 #5)
- [X] T075 [US8] Open a new dialog for an ask made after a decline and keep the earlier one closed and unanswerable, in `crates/vizier-core/src/questions/sensitive.rs` (US8 #3)
- [X] T076 [US8] Prove every closing path leaves no ask open and no script waiting in `webui/app/components/questions/SensitiveAskDialog.tsx` (reject, Escape, outside press, close control), with a vitest test for the dismissal wiring (FR-036, SC-024)

**Checkpoint**: a blocking ask is always refusable and always ends.

---

## Phase 11: User Story 9 — Questions are safe, accessible, and part of history (Priority: P2)

**Goal**: keyboard and screen-reader access, untrusted answers, one answer per set, and a card that survives a
reload and a restart.

**Independent Test**: complete a three-question set with the keyboard only and with a screen reader; try an answer
not in the options and an over-limit text; reload and restart while a set is pending.

- [X] T077 [P] [US9] Add tests in `crates/vizier-core/src/questions/validate.rs` that an answer not among the offered options, an over-limit text, too few or too many selections, and a whitespace-only required text are each refused with the named question and rule (FR-012, FR-059, US9 #1–§2)
- [X] T078 [US9] Prove an answer containing markup, script or a command-looking string is stored, rendered and delivered as text and executes nothing, in `crates/vizier-core/src/questions/validate.rs` and `webui/app/components/questions/QuestionCard.tsx` (FR-059, US9 #3)
- [ ] T079 [US9] Make the card keyboard-operable with focus moved to the first problem on a refused submission, and label every question and control for a screen reader, in `webui/app/components/questions/QuestionCard.tsx` and the design primitives it composes (FR-064, US9 #4–§5), so a keyboard-only person completes a set without a mouse and a screen reader names every question and control (SC-006), answering one question in under 10 seconds and three in under 30 on the WebUI (SC-001)
- [ ] T080 [US9] Prove the card renders in light and dark themes and on a phone, using the design tokens only, in `design/src/stories/` and `webui/app/components/questions/QuestionCard.tsx` (SC-007)
- [X] T081 [US9] Prove a pending set is still shown and still answerable after a restart in `crates/vizier-core/src/storage/sqlite/question.rs` (the read-time fill from storage, never memory) and by running `specs/042-agent-asking-question/quickstart.md` §7, confirming answers carry the correct set and question ids after a reload, a second tab and a restart (FR-016, SC-004, SC-011)
- [X] T082 [US9] Delete a conversation's sets and asks with the conversation and with the agent, in `crates/vizier-core/src/storage/sqlite/session.rs`'s explicit-delete block and the agent-deletion path, with a test each (US9 #9)
- [X] T083 [US9] Refuse an answer from someone who cannot access the conversation, in `crates/vizier-http/src/topic.rs` and the channel readers, so no answer is accepted from a person without access and no set is answered twice in 100% of tests (FR-018, FR-063, US9 #10, SC-010)
- [X] T084 [US9] Prove a second submission on a closed card is refused and starts no turn, in `webui/app/components/questions/QuestionCard.tsx` and `crates/vizier-core/src/questions/submit.rs` (FR-017, US9 #6)

**Checkpoint**: the feature is usable by everyone and survives real use.

---

## Phase 12: User Story 10 — The sensitive-input boundaries are stated, not implied (Priority: P2)

**Goal**: the system never claims protection it cannot deliver.

**Independent Test**: ask for a value shorter than the minimum and confirm the ask states it cannot be protected;
supply a long value, have the script write it into a file, and confirm the system does not claim it was protected.

- [X] T085 [P] [US10] Return `protected: false` for a value shorter than `MIN_VALUE_BYTES` and carry it into the script's result, in `crates/vizier-core/src/questions/sensitive.rs`, with a test (FR-043, SC-019)
- [X] T086 [US10] State in the dialog, before submission, that a value this short cannot be scrubbed from the report, in `webui/app/components/questions/SensitiveAskDialog.tsx` when the frame carries `protected: false` (FR-043, SC-019)
- [X] T087 [P] [US10] State in the tool's description that a transformed form (encoded, hashed, split, concatenated) is outside the guarantee, and that a value the script writes to a file, memory, a scratch file, an artifact or an outbound body is not protected, in `crates/vizier-core/src/questions/tool_sensitive.rs` (FR-057)
- [X] T088 [US10] State in the tool's description that a person can still type the value into an ordinary message, and instruct the agent to ask through the tool and never in prose, in `crates/vizier-core/src/questions/tool_sensitive.rs` (FR-058)
- [X] T089 [US10] Write `docs/src/configuration/questions.md`: the two tools in agent terms, the redaction guarantee and its exact limits, the per-channel availability, and the one open ask per conversation rule

**Checkpoint**: every promise the feature makes is one it keeps.

---
## Phase 13: Polish & Cross-Cutting Concerns

**Purpose**: documentation, the shared guarantees, and the gates a change of this size has to pass.

- [X] T090 [P] Document the new frame kinds and the `question_answer` request in `docs/src/api-integration/client-tools.md`, following its §9 versioning rule (additive, clients ignore what they do not know)
- [X] T091 [P] Document how a question set and a sensitive ask appear on each channel in `docs/src/configuration/channels.md`
- [X] T092 [P] Fix the `CLAUDE.md` category-count drift: `secrets` is described as "the 16th and last built-in category", but `coding` made it 17 and `questions` makes 18; also add both tools to the tool inventory and the category list
- [X] T093 Note in `CLAUDE.md` that a tool can be offered per turn through `VizierTool::available`, next to the existing note about `coding` being per-turn
- [X] T094 Confirm no migration or compatibility code was added to `crates/vizier-core/src/storage/sqlite/mod.rs` or `crates/vizier-core/src/dependencies.rs`, so nothing belongs in #82, and say so explicitly in the commit body (research R13)
- [X] T095 Run `cargo clippy --workspace --all-targets -- --deny warnings` over `crates/` and fix everything it reports
- [X] T096 Run `cargo test --workspace` over `crates/` and fix every failure without deleting, skipping or weakening a test
- [ ] T097 Run `cd webui && npm run typecheck` and `cd design && npm test` (the `webui/app/` and `design/src/` trees)
- [X] T098 Walk every offline step of `specs/042-agent-asking-question/quickstart.md` against a `dummyplug` agent on a running binary (constitution e2e gate) and record the results in this file
- [X] T099 Verify the pre-existing-database path by hand: create a workspace with the previous build, then start the new binary against `/tmp/vizier-ask/vizier.db` and confirm `question_set` and `sensitive_ask` appear without error (constitution: manual verification for storage changes)
- [ ] T100 Commit in the constitution's order — `design/`, then `webui/`, then `feat(questions):` covering `crates/` and `docs/src/` — with conventional messages, announcing the staged files first

---

## Traceability

Every functional requirement and success criterion of `spec.md` is cited by at least one task above:
67/67 FRs and 25/25 SCs (checked by script when this file was generated).

| Story | Tasks | FRs covered |
|---|---|---|
| US1 | T014–T027 | FR-001…FR-010, FR-013, FR-014, FR-015, FR-019, FR-065 |
| US2 | T028–T033 | FR-004, FR-011, FR-012, FR-020, FR-021, FR-060 |
| US3 | T034–T044 | FR-022…FR-031 |
| US4 | T045–T054 | FR-032…FR-040, FR-045, FR-053, FR-055, FR-067 |
| US5 | T055–T061 | FR-041, FR-042, FR-044, FR-054, FR-061 |
| US6 | T062–T067 | FR-006, FR-007, FR-008, FR-009, FR-062, FR-063, FR-066 |
| US7 | T068–T071 | FR-040, FR-046, FR-047, FR-052 |
| US8 | T072–T076 | FR-036, FR-049, FR-050, FR-051, FR-054 |
| US9 | T077–T084 | FR-012, FR-016, FR-017, FR-018, FR-059, FR-064 |
| US10 | T085–T089 | FR-043, FR-056, FR-057, FR-058 |

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies; T001 first, because the submodule branches must exist before any `webui/` or `design/` work
- **Foundational (Phase 2)**: depends on Setup; **blocks every user story**
- **User Stories (Phase 3–12)**: each depends on Foundational only; within a story the order is tests → types → storage → service → surface
- **Polish (Phase 13)**: depends on the stories that were delivered

### User Story Dependencies

| Story | Depends on | Why |
|---|---|---|
| **US1** (P1) | Foundational | the shared question loop; nothing else needed |
| **US2** (P1) | US1 | several questions in one card is the same loop with a bigger payload |
| **US3** (P1) | US1 | channels present the set US1 already stores and validates |
| **US4** (P1) | Foundational | its own table, frame and tool; independent of the question loop |
| **US5** (P1) | US4 | measures the guarantee US4 provides |
| **US6** (P2) | US1 | describes and gates the question tool |
| **US7** (P2) | US4 | describes and gates the sensitive tool |
| **US8** (P2) | US4 | the non-happy outcomes of the same ask |
| **US9** (P2) | US1, US2 | safety, access and history of the question card |
| **US10** (P2) | US4 | the stated limits of the same guarantee |

US3, US4 and US6 can run in parallel once US1 (for US3/US6) or Foundational (for US4) is done.

### Within Each User Story

- Tests before the implementation they cover, and watch them fail
- Types before storage, storage before the service, the service before the surface
- The story's checkpoint is a real run, not a compile

---

## Parallel Execution Examples

**Setup + Foundational (different files, no shared edit):**

```text
T003 limits table        crates/vizier-core/src/questions/limits.rs
T004 limits tests        crates/vizier-core/src/questions/limits.rs
T007 ToolContext field   crates/vizier-core/src/agents/tools/mod.rs
T008 model types         crates/vizier-core/src/questions/model.rs
T009 surface trait       crates/vizier-core/src/questions/surface.rs
```

**User Story 1 (tests first, then the independent files):**

```text
T014 validation tests    crates/vizier-core/src/questions/validate.rs
T015 claim tests         crates/vizier-core/src/storage/sqlite/question.rs
T017 storage trait       crates/vizier-core/src/storage/question.rs
T019 validation impl     crates/vizier-core/src/questions/validate.rs
T024 AskQuestion tool    crates/vizier-core/src/questions/tool_question.rs
```

**Once US1 lands, three stories in parallel:**

```text
US3  crates/vizier-core/src/channels/{discord,telegram}/mod.rs + questions/text_form.rs
US4  crates/vizier-core/src/questions/{sensitive,tool_sensitive}.rs + storage
US6  crates/vizier-core/src/questions/tool_question.rs (description and gating)
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Phase 1 Setup → Phase 2 Foundational (blocks everything)
2. Phase 3 US1: the question loop, WebUI only
3. **STOP and validate** with quickstart.md §2–§3: the turn ends, the card appears, the answer arrives
4. This is a usable feature on its own — an agent can ask one question and act on the answer

### Incremental Delivery

1. **US1 + US2** — questions on the WebUI (the core of the request)
2. **US4 + US5** — the sensitive-input path with its guarantee measured (the other half of the request)
3. **US3** — the channels, so the question tool is genuinely universal
4. **US6–US10** — the contracts the agent reads, and the boundaries stated honestly
5. Each step leaves the previous ones working; nothing needs the later steps to be useful

### Notes for the implementer

- [P] means a different file and no dependency on an incomplete task
- Every task names its file; if a task needs a file another task owns, it is not [P]
- The two tools are one module on purpose: shared validation, shared limits, shared access rule. Do not split them into two modules
- A `cargo test` pass is not "channels work" — the Discord and Telegram controls need a live bot and are marked LIVE in quickstart.md §10
- Commit after each story's checkpoint, in the constitution's order (design → webui → vizier)

---

## Implementation notes

Recorded when the remaining tasks were implemented. Everything below was observed on a real
binary (`target/debug/vizier run`, workspace in a scratch directory) with an agent on the
`dummyplug` provider, through the real HTTP and WebSocket routes.

### Defects found by running it, and fixed

Unit tests and `cargo check` on the default features had not reached any of these.

- **Every answer panicked its turn.** `QuestionAnswer` was routed into the chat arm of
  `agents/process.rs`, but the prompt `match` there ended in `unreachable!()`; the first answer in
  the quickstart logged `turn panicked`. `chat_turn_prompt` now answers for each kind the arm
  accepts and `every_kind_the_chat_arm_accepts_has_a_prompt` pins it. `QuestionAnswer` also joins
  the `ThinkingStart` and reply-notification kinds.
- **Every reloaded card read `withdrawn`.** `fill_question_sets` looked a set up by the history
  row's `uid`, which is a fresh UUID, not the set id. It now keys on the entry's `set_id`
  (`a_pending_set_and_its_card_survive_a_restart`). CLAUDE.md said the uid was the set id; corrected.
- **The Discord and Telegram question code had never compiled.** Both are behind features that are
  off by default (34 errors under `--features discord,telegram`). Rewritten on one platform-neutral
  press interpreter (`questions/press.rs`, 9 tests): select menus, a set-level Submit and Dismiss,
  half-made answers held in memory, a text-form fallback that drops no question, plain text and
  no mentions. The old code also closed a set on a multi-choice toggle (an empty `claim_question_set`).
- **A failed `open_sensitive_ask` leaked the conversation's one ask slot** for good.
- **`sensitive_input_answer` accepted an ask from any conversation.** It now answers only an ask
  open in the socket's own conversation (verified on the binary: an attacker socket on another
  topic is told `sensitive_ask_not_open`, the victim's real answer still lands).
- **Deleting a conversation or an agent left its question sets and sensitive asks behind.**
- **The WebUI cards were never wired in.** T031 and T052 were ticked, but `frames.ts`, `turns.ts`
  and the chat route had no question handling and nothing imported either component. Now wired,
  with the refusal frames carrying `set_id`, `question_id` and `rule` as the contract says.
- `MAX_SET_PAYLOAD_BYTES` and `MAX_ANSWER_PAYLOAD_BYTES` existed but nothing enforced them.

### T027, T033 (quickstart §2, §3, §8, SC-003, SC-012)

- A valid set: `result` `{status: pending, question_set_id, questions}`, `stats.ended_by_tool`
  `ask_question`, one `QuestionSet` history entry in state `pending`.
- **SC-003**: 3 of 3 successful calls on an existing topic made exactly **1** completion (an
  ordinary tool turn makes 2). A call on a new topic also shows the title-generation completion.
- Answer over the socket: `question_set` frame (`answered`, `answerer`), then `thinking_start` and
  a `message`; history gains a `Request` whose content is `question_answer` and a `Response`.
  The same answer again: `client_error question_not_open`. An answer from another conversation:
  `question_not_found`, and the set stays `pending`.
- **SC-012**: a three-question, five-option set is one card (one `QuestionSet` entry). A bad answer
  among three is refused naming `db` / `value_not_offered`; two faults (`too_many_selections` and a
  blank required text) are named together; the set stays `pending` until a valid submission.
- The quickstart's §3 used the REST `chat` route, which accepts string content only; the answer is
  a WebSocket request. Its scripts also assigned to `result`, which the sandbox does not return.
  Both corrected in `quickstart.md`.

### T054, T061, T098 (quickstart §1, §4–§9, §11)

- §1 both tools and the `questions` and `secrets` categories offered; one `ENDS_TURN_NOTICE`.
- §4 an invalid set names all three problems at once, no `ended_by_tool`, no card.
- §5 the second set in a turn is refused, one `QuestionSet` entry.
- §6 a scheduled task run offers neither tool and has no `questions` category (`set_secret` is
  still there); `/btw` mentions no `ask_` tool.
- §7 four pending sets survived a restart and read `pending`.
- §9 offline: the `sensitive_ask` frame reaches every socket; the script printed
  `[redacted]` and received the value; the turn kept running (`ended_by_tool` absent); a second
  tab closed on the first answer; decline gives `declined`; a 4-character value gives
  `protected: false`. **T061 (SC-022)**: the value was in 0 lines of the server log, 0 of the
  dummyplug traffic, 0 of the REST response and history, and 0 rows in **any text column of any
  table**.
- §11 deleting a conversation removed its set and ask rows; deleting the agent removed the rest.

### T099 (pre-existing database)

Simulated rather than run against an old binary: the two new tables were dropped from a
populated workspace database (99 history rows, 1 agent) to give it the shape the previous build
leaves, and the new binary was started on it. Both tables came back, no errors, history intact.
No migration or compatibility code was added (research R13), so nothing belongs in #82.

### Not done, and why

- **T044, the LIVE channel checks (§10)** need a real Discord bot and a real Telegram bot. The
  controls, the fallback and the press logic are unit-tested and compile under the features, but
  no platform has seen them.
- **T067 / SC-008 / SC-020 / SC-025** ("a valid set on the first or second attempt in at least
  95% of 20 prompts") need a real model and 20 prompts. The offline half (tool listing, refusal,
  availability) is verified above.
- **T079**: focus moves to the first problem on a refused submission and every field is labelled,
  both in code and unit-tested; a keyboard-only walk-through in a browser has not been done.
- **T080**: the light/dark/phone stories need Storybook in a browser and, per CLAUDE.md, a FigCraft
  pass. Not done; `design/` is unchanged by this feature.
- **T097**: `webui` typecheck and its 653 tests pass; `cd design && npm test` cannot start here
  because Playwright's Chromium is not installed (`npx playwright install`).
- **T100**: see the commits.
