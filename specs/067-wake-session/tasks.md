# Tasks: Waking an agent in any conversation

**Input**: Design documents from `/specs/067-wake-session/`

**Prerequisites**: plan.md, spec.md, research.md (D1–D16), data-model.md, contracts/session-tools.md,
quickstart.md

**Tests**: included. The constitution's e2e gate requires an agent-observable change to be exercised
end to end against a running binary with a `dummyplug` agent, and every requirement here is
agent-observable, so the test tasks are not optional.

**Organization**: Tasks are grouped by user story so each story can be implemented, tested and
delivered on its own. The request kind and the shared helpers sit in the Foundational phase,
because every story needs them and because the compiler refuses to build until every exhaustive
match on the new kind is classified.

**Paths**: this repository is a Rust workspace (`crates/`) with the WebUI in `webui/`. Engine code
goes in `crates/vizier-core/`, the WebUI in `webui/`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1…US6)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the branch, the submodule branches the constitution asks for, and a clean baseline.

- [ ] T001 Create the matching submodule branches with `just checkout 067-wake-session` — the `speckit.git.feature` hook branched only the workspace, and `landing` and `dummyplug` are submodules (constitution: matching submodule branches)
- [ ] T002 [P] Confirm a clean baseline: run `cargo clippy` and `cargo test` from the repository root and resolve any pre-existing failure before starting (record pre-existing failures in this file's Notes, so this feature's diff is not blamed for unrelated breakage)
- [ ] T003 [P] Confirm a clean WebUI baseline: run `cd webui && npm run typecheck` and record any pre-existing failure in this file's Notes

---

## Phase 2: Foundational — the woken request and the shared helpers (Blocking Prerequisites)

**Purpose**: the request kind the woken turn carries, its classification at every seam, the
channel-slug parser and the access helpers. Nothing an agent can call yet.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T004 Add `VizierRequestContent::Woken(String)` to the enum, with its `kind()` arm, in `crates/vizier-core/src/schema/request.rs`
- [ ] T005 Add the `Display` arm for `Woken`: the constant framing — that the agent started this turn itself, that it is not a person's message, and that nobody is waiting on the answer here — followed by the instruction, in `crates/vizier-core/src/schema/request.rs` (depends on T004)
- [ ] T006 Add the frontmatter arm in `generate_frontmatter` — `sender: self.user`, `via: wake`, `sent_at`, `metadata` — in `crates/vizier-core/src/schema/request.rs` (depends on T004)
- [ ] T007 Add `Woken` to the **history arm** of `handle_request` so the woken turn runs with the target conversation's earlier turns in view — it must not go into the `Unattended` arm, whose turns run with an empty history — in `crates/vizier-core/src/agents/process.rs` (depends on T004)
- [ ] T008 Add the `Woken` arm to `chat_turn_prompt` in `crates/vizier-core/src/agents/process.rs` (depends on T007)
- [ ] T009 Add `Woken` to `notifies_chat` so the woken reply raises the person's one inbox notification, in `crates/vizier-core/src/agents/process.rs` (depends on T007)
- [ ] T010 Add the `Woken` arm to `is_interactive_turn` — false, so the question and sensitive-input tools are not offered in a woken turn — in `crates/vizier-core/src/agents/tools/mod.rs` (depends on T004)
- [ ] T011 [P] Create `crates/vizier-core/src/agents/tools/sessions/mod.rs` with the module doc and the private `wakeable_channel(slug: &str) -> Option<VizierChannelId>` parser: it accepts `discord__{u64}`, `telegram__{i64}`, `http__{user}__{channel}` and `project__{user}__{id}`, and returns `None` for every other slug (research D2)
- [ ] T012 [P] Create `crates/vizier-core/src/agents/tools/sessions/access.rs` with `may_wake(ctx: &ToolContext) -> bool` — true only where a person's access can be decided: not in a dream, not in a subagent worker, and not in a Discord or Telegram conversation — plus the refusal-value helpers and the person's own identity, resolved through `consultations::access::consulter` (research D9, D10)
- [ ] T013 [P] Create `crates/vizier-core/src/agents/tools/sessions/tests.rs` with the parser's unit tests: the four wakeable slugs parse, `task__`, `SUBAGENT`, `consultation__`, `DREAM__`, `SYSTEM`, `inter_agent__` and `workspace__` do not, and a channel id containing `__` survives (depends on T011)

**Checkpoint**: `cargo build` passes. Every exhaustive match on the content kind is classified —
that is where FR-021 is enforced, and the compiler is the checklist.

---

## Phase 3: User Story 1 - An agent starts work in another conversation (Priority: P1) 🎯 MVP

