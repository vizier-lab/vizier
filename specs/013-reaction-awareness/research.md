# Research: Reaction Awareness

**Feature**: `013-reaction-awareness` · **Date**: 2026-10-10

These are the decisions that turn the spec into a design. Each one records what was chosen, why, and what was rejected. The state of the code at `e17e385` is the baseline: this is what the reaction work from `23bd3e1` left behind.

## Baseline (what exists today)

| Piece | Where | State |
|---|---|---|
| WebUI → server reaction frame | `channels/http/api/v1/agents/channel.rs` (`WebSocketReactionMessage`) | Works, but `action` is ignored by storage |
| Reaction persistence | `channels/reaction_store.rs` → `HistoryStorage::update_history_reactions` | Read-modify-write of the whole `data` JSON blob. It loads the **entire** session history to find one row, toggles, and silently no-ops on an unknown uid |
| Agent delivery | `VizierRequestContent::Reaction` → `agents/process.rs` | **Logged and dropped.** Never in history and never in a prompt |
| Platform ids | `VizierRequest.platform_message_id` | Set only on *incoming* person messages. Nothing records the ids of messages the agent sends |
| Discord inbound | `channels/discord/mod.rs` | `Intents::all()`, but `EventTypeFlags` excludes reactions and the match ignores them |
| Telegram inbound | `channels/telegram/mod.rs` | `get_updates()` without `allowed_updates`, so Telegram never sends `message_reaction` |
| Telegram outbound | `agents/tools/telegram/mod.rs` (`telegram_react_message`) | Posts a text message `Reaction: 👍` |
| WebUI | `routes/chat.tsx`, `components/MessageItem.tsx` | Live messages get client uids (`Date.now()`, response timestamp) that the server has never seen. `action` is always `'added'`. The memo comparator ignores `onReact`, so a handler captured while disconnected is kept. No cross-tab updates. `ReactionBar.tsx` is unused |

Libraries, as pinned in `Cargo.lock`: **twilight 0.17.1** has `Event::ReactionAdd/ReactionRemove/ReactionRemoveAll/ReactionRemoveEmoji`. `GatewayReaction` carries `user_id`, `channel_id`, `message_id`, `emoji: EmojiReactionType`, and an optional `member`. **teloxide-core 0.11.2** has `UpdateKind::MessageReaction(MessageReactionUpdated { chat, message_id, actor, old_reaction, new_reaction, .. })`, `AllowedUpdate::MessageReaction`, and the `set_message_reaction` payload. No new dependencies are needed.

---

## Decision 1: Reactions get their own table; the JSON blob stops holding them

**Decision**: Add a `message_reaction` table keyed `(history_uid, reactor_id, emoji)`, with `history_uid REFERENCES session_history(uid) ON DELETE CASCADE`. Adding is `INSERT OR IGNORE` and removing is `DELETE`. Both are single statements, so they're atomic under the connection mutex with no read-modify-write. Reads attach reactions to the `SessionHistory` entries a query returns, through one extra `WHERE history_uid IN (…)` query per page. `SessionHistory.reactions` stays the read model, so the history API shape is unchanged for the WebUI.

**Rationale**: FR-002 (add and remove are distinct) and FR-003 (no lost updates) are both properties of the current toggle-on-a-blob design, so neither can be patched on top of it. A table makes both structural. It also removes the full-history scan per reaction.

**Alternatives rejected**:
- *Keep the blob and make the toggle respect `action`, inside one locked transaction.* This fixes FR-002 and FR-003 but keeps rewriting a whole history row per reaction. It also leaves "which messages have reactions" unanswerable without deserialising every row, which Decision 4's digest needs.
- *Count columns per emoji.* That drops who reacted, which FR-007 and Story 1 scenario 3 need.

**Migration**: at schema init, every `session_history` row whose `data` has a non-empty `reactions` array gets those entries copied into `message_reaction` (`INSERT OR IGNORE`). The array is then cleared in `data`. This is idempotent and needs no version flag: a second run finds no non-empty arrays. Existing WebUI reactions survive, as the spec's assumption requires.

