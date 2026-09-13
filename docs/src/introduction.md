# Introduction

> 21st Century Digital Steward; Right-hand agent for you majesty

Vizier is a Rust-based AI agent framework: a single binary that runs multiple concurrent AI agents, each with its own provider, tools, memory, and identity, exposed over Discord, Telegram, and HTTP (REST + WebSocket + a bundled React WebUI). Storage is embedded SQLite — no external database service required.

## Features

- **Multi-Channel**: Discord, Telegram, HTTP (REST API & WebSocket), and a bundled WebUI. Each agent gets its own bot token per platform.
- **Many Providers**: Ollama, Llama.cpp, OpenAI, Anthropic, Gemini, DeepSeek, OpenRouter, Xiaomi MiMo, Groq, Mistral, xAI, Perplexity, Moonshot, Z.ai, MiniMax, Together, Cohere, Hugging Face, Hyperbolic, Voyage AI, Galadriel, Mira, ChatGPT (OAuth), GitHub Copilot, Azure OpenAI, OpenCode Zen/Go, and any OpenAI-compatible `custom` endpoint. Runs on [rig](https://github.com/0xPlaygrounds/rig).
- **Open-Format Memory**: Long-term memory is plain markdown on disk (YAML frontmatter + body), organized into per-agent **bundles** with a link graph (`[label](concept.md)` and `[[bundle/slug]]`), semantic search via a per-agent embedding model, and `.zip` export/import.
- **Tools**: Shell execution (local or Docker-sandboxed), web search (Brave), web fetch, HTTP client, scheduler (cron & one-time tasks), memory graph tools, skills, session files (documents, images), TTS / STT / image generation, sub-tasks, inter-agent consult/delegate, per-platform messaging tools, and per-agent MCP servers.
- **Skills**: Reusable `SKILL.md` instruction packages with resources and scripts — global or per-agent, installable from a registry, git, or local path, and recommended to the agent by embedding similarity.
- **Dream Cycle**: Optional per-agent cron-scheduled reflection that extracts insights from recent sessions and consolidates them into memory and the agent's `CORE.md`.
- **Checkpoints**: Automatic context-window handover when a session approaches the model's context limit, plus manual `/checkpoint` and `/lobotomy` commands.
- **Multi-user**: JWT + API-key auth, roles with granular permissions, agent ownership and sharing.
- **Config-less**: Runs with no config file at all — the WebUI/API manages everything at runtime.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                          Channels                            │
│   ┌─────────┐   ┌──────────┐   ┌────────────────────────┐    │
│   │ Discord │   │ Telegram │   │  HTTP (REST/WS/WebUI)  │    │
│   └────┬────┘   └────┬─────┘   └───────────┬────────────┘    │
└────────┼─────────────┼─────────────────────┼─────────────────┘
         └─────────────┴──────────┬──────────┘
                                  ▼
                       VizierTransport (in-process bus)
                                  │
┌─────────────────────────────────▼────────────────────────────┐
│                    One process per agent                     │
│  ┌───────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐  │
│  │  CORE.md  │ │  Memory  │ │  Tools   │ │ Skills + MCP   │  │
│  │ (identity)│ │ (bundles)│ │ (+shell) │ │ (per-agent)    │  │
│  └───────────┘ └──────────┘ └──────────┘ └────────────────┘  │
└─────────────────────────────────┬────────────────────────────┘
                                  │
      ┌───────────────────────────┼───────────────────────────┐
      ▼                           ▼                           ▼
  Scheduler                   Providers                   Storage
 (cron, tasks,           (29 LLM backends,           (embedded SQLite +
  dream cycle)           TTS/STT/image/embed)         markdown memory)
```

## Quick Start

```sh
# Install Vizier
curl -fsSL https://get.vizier.rs | sh

# (Optional) generate a seed .vizier.yaml
vizier onboard

# Run
vizier run
```

Then open `http://localhost:9999`, create the first user account, and create your agents in the WebUI. Vizier works without a config file — see [Quick Start](./getting-started/quick-start.md).

## Configuration Model

Vizier has a deliberately small file-based config and pushes almost everything to runtime:

1. **Seed config** (`.vizier.yaml`, optional) — `providers`, `storage`, `channels.http`, `worker_threads`. Providers listed here are migrated into storage on first run.
2. **Runtime config** (WebUI / HTTP API) — providers, users, roles, and *all* agent configuration (model, tools, shell, MCP servers, embedding, channel tokens, dream schedule).

See [Configuration Overview](./configuration/index.md).

## Project Status

> [!WARNING]
> **Disclaimer:** This project is under active, fast-moving development. Configuration shapes and APIs may change between minor versions.

## Next Steps

- [Installation Guide](./getting-started/installation.md) — Get Vizier installed
- [Quick Start](./getting-started/quick-start.md) — Run your first agent
- [Configuration](./configuration/index.md) — Configure providers and agents
- [REST API](./api-integration/rest-api.md) — Integrate programmatically