**Goal**: `wake_session` starts a turn in one of the calling agent's own conversations. The agent
works on it there, with that conversation's history in view, and its reply lands there.

**Independent Test**: quickstart §2 and §3. An agent wakes a conversation of its own; the work
happens in that conversation, the request that woke it carries `sender: <the agent>` and
`via: wake`, and the woken turn sees the conversation's earlier turns.

### Implementation for User Story 1

- [ ] T014 [US1] Implement `WakeSession` — the arguments `target`, `topic_id?`, `instruction`, `wait?` and the result `{channel, topic_id, reply?}` from `data-model.md`, with **no** agent id — in `crates/vizier-core/src/agents/tools/sessions/mod.rs`
- [ ] T015 [US1] Resolve the target against the calling agent: `"current"` is the calling turn's own conversation; any other value is parsed with `wakeable_channel`, and a slug that does not parse is refused as `unknown_channel` and one that names a conversation the machinery owns is refused as `not_wakeable`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T011)
- [ ] T016 [US1] Check the access before dispatching: a WebUI or project conversation must belong to the person this turn is working for, and a turn with no person must have an owner — a refusal for either is returned as data, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T012, T015)
- [ ] T017 [US1] Check the hop limit against `MAX_BACKGROUND_DEPTH` and carry `background_depth + 1` on the request, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T016)
- [ ] T018 [US1] Dispatch the request with `transport.send_request`, the `Woken` content and `user` set to the calling agent's id, and return the target's address at once, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T017)
- [ ] T019 [US1] Add `ToolCategory::Sessions` **appended** to the enum and to `ToolCategory::ALL` (24 → 25), with `name()` returning `"sessions"`, in `crates/vizier-core/src/agents/tools/mod.rs`
- [ ] T020 [US1] Register `wake_session` on `default_toolset`, add it to `every_tool()` and to the `EXPECTED` table in `every_tool_is_in_its_category`, and update `a_new_category_is_appended_to_the_end_of_the_list` (24 → 25, last is `sessions`), in `crates/vizier-core/src/agents/tools/mod.rs` (depends on T019)
- [ ] T021 [P] [US1] Add the `wake_session` line to `tool_call_title` in `crates/vizier-core/src/utils/mod.rs`
- [ ] T022 [US1] Write the caller's guidance into the tool's `description()`: waking starts a turn in the target conversation and writes into its history, posting a message only says something there, and the target must be named rather than guessed, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T014)
- [ ] T023 [US1] Write the unit tests for the wake — the target resolution, the refusals and the hop limit — in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T018)

**Checkpoint**: User Story 1 works on its own. An agent wakes one of its own conversations and the
work lands there.

---

## Phase 4: User Story 2 - An agent finds the conversation to work in (Priority: P1)

**Goal**: `list_sessions` lists the calling agent's own conversations, each with a reference the
wake takes back unchanged.

**Independent Test**: quickstart §1. The listing holds the calling conversation under `"current"`,
one row per topic, narrowed by a channel and by a title, and another person's conversations and
the machinery's own conversations are absent.

### Implementation for User Story 2

- [ ] T024 [US2] Implement `ListSessions` — the arguments `channel?`, `query?` and the rows `{channel, topic_id, title, is_thinking}` from `data-model.md`, with **no** agent id — in `crates/vizier-core/src/agents/tools/sessions/mod.rs`
- [ ] T025 [US2] Read the rows with `storage.get_session_list(agent_id, None)` and filter them to the wakeable channels and to the conversations that belong to the person this turn is working for, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T012, T024)
- [ ] T026 [US2] Apply the channel filter on the channel slug and the title search on the title, and always include the calling turn's own conversation under `"current"`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T025)
- [ ] T027 [US2] Register `list_sessions` on `default_toolset` and add it to `every_tool()` and to the `EXPECTED` table, in `crates/vizier-core/src/agents/tools/mod.rs` (depends on T020)
- [ ] T028 [P] [US2] Add the `list_sessions` line to `tool_call_title` in `crates/vizier-core/src/utils/mod.rs`
- [ ] T029 [US2] Write the unit tests for the listing — the two filters, the title search, the `"current"` row, and that another person's conversation is absent — in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T026)

**Checkpoint**: User Stories 1 and 2 both work independently.

---

## Phase 5: User Story 3 - Only the caller's own conversations may be woken (Priority: P1)

**Goal**: the boundary. The conversation that is woken belongs to the calling agent and to the
person it is working for, and no tool names another agent at all.

**Independent Test**: quickstart §5 and §6. Every refusal is a readable value, another person's
conversation is refused, a conversation the machinery owns is refused, and neither tool exposes an
agent id.

### Implementation for User Story 3

