---

description: "Task list for /btw Ephemeral Side-Chat"
---

# Tasks: /btw Ephemeral Side-Chat

**Input**: Design documents from `specs/033-btw-side-chat/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: Only the unit tests the plan names (Testing field, quickstart §12) and the trace-invariant test from the plan's risk list are included. They pin the no-trace guarantee (FR-006, SC-002), which is the feature's main promise. Everything else is verified end to end through the dummyplug quickstart, as the constitution requires.

**Organization**: Tasks are grouped by user story so each story can be implemented and tested on its own. WebUI tasks are in the `webui/` submodule (`vizier-lab/webui`); they land there first and the pin is bumped here (commit order design → webui → vizier).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story the task belongs to (US1, US2, US3)
- Paths are relative to the repository root (`src/`, `docs/src/`, `webui/app/`)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Module skeletons and shared constants, so later tasks only fill in bodies

- [ ] T001 Create `src/channels/side_chat.rs` with module docs pointing at `specs/033-btw-side-chat/` and constants `MAX_EXCHANGE_TURNS = 8`, `MAX_QUESTION_CHARS = 4_000`, `MAX_ANSWER_CHARS = 16_000`, `ASKS_PER_WINDOW = 5`, `ASK_WINDOW = 60 s`; register `pub mod side_chat;` in `src/channels/mod.rs`
- [ ] T002 [P] Create `src/agents/agent/system_prompt/side_chat.rs` with `SIDE_CHAT_MD` (the `## Side Chat` section text) and `SIDE_CHAT_NO_TOOLS` (the fixed reply), worded exactly as in `contracts/agent-side-chat.md`; register it in `src/agents/agent/system_prompt/mod.rs`
- [ ] T003 [P] Create an empty `src/agents/agent/side_chat.rs` (`impl VizierAgent {}` block, module docs) and add `mod side_chat;` in `src/agents/agent/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The channel-independent side chat: the request type, the agent-side answer, the no-trace dispatch, and the shared `ask` helper every channel calls. It also includes the in-memory exchange store that Discord and Telegram both use, so US2 and US3 stay independent of each other.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Request type and limiter

- [ ] T004 Add `SideChat { question, exchange: Vec<SideChatTurn> }` and `SideChatTurn { question, answer }` (deriving what `ClientEvent` derives) and `VizierRequestContent::Btw(SideChat)` in `src/schema/request.rs`. Extend `kind()` with `"btw"` and `Display` with the question, and add `pub fn is_side_chat(&self) -> bool`. Then fix every now-non-exhaustive `match` on `VizierRequestContent` that `cargo check` reports.
- [ ] T005 Unit tests in `src/schema/request.rs` `mod tests`: `Btw` serialises as `{"btw": {...}}`, `kind()` is `"btw"`, `is_side_chat()` is true only for `Btw`, and `is_durable` is false for a `Btw` request with `transient: true`
- [ ] T006 [P] Add `pub side_chat_limits: Arc<crate::channels::http::rate_limit::RateLimiter>` (built with `ASKS_PER_WINDOW`/`ASK_WINDOW` from T001) to `VizierDependencies` in `src/dependencies.rs`, constructed wherever the struct is built

### Agent side (contracts/agent-side-chat.md)

- [ ] T007 Extract the checkpoint-handover system message built inline in `chat_turn` into `fn checkpoint_context(handover: String) -> Message` in `src/agents/agent/mod.rs` and use it from `chat_turn`. The wording must not change (research R6).
- [ ] T008 Implement `VizierAgent::side_chat(&self, req: VizierRequest, session: VizierSession, history: Vec<SessionHistory>, handover: Option<String>) -> Result<VizierResponse>` in `src/agents/agent/side_chat.rs`:
  - System prompts come from `prepare_system_prompts(&snapshot.categories, &session)`.
  - Add `checkpoint_context` (T007), then `history_entries_to_messages`, then the exchange's turns as `User`/`Assistant` text.
  - The final message is `with_context(User(req.to_prompt()), context_md(&[], &[], None, None) + SIDE_CHAT_MD)`.
  - Make one `model.complete_until(completion_request(.., &snapshot.tools(), true), Some(now + prompt_timeout), &UsageScope::turn(Some(session), &TurnId::new(), 1))`.
  - Drop tool calls from the reply, and answer `SIDE_CHAT_NO_TOOLS` when no text is left.
  - Build `VizierResponseStats` the way `prompt_with` does, and return `Message { content, stats }` with no `history_uid`.
  - Run all of it inside `invoke_agent_span(&req, &session, false)`.
  - Make `completion_request` and the stats builder `pub(super)` if needed.
  - Never call `chat`, `prompt_with`, `save_history_or_log`, `store_session_file`, hooks or `publish_session_event`.
- [ ] T009 Add `async fn run_btw(agent, storage, session, request, response_tx)` in `src/agents/process.rs`. It loads `list_session_history_until_checkpoint(session, Some(request.timestamp))`, calls `agent.side_chat(..)`, and sends the `Message`. On `Err`, it sends `Error { kind: ErrorKind::classify(..), message }`. It never writes to storage.
- [ ] T010 Wire the dispatch in `src/agents/process.rs` (depends on T009). This is where the no-trace guarantee is enforced, research R1/R2:
  - In `agent_process`, skip the `refresh_session_detail` spawn when `request.content.is_side_chat()`.
  - In `start_session_job`, skip both `update_thinking_state` calls for side chats.
  - Add `VizierRequestContent::Btw(_) => { run_btw(..).await; Ok(()) }` beside the `checkpoint`/`lobotomy` arms.
  - Add a defensive `Btw(_)` arm in `handle_request` that returns `Err(VizierError("side chats are answered by run_btw"))` without saving anything.
- [ ] T011 Trace-invariant test in `src/agents/process.rs` `#[cfg(test)]`. Against a temp `SqliteStorage`, seed a session's history, run the side-chat path (`run_btw` with a stub/dummyplug-shaped model, or the storage-facing part if the model can't be stubbed), then assert all of the following:
  - `list_session_history`, `get_session_detail_by_topic` and `pending_turns` are unchanged.
  - The response has no `history_uid`.
  - A side chat for a topic with no detail row leaves none.

