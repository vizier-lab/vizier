# Introduction

> 21st Century Digital Steward; Right-hand agent for you majesty

Vizier is a Rust-based AI agent framework that provides a unified interface for AI assistants across multiple communication channels (Discord, HTTP, etc.) with memory, tool usage, and extensible architecture.

## Features

- **Multi-Channel Support**: Connect to Discord, HTTP (REST API & WebSocket), and WebUI
- **AI Model Integration**: Support for multiple AI providers (DeepSeek, OpenRouter, Ollama, Anthropic, OpenAI, Gemini)
- **Memory System**: Session-based short-term memory, configurable recall depth, and vector-based long-term memory
- **Tool System**: Extensible tool framework including CLI access, web search (Brave Search), Python interpreter (opt-in), scheduler (cron & one-time tasks), vector memory, and workspace document management
- **Scheduler**: Built-in task scheduler for automated agent execution
- **WebUI**: Modern React-based web interface for interaction and management
- **TUI Interface**: Built-in terminal user interface for local interaction (WIP)
- **Configuration Driven**: Flexible configuration via YAML files with environment-specific overrides

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Channels                            │
│  ┌─────────┐  ┌─────────┐  ┌─────────────────────────┐  │
│  │ Discord │  │  HTTP   │  │         WebUI           │  │
│  └────┬────┘  └────┬────┘  └─────────────────────────┘  │
└───────┼────────────┼────────────────────────────────────┘
        │            │
        └────────────┴───────────────────┐
                                         │
┌────────────────────────────────────────▼────────────────┐
│                    Agent Core                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │   Memory     │  │   Tools      │  │  Scheduler   │   │
│  │  (Session &  │  │  (Search,    │  │  (Cron &     │   │
│  │   Vector)    │  │   Python,    │  │   Tasks)     │   │
│  └──────────────┘  └──────────────┘  └──────────────┘   │
└─────────────────────────────────────────────────────────┘
                                         │
┌────────────────────────────────────────▼────────────────┐
│                    Providers                            │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐        │
│  │OpenAI   │ │Anthropic│ │DeepSeek │ │Gemini   │        │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘        │
│  ┌─────────┐ ┌──────────┐                               │
│  │Ollama   │ │OpenRouter│                               │
│  └─────────┘ └──────────┘                               │
└─────────────────────────────────────────────────────────┘
```

## Quick Start

```sh
# Install Vizier
cargo install vizier

# Generate configuration
vizier init

# Run the agent
vizier run
```

See the [Getting Started](./getting-started/installation.md) section for detailed installation instructions.

## Project Status

> [!WARNING]
> **Disclaimer:** This project is currently in high-speed development mode. Documentation may not always be up-to-date with the latest features.

## Planned Features (V1.0.0)

- [x] Web UI (React-based interface)
- [x] Scheduler and task system (cron & one-time tasks)
- [x] Vector memory for long-term retention
- [x] Python interpreter tool with programmatic tool calling
- [x] Brave Search integration
- [x] Local embedding model support
- [x] Docker Sandbox
- [x] Simple TUI (terminal user interface)
- [x] Additional AI providers (Google Gemini, OpenAI, Anthropic)
- [ ] WASM-based plugin system
- [ ] Model Context Protocol (MCP) integration
- [ ] Built-in HTTP client tool
- [ ] Skill system for reusable agent behaviors

## Next Steps

- [Installation Guide](./getting-started/installation.md) - Get Vizier installed
- [Quick Start](./getting-started/quick-start.md) - Run your first agent
- [Configuration](./configuration/index.md) - Configure providers and agents