## Decision 2: A reply's history uid travels on the response

**Decision**: Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub history_uid: Option<String>` to `VizierResponse`, and derive `Default` (with `VizierResponseContent::Empty` as `#[default]`). `HistoryStorage::save_session_history` returns the uid it assigned. `VizierAgent::chat` sets `response.history_uid` from the save of the final `Response` entry. The other construction sites get `..Default::default()`.

Everything downstream then has the uid with no new plumbing:
- **WebUI**: the per-message forwarder and the session-event broadcast already serialise `VizierResponse`, so the final frame now carries the uid the client must key the message by. This fixes WebUI bug 1.
- **Discord / Telegram**: the response loop knows which history entry it is posting when it sends the message, so it can record platform links (Decision 3).
- **Tools that post as the agent** (`discord_send_message`, `telegram_send_message`, `webui` messaging) already call `save_session_history(…, Response)` themselves. They now get the uid back and link it the same way.

**Rationale**: the uid is created exactly where the reply is saved, and every consumer already receives the `VizierResponse`. Carrying it there is one field. It isn't stored in `data` (`skip_serializing_if`, and it's set after the save), so history rows don't change.

**Alternatives rejected**:
- *Key by response timestamp* (what the WebUI does today). The server would have to match an RFC 3339 timestamp against a JSON blob, and the platform loops would need the same lookup. That's fragile and indirect.
- *A separate "history ack" WebSocket frame.* That needs a second mechanism for Discord and Telegram anyway, and the WebUI would have to correlate two frames.
- *Let the client re-fetch history after each turn and reconcile.* It only fixes the WebUI and costs a round trip per turn.

## Decision 3: Agent messages on platforms are linked, and only linked messages count

**Decision**: Add a `platform_message_link` table keyed `(agent_id, platform, chat_id, message_id)` → `history_uid`, also `ON DELETE CASCADE` to `session_history`. `utils::discord::send_message`/`send_file` and `utils::telegram::send_message` return the platform ids of every message they posted, including each chunk of a split reply. The response loops record one link per id. A reaction event is resolved through this table. **An event on an unlinked message is dropped.** That one rule implements FR-012 (only agent messages), the "messages from before this feature" assumption, and "reactions on other bots' messages" all at once, with no per-case checks.

**Rationale**: FR-005 requires a link from platform message to reply, and split replies need several links per reply. Resolving reactions through the agent's *own* outgoing ids avoids walking history and avoids guessing sessions from channel ids. The reaction event's channel id plus the link gives the history entry directly, and the entry gives the session.

**Alternatives rejected**:
- *Reuse `VizierRequest.platform_message_id` and `find_message_uid_by_platform_id`.* Those cover person messages, which FR-012 now excludes, and the lookup scans the whole session. Both are removed (Decision 9).
- *Store platform ids inside the `Response` JSON.* Lookups by platform id would then need a scan or a JSON index.

**Known gap (accepted)**: a reaction landing in the milliseconds between the platform accepting a message and the link insert is dropped. A person can't realistically react to a message they haven't seen yet.

## Decision 4: The agent sees a reaction digest in the per-request context block

**Decision**: `context_md` gains an optional `## Reactions` section built by one pure function, `reaction_digest(entries: &[SessionHistory]) -> Option<String>`. It considers the agent's `Response` entries *in the history the turn is already loading* (so it's bounded by the checkpoint window). It takes the most recent `MAX_DIGEST_MESSAGES` (10) entries that have reactions and renders each as one line: a short reference to the message (relative position and time, plus a ≤ 80-character excerpt), then per-emoji counts with up to `MAX_NAMES_PER_EMOJI` (5) reactor names and "+N more". It's omitted entirely when nothing qualifies, which is the existing pattern for the memory section. The section heading says these are reactions to the agent's own messages, not messages from anyone (FR-010).