### Shared ask helper (all channels)

- [ ] T012 Implement in `src/channels/side_chat.rs`:
  - `PersonKey` constructors: `http(username)`, `discord(id)`, `telegram(id)`.
  - `SideChatError { RateLimited { retry_after_secs }, Invalid(String), Timeout, Agent { kind, message } }`, with `user_message()`.
  - `fn validate(&SideChat) -> Result<(), SideChatError>`, enforcing the data-model limits. The 9th follow-up message is "side chat is full — start a new one".
  - `pub async fn ask(deps: &VizierDependencies, session: VizierSession, person: &PersonKey, user: String, side_chat: SideChat, timeout: Duration) -> Result<SideChatAnswer { answer, stats }, SideChatError>`. It validates, checks `deps.side_chat_limits` (refusing before sending), builds the request with `transient: true` and no attachments, sends it via `deps.transport.send_request`, then awaits the first `Message`/`Error`/`Abort` or a closed channel under `timeout`.
  - `retry_after_secs` is derived from `ASK_WINDOW`. Add a `RateLimiter::retry_after(key)` helper in `src/channels/http/rate_limit.rs` only if the window start is needed; otherwise report the full window.
- [ ] T013 Unit tests in `src/channels/side_chat.rs`:
  - Empty and over-long questions, 9 earlier turns and over-long answers are `Invalid`.
  - 8 earlier turns are valid.
  - The 6th ask in a window is `RateLimited` and nothing is sent; use a transport with no registered agent so a send would fail with a different error.

### Exchange store for channels without a client (Discord, Telegram)

