# 2.1 Overview

Vizier keeps file-based configuration deliberately small. The YAML seed file covers only process-level settings; everything about agents, users, and providers is managed at runtime through the WebUI or HTTP API and persisted in the embedded SQLite database.

## Configuration Model

```
┌──────────────────────────────────────────────────────────────┐
│  .vizier.yaml (optional, read once at startup)               │
│                                                              │
│    providers ──── migrated on first run ───► providers table │
│    storage        (always sqlite)                            │
│    channels.http  (port, jwt_secret, jwt_expiry, ws timeout) │
│    worker_threads                                            │
├──────────────────────────────────────────────────────────────┤
│  CLI flags / env vars (override the file)                    │
│    --port  --data-dir  --storage  --workers  --ws-idle-timeout│
├──────────────────────────────────────────────────────────────┤
│  Runtime (WebUI / HTTP API, stored in SQLite)                │
│    providers, users, roles, api keys                         │
│    agents: model, tools, shell, MCP servers, embedding,      │
│            discord/telegram tokens, dream schedule, sharing  │
│    agent CORE.md, memory bundles, tasks, skills              │
└──────────────────────────────────────────────────────────────┘
```

## What lives in `.vizier.yaml`

| Key | Purpose | Lifecycle |
|-----|---------|-----------|
| `providers` | API keys / base URLs per provider | **Seed** — copied into storage on first run, then edited via WebUI/API. Later changes to the file for an already-migrated provider are ignored. |
| `storage` | `type: sqlite` | Read every start |
| `channels.http` | `port`, `jwt_secret`, `jwt_expiry_hours`, `ws_idle_timeout_secs` | Read every start |
| `worker_threads` | Tokio worker threads | Read every start |

Full reference: [Main Configuration](./main-config.md).

> **Legacy keys** such as `embedding`, `shell`, `tools`, `channels.discord`, and `channels.telegram` are no longer part of the schema. They are silently ignored if present. Embedding, shell, MCP, Brave Search, and channel tokens are now configured **per agent**.

## What's runtime-only

| Setting | Where |
|---------|-------|
| Agents (everything) | WebUI **Agents** / `POST|PUT /api/v1/agents` — see [Agents](./agents.md) |
| Agent `CORE.md` | WebUI agent **Core** page / `/api/v1/agents/{id}/core` |
| Agent memory bundles | WebUI agent **Memory** page / `/api/v1/agents/{id}/memory/*` — see [Memory](./memory.md) |
| Per-agent shell, MCP servers, Brave key, TTS/STT/image tools | Agent settings `tools` — see [Tools & Embedding](./tools-embedding.md) |
| Per-agent embedding model & indexer | Agent settings — see [Tools & Embedding](./tools-embedding.md) |
| Discord / Telegram bot tokens | Agent settings `discord_token` / `telegram_token` — see [Channels](./channels.md) |
| Providers | WebUI **Settings → Providers** / `/api/v1/providers` |
| Users, roles, permissions, API keys, password | WebUI **Settings** / `/api/v1/auth/*` |
| Skills | WebUI agent **Skills** page, `/api/v1/skills`, or `vizier skill` CLI — see [Skills](./skills.md) |

## Config-less mode

`vizier run` works with no config file at all. Resolution order:

1. `-c/--config <path>`
2. `$VIZIER_CONFIG`
3. `./.vizier.yaml` in the current directory
4. Built-in defaults

With no file, the defaults are: `ollama` + `llama_cpp` providers at their localhost URLs, SQLite storage, HTTP on port `9999` with `jwt_secret: "${VIZIER_JWT_SECRET}"`, 4 worker threads. The workspace resolves to `$VIZIER_DATA_DIR` if set, otherwise `$HOME/.vizier`.

When a config file **is** used, the workspace is `<config dir>/.vizier/`.

> **`VIZIER_JWT_SECRET` is required in config-less mode.** The built-in default references `${VIZIER_JWT_SECRET}`, and an unset variable is a load error. The Docker image sets a placeholder secret (`vizier-default-secret-change-me`) — override it in production.

## Workspace layout

```
<workspace>/
  agents/<agent_id>/
    memory/<bundle>/        # markdown memory concepts + index.md + log.md
    skills/<slug>/          # per-agent skills
  skills/<slug>/            # global skills
  uploads/<file_id>/        # uploaded session files
  .runtime/
    vizier.db               # SQLite database (agents, sessions, history, users, memory index…)
    .vizier.sock            # command socket used by `vizier shutdown` / `agent ps`
    logs/                   # stdout/stderr when running detached
```

## Environment variables

`.vizier.yaml` supports `${VAR}` expansion anywhere (via `shellexpand`):

```yaml
vizier:
  providers:
    openrouter:
      api_key: "${OPENROUTER_API_KEY}"
```

Variables used by the defaults and the Docker entrypoint:

| Variable | Used for |
|----------|----------|
| `VIZIER_CONFIG` | Config file path (when `-c` isn't given) |
| `VIZIER_DATA_DIR` / `VIZIER_WORKSPACE` | Workspace directory in config-less mode / Docker |
| `VIZIER_JWT_SECRET` | Referenced by the default `channels.http.jwt_secret` |
| `VIZIER_PORT`, `VIZIER_STORAGE`, `VIZIER_WORKERS`, `VIZIER_WS_IDLE_TIMEOUT`, `VIZIER_EXTRA_ARGS` | Docker entrypoint → CLI flags |
| `RUST_LOG` | Tracing filter (e.g. `vizier=debug`) |
| `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, `DEEPSEEK_API_KEY`, `OPENROUTER_API_KEY`, `XIAOMI_MIMO_API_KEY`, `GROQ_API_KEY`, `MISTRAL_API_KEY`, `XAI_API_KEY`, … | Referenced by each provider's default `api_key` placeholder — see [Providers](./providers.md) |

## Sections

- **[Main Configuration](./main-config.md)** — full `.vizier.yaml` reference
- **[Providers](./providers.md)** — supported LLM providers and their keys
- **[Channels](./channels.md)** — HTTP server, Discord, Telegram
- **[Tools & Embedding](./tools-embedding.md)** — per-agent tools, embedding, indexer
- **[Storage & Shell](./storage-shell.md)** — SQLite storage and per-agent shell (local/Docker)
- **[Agents](./agents.md)** — agent fields, CORE.md, dream cycle, checkpoints, sharing
- **[CLI](./cli.md)** — `run`, `shutdown`, `onboard`, `skill`, `agent`, Docker
- **[MCP Servers](./mcp.md)** — per-agent Model Context Protocol servers
- **[Skills](./skills.md)** — skill packages
- **[Memory](./memory.md)** — bundles, concept documents, link graph