- [ ] T030 [US3] Make every refusal one value — `{"status": …, "message": …}` with the statuses from `data-model.md` — returned as data the agent can read, never as a failure of the call, in `crates/vizier-core/src/agents/tools/sessions/mod.rs`
- [ ] T031 [US3] Offer the two tools only where a person's access can be decided, using `sessions::access::may_wake` — so they are absent in a dream, in a subagent worker, in a Discord or Telegram conversation and in work started from one, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T012)
- [ ] T032 [US3] Refuse a WebUI or project conversation of another person with `other_person`, comparing the target's `web_user()` with the consulting person's username, and refuse a turn with no person with `no_owner`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T030)
- [ ] T033 [US3] Write the unit test for every refusal status, in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T032)
- [ ] T034 [US3] Write the test that neither tool's arguments carry an agent id, so no conversation of another agent can be named, in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T033)

**Checkpoint**: All three P1 stories work independently.

---

## Phase 6: User Story 5 - The caller waits for the answer (Priority: P2)

**Goal**: a wake that asks to wait returns the woken turn's answer in the same call.

**Independent Test**: quickstart §4. With `wait`, the call returns the answer; without it, the
call returns at once with no answer and the answer still lands in the target conversation; an
interrupted wait ends without stopping the woken turn.

### Implementation for User Story 5

- [ ] T035 [US5] Open a reply channel with `transport.reply_channel()` and, when `wait` is true, send the request with the channel attached, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T018)
- [ ] T036 [US5] Read the frames until one that ends the turn (`VizierResponse::ends_turn()`) and return the woken turn's answer, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T035)
- [ ] T037 [US5] End the wait without stopping the woken turn when the caller is interrupted or the call is dropped, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T036)
- [ ] T038 [US5] Write the unit tests for the wait and for the interrupted wait, in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T037)

**Checkpoint**: User Story 5 works on its own.

---

## Phase 7: User Story 4 - A scheduled run puts its work in the person's own conversation (Priority: P2)

**Goal**: a job's work happens where the person sees it. No change to how jobs are scheduled — the
agent in the run uses the same two tools.

**Independent Test**: quickstart §7. A task whose instruction names the person's WebUI conversation
puts its answer in that conversation, and a task that names none behaves exactly as today.

### Implementation for User Story 4

- [ ] T039 [US4] Refuse `"current"` in a conversation the machinery owns with `not_wakeable`, so a scheduled run reaches the person's conversation by naming it and not by naming `"current"`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T015)
- [ ] T040 [US4] Say in the tool's `description()` that a job may name the conversation to work in, and that the work then runs there, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T022)
- [ ] T041 [US4] Write the e2e case: a task whose instruction wakes the person's WebUI conversation, and the answer lands in that conversation rather than in the task's own, in `crates/vizier-core/tests/dummyplug_e2e.rs` (depends on T039)

**Checkpoint**: User Story 4 works on its own.

---

## Phase 8: User Story 6 - The person sees what woke their conversation (Priority: P3)

**Goal**: in a conversation of their own, the person sees that the agent started work there and
what it produced — the request drawn as an agent's message, not the person's.

**Independent Test**: quickstart §8. The activity trail names the session tools, and the woken
request is drawn as an agent's message.

### Implementation for User Story 6

