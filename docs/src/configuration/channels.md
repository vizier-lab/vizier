# 2.4 Channels

Vizier exposes agents over three channels. The HTTP channel is process-wide and configured in `.vizier.yaml`; Discord and Telegram are **per agent** — each agent owns its own bot token. Bot tokens are encrypted at rest; back up the workspace key separately — see [Encryption at Rest](./encryption.md).

## HTTP (REST + WebSocket + WebUI)

```yaml
vizier:
  channels:
    http:
      port: 9999
      jwt_secret: "${VIZIER_JWT_SECRET}"
      jwt_expiry_hours: 720           # default 30 days
      ws_idle_timeout_secs: 300       # default 5 minutes
```

| Field | Default | Description |
|-------|---------|-------------|
| `port` | `9999` | Listens on `0.0.0.0:<port>` |
| `jwt_secret` | `${VIZIER_JWT_SECRET}` | Signing secret for JWTs and hashing of API keys |
| `jwt_expiry_hours` | `720` | Lifetime of login tokens |
| `ws_idle_timeout_secs` | `300` | Idle WebSocket connections are closed after this many seconds (ping/pong keeps them alive) |

CLI overrides: `--port`, `--ws-idle-timeout`. Docker: `VIZIER_PORT`, `VIZIER_WS_IDLE_TIMEOUT`.

What the HTTP channel serves:

| Path | What |
|------|------|
| `/` and static assets | The bundled WebUI (`webui/build/client/`) |
| `/api/v1/...` | REST API — see [REST API](../api-integration/rest-api.md) |
| `/api/v1/agents/{id}/channel/{channel_id}/topic/{topic_id}/chat` | WebSocket chat |
| `/swagger` and `/openapi.json` | Swagger UI / OpenAPI spec |

CORS is fully open (`Any` origin), so a separately hosted frontend can talk to the API directly.

### Authentication

