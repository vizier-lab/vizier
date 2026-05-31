# Introduction

> 21st Century Digital Steward; Right-hand agent for you majesty

Vizier is a Rust-based AI agent framework that provides a unified interface for AI assistants across multiple communication channels (Discord, Telegram, HTTP, WebUI) with memory, tool usage, and extensible architecture.

## Features

- **Multi-Channel Support**: Connect to Discord, Telegram, HTTP (REST API & WebSocket), and WebUI
- **AI Model Integration**: Support for multiple AI providers (DeepSeek, OpenRouter, Ollama, Anthropic, OpenAI, Gemini, Xiaomi MiMo)
- **Memory System**: Session-based short-term memory, configurable recall depth, and vector-based long-term memory
- **Tool System**: Extensible tool framework including shell execution, web search (Brave Search), HTTP client, web fetch, scheduler (cron & one-time tasks), vector memory, workspace document management, sub-agent spawning, Python sandbox, and inter-agent communication
- **Scheduler**: Built-in task scheduler for automated agent execution
- **WebUI**: Modern React-based web interface for interaction and management
- **Configuration Driven**: YAML seed config with runtime management via WebUI

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Channels                            │
│  ┌─────────┐  ┌──────────┐  ┌─────────┐  ┌───────────┐  │
│  │ Discord │  │ Telegram │  │  HTTP   │  │   WebUI   │  │
│  └────┬────┘  └────┬─────┘  └────┬────┘  └─────┬─────┘  │
└───────┼────────────┼─────────────┼──────────────┼────────┘
        │            │             │              │
        └────────────┴─────────────┴──────────────┘
                                 │
┌────────────────────────────────▼────────────────────────┐
│                    Agent Core                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │   Memory     │  │   Tools      │  │  Scheduler   │   │
│  │  (Session &  │  │  (Search,    │  │  (Cron &     │   │
│  │   Vector)    │  │   Sandbox,   │  │   Tasks)     │   │
│  │              │  │   Subagents) │  │              │   │
│  └──────────────┘  └──────────────┘  └──────────────┘   │
└─────────────────────────────────────────────────────────┘
                                 │
┌────────────────────────────────▼────────────────────────┐
│                    Providers                            │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐        │
│  │OpenAI   │ │Anthropic│ │DeepSeek │ │Gemini   │        │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘        │
│  ┌─────────┐ ┌──────────┐ ┌─────────┐                   │
│  │Ollama   │ │OpenRouter│ │  MiMo   │                   │
│  └─────────┘ └──────────┘ └─────────┘                   │
└─────────────────────────────────────────────────────────┘
```

## Quick Start

```sh
# Install Vizier
curl -fsSL https://get.vizier.rs | sh

# Generate configuration
vizier onboard

# Run the agent
vizier run
```

Then open `http://localhost:9999` to create and manage agents via the WebUI.

See the [Getting Started](./getting-started/installation.md) section for detailed installation instructions.

## Project Status

> [!WARNING]
> **Disclaimer:** This project is currently in high-speed development mode. Documentation may not always be up-to-date with the latest features.

## Configuration Model

Vizier uses a **two-tier configuration system**:

1. **Seed config** (`.vizier.yaml`) — provides initial values for providers, embedding, storage, channels, and tools. Auto-migrated to storage on first run.
2. **Runtime config** (WebUI/API) — after migration, most settings are managed via the WebUI or HTTP API and stored in the database.

See [Configuration Overview](./configuration/index.md) for details.

## Planned Features (V1.0.0)

- [x] Web UI (React-based interface)
- [x] Scheduler and task system (cron & one-time tasks)
- [x] Vector memory for long-term retention
- [x] Brave Search integration
- [x] Local embedding model support
- [x] Docker Sandbox
- [x] Additional AI providers (Google Gemini, OpenAI, Anthropic, Xiaomi MiMo, etc.)
- [x] Sub-agent spawning for parallel task execution
- [x] Model Context Protocol (MCP) integration
- [x] Skill system for reusable agent behaviors
- [x] Built-in HTTP client tool
- [ ] WASM-based plugin system

## Next Steps

- [Installation Guide](./getting-started/installation.md) - Get Vizier installed
- [Quick Start](./getting-started/quick-start.md) - Run your first agent
- [Configuration](./configuration/index.md) - Configure providers and agents