**Rationale**:
- `context_md` is already the per-request block in the **user** message. It sits there precisely so the system prompt and replayed history stay byte-identical across turns and remain cacheable. A test guards this: `context_is_prepended_to_the_user_message_and_never_to_a_system_message`. Reactions change after the fact, so annotating the replayed assistant messages instead would invalidate the provider's prompt cache for the whole conversation every time someone reacted.
- It shows *current* state, so a reaction removed before the turn is simply absent (FR-009) with no event log to reconcile.
- The caps make the size fixed (SC-005): at most 10 lines, each with bounded names.
- `chat` (including the silent-read path) and `dream_chat` both take the `Vec<SessionHistory>` and both call `context_md`, so one function covers both.

**Alternatives rejected**:
- *Inject reactions as user messages in history.* This violates FR-010, breaks caching, and makes reactions look like things the person said.
- *Show only reactions changed since the agent's last turn.* That needs per-agent read markers, and a reaction made before a checkpoint would never be seen again. Current state is simpler and is what FR-007 asks for.
- *A tool the agent calls to fetch reactions.* The agent wouldn't know to call it, and Story 1 is about feedback arriving unprompted.

## Decision 5: Reactions reach memory through the checkpoint handover

**Decision**: `generate_handover_with_model` takes the digest as an optional extra input and adds a sixth item to its instructions: "**Feedback**: reactions people gave to your messages and what they indicate". The manual and pre-dream checkpoint path in `agents/process.rs` builds the digest from the `SessionHistory` it already loads. The automatic mid-turn checkpoint in `VizierAgent::prompt` already sees the current user message, which carries the digest (Decision 4).

**Rationale**: the dream cycle's `pre_check_sessions` checkpoints every source session before extraction, so extraction reads the **handover**, not the raw history (the post-checkpoint window is empty by then). The handover is the only path from a conversation into memory, so that's where feedback has to be (FR-011). `dream_chat` still gets the digest through `context_md` for any history after the checkpoint, at no extra cost.

## Decision 6: One shared ingest path for every channel

**Decision**: add `channels/reactions.rs` with one entry point:

```rust
pub async fn apply(deps: &VizierDependencies, change: ReactionChange) -> Result<()>
```

`ReactionChange` is `{ target, reactor, kind }`:
- `target`: `ReactionTarget::History(uid)` (WebUI) or `ReactionTarget::Platform { agent_id, platform, chat_id, message_id }` (Discord, Telegram).
- `kind`: `Add(emoji)`, `Remove(emoji)`, `ClearEmoji(emoji)`, or `ClearAll`.

`apply` resolves the target to a history entry, ignores unknown or unlinked targets, refuses targets that aren't agent `Response` entries (WebUI clients can send any uid), writes to storage, and publishes the message's new reaction set as `SessionFrame::Reactions` on the session-event broadcast.

Each channel only *translates* its native event into a `ReactionChange`. It also filters its own bots (FR-006), because bot identity is channel knowledge: Discord's `BotIdentity` and `member.user.bot`, and Telegram's `User::is_bot`.

**Rationale**: Principle II. Three channels share one rule set (resolve, check, write, publish), so it lives in one function and isn't copied three times. This is a shared helper, not a new trait. There's one implementation, and Principle I forbids a trait for that.

**Not done**: `VizierRequestContent::Reaction` is removed. FR-008 says reactions never start a turn, so routing them through the agent's request queue only queued a log line behind running turns. Reaction requests were never written to history, so no stored data mentions the variant.

## Decision 7: WebUI correctness is server-confirmed, not optimistic

