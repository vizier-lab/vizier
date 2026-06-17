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
| `-d, --detached` | Run in the background (daemonize) |

By default, `vizier run` runs in the foreground (useful for development; Ctrl-C stops it). Use `-d` / `--detached` to run in the background — in that case:

- PID is written to `/tmp/vizier.pid`
- Logs go to `.vizier/.runtime/logs/`

### `vizier shutdown`

Stop a running instance.

```sh
vizier shutdown --config /path/to/.vizier.yaml
```

| Flag | Description |
|------|-------------|
| `-c, --config <PATH>` | Path to `.vizier.yaml` config file |

### `vizier skill`

Manage skills — install, list, uninstall, and update.

#### `vizier skill install <source>`

Install a skill from registry, git repository, or local path.

```sh
# Install from registry (vizier-lab/vizier)
vizier skill install code-review

# Install from git repository
vizier skill install https://github.com/user/custom-skills.git

# Install from local path
vizier skill install ./my-local-skill

# Install for a specific agent
vizier skill install code-review --agent my-agent
```

| Flag | Description |
|------|-------------|
| `-a, --agent <ID>` | Install for a specific agent (optional) |

**Source detection:**
- Plain slug (e.g., `calculator`) → fetches from vizier-lab/vizier registry
- Git URL (e.g., `https://github.com/...`) → clones and installs
- Local path (e.g., `./my-skill`) → copies files

> **Note:** Requires `git` to be installed for registry and git sources.

#### `vizier skill list`

List installed skills.

```sh
vizier skill list
vizier skill list --activation contextual
```

| Flag | Description |
|------|-------------|
| `-a, --activation <MODE>` | Filter by activation mode (`always`, `on_demand`, `contextual`) |

#### `vizier skill uninstall <slug>`

Remove a skill.

```sh
vizier skill uninstall code-review
vizier skill uninstall code-review --agent my-agent
```

| Flag | Description |
|------|-------------|
| `-a, --agent <ID>` | Uninstall from a specific agent (optional) |

#### `vizier skill update <slug>`

Update a skill from its registry source.

```sh
vizier skill update code-review
```

> **Note:** Only skills installed from the registry can be updated. Skills created locally or from external git repos cannot be updated via CLI.

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
