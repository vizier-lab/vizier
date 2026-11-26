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

- [X] T001 Create the matching submodule branches with `just checkout 067-wake-session` — the `speckit.git.feature` hook branched only the workspace, and `landing` and `dummyplug` are submodules (constitution: matching submodule branches)
- [X] T002 [P] Confirm a clean baseline: run `cargo clippy` and `cargo test` and record any pre-existing failure in this file's Notes — **two Windows path failures, pre-existing** (see Notes)
- [X] T003 [P] Confirm a clean WebUI baseline: run `cd webui && npm run typecheck` and record any pre-existing failure in this file's Notes — clean

---

## Phase 2: Foundational — the woken request, the tools' helpers, the category (Blocking Prerequisites)

**Purpose**: the request kind the woken turn carries, its classification at every seam, the
channel-slug parser, the access helpers, and the category the tools live in. Nothing an agent can
call yet.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T004 Add `VizierRequestContent::Woken(String)` to the enum, with its `kind()` arm, in `crates/vizier-core/src/schema/request.rs`
- [X] T005 [P] Write the woken framing in a new `crates/vizier-core/src/agents/agent/system_prompt/woken.rs` — the `## Woken turn` section and the `woken_note` reader — and register the module in `system_prompt/mod.rs` (research D4: the framing rides the **context block**, not the message)
- [X] T006 Add the frontmatter arm in `generate_frontmatter` — `sender: self.user`, `via: wake`, `sent_at`, `metadata` — in `crates/vizier-core/src/schema/request.rs`
- [X] T007 Add `Woken` to the **history arm** of `handle_request` so the woken turn runs with the target conversation's earlier turns in view — it must not go into the `Unattended` arm, whose turns run with an empty history — in `crates/vizier-core/src/agents/process.rs`
- [X] T008 Add the `Woken` arm to `chat_turn_prompt` in `crates/vizier-core/src/agents/process.rs`
- [X] T009 Add `Woken` to `notifies_chat` so the woken reply raises the person's one inbox notification, in `crates/vizier-core/src/agents/process.rs`
- [X] T010 Add `Woken` to the thinking-start list, so the conversation it names becomes busy, in `crates/vizier-core/src/agents/process.rs`
- [X] T011 Add the `Woken` arm to `is_interactive_turn` — false, so the question and sensitive-input tools are not offered in a woken turn — in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T012 [P] Put the woken note in the turn's context block, so the request's own text stays exactly what the agent wrote, in `crates/vizier-core/src/agents/agent/mod.rs`
- [X] T013 [P] Create `crates/vizier-core/src/agents/tools/sessions/mod.rs` with the module doc, the private `wakeable_channel(slug) -> Result<VizierChannelId, Refusal>` parser and the `machinery_kind` reader (research D2)
- [X] T014 [P] Create `crates/vizier-core/src/agents/tools/sessions/access.rs` with `may_wake(ctx)`, the `Refusal` enum and its `to_json`, the person's identity through the consultation module's `consulter`, and `belongs_to_person` (research D9, D10)
- [X] T015 Add `ToolCategory::Sessions` **appended** to the enum and to `ToolCategory::ALL` (24 → 25), with `name()` returning `"sessions"` and a description, in `crates/vizier-core/src/agents/tools/mod.rs`

**Checkpoint**: `cargo build` passes. Every exhaustive match on the content kind is classified —
that is where FR-021 is enforced, and the compiler is the checklist.

---

## Phase 3: User Story 1 - An agent starts work in another conversation (Priority: P1) 🎯 MVP

**Goal**: `wake_session` starts a turn in one of the calling agent's own conversations. The agent
works on it there, with that conversation's history in view, and its reply lands there.

**Independent Test**: quickstart §2 and §3, and `a_wake_starts_a_turn_in_another_conversation` in
`tests/dummyplug_e2e.rs`.

### Implementation for User Story 1

