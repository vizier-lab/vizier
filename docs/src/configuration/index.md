# 2.1. Overview

This chapter covers everything you need to know about configuring Vizier.

## Configuration Model

Vizier uses a **two-tier configuration system**:

1. **Seed config** (`.vizier.yaml`) — provides initial values that are auto-migrated to storage on first run
2. **Runtime config** (WebUI/API) — after migration, most settings are managed via the WebUI or HTTP API

```
┌─────────────────────────────────────────────────────────┐
│                    First Run                            │
│                                                         │
│  .vizier.yaml ──auto-migrate──► Storage (SurrealDB)     │
│    (providers,       │           (providers table)       │
│     mcp_servers,     ├──► global_config table            │
│     shell,           ├──► agent configs (tokens)         │
│     channel tokens)  └──► user table                     │
│                                                         │
├─────────────────────────────────────────────────────────┤
│                    Subsequent Runs                       │
│                                                         │
│  .vizier.yaml (read for embedding, storage, http port)  │
│  Storage (all other config managed via WebUI/API)       │
└─────────────────────────────────────────────────────────┘
```

## What Stays in `.vizier.yaml`

These settings are **active file-based config** (read once at startup):

| Setting | Purpose |
|---------|---------|
| `embedding` | Embedding model selection |
| `storage` | Storage backend (filesystem vs SurrealDB) |
| `channels.http` | HTTP port, JWT secret, JWT expiry |
| `primary_user` | User identity (auto-migrated to user table) |

## What Migrates to Runtime

These settings are **seed values only** — auto-migrated to storage on first run:

| Setting | Runtime Location | Management |
|---------|-----------------|------------|
| `providers` | Providers table | WebUI Settings > Providers, or `PUT /api/v1/providers/{variant}` |
| `tools.mcp_servers` | Global config table | WebUI Settings > MCP Servers, or `PUT /api/v1/global-config/mcp_servers` |
| `shell` | Global config table | WebUI Settings > Shell, or `PUT /api/v1/global-config/shell` |
| `channels.discord/telegram` | Per-agent config | WebUI agent settings, or `PUT /api/v1/agents/{id}` |
| `tools.brave_search` | Per-agent config | WebUI agent settings (per-agent API key) |

## What's Runtime-Only (No YAML)

These are managed exclusively via WebUI/API:

| Setting | Management |
|---------|------------|
| Agents (all config) | WebUI Agents page, or CRUD `/api/v1/agents` |
| Agent documents (AGENT.md, IDENTITY.md, HEARTBEAT.md) | WebUI markdown editor, or `/api/v1/agents/{id}/documents` |
| Password & API keys | WebUI Settings > Password/API Keys, or `/api/v1/auth/*` |

## Quick Reference

| File | Purpose | Location |
|------|---------|----------|
| `.vizier.yaml` | Seed configuration (providers, channels, tools, storage, shell, embedding) | Project root |
| `.vizier/` | Workspace directory (auto-created) | Project root |

## Configuration Sections

- **[Main Configuration](./main-config.md)** - User identity and environment variables
- **[Providers](./providers.md)** - AI model provider settings
- **[Channels](./channels.md)** - Discord, Telegram, and HTTP server configuration
- **[Tools & Embedding](./tools-embedding.md)** - Global tool settings and embedding models
- **[Storage & Shell](./storage-shell.md)** - Data persistence and execution environment
- **[Agent Configuration](./agents.md)** - Runtime agent management via WebUI/API
- **[CLI Commands](./cli.md)** - `vizier run`, `vizier onboard`, `vizier shutdown`
- **[MCP Servers](./mcp.md)** - Model Context Protocol server integration
- **[Skills](./skills.md)** - Reusable agent behaviors

## Environment Variables

All configuration files support environment variable expansion:

```yaml
providers:
  openrouter:
    api_key: "${OPENROUTER_API_KEY}"
```

Common environment variables:

| Variable | Used For |
|----------|----------|
| `OPENROUTER_API_KEY` | OpenRouter provider |
| `DEEPSEEK_API_KEY` | DeepSeek provider |
| `ANTHROPIC_API_KEY` | Anthropic provider |
| `OPENAI_API_KEY` | OpenAI provider |
| `GEMINI_API_KEY` | Gemini provider |
| `XIAOMI_MIMO_API_KEY` | Xiaomi MiMo provider |
| `DISCORD_BOT_TOKEN` | Discord bot authentication |
| `BRAVE_API_KEY` | Brave Search API |

## Generating Configuration

### Initialize a New Project

```sh
vizier onboard
```

Creates `.vizier.yaml` with your chosen settings. Agents are then created via the WebUI.

## Loading Configuration

Vizier automatically looks for `.vizier.yaml` in the current directory. You can specify a custom path:

```sh
vizier run --config /path/to/.vizier.yaml
```

## Next Steps

- **[CLI Commands](./cli.md)** - `vizier run`, `vizier onboard`, `vizier shutdown`
- Learn about [Agents](./agents.md) - Create and manage agents via WebUI
