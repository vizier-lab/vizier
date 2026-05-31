# 2.8 CLI Commands

## Subcommands

### `vizier onboard`

Interactive wizard to generate the seed configuration file (`.vizier.yaml`).

```sh
vizier onboard --path /path/to/workspace
```

| Flag | Description |
|------|-------------|
| `-p, --path <PATH>` | Workspace path (where `.vizier.yaml` will be created) |

The wizard walks you through:
- Workspace path
- Username and primary user details
- HTTP port and JWT secret
- Provider selection and API keys
- Embedding model selection
- Storage backend choice

> **Note:** After onboarding, agents are created and managed via the WebUI, not via this command.

### `vizier run`

Start agents, server, and channels.

```sh
vizier run --config /path/to/.vizier.yaml
```

| Flag | Description |
|------|-------------|
| `-c, --config <PATH>` | Path to `.vizier.yaml` config file |
| `-a, --attached` | Run in foreground (no daemonization) |

By default, `vizier run` daemonizes the process:
- PID is written to `/tmp/vizier.pid`
- Logs go to `.vizier/.runtime/logs/`
- Use `-a` / `--attached` to run in the foreground (useful for development)

### `vizier shutdown`

Stop a running daemonized instance.

```sh
vizier shutdown --config /path/to/.vizier.yaml
```

| Flag | Description |
|------|-------------|
| `-c, --config <PATH>` | Path to `.vizier.yaml` config file |

## Configuration Loading

Vizier automatically looks for `.vizier.yaml` in the current directory. You can specify a custom path:

```sh
vizier run --config /path/to/.vizier.yaml
```

### Loading Order

1. Load `.vizier.yaml` from current directory (or specified path)
2. Initialize storage backend
3. Auto-migrate seed config to storage (providers, MCP servers, shell, channel tokens)
4. Start HTTP server, agents, and channels

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

### Example `.env` file

```bash
OPENROUTER_API_KEY=sk-or-v1-...
DISCORD_TOKEN=MTA0...
BRAVE_API_KEY=BS...
```
