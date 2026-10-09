---

description: "Task list for 013-reaction-awareness"
---

# Tasks: Reaction Awareness

**Input**: Design documents from `/specs/013-reaction-awareness/`

**Prerequisites**: plan.md, spec.md, research.md (Decisions 1–10), data-model.md, contracts/ (agent-context, websocket, platforms, dummyplug-context), quickstart.md

**Tests**: the spec doesn't ask for TDD. The plan names unit tests for the pure and storage parts: `reaction_digest`, the Telegram old/new diff, emoji normalisation, `ReactionStorage` round-trips and the migration's idempotence, and dummyplug §6. They're included as ordinary tasks next to the code they cover. End-to-end verification uses the dummyplug steps in `quickstart.md` (constitution gate).

**Organization**: tasks are grouped by user story. Paths are repository-relative. The backend is `src/` and the frontend is `webui/app/`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US5 from spec.md

---

## Phase 1: Setup

**Purpose**: empty module skeletons, so later tasks only fill files in.

- [ ] T001 Create empty modules and register them:
  - `src/storage/reaction.rs` (`pub mod reaction;` in `src/storage/mod.rs`, alphabetically after `provider`)
  - `src/storage/sqlite/reaction.rs` (`mod reaction;` in `src/storage/sqlite/mod.rs`)
  - `src/channels/reactions.rs` (`pub mod reactions;` in `src/channels/mod.rs`)
  - `src/agents/agent/system_prompt/reactions.rs` (`pub mod reactions;` in `src/agents/agent/system_prompt/mod.rs`)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the reaction tables and storage trait, `history_uid` on responses, the shared ingest path, and moving the WebUI server side onto it. The old reaction path is deleted here, so the tree compiles on the new one. Every story builds on this.

**⚠️ CRITICAL**: no user story work can begin until this phase is complete.

### Types

- [ ] T002 [P] In `src/schema/request.rs`:
  - add `#[serde(default, skip_serializing_if = "Option::is_none")] pub user_name: Option<String>` to `ReactionEntry`
  - delete `ReactionEvent` and its `impl`, and the `VizierRequestContent::Reaction` variant with its `Display` arm
  - keep `ReactionAction`, since the WebSocket frame still uses it

  Remove the now-dead re-exports from `src/schema/mod.rs` (line ~45).
- [ ] T003 [P] In `src/schema/response.rs`:
  - add `#[derive(Default)]` to `VizierResponse` and `#[default]` on `VizierResponseContent::Empty`; derive `Default` on that enum too
  - add the field `#[serde(default, skip_serializing_if = "Option::is_none")] pub history_uid: Option<String>` to `VizierResponse`, with the doc comment from data-model.md

  Then fix every struct-literal construction of `VizierResponse { timestamp: …, content: …, attachments: … }` across `src/` by appending `..Default::default()`. There are about 55 sites; `cargo build` lists them. Patterns that already end in `..` need no change.
- [ ] T004 [P] In `src/transport.rs`, add the variant `SessionFrame::Reactions { history_uid: String, reactions: Vec<ReactionEntry> }` with a doc comment ("the full current reaction set of one message, after any change to it").

### Storage

- [ ] T005 Add `init_reaction_schema(conn: &Connection) -> Result<()>` to `src/storage/sqlite/mod.rs`, next to `init_history_schema`, and call it right after `init_history_schema(conn)?;` (line ~328). It contains:
  - the DDL for `message_reaction` and `platform_message_link` exactly as in data-model.md (both FKs `REFERENCES session_history(uid) ON DELETE CASCADE`, and `idx_reaction_uid`)
  - the one-time migration: `SELECT uid, timestamp, data FROM session_history WHERE json_array_length(json_extract(data, '$.reactions')) > 0`. For each row, deserialize `data` into `SessionHistory`, `INSERT OR IGNORE` each `{user_id, emoji}` with `reactor_name` NULL and `added_at` set to the row's `timestamp`, set `entry.reactions = vec![]`, then `UPDATE session_history SET data = ?` — all in one transaction.