- [ ] T014 Implement `SideChats` in `src/channels/side_chat.rs`:
  - Types: `Lifetime { idle, max: Option<Duration> }`, `ExchangeId` (nanoid 10), `LiveExchange { id, person, session, turns, started_at, last_active, posted: Vec<P> }`, generic over the platform's posted-message type `P`.
  - Methods:
    - `start(person, session) -> ExchangeId`
    - `get_for(id, person) -> Option<(VizierSession, Vec<SideChatTurn>)>` (refuses another person's exchange)
    - `record(id, turn)`
    - `touch(id)`
    - `add_posted(id, P)`
    - `sweep(now) -> Vec<LiveExchange<P>>` (removes and returns ended exchanges)
  - It's in memory only, behind a `std::sync::Mutex`, and never written to storage.
- [ ] T015 Unit tests for `SideChats` in `src/channels/side_chat.rs`: idle expiry, max-age expiry with idle activity, another person's id refused, and an unknown or ended id gives `None`

**Checkpoint**: A `Btw` request sent on the transport is answered with no trace. Every channel can now call `side_chat::ask`.

---

## Phase 3: User Story 1 - Ask a quick off-the-record question in the WebUI (Priority: P1) 🎯 MVP

**Goal**: `/btw` in the WebUI composer opens a side panel that asks the agent beside the current conversation, supports follow-ups, and discards everything on close.

**Independent Test**: [quickstart.md](quickstart.md) §1–9: the HTTP endpoint answers, history/title/topics are unchanged, the context block shows `## Side Chat`, follow-ups carry the exchange, tool requests get the no-tools notice, there's a 429 on the 6th ask, usage rows exist, and the WebUI panel opens, answers and leaves nothing behind.

### Backend for User Story 1

- [ ] T016 [US1] Add `BtwRequest { channel_id, topic_id, question, #[serde(default)] exchange }` and `BtwResponse { answer, stats }`, both `utoipa::ToSchema`, and `async fn agent_btw` with its `#[utoipa::path(post, path = "/agents/{agent_id}/btw", ...)]`, in `src/channels/http/api/v1/agents/mod.rs`. The handler:
  - Looks up the agent and returns 404 if it's missing.
  - Checks `user_can_view_agent`, returning 403.
  - Builds the session as `HTTP(user.username, channel_id)` with the topic.
  - Calls `side_chat::ask(&state.deps, .., &PersonKey::http(&user.username), user.username.clone(), .., *config.prompt_timeout)`.
  - Maps errors as in `contracts/http-btw.md`: 422, 429 plus a `Retry-After` header, 408, 500, and 503 for `AgentUnavailable`.
  - Register `.route("/{agent_id}/btw", post(agent_btw))` beside `/chat`, and add the path and schemas to the OpenAPI document.
- [ ] T017 [US1] Remove the duplicated send-and-await loop: make `agent_chat` in `src/channels/http/api/v1/agents/mod.rs` and `side_chat::ask` share one helper (e.g. `side_chat::await_reply(rx, timeout)`, or a transport-level helper) without changing `agent_chat`'s responses (Principle II, research R10)
- [ ] T018 [US1] Regenerate `webui/app/api/schema.d.ts` with `scripts/openapi.mjs` so `BtwRequest`/`BtwResponse` and the `btw` content variant are typed

### WebUI for User Story 1 (submodule `webui/`)

- [ ] T019 [P] [US1] Add `askSideChat(agentId, { channel_id, topic_id, question, exchange })` to `webui/app/api/endpoints/chat.ts`. It uses the typed `request` client and returns the answer or a typed error that carries the status, message and `Retry-After`.
- [ ] T020 [P] [US1] Create `webui/app/lib/sideChat.ts`:
  - A pure exchange reducer with turns, pending question, per-turn error, and `full` at 9 questions.
  - Actions `ask`/`answered`/`failed`/`retry`/`reset`.
  - `toExchange(state)` for the request.
  - An error mapper: 429 becomes "Slow down — try again in {n}s", 422 shows its message, 408/5xx/network failures become a retryable error.
  - Add `webui/app/lib/sideChat.test.ts` (vitest) covering the reducer, the 9-question limit and the error mapping.
- [ ] T021 [US1] Create `webui/app/screens/chat/SideChatPanel.tsx` (depends on T019, T020):
  - Use the `@vizier/design` `SlideOver` titled "Side chat".
  - Show the agent name and the model read-only, plus the muted line "Not saved — closing this panel discards it."
  - Render answers as Markdown with `AgentReply`, show a working indicator while pending, and give an inline error a **Retry** action.
  - Add a compact composer that's disabled when the exchange is full.
  - If opened with an initial question, ask it at once.
  - Keep state in `useReducer` only (no store, no storage). It's discarded on close and unmount.
- [ ] T022 [US1] Add `{ name: 'btw', description: 'Ask a side question that is never saved' }` to `BUILTIN` in `webui/app/screens/chat/Composer.tsx`. In `send()`, intercept `/^\/btw(?:\s+([\s\S]*))?$/` before the command regex and before anything is uploaded or sent, and call a new `onSideChat(question?: string)` prop.
- [ ] T023 [US1] Mount `SideChatPanel` in `webui/app/routes/chat.tsx`. Pass `onSideChat` to `<Composer>` to open it with the agent, the conversation's channel id and topic id, and the optional question. Close it when the conversation or agent changes.
- [ ] T024 [US1] Run `cd webui && npm run typecheck && npx vitest run side`, then commit in `webui/` and bump the `webui` gitlink here once the webui PR lands

**Checkpoint**: User Story 1 is fully usable from the WebUI and the HTTP API — the MVP.

---

## Phase 4: User Story 2 - Ask a quick off-the-record question on Discord (Priority: P2)

**Goal**: `/btw message:<text>` answers ephemerally. **Follow up** opens a modal for more turns. The exchange's messages are deleted after 5 minutes idle or 14 minutes total.

**Independent Test**: [quickstart.md](quickstart.md) §10 (LIVE): only the invoker sees the question and answers, and so do follow-ups. Two people at once each see only their own. After expiry the messages are gone, and a late **Follow up** says the side chat ended. The channel's history has no trace.

- [ ] T025 [US2] Add `CommandEntry::new("btw", "ask me a side question only you can see; never saved")` with a required `message` option to `builtin_commands()` in `src/channels/discord/mod.rs`. Add `/btw` to the `/help` text.
- [ ] T026 [US2] Add a `side_chats: Arc<SideChats<DiscordPosted>>` field to the Discord reader in `src/channels/discord/mod.rs`, where `DiscordPosted { token, message_id: Option<Id<MessageMarker>> }`. Use `Lifetime { idle: 5 min, max: Some(14 min) }`. Spawn a sweep task every 15 s, alongside the reader's other tasks, that deletes each ended exchange's messages through `http.interaction(app_id).delete_response(token)` / `delete_followup(token, id)`. The deletes are best-effort and logged at `debug`.
- [ ] T027 [US2] Implement `async fn side_chat(&self, interaction, channel, question, exchange: Option<ExchangeId>)` in `src/channels/discord/mod.rs`:
  1. Respond `DeferredChannelMessageWithSource` with `MessageFlags::EPHEMERAL`.
  2. Resolve the host session from `load_state(&channel).active_topic`.
  3. Use the user string the way `run_skill_command` builds it, and `PersonKey::discord(author id)`.
  4. Start a new exchange or continue one (`get_for`). An unknown, ended or someone-else's id gets an ephemeral "This side chat has ended — start a new one with /btw."
  5. Call `side_chat::ask` with the agent's `prompt_timeout` (from storage).
  6. Edit the response to the notice line (first answer only) plus the answer, split at 2,000 characters, with the remaining chunks as ephemeral `create_followup`. Put a **Follow up** button (`custom_id = "btw:{id}"`) on the last chunk.
  7. Record the turn and every posted message. Errors are rendered as `**{kind label}**: {message}` or the slow-down text, and they're tracked for deletion too.
- [ ] T028 [US2] Route `/btw` in `interaction()`'s command match in `src/channels/discord/mod.rs` to `side_chat(.., new exchange)`. Extend `interaction()` to handle:
  - `InteractionData::MessageComponent` with a `btw:` custom id: reply with an `InteractionResponseType::Modal` "Side chat follow-up" holding one paragraph `TextInput` (max 4,000, `custom_id = "btw:{id}"`).
  - `InteractionData::ModalSubmit` with a `btw:` id: call `side_chat(.., Some(id))` with the input's value.
  - Everything else keeps the existing early return.

**Checkpoint**: User Stories 1 and 2 both work independently.

---

## Phase 5: User Story 3 - Ask a quick off-the-record question on Telegram (Priority: P3)

**Goal**: `/btw <text>` gets a reply with a "not saved, deleted after 2 minutes" notice. Replying to an answer follows up. Everything in the exchange is deleted after 2 minutes idle, even across a restart.

**Independent Test**: [quickstart.md](quickstart.md) §11 (LIVE). In a private chat, the notice and answer appear, a reply follows up, and everything is gone 2 min 15 s after the last activity. In a group without admin rights, the bot's replies are deleted and the person's message stays, as the notice says. A restart before the deadline still deletes. History has no trace, and ordinary group messages are still `SilentRead`.

- [ ] T029 [US3] Add `CommandEntry::new("btw", "ask me a side question; never saved, deleted after 2 minutes")` to `telegram_rules().builtins` in `src/channels/telegram/mod.rs`
- [ ] T030 [US3] Implement the deletion ledger in `src/channels/telegram/mod.rs` (or a `side_chat_ledger` submodule beside it):
  - The value `{ items: [{ chat_id, message_ids, delete_after }] }` is stored under `side_chat_pending__{agent_id}__{chat_id}` via `StateStorage`.
  - Functions: `merge(chat, message_ids, delete_after)`, `clear(chat, message_ids)` (which removes the key when the list is empty), and `due(now)`.
  - The value holds no text, user or exchange id.
  - Add unit tests for merge (moving `delete_after` forward), clear, and empty-key removal.
- [ ] T031 [US3] Add `side_chats: Arc<SideChats<TelegramPosted>>` to the Telegram reader in `src/channels/telegram/mod.rs`, where `TelegramPosted { chat_id, message_id }` and the lifetime is `Lifetime { idle: 2 min, max: None }`.
  - Spawn a sweep task every 15 s that `delete_message`s every posted message of an ended exchange, then `clear`s the ledger. Deletes are best-effort, and a refusal is logged at `debug`.
  - At reader start, process the ledger's due items and re-arm the rest.
- [ ] T032 [US3] At the top of `handle_message` in `src/channels/telegram/mod.rs`, before the `new`/`session`/`abort`/`checkpoint` commands and before skill/`Chat`/`SilentRead` parsing:
  1. If `msg.reply_to_message()` is one of a live exchange's posted messages (from the same person), treat it as a follow-up.
  2. Otherwise, if `is_command(&text, "btw", &bot_username)`, start a new exchange (an empty question gets the usage reply).
  3. In both cases, call `side_chat::ask` with `PersonKey::telegram(from.id)` and the host session `(agent, channel, topic_id)`.
  4. Reply to the person's message with the notice on the first answer. In groups, add "…your messages too, if I'm allowed to delete them here".
  5. Split long answers with the existing Telegram helper.
  6. Record the person's message, every reply and any error reply in `posted` and in the ledger (`merge`).
  7. `return Ok(())` so the message is never sent as `Chat`/`SilentRead`.

**Checkpoint**: All three user stories work independently.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Docs, verification gates, and the constitution's e2e check

- [ ] T033 [P] Document `/btw` for the WebUI, Discord and Telegram in `docs/src/configuration/channels.md`: the privacy model per channel, the timings (5/14 min Discord, 2 min Telegram), the Telegram admin right for deleting people's messages, the rate limit, the limit of 9 questions, and that usage is still counted
- [ ] T034 [P] Document `POST /agents/{agent_id}/btw` (request, responses, guarantees) in `docs/src/api-integration/rest-api.md`, following `contracts/http-btw.md`
- [ ] T035 [P] Add a side-chat note to `docs/src/configuration/observability.md`: spans carry `vizier.request.kind = "btw"`, and message content is exported only under the operator's existing content-capture opt-in (research R15)
- [ ] T036 [P] Add a `/btw` paragraph to the Agents section of `CLAUDE.md` (dispatch beside checkpoint/lobotomy, `is_side_chat` skips, `channels/side_chat.rs`, the Telegram ledger), matching the density of the neighbouring paragraphs
- [ ] T037 Run `cargo clippy` and `cargo test` (including `side_chat`, `request`, `process` and Telegram ledger tests) and fix anything they report
- [ ] T038 End-to-end check with a `dummyplug` agent on a running binary (constitution e2e gate): walk [quickstart.md](quickstart.md) §1–9 and record any step whose observed result differs from the expected one
- [ ] T039 LIVE checks: walk [quickstart.md](quickstart.md) §10 (Discord, two accounts) and §11 (Telegram, private chat and a non-admin group, including the restart case), and record results
- [ ] T040 Run the release build (`cargo build --release`) to confirm the embedded WebUI with the new panel builds into the binary

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none
- **Foundational (Phase 2)**: depends on Setup; **blocks every user story**
  - T004 → T005, T008, T009, T012
  - T007 → T008 → T009 → T010 → T011
  - T006 → T012 → T013
  - T014 → T015 (independent of T004–T013 except the types from T004)
- **US1 (Phase 3)**: depends on Phase 2 (T012). T016 → T017, T018. Then T019/T020 run in parallel, then T021 → T022 → T023 → T024.
- **US2 (Phase 4)**: depends on Phase 2 (T012, T014). T025 → T026 → T027 → T028.
- **US3 (Phase 5)**: depends on Phase 2 (T012, T014). T029 and T030 run in parallel, then T031 → T032.
- **Polish (Phase 6)**: docs (T033–T036) can start once the contracts are stable. T037–T040 run after the stories they cover.

### User Story Dependencies

- **US1 (P1)**: needs only Phase 2
- **US2 (P2)**: needs only Phase 2; does not touch US1 files
- **US3 (P3)**: needs only Phase 2; does not touch US1 or US2 files

### Parallel Opportunities

- Setup: T002 and T003
- Foundational: T006 alongside T004/T005; T014/T015 alongside the agent-side chain T007–T011
- After Phase 2, **US1, US2 and US3 can proceed in parallel**: they touch disjoint files (`api/v1/agents/mod.rs` + `webui/`, `discord/mod.rs`, `telegram/mod.rs`)
- US1: T019 and T020; US3: T029 and T030
- Polish: T033–T036

---

## Parallel Example: after Phase 2

```bash
# Three streams on disjoint files:
Task: "T016 [US1] POST /agents/{agent_id}/btw in src/channels/http/api/v1/agents/mod.rs"
Task: "T025–T028 [US2] /btw on Discord in src/channels/discord/mod.rs"
Task: "T029–T032 [US3] /btw on Telegram in src/channels/telegram/mod.rs"

# Within US1 (WebUI submodule):
Task: "T019 [US1] askSideChat in webui/app/api/endpoints/chat.ts"
Task: "T020 [US1] exchange reducer + tests in webui/app/lib/sideChat.ts"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 Setup → Phase 2 Foundational (the no-trace core, with T011 pinning it)
2. Phase 3 US1: the HTTP endpoint, then the WebUI panel
3. **Stop and validate**: quickstart §1–9 on dummyplug
4. Ship. The vizier PR bumps the `webui` pin after the webui PR merges (`/cascade-merge`).

### Incremental Delivery

1. Foundation → US1 (WebUI + API) → validate → ship
2. US2 (Discord) → LIVE §10 → ship
3. US3 (Telegram) → LIVE §11 → ship (the weakest privacy guarantee, so last, per the spec)

### Notes

- Conventional commits (`feat:` for the feature, `doc:` for docs-only commits).
- No migration, legacy shim or alias is added, so there is no #82 entry.
- Never log a side chat's question or answer (`tracing` fields carry ids and kinds only).