- [X] T016 [US1] Implement `WakeSession` — the arguments `target`, `topic_id?`, `instruction`, `wait?` and the result from `data-model.md`, with **no** agent id — in `crates/vizier-core/src/agents/tools/sessions/mod.rs`
- [X] T017 [US1] Resolve the target against the calling agent: `"current"` is the calling turn's own conversation, and any other value is parsed with `wakeable_channel`; a machinery conversation and an unknown address are refused, and a channel that has conversations of its own needs its topic, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T013)
- [X] T018 [US1] Check the access before dispatching: a WebUI or project conversation must belong to the person this turn is working for, and a turn with no person is refused — every refusal returned as data, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T014, T017)
- [X] T019 [US1] Check the hop limit against `MAX_BACKGROUND_DEPTH` and carry `background_depth + 1` on the request, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T018)
- [X] T020 [US1] Dispatch the request with `transport.send_request`, the `Woken` content and `user` set to the calling agent's id, and return the target's address at once, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T019)
- [X] T021 [US1] Register `wake_session` on `default_toolset`, add it to `every_tool()` and to the `EXPECTED` table in `every_tool_is_in_its_category`, and update `a_new_category_is_appended_to_the_end_of_the_list` (24 → 25, last is `sessions`), in `crates/vizier-core/src/agents/tools/mod.rs` (depends on T015)
- [X] T022 [P] [US1] Add the `wake_session` line to `tool_call_title` in `crates/vizier-core/src/utils/mod.rs`
- [X] T023 [US1] Write the caller's guidance into the tool's `description()`: waking starts a turn in the target conversation and writes into its history, posting a message only says something there, and the target must be named rather than guessed, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T016)
- [X] T024 [US1] Write the unit tests for the wake — the target resolution, the refusals and the hop limit — in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T020)

**Checkpoint**: User Story 1 works on its own. An agent wakes one of its own conversations and the
work lands there.

---

## Phase 4: User Story 2 - An agent finds the conversation to work in (Priority: P1)

**Goal**: `list_sessions` lists the calling agent's own conversations, each with a reference the
wake takes back unchanged.

**Independent Test**: quickstart §1, and `a_listing_holds_the_callers_own_conversations` in
`sessions/tests.rs`.

### Implementation for User Story 2

- [X] T025 [US2] Implement `ListSessions` — the arguments `channel?`, `query?` and the rows from `data-model.md`, with **no** agent id — in `crates/vizier-core/src/agents/tools/sessions/mod.rs`
- [X] T026 [US2] Read the rows with `storage.get_session_list(agent_id, None)` and filter them to the wakeable channels and to the conversations that belong to the person this turn is working for, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T014, T025)
- [X] T027 [US2] Apply the channel filter on the channel slug and the title search on the title, and always include the calling turn's own conversation under `"current"`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T026)
- [X] T028 [US2] Register `list_sessions` on `default_toolset` and add it to `every_tool()` and to the `EXPECTED` table, in `crates/vizier-core/src/agents/tools/mod.rs` (depends on T021)
- [X] T029 [P] [US2] Add the `list_sessions` line to `tool_call_title` in `crates/vizier-core/src/utils/mod.rs`
- [X] T030 [US2] Write the unit test for the listing — the two filters, the title search, another person's conversation absent and the machinery's own absent — in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T027)

**Checkpoint**: User Stories 1 and 2 both work independently.

---

## Phase 5: User Story 3 - Only the caller's own conversations may be woken (Priority: P1)

**Goal**: the boundary. The conversation that is woken belongs to the calling agent and to the
person it is working for, and no tool names another agent at all.

**Independent Test**: quickstart §5 and §6, and `a_wake_of_a_conversation_it_may_not_reach_is_refused_as_data`
in `tests/dummyplug_e2e.rs`.

### Implementation for User Story 3

- [X] T031 [US3] Make every refusal one value — `{"status": …, "message": …}` with the statuses from `data-model.md` — returned as data the agent can read, never as a failure of the call, in `crates/vizier-core/src/agents/tools/sessions/access.rs`
- [X] T032 [US3] Offer the two tools only where a person's access can be decided, using `sessions::access::may_wake` — so they are absent in a dream, in a subagent worker, in a Discord or Telegram conversation and in work started from one, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T014)
- [X] T033 [US3] Refuse a WebUI or project conversation of another person with `other_person`, comparing the target's `web_user()` with the consulting person's username, and refuse a turn with no person with `no_owner`, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T031, T018)
- [X] T034 [US3] Write the unit test for every refusal status, and the test that neither tool's arguments carry an agent id, in `crates/vizier-core/src/agents/tools/sessions/tests.rs` (depends on T033)