- [ ] T042 [US6] Add the woken arm to the `VizierRequestContent` union in `webui/app/api/frames.ts`
- [ ] T043 [US6] Add the woken arm to `requestText` so the instruction is what the chat shows, and draw the woken request as an agent's message rather than the person's bubble, in `webui/app/lib/messages.ts` and `webui/app/screens/chat/MessageList.tsx` (depends on T042)
- [ ] T044 [P] [US6] Add `'sessions'` to `BuiltinCategory` and `list_sessions` and `wake_session` to `BUILTIN_TOOLS` in `webui/app/lib/toolCategories.ts`
- [ ] T045 [P] [US6] Add the category's label, icon and phrase to the map in `webui/app/lib/toolIcons.ts`, so the trail says what the agent did
- [ ] T046 [US6] Regenerate `webui/app/api/schema.d.ts` and `webui/openapi.json` with `just openapi` (needs a running server; the recipe's default is `http://localhost:9999`)
- [ ] T047 [US6] Write the test that the woken request is drawn as an agent's message and that `requestText` returns its instruction, in `webui/app/api/frames.wake.test.ts` (depends on T043)

**Checkpoint**: all six stories work independently.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T048 Regenerate the public API listing with `just public-api` — the new enum variant and the new public types change `crates/vizier-core/public-api.txt`
- [ ] T049 Update `CLAUDE.md`: the tools section (the two tools, the category, the request kind), the category count (24 → 25) and the SPECKIT pointer
- [ ] T050 Run `cargo clippy` and `cargo test` and resolve every warning and failure in the touched files
- [ ] T051 Run `cd webui && npm run typecheck` and `just web-check`
- [ ] T052 Run `just test-scripts`, which includes `scripts/trail-tool-categories.test.mjs` — the test that fails when the WebUI's copy of the categories and the engine's disagree
- [ ] T053 The e2e gate (constitution): on a fresh `just run`, walk every step of `quickstart.md` with dummyplug agents and mark each step pass or fail in this file's Notes. Run the live-model check only where behaviour depends on real model output, and mark it as such
- [ ] T054 Commit with a conventional message (`feat: add the session tools — list_sessions and wake_session`)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies — start immediately
- **Foundational (Phase 2)**: depends on Phase 1 — **blocks every user story**
- **User Stories (Phases 3–8)**: all depend on Phase 2
  - US1, US2 and US3 are P1 and are sequential in practice, because each of them edits
    `agents/tools/mod.rs` and `agents/tools/sessions/mod.rs`
  - US5 depends on US1 (it adds the wait to the same tool)
  - US4 depends on US1 (a job uses the wake) and on US2 (it finds the conversation)
  - US6 depends on the request kind, so it may start after Phase 2
- **Polish (Phase 9)**: depends on the stories being done

### User Story Dependencies

- **US1 (P1)**: after Phase 2 — no dependency on another story
- **US2 (P1)**: after Phase 2 — shares the module and the access helpers with US1
- **US3 (P1)**: after Phase 2 — the refusals live in the tools US1 and US2 build
- **US5 (P2)**: after US1 — it adds the wait to `wake_session`
- **US4 (P2)**: after US1 and US2 — a job lists the conversation and wakes it
- **US6 (P3)**: after Phase 2 — the WebUI renders the kind Phase 2 adds

### Within Each User Story

- Tests are written with the implementation and must fail before it passes
- The access check lands **with** the tool, never after it — an intermediate commit must not offer
  a wake that reaches another person's conversation
- The category and the registration land with the first tool that needs them (US1), and the second
  tool adds its own line to the same tables (US2)
- **Neither tool grows an agent id.** That is the user's call, and it is what keeps the model from
  having to name an agent at all

### Parallel Opportunities

- T002 and T003 (the two baselines) can run together
- In Phase 2: the `schema/request.rs` group (T004–T006), the `agents/process.rs` group (T007–T009),
  the `agents/tools/mod.rs` arm (T010), the sessions module (T011) and the sessions access module
  (T012) are four groups over four different files
- T021 and T028 (the two `tool_call_title` lines) can run together, and separately from the tool
  bodies
- T044 and T045 (the two WebUI maps) can run together
- US6's WebUI work can run in parallel with the remaining core stories, since it touches only
  `webui/`

---

## Parallel Example: Phase 2

```bash
# Four groups over four files, at the same time:
Task: "T004–T006 the woken request, its framing and its frontmatter — crates/vizier-core/src/schema/request.rs"
Task: "T007–T009 the history arm and its two neighbours — crates/vizier-core/src/agents/process.rs"
Task: "T010 the interactive-turn arm — crates/vizier-core/src/agents/tools/mod.rs"
Task: "T011–T012 the slug parser and the access helpers — crates/vizier-core/src/agents/tools/sessions/"
```

---

## Parallel Example: User Story 1

```bash
Task: "T018 the dispatch — crates/vizier-core/src/agents/tools/sessions/mod.rs"
Task: "T021 the call title — crates/vizier-core/src/utils/mod.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks every story)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: walk quickstart §2 and §3 against a running binary
5. Ship it: an agent can already work in another of its own conversations

### Incremental Delivery

1. Setup + Foundational → the foundation is ready
2. US1 → the wake works → ship (MVP)
3. US2 → the listing works → ship (the agent can find what to wake)
4. US3 → the boundary holds → ship (the access rules are proven)
5. US5 → the caller can wait → ship
6. US4 → a job can work in the person's thread → ship
7. US6 → the person sees it → ship
8. Polish: docs, the public API, the full quickstart run

Each story adds value without breaking the ones before it.

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps each task to its story for traceability
- Every task names the file it changes, so it can be picked up cold
- Stop at any checkpoint to validate that story independently
- The access check and the `available()` rule land **with** the tools: an intermediate commit must
  never offer a wake that reaches another person's conversation
- The exhaustive matches on `VizierRequestContent` fail the build until the new kind is classified
  — that is deliberate, and Phase 2 is where it is satisfied
- `just openapi` needs a running server; `just public-api` needs the nightly rustdoc toolchain
  (`just install`)
- The consultation chain is **not** touched by this feature: with one agent, there is no cycle it
  would catch, and the hop limit is the only guard a wake needs (research D8)
