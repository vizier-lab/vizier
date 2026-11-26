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

- [ ] T004 Add `WokenRequest { from_agent: AgentId, instruction: String }` with the `Serialize`/`Deserialize`/`JsonSchema` derives the other request payloads use, in `crates/vizier-core/src/schema/request.rs`
- [ ] T005 [P] Add `VizierRequestContent::Woken(WokenRequest)` to the enum, with its `kind()` arm and its `Display` arm, in `crates/vizier-core/src/schema/request.rs` (depends on T004)
- [ ] T006 Add the frontmatter arm in `generate_frontmatter` — `sender: <from_agent>`, `via: wake`, `sent_at`, `metadata` — in `crates/vizier-core/src/schema/request.rs` (depends on T005)
- [ ] T007 Widen `VizierRequest::consultation_chain`'s doc comment to say it is the agents above this turn, consultations and wakes alike, keeping its name and type, in `crates/vizier-core/src/schema/request.rs` (depends on T005)
- [ ] T008 Add `Woken` to the **history arm** of `handle_request` so the woken turn runs with the target conversation's earlier turns in view — it must not go into the `Unattended` arm, whose turns run with an empty history — in `crates/vizier-core/src/agents/process.rs` (depends on T005)
- [ ] T009 Add the `Woken` arm to `chat_turn_prompt` in `crates/vizier-core/src/agents/process.rs` (depends on T008)
- [ ] T010 Add `Woken` to `notifies_chat` so the woken reply raises the person's one inbox notification, in `crates/vizier-core/src/agents/process.rs` (depends on T008)
- [ ] T011 Add the `Woken` arm to `is_interactive_turn` — false, so the question and sensitive-input tools are not offered in a woken turn — in `crates/vizier-core/src/agents/tools/mod.rs` (depends on T005)
- [ ] T012 [P] Create `crates/vizier-core/src/agents/tools/sessions/mod.rs` with the module doc and the private `wakeable_channel(slug: &str) -> Option<VizierChannelId>` parser: it accepts `discord__{u64}`, `telegram__{i64}`, `http__{user}__{channel}` and `project__{user}__{id}`, and returns `None` for every other slug (research D2)
- [ ] T013 [P] Create `crates/vizier-core/src/agents/tools/sessions/access.rs` with `may_wake(ctx: &ToolContext) -> bool` — true only where a person's access can be decided: not in a dream, not in a subagent worker, and not in a Discord or Telegram conversation — plus the refusal-value helpers (research D9, D10)
- [ ] T014 [P] Create `crates/vizier-core/src/agents/tools/sessions/tests.rs` with the parser's unit tests: the four wakeable slugs parse, `task__`, `SUBAGENT`, `consultation__`, `DREAM__`, `SYSTEM`, `inter_agent__` and `workspace__` do not, and a channel id containing `__` survives (depends on T012)

**Checkpoint**: `cargo build` passes. Every exhaustive match on the content kind is classified —
that is where FR-023 is enforced, and the compiler is the checklist.

---

## Phase 3: User Story 1 - An agent starts work in another conversation (Priority: P1) 🎯 MVP

**Goal**: `wake_session` starts a turn in a named conversation. The woken agent runs as itself, in
that conversation, with that conversation's history in view, and its reply lands there.

**Independent Test**: quickstart §2 and §3. An agent wakes a conversation of its own; the work
happens in that conversation, the request that woke it carries `sender: <the waking agent>` and
`via: wake`, and the woken turn sees the conversation's earlier turns.

### Implementation for User Story 1