**Decision**:
- The client keys agent replies by `response.history_uid` (Decision 2), falling back to today's timestamp key only when the field is absent (an older server). A message without a server uid shows no react control, since a reaction on it couldn't be saved.
- The client sends `{"reaction": {"message_uid", "emoji", "action"}}`, with `action` computed from whether the person's own `(user, emoji)` is currently shown. It marks that badge *pending* and doesn't change the set.
- The server answers through the broadcast: every socket on the session, including the sender's, gets `{"reactions": {"message_uid", "reactions": [...]}}` and replaces that message's set. That covers FR-015 (other tabs) and acts as the sender's ack (FR-014). On a rejected or failed save, the server sends `{"reaction_error": {"message_uid", "message"}}` to the sending socket only. The client clears the pending state and shows a toast. A pending marker with no answer within 5 seconds is cleared the same way.
- `handleReact` reads `connected` through a ref so its identity is stable, and the `MessageItem` comparator also compares `onReact` (FR-016).
- The react control is only on agent messages (FR-012). The unused `ReactionBar.tsx` is deleted.

**Rationale**: FR-014 forbids showing an unsaved reaction as saved. Since the broadcast is the ack, multi-tab and single-tab use go through one code path instead of two.

## Decision 8: Platform specifics

**Discord**:
- Add `REACTION_ADD | REACTION_REMOVE | REACTION_REMOVE_ALL | REACTION_REMOVE_EMOJI` to `EventTypeFlags`. `Intents::all()` already covers guild and DM reactions.
- **Emoji**: Unicode is stored as itself. Custom emoji are stored as `<:name:id>` (`<a:name:id>` if animated). That's Discord's own wire form, so the agent can read the name (FR-018) and its react tool could reuse it. A custom emoji whose name was deleted becomes `:unknown_emoji:`.
- **Display name**: `member.nick`, then `member.user.global_name`, then `member.user.name` on add. In DMs, `member` is absent, so one `http.user(user_id)` call resolves the name. A failure falls back to the id. Removal events carry no member and don't need a name.
- **Self-filter**: `user_id == bot.id`, or `member.user.bot` on add.

**Telegram**:
- `get_updates().allowed_updates([Message, EditedMessage, MessageReaction])`.
- Each `MessageReactionUpdated` is diffed: `old − new` gives removes and `new − old` gives adds. A swap (FR-019 / Story 4 scenario 2) is a remove plus an add.
- `ReactionType::Emoji` is stored as-is. `CustomEmoji` is stored as `custom_emoji:<id>`, since Telegram doesn't give bots the name without another call. `Paid` is stored as `⭐`.
- An anonymous actor (`MaybeAnonymousUser::Chat`) reacts as the chat, named by the chat title.
- **Self-filter**: `user.is_bot`.
- `telegram_react_message` calls `bot.set_message_reaction(chat_id, message_id).reaction(vec![ReactionType::Emoji { emoji }])`. Telegram's error becomes the tool's `VizierError` (FR-020), and nothing is posted.
- The agent-settings tooltip names both requirements: admin rights in groups, and that private chats work out of the box (FR-021).

## Decision 9: What gets deleted

Removed: `channels/reaction_store.rs` (both functions), `HistoryStorage::update_history_reactions` (and its `fs` and `sqlite` impls), `VizierRequestContent::Reaction`, `ReactionEvent`, and `webui/app/components/ReactionBar.tsx`.

Kept: `VizierRequest.platform_message_id`. Tools and metadata still use incoming ids to reply to or react to a person's message, and dropping it would touch the Discord and Telegram channels for no gain.

## Decision 10: Make the digest observable offline

**Decision**: extend the dummyplug protocol with **§6 `context`**. A message that is exactly `context` replies with the verbatim text of the per-request context block the agent received for that turn. This is additive: no existing keyword or tool name collides, so every existing quickstart script keeps working, as the constitution requires when the protocol changes.

**Rationale**: the constitution requires quickstart checks to be dummyplug steps. Dummyplug otherwise answers with lorem ipsum, so without this there's no offline way to show that the agent *received* the reactions, which is FR-007 and SC-002/003.

**Alternative rejected**: a debug log line the quickstart greps for. It depends on the log level, doesn't show the exact text the model gets, and isn't part of any contract.
