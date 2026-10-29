# Contract: `/btw` on each channel

**Feature**: `033-btw-side-chat`

What a person sees. Every channel asks through `side_chat::ask` (agent-side-chat.md), so answers, limits and errors are the same everywhere; only presentation differs.

## Starting, continuing and ending (all channels)

A side chat is one aside with follow-ups, discarded in full when it ends (research R0). `/btw` always starts a new, empty one. On the WebUI an aside is one question and its answer: the window has no input, and another question is another `/btw`.

| | WebUI | Discord | Telegram |
|---|---|---|---|
| Start | `/btw <question>` in the composer | `/btw message:<question>` | `/btw <question>` |
| Visible to | The person only | **Everyone in the channel** | Everyone in the chat, until deleted |
| Continue (follow-up) | Not offered: another `/btw` starts a new one | Reply to one of its answers | Reply to one of its answers |
| Ends | Window closed, page left, conversation switched | 5 min idle | 2 min idle |
| After it ends | Gone | Messages stay in the channel | Messages deleted |
| Earlier turns held by | Nothing: one question per window | The reader's process memory | The reader's process memory |
| Size limit | 4,000 characters per question | 32,000 characters | 32,000 characters |

On every channel the agent keeps no record of the side chat; only its visibility differs. Replies to a side chat's messages on Discord and Telegram are never passed to the agent as ordinary messages (research R11–R13).

## WebUI

| Action | Result |
|--------|--------|
| Type `/btw <question>` and send | A small floating **Side chat** window opens and asks `<question>` at once: the question, then a working indicator, then the answer (rendered as Markdown) |
| Type `/btw` (alone) and send | Nothing is sent and the text stays in the composer, so the question can be finished there |
| Pick `btw` from the `/` menu | Completes to `/btw`; a partly typed name (`/bt`) is completed by the menu, never sent as a message |
| Want another question | Another `/btw <question>`: a new, empty side chat. The window has no input of its own, because it is throwaway |
| 429 | Inline "Slow down — try again in {n}s" with a **Retry** action |
| 408 / 500 / 503 / network failure | Inline error with a **Retry** action |
| Close the window (`esc`, Esc, or the `/btw` command again) | The answer is discarded; no confirmation |
| Navigate away, reload, switch conversation | The answer is discarded |

- The window has no scrim, so the conversation stays visible and usable behind it. Its header shows `/btw` and the question and moves the window; the bottom-right corner resizes it. Position and size are remembered in memory only, for the next `/btw` on the page.
- The window shows the agent's name and the model read-only, and a muted line reads "Not saved — closing this discards it."
- Nothing is sent on the conversation's socket; the main message list, trail, and conversation list do not change. `/btw` never reaches the agent as `Chat`.
- The answer lives in component state only (no Zustand persistence, no `localStorage`/IndexedDB). The window sends no earlier turns, so a question over 4,000 characters is refused in the window without a request.

## Discord

Every message the bot posts in a side chat starts with the marker line `-# 🤫 side chat — visible to everyone, not remembered · reply to follow up` (Discord small text).

| Action | Result |
|--------|--------|
| `/btw message:<question>` | Public: Discord shows "<person> used /btw" with the question, then "thinking…", then the answer with the marker line |
| The starter replies to one of its answers | A follow-up, answered publicly as a reply to their message |
| Someone else replies to one of its answers | Ignored: no answer, and the reply is not passed to the agent |
| Anyone replies after it ended (5 min idle, or a server restart) | A reply "This side chat has ended — start a new one with /btw." with the marker line |
| It ends | Nothing visible happens: its messages stay in the channel, and the bot forgets the aside |
| Over the rate limit | On `/btw`: an ephemeral "Slow down — try again in {n}s." (only the person sees it). On a follow-up: a reply with the same text |
| The side chat is full (32,000 characters) | A reply "This side chat is full — start a new one with /btw." |
| Timeout / agent error | A reply `**{kind label}**: {message}` |

- `/btw` is a built-in command in `builtin_commands()`, with a required `message` string option; `/help` lists it as "ask me a side question; public, not remembered".
- The host conversation is the channel's active topic (`ChannelState.active_topic`), as for every other command.
- Replies to side-chat messages are recognised before abort, `Chat` and `SilentRead` handling, so they are never sent to the agent as ordinary messages. Recognition uses the live aside's message ids, or the marker line once no live aside owns the message.
- Long answers are split at 2,000 characters; every chunk has the marker line, so a reply to any of them is recognised.
- Want it private? Use the WebUI, or `/btw` in a DM with the bot.

## Telegram

| Action | Result |
|--------|--------|
| `/btw <question>` (or `/btw@<bot> <question>`) | A reply to that message: `🤫 Side chat — not saved. I'll delete this after 2 minutes of inactivity. Reply to an answer to follow up.` then the answer |
| `/btw` with no question | A reply explaining the usage (`/btw <question>`), deleted with the same 2-minute rule |
| Reply to one of the exchange's answers | A follow-up, answered as a reply to it |
| 2 minutes with no activity in the exchange | The person's `/btw` and follow-up messages and every bot reply in the exchange are deleted (within 15 s of the deadline) |
| Over the rate limit | A reply "Slow down — try again in {n}s.", deleted with the exchange (or after 2 minutes if there is none) |
| The side chat is full (32,000 characters) | A reply "This side chat is full — start a new one with /btw." |
| Timeout / agent error | A reply `**{kind label}**: {message}` |

- In a group the notice adds "…your messages too, if I'm allowed to delete them here": a bot that is not an admin with *delete messages* can delete only its own messages there. In private chats it can delete both.
- In a group the exchange is **visible to the group until deleted** (US3-3); the notice says it is not saved, not that it is private.
- `/btw` is added to the bot's built-in command menu (`telegram_rules().builtins`).
- `/btw` and replies to live side-chat answers are recognised before skill and chat parsing, so they are never sent as `Chat` or `SilentRead`.
- Long answers are split with the existing Telegram message splitting; every part is tracked for deletion.
- Pending deletions survive a restart through the content-free ledger (data-model.md); removal after a restart happens when the reader reconnects.