**Checkpoint**: All three P1 stories work independently.

---

## Phase 6: User Story 5 - The caller waits for the answer (Priority: P2)

**Goal**: a wake that asks to wait returns the woken turn's answer in the same call.

**Independent Test**: quickstart §4, and `a_wake_starts_a_turn_in_another_conversation` (which
waits) in `tests/dummyplug_e2e.rs`.

### Implementation for User Story 5

- [X] T035 [US5] Open a reply channel with `transport.reply_channel()` and, when `wait` is true, send the request with the channel attached, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T020)
- [X] T036 [US5] Read the frames until one that ends the turn (`VizierResponse::ends_turn()`) and return the woken turn's answer, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T035)
- [X] T037 [US5] End the wait without stopping the woken turn when the caller is interrupted or the call is dropped, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T036)

**Checkpoint**: User Story 5 works on its own. The caller's turn and the woken turn run
concurrently — the engine spawns a turn per session — so a same-agent wake cannot deadlock.

---

## Phase 7: User Story 4 - A scheduled run puts its work in the person's own conversation (Priority: P2)

**Goal**: a job's work happens where the person sees it. No change to how jobs are scheduled — the
agent in the run uses the same two tools.

**Independent Test**: quickstart §7.

### Implementation for User Story 4

- [X] T038 [US4] Refuse `"current"` in a conversation the machinery owns with `not_wakeable`, so a scheduled run reaches the person's conversation by naming it, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T017)
- [X] T039 [US4] Say in the tool's `description()` that a job may name the conversation to work in, and that the work then runs there, in `crates/vizier-core/src/agents/tools/sessions/mod.rs` (depends on T023)

**Checkpoint**: User Story 4 works on its own — the agent in a scheduled run finds the conversation
with `list_sessions` and wakes it.

---

## Phase 8: User Story 6 - The person sees what woke their conversation (Priority: P3)

**Goal**: in a conversation of their own, the person sees that the agent started work there and
what it produced — the request drawn as the agent's own card, not the person's bubble.

**Independent Test**: quickstart §8, and `frames.wake.test.ts`.

### Implementation for User Story 6

- [X] T040 [US6] Add the woken arm to the `VizierRequestContent` union in `webui/app/api/frames.ts`
- [X] T041 [US6] Add the woken arm to `requestText` so the instruction is what the chat shows, in `webui/app/lib/messages.ts` (depends on T040)
- [X] T042 [US6] Draw the woken request as the agent's own card ("Worked on this") instead of the person's bubble, in `webui/app/screens/chat/MessageList.tsx` (depends on T040)
- [X] T043 [P] [US6] Add `'sessions'` to `BuiltinCategory` and `list_sessions` and `wake_session` to `BUILTIN_TOOLS` in `webui/app/lib/toolCategories.ts`
- [X] T044 [P] [US6] Add the category's label, icon and phrase to the map in `webui/app/lib/toolIcons.ts`, and the assertion for it in `webui/app/lib/toolIcons.test.ts`
- [X] T045 [US6] Regenerate `webui/openapi.json` (`VIZIER_OPENAPI_OUT=../../webui/openapi.json cargo test -p vizier-http write_openapi -- --ignored`) and `webui/app/api/schema.d.ts` (`npm run gen:api`)
- [X] T046 [US6] Write the test that the woken request shows its instruction, in `webui/app/api/frames.wake.test.ts` (depends on T041)

