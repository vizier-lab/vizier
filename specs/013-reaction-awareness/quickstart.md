# Quickstart: Reaction Awareness

**Feature**: `013-reaction-awareness` · **Date**: 2026-10-10

These are end-to-end checks against a running binary with an agent on the offline `dummyplug` provider, as the constitution's e2e gate requires. They rely on dummyplug's new **§6 `context`** (`contracts/dummyplug-context.md`): sending exactly `context` makes the agent reply with the context block it received. That's how these steps see what the agent sees.

- **Sections 1 to 5** need no credentials and no network.
- **Sections 6 and 7** use a real Discord or Telegram bot token. The agent is still on dummyplug, so no model key is needed. They're marked **[platform]**.

## Setup

```sh
just install && just run-d
```

Create a dummyplug agent **`$A`** and open a WebUI topic on it. `$T` is the base URL plus an auth header for user **`$U`**, and `$TOPIC` is the topic id.

---

## 1. A reaction on a just-arrived reply persists (US2, FR-013, SC-001)

1. Send `hello`. A lorem reply arrives.
2. React 👍 on that reply. The badge appears dimmed, then solid within about a second (the `reactions` frame acknowledges it).
3. Reload the page.

**Expect**: the 👍 is still on the reply. `curl $T/api/v1/agents/$A/channel/<ch>/topic/$TOPIC/history` shows the newest `Response` entry with `"reactions":[{"user_id":"$U","emoji":"👍"}]`, and its `uid` matches the `history_uid` the WebSocket frame carried (check in devtools, Network → WS).

## 2. The agent sees it, and stops seeing it once removed (US1, FR-007, FR-009, SC-002, SC-003)

1. Continuing from 1, send `context`.

   **Expect**: the reply contains:

   ```text
   ## Reactions
   People reacted to your own earlier messages in this conversation. These are reactions, not
   messages: …
   - your reply 1 message ago, at HH:MM — "<first 80 chars of the lorem reply>"
     👍 ×1 ($U)
   ```

   (The count is the person's `context` request. The `context` reply is the one being written.)

2. Click the 👍 again to remove it. It disappears after the acknowledgement.
3. Send `context` again.

**Expect**: no `## Reactions` section at all.

## 3. Your own messages can't be reacted to, and bad targets are refused (FR-012, FR-014)

1. Hover your own `hello`. **Expect**: no react control.
2. In the devtools console, on the open socket, send `{"reaction":{"message_uid":"<uid of your hello from the history API>","emoji":"👍","action":"added"}}`.

**Expect**: a `reaction_error` frame, and `history` shows no reaction on the request entry. Repeat with `message_uid: "nope"` and get the same result.

## 4. Two tabs, and a reconnect (FR-015, FR-016, SC-004)

1. Open the same topic in a second tab. React 🎉 on the lorem reply in tab 1.
   **Expect**: tab 2 shows 🎉 within 3 seconds, with no reload.
2. Run `just shutdown && just run-d`. Wait for both tabs to show connected again, without reloading them.
3. In tab 2, react 👀 on the same reply.

**Expect**: it's acknowledged and shows in both tabs.

## 5. Size stays bounded and nothing wakes the agent (FR-007, FR-008, SC-005, SC-006)

1. Add 30 reactions with distinct emoji to one reply, by sending 30 `reaction` frames from the console in a loop.
2. Watch the topic for 10 seconds.
   **Expect**: no new agent turn and no thinking indicator. `history` gains no new `Request` or `Response` entries.
3. Send `context`.

**Expect**: the reply is listed with all 30 emoji, `×1 ($U)` each, on one line, and the section has a single entry. Then send 12 more `hello`s, react on each reply, and send `context`. **Expect**: exactly 10 entries, the most recent ones.

Migration check: on a database from before this feature that had WebUI reactions, the first startup keeps them. They show after reload and in `context`. A second startup changes nothing.

## 6. [platform] Discord

Give `$A` a Discord bot token and invite the bot to a test server.

1. Mention the bot with `@bot hello`, so it posts a lorem reply.
2. React 👍 on the reply, then mention the bot with `@bot context`.
   **Expect**: `## Reactions` lists 👍 with your display name.
3. Remove the 👍 and send `@bot context`. **Expect**: no section.
4. React with a custom server emoji, then send `@bot context`. **Expect**: it's shown as `<:name:id>`.
5. Send a JSON request for `discord_react_message` that has the bot react 🤖 to its own lorem reply, then `@bot context`. **Expect**: 🤖 isn't listed (FR-006).
6. Send `@bot tools`. The tool list is well over Discord's 2000-character limit, so it arrives as several messages. React on the **second** one, then `@bot context`. **Expect**: it's listed against that reply (FR-005).
7. React on someone else's message. **Expect**: nothing listed (FR-012).

## 7. [platform] Telegram

Give `$A` a Telegram bot token. Use a private chat with the bot.

1. Send `hello`, react 👍 on the lorem reply, then send `context`. **Expect**: 👍 is listed.
2. Change the reaction to 🔥 (tap and pick another), then send `context`. **Expect**: only 🔥 (FR-019).
3. Send `{"tool":"telegram_react_message","arguments":{"chat_id":<id>,"message_id":<your message id>,"emoji":"👍"}}`.
   **Expect**: a native 👍 appears on your message, and **no** `Reaction: 👍` message is posted (SC-007).
4. Repeat with `"emoji":"🦀"`. **Expect**: the tool result is an error naming Telegram's rejection, and nothing is posted.
5. Add the bot to a group as a **non-admin**, have it reply, and react there. **Expect**: no error in the logs, and the reaction isn't listed. Promote it to admin and react again: it's now listed.

## Live-model check (optional, marked: depends on real model output)

With a real provider, reply 👎 to an answer, then ask "anything you'd change about that last answer?". The agent should take the 👎 into account. Run a dream cycle on the agent afterwards. The extraction's source handover should mention the feedback.