- **Login**: `POST /api/v1/auth/login` → JWT. Send as `Authorization: Bearer <jwt>`.
- **API keys**: created in Settings → API Keys (or `POST /api/v1/auth/api-keys`). Keys look like `vk_…` and are sent as `Authorization: ApiKey <key>`.
- **WebSocket**: pass the JWT as `?token=<jwt>` (browsers can't set headers on WS upgrades).

### First run

On a fresh database `GET /api/v1/auth/setup-status` returns `{ "needs_setup": true }` and the WebUI redirects to `/onboarding`, which calls `POST /api/v1/auth/setup` to create the first user. That user is assigned the built-in `superadmin` system role, and the WebUI then opens Home, whose setup guide covers connecting a provider and creating the first agent.

### WebUI-only agent tools

Every agent gets two tools for pushing messages into WebUI conversations:

- `webui_send_message` — send a message to a user's WebUI topic (used for proactive messages / task results)
- `webui_list_topics` — list a user's WebUI topics

## Discord

Set the agent's `discord_token` (WebUI agent settings, or `PUT /api/v1/agents/{id}` with `discord_token`). The channel connects when the agent starts and reconnects whenever the token changes.

**When the bot responds**

- Direct messages: always
- Guild channels: only when **@mentioned**. Un-mentioned guild messages are still delivered to the agent as `silent_read` requests, so the agent can follow the conversation; it replies unprompted only with probability `silent_read_initiative_chance` (default `0`).
- Message attachments are downloaded and attached to the request as session files.

**Slash commands** (registered globally by the bot):

| Command | Description |
|---------|-------------|
| `/ping` | Health check |
| `/new` | Start a fresh session (new topic) in this channel |
| `/session [topic_id]` | List sessions, or switch to `topic_id` (`DEFAULT` for the default topic) |
| `/abort` | Abort the agent's current in-flight response |
| `/checkpoint` | Save a checkpoint with a generated handover summary and start fresh context |
| `/lobotomy` | Save a checkpoint **without** a handover — clean break |
| `/thinking` | Toggle streaming the model's thinking output into the channel |
| `/tool_calls` | Toggle showing tool-call details |
| `/btw message:<text>` | Ask a side question: answered in the channel, visible to everyone, not remembered (see [Side chats](#side-chats-btw)) |

**Discord tools** (enabled with `tools.discord = true`; require `discord_token`):

| Tool | Description |
|------|-------------|
| `discord_send_message` | Send a message to a channel |
| `discord_react_message` | React to a message with an emoji |
| `discord_get_message_by_id` | Fetch a message |
| `discord_get_guild_info` | Guild metadata |
| `discord_get_channel_info` | Channel metadata |
| `discord_get_member_info` | Member metadata |

## Telegram

Set the agent's `telegram_token`. Same lifecycle as Discord.

**When the bot responds**

- Private chats: always
- Groups: when @mentioned or addressed as `/<bot_username>`; other messages are `silent_read`.

**Commands** (plain-text `/command` messages):

| Command | Description |
|---------|-------------|
| `/ping` | Health check |
| `/new` | Start a fresh session |
| `/session [topic_id]` | List sessions or switch (`DEFAULT` = default topic) |
| `/abort` | Abort the current response |
| `/checkpoint` | Checkpoint with handover summary |
| `/lobotomy` | Checkpoint without handover |
| `/thinking` | Toggle thinking output |
| `/tool_calls` | Toggle tool-call details |
| `/btw <text>` | Ask a side question: not saved, and deleted after 2 minutes of inactivity (see [Side chats](#side-chats-btw)) |

**Telegram tools** (enabled with `tools.telegram = true`; require `telegram_token`):

| Tool | Description |
|------|-------------|
| `telegram_send_message` | Send a message to a chat |
| `telegram_react_message` | React with an emoji |
| `telegram_get_message_by_id` | Fetch a message |

## Side chats (`/btw`)

`/btw` asks the agent you are talking to a quick, off-the-record question without disturbing the conversation. The agent answers as itself, with the conversation so far as context, then **forgets the exchange**: nothing is written to the conversation's history, title or memory, and the agent has no tools in a side chat, so it cannot act. A side chat is one aside with follow-ups, and ends when you close it or it goes idle. In the WebUI it is a single question and its answer, in a small window with no input of its own, so it stays out of the way: it floats over the conversation without dimming it, you can drag it, resize it, and keep using the conversation behind it.

What "not saved" means depends on the channel:

| | WebUI | Discord | Telegram |
|---|---|---|---|
| Start | `/btw <question>` in the composer | `/btw message:<text>` | `/btw <text>` |
| Who sees it | Only you, in a small floating window | **Everyone in the channel** | Everyone in the chat, until it deletes itself |
| Follow up | Not offered: another `/btw` is a new side chat | Reply to one of its answers | Reply to one of its answers |
| Ends | You close the window (`esc`) | 5 minutes idle, or a restart | 2 minutes idle |
| What remains | Nothing | The messages stay in the channel; the bot forgets the exchange | Nothing: the messages are deleted |

- **Discord asides are public.** Discord cannot reply to a private (ephemeral) message, so the answer is posted in the channel and stays there. Every message the bot posts in a side chat starts with a small marker line so people can tell it is not remembered. Replies to those messages are never passed to the agent as ordinary messages: from the person who started the aside they are follow-ups, from anyone else they are ignored, and after the aside ended they get "This side chat has ended". Want it private? Use the WebUI, or `/btw` in a direct message with the bot.
- **Telegram deletes after 2 minutes of inactivity**, within about 15 seconds of the deadline. Your `/btw` message, your follow-ups and every reply in the exchange are deleted. In a **private chat** the bot can delete both. In a **group** the bot can delete its own messages, but it can delete other people's only if it is an **administrator with the *Delete messages* right**; without it, your own messages stay and the bot says so. Telegram will not let a bot delete a message older than 48 hours.
- **Restarts.** A restart ends every live side chat. On Telegram the bot keeps a small list of chat and message ids waiting for deletion (no text, no names) so that messages are still deleted after a restart; the list is emptied as it goes and is removed with the agent.
- **Limits.** Each question is at most 4,000 characters, and on Discord and Telegram a whole side chat (every question and answer so far) at most 32,000. A full side chat refuses new questions: start a new one. Each person can ask 5 side-chat questions per minute, counted per platform account; the sixth is refused before it reaches the agent.
- **Usage is still counted.** Each answer is a model call, so its tokens show in the agent's usage, under the conversation it was asked beside. The question and answer text are not stored. If you export traces with content capture on, see [Observability](./observability.md#side-chats).
- **Timing.** A side chat waits behind a turn already running in the same conversation, like any message, and fails like any turn if the agent takes longer than its prompt timeout.

The same call is available to other clients as [`POST /agents/{id}/btw`](../api-integration/rest-api.md#side-chat).

## Sessions and topics

A session is `(agent, channel, topic)`. Each Discord channel / Telegram chat / WebUI `channel_id` has a default topic plus any number of named topics created with `/new`. History is per session. See [Agents → Checkpoints](./agents.md#checkpoints) for how long sessions are compacted.

## Managing tokens at runtime

- **WebUI**: agent Settings → Channels
- **API**: `PUT /api/v1/agents/{agent_id}` with `discord_token` / `telegram_token` (send `null` to disconnect)

The agent process is restarted on update, so the old bot connection is dropped and the new one established automatically.


## Clearing session files

`/clear-files` on Discord and `/clear_files` on Telegram remove the current conversation's
transient session files. Both are two-step: send the command to see the count, then
`/clear-files confirm` (or `/clear_files confirm`) within 60 seconds. A clear is refused while a
turn is running or queued. Artifacts are unaffected, and there is no automatic collection.
