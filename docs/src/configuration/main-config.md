# 2.2 Main Configuration

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
