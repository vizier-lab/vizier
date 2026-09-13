# 2.4 Channels

Vizier exposes agents over three channels. The HTTP channel is process-wide and configured in `.vizier.yaml`; Discord and Telegram are **per agent** — each agent owns its own bot token.

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

On a fresh database `GET /api/v1/auth/setup-status` returns `{ "needs_setup": true }` and the WebUI redirects to `/onboarding`, which calls `POST /api/v1/auth/setup` to create the first user. That user is assigned the built-in `superadmin` system role.

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

**Telegram tools** (enabled with `tools.telegram = true`; require `telegram_token`):

| Tool | Description |
|------|-------------|
| `telegram_send_message` | Send a message to a chat |
| `telegram_react_message` | React with an emoji |
| `telegram_get_message_by_id` | Fetch a message |

## Sessions and topics

A session is `(agent, channel, topic)`. Each Discord channel / Telegram chat / WebUI `channel_id` has a default topic plus any number of named topics created with `/new`. History is per session. See [Agents → Checkpoints](./agents.md#checkpoints) for how long sessions are compacted.

## Managing tokens at runtime

- **WebUI**: agent Settings → Channels
- **API**: `PUT /api/v1/agents/{agent_id}` with `discord_token` / `telegram_token` (send `null` to disconnect)

The agent process is restarted on update, so the old bot connection is dropped and the new one established automatically.