- [ ] T015 [US1] Implement `WakeSession` — the arguments `target`, `agent_id?`, `topic_id?`, `instruction`, `wait?` and the result `{agent_id, channel, topic_id, reply?}` from `data-model.md` — in `crates/vizier-core/src/agents/tools/sessions/mod.rs`
- [ ] T016 [US1] Resolve the target: `"current"` resolves to the calling turn's own conversation; any other value is parsed with `wakeable_channel`, and a slug that does not parse is refused as `unknown_channel` and one that names a conversation the machinery owns is refused as `not_wakeable`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T012)
- [ ] T017 [US1] Check the access before dispatching: the target agent must be one this turn may consult, and a WebUI or project conversation must belong to the person this turn is working for — a refusal for either is returned as data, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T013, T016)
- [ ] T018 [US1] Check the hop limit against `MAX_BACKGROUND_DEPTH` and the chain against `consultation_chain`, and carry `background_depth + 1` and `consultation_chain + [the calling agent]` on the request, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T016)
- [ ] T019 [US1] Dispatch the request with `transport.send_request`, the `Woken` content and `user` set to the waking agent's id, and return the target's address at once, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T018)
- [ ] T020 [US1] Add `ToolCategory::Sessions` **appended** to the enum and to `ToolCategory::ALL` (24 → 25), with `name()` returning `"sessions"`, in `crates/vizier-core/src/agents/tools/mod.rs`
- [ ] T021 [US1] Register `wake_session` on `default_toolset`, add it to `every_tool()` and to the `EXPECTED` table in `every_tool_is_in_its_category`, and update `a_new_category_is_appended_to_the_end_of_the_list` (24 → 25, last is `sessions`), in `crates/vizier-core/src/agents/tools/mod.rs` (depends on T020)
- [ ] T022 [P] [US1] Add the `wake_session` line to `tool_call_title` in `crates/vizier-core/src/utils/mod.rs`
- [ ] T023 [US1] Write the caller's guidance into the tool's `description()`: waking starts a turn in the target conversation at that agent's cost and writes into its history, posting a message only says something there at the caller's cost, and the target must be named rather than guessed, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T015)
- [ ] T024 [US1] Write the unit tests for the wake — the target resolution, the refusals, the hop limit and the chain — in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T019)

**Checkpoint**: User Story 1 works on its own. An agent wakes a conversation it owns and the work
lands there.

---

## Phase 4: User Story 2 - An agent finds the conversation to work in (Priority: P1)

**Goal**: `list_sessions` lists the conversations the calling turn may reach, each with a reference
the wake takes back unchanged.

**Independent Test**: quickstart §1. The listing holds the calling conversation under `"current"`,
one row per topic, narrowed by a channel and by a title, and another person's conversations and
the machinery's own conversations are absent.

### Implementation for User Story 2

- [ ] T025 [US2] Implement `ListSessions` — the arguments `agent_id?`, `channel?`, `query?` and the rows `{agent_id, channel, topic_id, title, is_thinking}` from `data-model.md` — in `crates/vizier-core/src/agents/tools/sessions/mod.rs`
- [ ] T026 [US2] Read the rows with `storage.get_session_list(agent_id, None)` and filter them to the wakeable channels, to the agents this turn may reach, and to the conversations that belong to the person this turn is working for, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T013, T025)
- [ ] T027 [US2] Apply the channel filter on the channel slug and the title search on the title, and always include the calling turn's own conversation under `"current"`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T026)
- [ ] T028 [US2] Register `list_sessions` on `default_toolset` and add it to `every_tool()` and to the `EXPECTED` table, in `crates/vizier-core/src/agents/tools/mod.rs` (depends on T021)
- [ ] T029 [P] [US2] Add the `list_sessions` line to `tool_call_title` in `crates/vizier-core/src/utils/mod.rs`
- [ ] T030 [US2] Write the unit tests for the listing — the three filters, the title search, the `"current"` row, and that another person's conversation is absent — in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T027)

**Checkpoint**: User Stories 1 and 2 both work independently.

---

## Phase 5: User Story 3 - Only conversations the caller may reach can be woken (Priority: P1)

**Goal**: the boundary. A person who may not use an agent cannot cause a turn to reach one of its
conversations, and cannot tell the agent exists.