- [ ] T006 Define `pub trait ReactionStorage` in `src/storage/reaction.rs` with the seven methods in data-model.md "Storage trait". In the same file, also define:
  - `pub enum Platform { Discord, Telegram }` with `as_str()` returning `"discord"` or `"telegram"`
  - `pub struct Reactor { pub id: String, pub name: Option<String> }`
  - `#[async_trait] impl ReactionStorage for VizierStorage`, forwarding each method to `self.0`, matching how `src/storage/background_job.rs` forwards
- [ ] T007 Add `ReactionStorage` to the `VizierStorageProvider` supertrait list and the `use` block in `src/storage/mod.rs` (lines ~12 and ~43–58).
- [ ] T008 Implement `ReactionStorage for SqliteStorage` in `src/storage/sqlite/reaction.rs`:
  - add: `INSERT OR IGNORE` with `added_at = Utc::now().timestamp_millis()`
  - remove: `DELETE` on the full key
  - clear: `DELETE … WHERE history_uid=?1 [AND emoji=?2]`
  - `list_reactions`: `ORDER BY added_at`, mapping `reactor_id → user_id` and `reactor_name → user_name`
  - `link_platform_messages`: one `INSERT OR REPLACE` per id inside a transaction
  - `find_linked_message`: join `platform_message_link` to `session_history`, parse with the existing `parse_history_row`, then fill `reactions` (T010's helper)
  - `get_history_entry`: `SELECT data, seq FROM session_history WHERE uid=?1`, filled the same way

  Every method takes `self.conn.lock()` once.
- [ ] T009 In `src/storage/history.rs` and `src/storage/sqlite/history.rs`:
  - change `save_session_history` to return `Result<String>` (the generated uid) in the trait, the `VizierStorage` forwarder and the sqlite impl
  - delete `update_history_reactions` from the trait, the forwarder, the sqlite impl and `src/storage/fs/history.rs`
  - fix callers that bind the result with `let _ = …` or a `match` (`.await?;` statements compile as-is)
- [ ] T010 In `src/storage/sqlite/history.rs`, add a private helper `fill_reactions(conn, entries: &mut [SessionHistory])`. It runs one `SELECT history_uid, reactor_id, reactor_name, emoji FROM message_reaction WHERE history_uid IN (…) ORDER BY added_at` and assigns each entry's `reactions`. It's a no-op for an empty slice, and it chunks the `IN` list at 500 ids. Call it at the end of every read that returns `SessionHistory`: `list_session_history`, `list_session_history_until_checkpoint`, and any other `fn` in that file returning `Vec<SessionHistory>`. It's `pub(super)` so `reaction.rs` (T008) can use it.
- [ ] T011 [P] Unit tests in `src/storage/sqlite/reaction.rs` (`#[cfg(test)]`, in-memory connection with `init_history_schema` + `init_reaction_schema`):
  - add twice gives one row
  - removing an absent reaction is a no-op and never adds one (FR-002)
  - remove after add gives none
  - `clear_reactions(uid, Some("👍"))` leaves the other emoji
  - `clear_reactions(uid, None)` leaves nothing
  - links resolve through `find_linked_message` and an unknown id returns `None`
  - **migration**: a row whose `data` carries `reactions:[{user_id:"a",emoji:"👍"}]` ends with one `message_reaction` row and `data.reactions == []`; running `init_reaction_schema` a second time changes nothing
  - `list_session_history` returns that entry with `reactions` filled

### Responses carry their uid

- [ ] T012 In `src/agents/agent/mod.rs::chat` (the final save around line ~530), bind `let uid = self.storage.save_session_history(…Response(response.clone())).await?;` and set `response.history_uid = Some(uid);` before `hooks.on_response`.
- [ ] T013 [P] In `src/channels/http/api/v1/agents/jobs.rs` (~line 207) and `src/agents/tools/webui/mod.rs` (~line 60), set `history_uid` on the `VizierResponse` they publish or return from the uid `save_session_history` now returns. That way a woken or tool-sent WebUI message also arrives keyed by its server uid.

### Shared ingest path

- [ ] T014 Implement `src/channels/reactions.rs` per research Decision 6 and data-model.md "New":
  - `ReactionTarget`, `ReactionKind` and `ReactionChange`, re-using `Platform` and `Reactor` from `storage::reaction`
  - `pub async fn apply(deps: &VizierDependencies, change: ReactionChange) -> anyhow::Result<ApplyOutcome>`, where `enum ApplyOutcome { Applied, Ignored(&'static str) }`

  Steps inside `apply`:
  1. Resolve the entry: `History(uid)` → `get_history_entry`; `Platform{..}` → `find_linked_message`. `None` → `Ignored("unknown message")`.
  2. Require `SessionHistoryContent::Response` with content `Message{..}` or `AudioReply(..)`, else `Ignored("not an agent reply")`.
  3. Validate the emoji: non-empty and ≤ 64 bytes, else `Ignored("invalid emoji")`.
  4. Write via `add_reaction`, `remove_reaction` or `clear_reactions`.
  5. `list_reactions`, then `deps.transport.publish_session_event(SessionEvent { session: entry.vizier_session, frame: SessionFrame::Reactions { history_uid, reactions } })`.

  Log `Ignored` at `debug!`. Storage errors bubble up.
- [ ] T015 Move the WebUI server side onto `apply` in `src/channels/http/api/v1/agents/channel.rs` (contracts/websocket.md):
  1. In the reaction branch (~lines 421–460), drop the `record_reaction` call and the `transport.send_request(… Reaction …)`.
  2. First check the entry's session: `get_history_entry(uid)`, then compare `vizier_session` to `curr_session`. On a mismatch, reply to this socket with `reaction_error` "message not found in this conversation".
  3. Then call `apply` with `ReactionTarget::History`, the reactor `{ id: username, name: None }` and `kind` from `payload.action` (`Added` → `Add`, `Removed` → `Remove`).
  4. On `Ok(Ignored(reason))` or `Err(e)`, send `{"reaction_error":{"message_uid","message"}}` to this socket only, via `write_tx`.
  5. In the session-event arm (~line 396), serialize `SessionFrame::Reactions { history_uid, reactions }` as `{"reactions":{"message_uid": history_uid, "reactions": reactions}}`.

  Remove the `reaction_store` import.
- [ ] T016 Delete `src/channels/reaction_store.rs` and its `pub mod` line in `src/channels/mod.rs`. Delete the `VizierRequestContent::Reaction` arm in `src/agents/process.rs` (~line 1204). Run `cargo build` and fix any remaining reference to `ReactionEvent`, `PlatformMessageId` lookups via `find_message_uid_by_platform_id`, or `update_history_reactions`.

**Checkpoint**: `cargo build` and `cargo test` pass. Reactions sent from the WebUI on messages loaded from history are stored in `message_reaction`, and every socket on the session receives a `reactions` frame.

---

## Phase 3: User Story 1 - The agent knows how its replies landed (Priority: P1) 🎯 MVP

**Goal**: on each turn, the agent sees a bounded digest of the current reactions on its replies. Reactions are also carried into checkpoint handovers, and so into the dream cycle.

**Independent Test**: quickstart §2 and §5. React on a reply loaded from history (Phase 2 already persists those), send `context`, and `## Reactions` lists it. Remove it, send `context`, and the section is gone.

- [ ] T017 [US1] Implement `pub fn reaction_digest(entries: &[SessionHistory]) -> Option<String>` in `src/agents/agent/system_prompt/reactions.rs`, exactly per contracts/agent-context.md rules C1–C6:
  - `const MAX_DIGEST_MESSAGES: usize = 10`, `const MAX_NAMES_PER_EMOJI: usize = 5`, `const EXCERPT_CHARS: usize = 80`, `const NAME_CHARS: usize = 32`
  - excerpt via the existing `remove_think_tags` (find it with `grep -rn "fn remove_think_tags" src`), with whitespace collapsed
  - local time via `chrono::Local`
  - the result is the list lines only (no heading); T018 adds the heading

  Also add `pub fn render_reaction_section(list: &str) -> String`, which returns the `## Reactions` heading plus the preamble from the contract plus the list.
- [ ] T018 [P] [US1] Unit tests in the same file:
  - no reactions → `None`
  - a reaction on a `Request` entry is ignored
  - 12 reacted replies → 10 lines, oldest first, the most recent kept
  - 7 reactors on one emoji → 5 names + `+2 more`
  - emoji ordering by count, then by first appearance in `reactions` (which `list_reactions` already returns in `added_at` order, so `ReactionEntry` needs no timestamp)
  - a name containing `)\n## System` is rendered with no newline and no `)`
  - `1 message ago` is singular
  - a 200-char reply is cut to 80 chars + `…`
- [ ] T019 [US1] In `src/agents/agent/system_prompt/context.rs`, change `context_md(memory, skills)` to `context_md(memory, skills, reactions: Option<&str>)`. When it's `Some`, push `render_reaction_section(list)` right after the `## Time` section. Update every caller:
  - `src/agents/agent/mod.rs:451`: pass `reaction_digest(&session_history).as_deref()`
  - `src/agents/agent/mod.rs:974` (`dream_chat`): pass `reaction_digest(&session_history).as_deref()`
  - test callers in `context.rs`, `process.rs` (~lines 1386–1422) and `dummyplug.rs:569`: pass `None`

  Add one test asserting the section lands between `## Time` and `## Possibly Related Memories` and stays in the user message.
- [ ] T020 [US1] Handover (contracts/agent-context.md "The handover"):
  - change `generate_handover_with_model(model, history)` in `src/agents/agent/mod.rs` (~line 1251) to take `reactions: Option<&str>`. When `Some`, push a user message `"Reactions people gave to your messages in this conversation (feedback, not messages):\n{list}"` before the instruction message, and add item **6. Feedback** to the instruction text.
  - thread the parameter through `generate_handover_message`.
  - in the manual/pre-dream checkpoint path in `src/agents/process.rs` (~line 333), pass `reaction_digest(&history).as_deref()`; `history` is the `Vec<SessionHistory>` loaded just above.
  - in the automatic checkpoint inside `VizierAgent::prompt` (~line 671), pass `None`, since the current user message already carries the digest (research Decision 5).
- [ ] T021 [P] [US1] Dummyplug §6 in `src/agents/agent/model/dummyplug.rs` (contracts/dummyplug-context.md):
  - in `completion`, after the `tools` check and before the tool-name check, handle `text.eq_ignore_ascii_case("context")`: reply with the text of the first `UserContent::Text` in `message` that starts with `CONTEXT_HEADER`, or `"(no context block)"`
  - add tests: a message built with `with_context(user_message("context"), context_md(&[], &[], Some("- x")))` replies with text containing `## Reactions`, and a bare `user_message("context")` replies `(no context block)`
- [ ] T022 [P] [US1] Append section **§6 `context`** to `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`, using the Behaviour, Precedence and Compatibility text from contracts/dummyplug-context.md.

**Checkpoint**: quickstart §2 and §5 pass for replies loaded from history. The agent sees reactions and never wakes on them.

---

## Phase 4: User Story 2 - WebUI reactions stick and are honest (Priority: P1)

**Goal**: reactions on any agent message, including just-arrived ones, persist; are confirmed by the server before being shown; sync across tabs; and survive reconnects.

**Independent Test**: quickstart §1, §3 and §4.

- [ ] T023 [P] [US2] In `webui/app/interfaces/types.ts`:
  - add `history_uid?: string` to `WebSocketResponse`
  - add `ReactionEntry.user_name?: string`
  - add `interface WebSocketReactionsFrame { reactions: { message_uid: string; reactions: ReactionEntry[] } }` and `interface WebSocketReactionErrorFrame { reaction_error: { message_uid: string; message: string } }`
  - remove the now-unused `ReactionEvent` and the `{ reaction: ReactionEvent }` member if nothing else uses them; keep `ReactionAction`
- [ ] T024 [US2] In `webui/app/routes/chat.tsx`, key agent replies by the server uid: in the three places that build a `ChatMessage` from a final frame (~lines 830, 868, 906, where `uid: timestamp`), use `wsResponse.history_uid ?? timestamp`. Also store a flag `serverUid: wsResponse.history_uid !== undefined` (add an optional field to `ChatMessage` in `types.ts`), so messages without a server uid render no react control (contracts/websocket.md W6).
- [ ] T025 [US2] In `webui/app/routes/chat.tsx`, handle the new frames in the `lastMessage` effect (~line 757), before the `WebSocketResponse` handling. Mirror however `background_job` frames are detected (`grep -n background_job webui/app/hooks/connectionStore.tsx webui/app/routes/chat.tsx`):
  - `'reactions' in msg`: `setReactions(prev => ({ ...prev, [uid]: frame.reactions }))`, and clear pending entries for that uid
  - `'reaction_error' in msg`: clear pending for that uid and show the existing toast/notification helper with the message (find it with `grep -rn "toast\|notify" webui/app/hooks webui/app/components | head`)
- [ ] T026 [US2] Rewrite `handleReact` in `webui/app/routes/chat.tsx` (~line 715) per contracts/websocket.md W1–W5:
  - read `connected` from a `useRef` kept in sync by an effect, so the callback's deps are `[sendMessage]` only
  - when disconnected, show a toast and return
  - `action` is `'removed'` iff `reactions[uid]` has `{user_id: currentUser, emoji}`, else `'added'`
  - add `${uid}|${emoji}` to a `pending` `useState<Set<string>>` and send the frame; **don't** mutate `reactions`
  - start a 5 s timer that, if that key is still pending, removes it and shows a "reaction not saved" toast

  Pass `pending` down to `MessageItem`.
- [ ] T027 [P] [US2] In `webui/app/components/MessageItem.tsx`:
  - render the react control (picker button + clickable badges) only when `!isUserMessage && serverUid`; for user messages with existing reactions, render `ReactionBadges` read-only (no `onToggleReaction`)
  - accept a `pendingEmojis?: string[]` prop and pass it on
  - add `onReact`, `pendingEmojis`, `trail` and `isError` to the memo comparator (~line 332)
- [ ] T028 [P] [US2] In `webui/app/components/ReactionBadges.tsx`, accept `pendingEmojis?: string[]`. A pending emoji renders dimmed (`opacity: 0.5`) with a small spinner or `…`, and isn't clickable while pending. Make `onToggleReaction` optional; without it, badges render as non-interactive spans.
- [ ] T029 [P] [US2] Delete `webui/app/components/ReactionBar.tsx` (unused).
- [ ] T030 [US2] Run `cd webui && npm run typecheck` and fix any errors.

**Checkpoint**: quickstart §1, §3 and §4 pass. With US1, the MVP is complete for the WebUI.

---

## Phase 5: User Story 3 - Discord reactions reach the agent (Priority: P2)

**Goal**: native Discord reactions on the agent's replies (including split replies and custom emoji) are recorded and removed. The agent's own reactions are ignored.

**Independent Test**: quickstart §6 [platform].

- [ ] T031 [US3] In `src/utils/discord.rs`:
  - change `send_message` to return `Result<Vec<Id<MessageMarker>>, VizierError>`. For each `create_message(...).await`, call `.model().await` on success and push `message.id`. Errors are still logged and skipped as today. Keep the spawn-based chunk loop's semantics, but return the collected ids from the spawned task's `JoinHandle` output.
  - change `send_file` to return `Result<Id<MessageMarker>, VizierError>` the same way.

  Callers that ignore the result (`let _ =`) compile unchanged.
- [ ] T032 [US3] Link replies in the Discord response loop in `src/channels/discord/mod.rs` (~lines 600–675):
  - in the final `Message` and `AudioReply` arms, bind `history_uid` from the pattern (add `history_uid,` before `..`)
  - collect the ids returned by `send_message` and every `send_file`
  - if `history_uid` is `Some`, call `storage.link_platform_messages(&agent_id, Platform::Discord, &channel_id.to_string(), &ids_as_strings, &uid)`; on error, `warn!`

  Thinking, tool-call, abort and error posts aren't linked.
- [ ] T033 [P] [US3] In `src/agents/tools/discord/mod.rs` (`discord_send_message`, ~lines 79–120), capture the ids returned by the send and the uid returned by `save_session_history`, then call `link_platform_messages(…, Platform::Discord, …)`. The tool needs access to storage and agent id; it already has `storage` for the history save — check its struct and add `agent_id` if missing.
- [ ] T034 [US3] Inbound events in `src/channels/discord/mod.rs`:
  - add `| EventTypeFlags::REACTION_ADD | EventTypeFlags::REACTION_REMOVE | EventTypeFlags::REACTION_REMOVE_ALL | EventTypeFlags::REACTION_REMOVE_EMOJI` to `events` (~line 61)
  - add the match arms `Event::ReactionAdd(r)`, `Event::ReactionRemove(r)`, `Event::ReactionRemoveAll(r)` and `Event::ReactionRemoveEmoji(r)`, each `tokio::spawn`ing a new `handler.reaction(...)`
  - implement `Handler::reaction` per contracts/platforms.md "Discord inbound": skip if `user_id == self.bot.get().map(|b| b.id)` or `member.user.bot`; resolve the name (`member.nick` → `user.global_name` → `user.name`; when there's no member, use `self.http.user(user_id).await…model()`, and on failure leave it `None`); build the `ReactionChange` with target `Platform { agent_id, Discord, chat_id: channel_id, message_id }`; call `reactions::apply`; log `Err` at `warn!`
- [ ] T035 [P] [US3] Add `fn discord_emoji_key(emoji: &EmojiReactionType) -> String` in `src/channels/discord/mod.rs`: `Unicode { name }` → `name`; `Custom { animated, id, name }` → `<a:name:id>` or `<:name:id>`, with `unknown_emoji` when `name` is `None`. Add unit tests for all three shapes next to the existing `mentions_are_rendered_not_stripped` test.

**Checkpoint**: quickstart §6 passes against a test server.

---

## Phase 6: User Story 4 - Telegram reactions reach the agent (Priority: P2)

**Goal**: native Telegram reactions on the agent's replies, in private chats and in groups where the bot is admin, are recorded, removed and swapped.

**Independent Test**: quickstart §7 steps 1, 2 and 5 [platform].

- [ ] T036 [US4] In `src/utils/telegram.rs`, change `send_message` to return `Result<Vec<MessageId>, VizierError>`, pushing `sent.id` from each successful `send_message(...).await`. Errors stay logged and skipped.
- [ ] T037 [US4] Link replies in the Telegram response loop in `src/channels/telegram/mod.rs` (~lines 580–700):
  - in the final `Message` and `AudioReply` arms, bind `history_uid`
  - collect the ids from `send_message` and from every attachment send (`send_photo`, `send_document`, `send_voice`/`send_audio` — take `.id` from each `Ok(msg)`)
  - if `Some(uid)`, call `link_platform_messages(&agent_id, Platform::Telegram, &chat_id.0.to_string(), …)`; on error, `warn!`
- [ ] T038 [P] [US4] In `src/agents/tools/telegram/mod.rs` (`telegram_send_message`, ~lines 58–95), link the sent ids to the uid returned by `save_session_history`, with `Platform::Telegram`, the same way as T033.
- [ ] T039 [US4] Inbound in `src/channels/telegram/mod.rs`:
  - add `.allowed_updates(vec![AllowedUpdate::Message, AllowedUpdate::EditedMessage, AllowedUpdate::MessageReaction])` to `get_updates()` (~line 49)
  - add the arm `UpdateKind::MessageReaction(u) => self.handle_reaction(u).await?` in `handle_update`
  - implement `handle_reaction` per contracts/platforms.md "Telegram inbound": skip a bot actor; build the reactor from `MaybeAnonymousUser::{User, Chat}`; diff the reactions with T040's helper; call `reactions::apply` once per change, removals first; log `Err` at `warn!`
- [ ] T040 [P] [US4] Add pure helpers in `src/channels/telegram/mod.rs`:
  - `fn telegram_emoji_key(r: &ReactionType) -> String`: `Emoji{emoji}` → `emoji`; `CustomEmoji{custom_emoji_id}` → `custom_emoji:{id}`; `Paid` → `⭐`
  - `fn reaction_diff(old: &[ReactionType], new: &[ReactionType]) -> (Vec<String>, Vec<String>)`, returning `(removed, added)` keys

  Unit tests: add only; remove only; swap 👍→🔥 gives `(["👍"], ["🔥"])`; unchanged gives empty; custom and paid keys.
- [ ] T041 [P] [US4] Update the Telegram bot-token tooltip in `webui/app/routes/agent-settings.tsx` (~line 1158) to the copy in contracts/platforms.md "Agent settings copy" (FR-021).

**Checkpoint**: quickstart §7 steps 1, 2 and 5 pass.

---

## Phase 7: User Story 5 - Agents react for real on Telegram (Priority: P3)

**Goal**: `telegram_react_message` places a native reaction and never posts a message.

**Independent Test**: quickstart §7 steps 3 and 4 [platform].

- [ ] T042 [US5] In `src/agents/tools/telegram/mod.rs` (`ReactTelegramMessage::call`, ~line 128), replace the `send_message(... "Reaction: …")` with `self.bot.set_message_reaction(chat_id, message_id).reaction(vec![ReactionType::Emoji { emoji: args.emoji.clone() }]).await`. Map an error to `VizierError(format!("telegram rejected the reaction: {err}"))`. Extend `description()` with the allowed-emoji sentence from contracts/platforms.md.

**Checkpoint**: quickstart §7 steps 3 and 4 pass.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T043 [P] Update `CLAUDE.md`: under "Channels", add a short **Reactions** paragraph covering:
  - reactions live in `message_reaction`, keyed by agent `Response` history uid
  - platform messages are linked via `platform_message_link`, filled from `VizierResponse.history_uid`
  - every channel feeds `channels::reactions::apply`
  - the agent sees them only as the `## Reactions` section of `context_md` and in checkpoint handovers
  - they never start a turn (by decision)
  - reactions on unlinked messages (people's messages, pre-feature messages) are dropped

  Mention dummyplug §6 in the Commands section's e2e sentence.
- [ ] T044 Run `cargo clippy` and `cargo test`, and fix warnings introduced by this feature.
- [ ] T045 Run `cd webui && npm run typecheck`.
- [ ] T046 Run quickstart.md §1–§5 end to end against `just run-d` with a dummyplug agent, including the migration check in §5 on a copy of a pre-feature database. Record the results in the PR description.
- [ ] T047 Run quickstart.md §6 and §7 with real bot tokens [platform], or note in the PR which steps weren't run and why.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)** comes first.
- **Foundational (Phase 2)** depends on Setup and blocks every story. Within it:
  - T002, T003 and T004 can run in parallel
  - T005 runs before T006, then T007, then T008
  - T009 and T010 come after T006
  - T011 comes after T008 and T010
  - T012 and T013 come after T003 and T009
  - T014 comes after T004, T006 and T008
  - T015 comes after T014
  - T016 is last
- **US1 (Phase 3)** depends only on Phase 2.
- **US2 (Phase 4)** depends only on Phase 2. It's independent of US1 (different files: `webui/` vs `src/agents/`).
- **US3 (Phase 5)** depends on Phase 2. It's independent of US1 and US2 for building. Its test step uses `context`, which needs T021 from US1.
- **US4 (Phase 6)** depends on Phase 2, with the same note as US3.
- **US5 (Phase 7)** depends only on Phase 2, and touches the same file as T038, so run it after T038.
- **Polish (Phase 8)** comes after the stories you're shipping.

### Story independence

| Story | Can ship alone after Phase 2? | Notes |
|---|---|---|
| US1 | Yes | Reactions on WebUI messages loaded from history already reach storage after T015 |
| US2 | Yes | Purely WebUI client, plus the server side done in Phase 2 |
| US3 | Yes | Verification uses dummyplug `context` (T021) |
| US4 | Yes | Same as US3 |
| US5 | Yes | A single tool change |

### Parallel opportunities

- **Phase 2**: T002 ∥ T003 ∥ T004, and T011 ∥ T012 ∥ T013 once their inputs are done.
- **Across stories**: once Phase 2 is done, US1 (`src/agents/…`), US2 (`webui/…`), US3 (`src/channels/discord`, `src/utils/discord.rs`) and US4 (`src/channels/telegram`, `src/utils/telegram.rs`) touch disjoint files and can proceed in parallel.
- **Within US1**: T018 ∥ T021 ∥ T022 after T017.
- **Within US2**: T023, T027, T028 and T029 in parallel, then T024–T026 (all `chat.tsx`, sequential), then T030.
- **Within US3**: T033 ∥ T035 alongside T031 → T032 → T034.
- **Within US4**: T038 ∥ T040 ∥ T041 alongside T036 → T037 → T039.

### Parallel example: after Phase 2

```text
Agent A (US1): T017 → T018 ∥ T021 ∥ T022 → T019 → T020
Agent B (US2): T023 ∥ T027 ∥ T028 ∥ T029 → T024 → T025 → T026 → T030
Agent C (US3): T031 → T032 → T034, with T033 ∥ T035
```

---

## Implementation Strategy

### MVP (US1 + US2: the WebUI loop closed)

1. Phase 1 and Phase 2. Old reaction path deleted, the new one live, and the tree green.
2. Phase 3 (US1). Validate with quickstart §2 and §5 on replies loaded from history.
3. Phase 4 (US2). Validate with quickstart §1, §3 and §4.
4. **Stop and validate**: a WebUI user can react to any agent reply, it persists, and the agent sees it. Shippable.

### Incremental delivery

5. US3 (Discord), then quickstart §6.
6. US4 (Telegram inbound), then quickstart §7 steps 1, 2 and 5.
7. US5 (Telegram tool), then quickstart §7 steps 3 and 4.
8. Phase 8.

Each step leaves `cargo test`, `cargo clippy` and `npm run typecheck` green.

## Notes

- Conventional commits. The removal of `VizierRequestContent::Reaction` and the rename of the `save_session_history` return type are internal. No public API breaks: the WebSocket `reaction` frame keeps its shape, and the history API keeps `reactions`. The commit doesn't need `[**breaking**]`.
- Don't key anything on `VizierChannelId` inside `channels/reactions.rs`. Channels translate, and `apply` stays channel-agnostic (Principle II).
