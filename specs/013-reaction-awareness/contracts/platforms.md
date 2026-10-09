# Contract: Discord and Telegram

**Feature**: `013-reaction-awareness` · Covers FR-005, FR-006, FR-018 to FR-021, Stories 3 to 5

## Linking what the agent posts (FR-005)

Every platform message the agent posts *as a reply or as an agent-authored message* is linked to the history entry it renders. Each send helper returns the platform ids it posted:

| Helper | Returns |
|---|---|
| `utils::discord::send_message` | `Vec<Id<MessageMarker>>`, one per chunk |
| `utils::discord::send_file` | `Id<MessageMarker>` |
| `utils::telegram::send_message` | `Vec<MessageId>`, one per chunk |
| attachment / voice sends in the Telegram loop | the posted `MessageId` |

| Sender | Links to |
|---|---|
| The channel response loop, final `Message` / `AudioReply` frame | `response.history_uid`, for every chunk and attachment |
| `discord_send_message`, `telegram_send_message` tools | the uid returned by their own `save_session_history(…, Response)` |
| Thinking, tool-call, abort and error posts | **not linked**. They're channel chrome, not replies |

Recording a link failure is logged with `warn!` and never fails the send.

## Discord inbound

`EventTypeFlags` adds `REACTION_ADD | REACTION_REMOVE | REACTION_REMOVE_ALL | REACTION_REMOVE_EMOJI`.

| Event | Becomes |
|---|---|
| `ReactionAdd` | `Add(emoji)` by `Reactor { id: user_id, name }` |
| `ReactionRemove` | `Remove(emoji)` by `Reactor { id: user_id, name: None }` |
| `ReactionRemoveEmoji` | `ClearEmoji(emoji)` |
| `ReactionRemoveAll` | `ClearAll` |

The target is `Platform { agent_id, Discord, chat_id: channel_id, message_id }`.

- **Emoji**: Unicode is stored as-is. Custom emoji are `<:name:id>` (or `<a:name:id>` if animated). A missing name gives `<:unknown_emoji:id>`.
- **Name** (add only): `member.nick`, then `member.user.global_name`, then `member.user.name`. If there's no member (a DM), one `http.user(user_id)` call resolves it, and an error leaves it `None`.
- **Ignored** (FR-006): `user_id == bot.id`, or `member.user.bot == true`.
- Super-reactions (`burst`) are treated like normal ones.

## Telegram inbound

`get_updates()` adds `.allowed_updates([Message, EditedMessage, MessageReaction])`.

For `UpdateKind::MessageReaction(u)`:
- The target is `Platform { agent_id, Telegram, chat_id: u.chat.id, message_id: u.message_id }`.
- The reactor is `u.actor`:
  - `User(user)`: `id = user.id`, `name = user.first_name` plus `" " + last_name` when present, or `@username` if the first name is empty.
  - `Chat(chat)` (anonymous admin): `id = chat.id`, `name = chat.title`.
- **Ignored** (FR-006): `User(user)` where `user.is_bot`.
- **Changes**: every type in `old_reaction` but not in `new_reaction` gives `Remove`. Every type in `new_reaction` but not in `old_reaction` gives `Add`. They're applied in that order, so a swap ends with only the new emoji (Story 4 scenario 2).
- **Emoji**: `Emoji { emoji }` is stored as-is, `CustomEmoji { custom_emoji_id }` as `custom_emoji:<id>`, and `Paid` as `⭐`.

`MessageReactionCount` (anonymous totals in channels) isn't requested and isn't handled.

## Telegram outbound: `telegram_react_message` (FR-020)

The arguments are unchanged: `{ chat_id, message_id, emoji }`.

```rust
bot.set_message_reaction(ChatId(chat_id), MessageId(message_id))
   .reaction(vec![ReactionType::Emoji { emoji }])
   .await
```

| Outcome | Tool returns |
|---|---|
| Ok | `Ok("Reacted with {emoji} to message {message_id}")` |
| Telegram rejects (`REACTION_INVALID`, message not found, no rights) | `Err(VizierError("telegram rejected the reaction: {telegram error}"))` |

It never posts a chat message. Its description gains: "Telegram allows only its standard reaction emoji (for example 👍 👎 ❤ 🔥 🎉 🤔 👀 🙏). Other emoji are rejected."

## Agent settings copy (FR-021)

The Telegram bot-token tooltip becomes: "Bot token from @BotFather. Reactions in private chats reach the agent automatically. In groups, the bot must be an administrator to receive them."