**Independent Test**: quickstart §5 and §6. Every refusal is a readable value, an agent the person
may not use is refused exactly as an agent that does not exist, another person's conversation is
refused, and a conversation the machinery owns is refused.

### Implementation for User Story 3

- [ ] T031 [US3] Make every refusal one value — `{"status": …, "message": …}` with the statuses from `data-model.md` — returned as data the agent can read, never as a failure of the call, in `crates/vizier-core/src/agents/tools/sessions/mod.rs`
- [ ] T032 [US3] Offer the two tools only where a person's access can be decided, using `sessions::access::may_wake` — so they are absent in a dream, in a subagent worker, in a Discord or Telegram conversation and in work started from one, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T013)
- [ ] T033 [US3] Answer an agent the person may not use exactly as an agent that does not exist — the same status and the same message — using `consultations::access::consultable`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T031)
- [ ] T034 [US3] Refuse a WebUI or project conversation of another person with `other_person`, comparing the target's `web_user()` with the consulting person's username, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T031)
- [ ] T035 [US3] Write the unit test for every refusal status, and the test that an unknown agent and a forbidden agent answer identically, in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T034)

**Checkpoint**: All three P1 stories work independently.

---

## Phase 6: User Story 5 - The caller waits for the answer (Priority: P2)

**Goal**: a wake that asks to wait returns the woken turn's answer in the same call.

**Independent Test**: quickstart §4. With `wait`, the call returns the answer; without it, the
call returns at once with no answer and the answer still lands in the target conversation; an
interrupted wait ends without stopping the woken turn.

### Implementation for User Story 5

- [ ] T036 [US5] Open a reply channel with `transport.reply_channel()` and, when `wait` is true, send the request with the channel attached, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T019)
- [ ] T037 [US5] Read the frames until one that ends the turn (`VizierResponse::ends_turn()`) and return the woken turn's answer, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T036)
- [ ] T038 [US5] End the wait without stopping the woken turn when the caller is interrupted or the call is dropped, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T037)
- [ ] T039 [US5] Write the unit tests for the wait and for the interrupted wait, in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T038)

**Checkpoint**: User Story 5 works on its own.

---

## Phase 7: User Story 4 - A scheduled run puts its work in the person's own conversation (Priority: P2)

**Goal**: a job's work happens where the person sees it. No change to how jobs are scheduled — the
agent in the run uses the same two tools.

**Independent Test**: quickstart §7. A task whose instruction names the person's WebUI conversation
puts its answer in that conversation, and a task that names none behaves exactly as today.

### Implementation for User Story 4

- [ ] T040 [US4] Refuse `"current"` in a conversation the machinery owns with `not_wakeable`, so a scheduled run reaches the person's conversation by naming it and not by naming `"current"`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T016)
- [ ] T041 [US4] Say in the tool's `description()` that a job may name the conversation to work in, and that the work then runs there, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T023)
- [ ] T042 [US4] Write the e2e case: a task whose instruction wakes the person's WebUI conversation, and the answer lands in that conversation rather than in the task's own, in `crates/vizier-core/tests/dummyplug_e2e.rs` (depends on T040)

**Checkpoint**: User Story 4 works on its own.

---

## Phase 8: User Story 6 - The person sees what woke their conversation (Priority: P3)

**Goal**: in a conversation of their own, the person sees that another agent started work there and
what it produced — the request drawn as an agent's message, not the person's.

**Independent Test**: quickstart §8. The activity trail names the session tools, and the woken
request is drawn as an agent's message.

### Implementation for User Story 6

