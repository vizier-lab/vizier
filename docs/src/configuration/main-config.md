# 2.2 Main Configuration

## `primary_user`

Defines the primary user who interacts with agents:

```yaml
primary_user:
  name: "Your Name"                    # Your name
  discord_id: "123456789"              # Your Discord user ID (optional)
  discord_username: "username"        # Your Discord username (optional)
  telegram_username: "username"       # Your Telegram username (optional)
  alias: ["you", "master", "boss"]    # Aliases the agent can use for you
```

## Environment Variable Expansion

Vizier supports environment variable expansion in configuration files using the `${VAR}` syntax:

```yaml
providers:
  openrouter:
    api_key: "${OPENROUTER_API_KEY}"
```

This allows you to keep sensitive credentials in environment variables or `.env` files while keeping your configuration clean. The following fields support environment variable expansion:

- All API keys in `providers.*.api_key`
- Discord tokens in `channels.discord.*.token`
- Brave Search API key in `tools.brave_search.api_key`
- Any other string field in the configuration

**Example `.env` file:**

```bash
OPENROUTER_API_KEY=sk-or-v1-...
DISCORD_TOKEN=MTA0...
BRAVE_API_KEY=BS...
```
