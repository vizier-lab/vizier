# 2.4 Channels

## `channels`

Configure communication channels in `.vizier.yaml`:

```yaml
channels:
  discord:                              # Discord bot configuration
    vizier:                             # Agent-specific Discord config
      token: "${DISCORD_TOKEN}"
    assistant:                          # Another agent's Discord config
      token: "${DISCORD_TOKEN_2}"

  telegram:                             # Telegram bot configuration
    vizier:                             # Agent-specific Telegram config
      token: "${TELEGRAM_BOT_TOKEN}"

  http:                                 # HTTP/WebSocket server
    port: 9999                          # Default port
    jwt_secret: "${VIZIER_JWT_SECRET}"  # Secret for JWT signing
    jwt_expiry_hours: 720               # Token expiry (default: 30 days)
```

> **Note:** Discord and Telegram tokens are auto-migrated to per-agent configs on first run. After migration, tokens are managed via the WebUI agent settings, not the YAML file.

## Discord Channel

Each agent can have its own Discord bot configuration:

```yaml
channels:
  discord:
    <agent_name>:
      token: "${DISCORD_TOKEN}"
```

### Discord Tools

When enabled, agents can use these tools to interact with Discord:

- `discord_send_message` - Send a message to a Discord channel
- `discord_react_message` - React to a message with an emoji
- `discord_get_message_by_id` - Retrieve a message by its ID

## Telegram Channel

Each agent can have its own Telegram bot configuration:

```yaml
channels:
  telegram:
    <agent_name>:
      token: "${TELEGRAM_BOT_TOKEN}"
```

### Telegram Commands

When the Telegram channel is enabled, the following commands are available:

- `/ping` - Check if the bot is responsive
- `/new` - Create a new session with a fresh topic
- `/session [topic_id]` - Switch to a specific session or list all sessions if no topic_id provided

### Telegram Tools

When enabled, agents can use these tools to interact with Telegram:

- `telegram_send_message` - Send a message to a Telegram chat
- `telegram_react_message` - React to a message with an emoji
- `telegram_get_message_by_id` - Retrieve a message by its ID

## HTTP Channel

Configure the HTTP/WebSocket server:

```yaml
channels:
  http:
    port: 9999                          # Server port
    jwt_secret: "${VIZIER_JWT_SECRET}"  # Secret for JWT signing
    jwt_expiry_hours: 720               # Token expiry (default: 30 days)
```

### Authentication

The HTTP channel uses JWT (JSON Web Token) authentication:
- `jwt_secret`: Secret key used to sign tokens (use environment variable)
- `jwt_expiry_hours`: How long tokens remain valid

### WebUI Access

When HTTP channel is enabled, the WebUI is served at `http://localhost:<port>`.

## Managing Channel Tokens at Runtime

After the initial seed config is migrated, channel tokens are managed per-agent:

- **WebUI**: Agent settings > Channels
- **API**: `PUT /api/v1/agents/{agent_id}` with `discord_token` or `telegram_token`

When you update an agent's channel token via the API, the channel automatically reconciles — disconnecting the old bot connection and establishing a new one with the updated token.