- [ ] T043 [US6] Add the woken arm and its `WokenContent` to the `VizierRequestContent` union in `webui/app/api/frames.ts`
- [ ] T044 [US6] Add the woken arm to `requestText` so the instruction is what the chat shows, and draw the woken request as an agent's message rather than the person's bubble, in `webui/app/lib/messages.ts` and `webui/app/screens/chat/MessageList.tsx` (depends on T043)
- [ ] T045 [P] [US6] Add `'sessions'` to `BuiltinCategory` and `list_sessions` and `wake_session` to `BUILTIN_TOOLS` in `webui/app/lib/toolCategories.ts`
- [ ] T046 [P] [US6] Add the category's label, icon and phrase to the map in `webui/app/lib/toolIcons.ts`, so the trail says what the agent did
- [ ] T047 [US6] Regenerate `webui/app/api/schema.d.ts` and `webui/openapi.json` with `just openapi` (needs a running server; the recipe's default is `http://localhost:9999`)
- [ ] T048 [US6] Write the test that the woken request is drawn as an agent's message and that `requestText` returns its instruction, in `webui/app/api/frames.wake.test.ts` (depends on T044)

**Checkpoint**: all six stories work independently.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T049 Regenerate the public API listing with `just public-api` — the new enum variant and the new public types change `crates/vizier-core/public-api.txt`
- [ ] T050 Update `CLAUDE.md`: the tools section (the two tools, the category, the request kind), the category count (24 → 25) and the SPECKIT pointer
- [ ] T051 Run `cargo clippy` and `cargo test` and resolve every warning and failure in the touched files
- [ ] T052 Run `cd webui && npm run typecheck` and `just web-check`
- [ ] T053 Run `just test-scripts`, which includes `scripts/trail-tool-categories.test.mjs` — the test that fails when the WebUI's copy of the categories and the engine's disagree
- [ ] T054 The e2e gate (constitution): on a fresh `just run`, walk every step of `quickstart.md` with dummyplug agents and mark each step pass or fail in this file's Notes. Run the live-model check only where behaviour depends on real model output, and mark it as such
- [ ] T055 Commit with a conventional message (`feat: add the session tools — list_sessions and wake_session`)

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
- The access check lands **with** the tool, never after it — an intermediate commit must not offer a
  wake that reaches an agent the caller may not use
- The category and the registration land with the first tool that needs them (US1), and the second
  tool adds its own line to the same tables (US2)

### Parallel Opportunities

- T002 and T003 (the two baselines) can run together
- In Phase 2: the `schema/request.rs` group (T004–T007), the `agents/process.rs` group (T008–T010),
  the `agents/tools/mod.rs` arm (T011), the sessions module (T012) and the sessions access module
  (T013) are four groups over four different files
- T022 and T029 (the two `tool_call_title` lines) can run together, and separately from the tool
  bodies
- T045 and T046 (the two WebUI maps) can run together
- US6's WebUI work can run in parallel with the remaining core stories, since it touches only
  `webui/`

---

## Parallel Example: Phase 2

```bash
# Four groups over four files, at the same time:
Task: "T004–T007 the woken request and its frontmatter — crates/vizier-core/src/schema/request.rs"
Task: "T008–T010 the history arm and its two neighbours — crates/vizier-core/src/agents/process.rs"
Task: "T011 the interactive-turn arm — crates/vizier-core/src/agents/tools/mod.rs"
Task: "T012–T013 the slug parser and the access helpers — crates/vizier-core/src/agents/tools/sessions/"
```

---

## Parallel Example: User Story 1

```bash
Task: "T019 the dispatch — crates/vizier-core/src/agents/tools/sessions/mod.rs"
Task: "T022 the call title — crates/vizier-core/src/utils/mod.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks every story)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: walk quickstart §2 and §3 against a running binary
5. Ship it: an agent can already work in another conversation

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
  never offer a wake that reaches an agent the caller may not use
- The exhaustive matches on `VizierRequestContent` fail the build until the new kind is classified
  — that is deliberate, and Phase 2 is where it is satisfied
- `just openapi` needs a running server; `just public-api` needs the nightly rustdoc toolchain
  (`just install`)