**Checkpoint**: all six stories work independently.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [X] T047 Regenerate the public API listing with `just public-api` — the new enum variant, the new tools and the new module change `crates/vizier-core/public-api.txt`
- [X] T048 Update `CLAUDE.md`: the tools section (the two tools, the category, the request kind), the category count (24 → 25) and the SPECKIT pointer
- [X] T049 Run `cargo clippy` and `cargo test` and resolve every warning and failure in the touched files
- [X] T050 Run the web gate: `cd webui && npm run typecheck`, `cd design && npm run typecheck`, and `cd webui && npm test` (1757 tests) — the `just web-check` recipe needs a POSIX shell for its `&&`, so the same steps were run by hand on Windows
- [X] T051 Run `just test-scripts`, which includes `scripts/trail-tool-categories.test.mjs` — the test that fails when the WebUI's copy of the categories and the engine's disagree
- [X] T052 The e2e gate (constitution): walk every step of `quickstart.md` with dummyplug agents and mark each step pass or fail in this file's Notes
- [X] T053 Commit with a conventional message (`feat: add the session tools — list_sessions and wake_session`)

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
- In Phase 2: the `schema/request.rs` group (T004, T006), the `agents/process.rs` group
  (T007–T010), the `agents/tools/mod.rs` arm (T011), the sessions module (T013), the sessions
  access module (T014) and the new prompt section (T005) are groups over six different files
- T022 and T029 (the two `tool_call_title` lines) can run together
- T043 and T044 (the two WebUI maps) can run together
- US6's WebUI work can run in parallel with the remaining core stories, since it touches only
  `webui/`

---

## Notes

- **Pre-existing failures on Windows** (T002, recorded before the change and untouched by it):
  - `projects::path::tests::absolute_escaping_and_missing_paths_are_refused` — a path-escaping
    check that does not hold on Windows separators
  - `skill::scopes::tests::a_slug_in_both_project_roots_appears_once_and_agents_wins` — the same
    separator difference (`.agents/skills\\standup` vs `.agents/skills/standup`)
  - Neither file is one this feature touches. Everything else passes: **2110 passed, 2 failed** in
    `vizier-core`, and the whole `vizier-http` and `vizier-schema` suites pass.
- **`cargo clippy --workspace` cannot run on Windows**: the `vizier` binary's `daemonize`
  dependency is Unix-only (`std::os::unix` in a Windows build). This is pre-existing and
  independent of the feature. Clippy was run over `vizier-core`, `vizier-http` and `vizier-schema`
  with `--all-targets`: no errors, and no warning in any file this feature touches.
- **The framing rides the context block, not the message** (research D4, changed from the plan).
  The plan put it in the content kind's `Display`; that broke the offline `dummyplug` harness,
  which parses the request's text as JSON, and it would have made the request unreadable as a tool
  call in a script. The context block is a separate part of the user message, so the request's own
  text stays exactly what the agent wrote, and the harness ignores the block. The framing is
  selected on the **request kind**, which only a wake carries.
- **A machinery conversation's slug is refused with the reason** (`not_wakeable`), not as an
  unknown address: the model is told a scheduled run's conversation belongs to the scheduler
  rather than that it made a typo.
- **`belongs_to_person` takes the person's username**, not the whole `Consulter`, so the rule is
  testable without a storage fixture.
- **A same-agent wake cannot deadlock**: the engine spawns a turn per session
  (`agents/process.rs`, `turns.spawn`), so the caller's turn and the woken turn run concurrently
  and a waiting caller gets its answer.
- **The webui typecheck has to run after every webui file lands.** The first run of it here
  happened before the last WebUI files existed, so it never saw `frames.wake.test.ts`; CI caught
  the file (a conditional type that resolved to `never`) and it now takes the exported
  `VizierRequestContent`. The gate was re-run over the whole package afterwards.
- **`just web-check` cannot run on Windows**: the recipe joins its steps with `&&`, which
  PowerShell 5.1 rejects, so `just` runs it through `powershell.exe`. The same steps were run by
  hand: design typecheck, webui typecheck, design + webui token lint, webui vitest.
- [P] tasks = different files, no dependencies
- [Story] label maps each task to its story for traceability
- Every task names the file it changes, so it can be picked up cold
- Stop at any checkpoint to validate that story independently
- The exhaustive matches on `VizierRequestContent` fail the build until the new kind is classified
  — that is deliberate, and Phase 2 is where it is satisfied
