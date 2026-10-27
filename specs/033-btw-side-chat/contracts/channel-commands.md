# Contract: `/btw` on each channel

**Feature**: `033-btw-side-chat`

What a person sees. Every channel asks through `side_chat::ask` (agent-side-chat.md), so answers, limits and errors are the same everywhere; only presentation differs.

## WebUI

| Action | Result |
|--------|--------|
| Type `/btw` (alone) and send | The **Side chat** panel opens, empty, with its own composer focused |
| Type `/btw <question>` and send | The panel opens and asks `<question>` at once |
| Pick `btw` from the `/` menu | Inserts `/btw ` into the composer |
| Ask in the panel | The question shows, then a working indicator, then the answer (rendered as Markdown) |
| Ask again in the panel | A follow-up: the earlier turns are sent as `exchange` |
| 9th question in one panel | Inline notice "This side chat is full — close it and start a new one"; the composer is disabled |
| 429 | Inline "Slow down — try again in {n}s"; the question stays in the composer |
| 408 / 500 / 503 / network failure | Inline error under the question with a **Retry** action; the turn is not added to the exchange |
| Close the panel (button, Esc, scrim) | The exchange is discarded; no confirmation |
| Navigate away, reload, switch conversation | The exchange is discarded |

- The panel header shows "Side chat", the agent's name, and the model read-only; a muted line reads "Not saved — closing this panel discards it."
- Nothing is sent on the conversation's socket; the main message list, trail, and conversation list do not change. `/btw` never reaches the agent as `Chat`.
- The panel's turns live in component state only (no Zustand persistence, no `localStorage`/IndexedDB).

## Discord

| Action | Result (all **ephemeral**: only the invoker sees it) |
|--------|------------------------------------------------------|
| `/btw message:<question>` | "thinking…" (deferred), then the answer, with a **Follow up** button. A first line notes "Side chat — only you can see this, and it's deleted after 5 minutes idle (14 at most)." |
| Click **Follow up** | A modal "Side chat follow-up" with one paragraph input (max 4,000) |
| Submit the modal | A new ephemeral answer with its own **Follow up** button |
| Exchange ends (5 min idle or 14 min total) | Every ephemeral message of the exchange is deleted |
| Click **Follow up** after it ended, or on someone else's exchange | Ephemeral "This side chat has ended — start a new one with /btw." |
| Over the rate limit | Ephemeral "Slow down — try again in {n}s." |
| 9th question | Ephemeral "This side chat is full — start a new one with /btw." |
| Timeout / agent error | Ephemeral `**{kind label}**: {message}` |

- `/btw` is a built-in command in `builtin_commands()`, with a required `message` string option; `/help` lists it.
- The host conversation is the channel's active topic (`ChannelState.active_topic`), as for every other command.
- Long answers are split at 2,000 characters; the button is on the last chunk.

## Telegram

| Action | Result |
|--------|--------|
| `/btw <question>` (or `/btw@<bot> <question>`) | A reply to that message: `🤫 Side chat — not saved. I'll delete this after 2 minutes of inactivity. Reply to an answer to follow up.` then the answer |
| `/btw` with no question | A reply explaining the usage (`/btw <question>`), deleted with the same 2-minute rule |
| Reply to one of the exchange's answers | A follow-up, answered as a reply to it |
| 2 minutes with no activity in the exchange | The person's `/btw` and follow-up messages and every bot reply in the exchange are deleted (within 15 s of the deadline) |
| Over the rate limit | A reply "Slow down — try again in {n}s.", deleted with the exchange (or after 2 minutes if there is none) |
| 9th question | A reply "This side chat is full — start a new one with /btw." |
| Timeout / agent error | A reply `**{kind label}**: {message}` |

- In a group the notice adds "…your messages too, if I'm allowed to delete them here": a bot that is not an admin with *delete messages* can delete only its own messages there. In private chats it can delete both.
- In a group the exchange is **visible to the group until deleted** (US3-3); the notice says it is not saved, not that it is private.
- `/btw` is added to the bot's built-in command menu (`telegram_rules().builtins`).
- `/btw` and replies to live side-chat answers are recognised before skill and chat parsing, so they are never sent as `Chat` or `SilentRead`.
- Long answers are split with the existing Telegram message splitting; every part is tracked for deletion.
- Pending deletions survive a restart through the content-free ledger (data-model.md); removal after a restart happens when the reader reconnects.
