# Vizier

> 21st Century Digital Steward; Right-hand agent for you majesty

Vizier is a Rust-based AI agent framework that provides a unified interface for AI assistants across multiple communication channels (Discord, Telegram, HTTP, WebUI) with memory, tool usage, and extensible architecture.

## Features

- **Multi-Channel Support**: Connect to Discord, Telegram, HTTP (REST API & WebSocket), and WebUI
- **AI Model Integration**: Support for multiple AI providers (DeepSeek, OpenRouter, Ollama, Anthropic, OpenAI, Gemini, Xiaomi MiMo, Llama.cpp)
- **Memory System**: Session-based short-term memory, configurable recall depth, and vector-based long-term memory
- **Tool System**: Extensible tool framework including shell execution, web search (Brave Search), HTTP client, web fetch, scheduler (cron & one-time tasks), vector memory, workspace document management, sub-agent spawning, Python sandbox, and inter-agent communication
- **Scheduler**: Built-in task scheduler for automated agent execution
- **WebUI**: Modern React-based web interface for interaction and management
- **Configuration Driven**: YAML seed config with runtime management via WebUI

## Installation and Configuration

### Prerequisites

No prerequisites required for standard installation. The install script handles everything automatically.

#### For Custom Installation (Building from Source)

- [Rust and Cargo](https://rust-lang.org/) installed

### Quick Start

1. **Install Vizier** (Recommended):
   ```sh
   curl -fsSL https://get.vizier.rs | sh
   ```
   
   Or install via cargo (requires Rust):
   ```sh
   cargo install vizier
   # Or using cargo-binstall (faster)
   cargo binstall vizier
   ```

2. **Generate configuration and workspace:**
   ```sh
   vizier onboard
   ```
   This will walk you through provider selection, embedding config, storage backend, and HTTP server setup.

3. **Run the agent:**
   ```sh
   vizier run
   ```

4. **Open the WebUI** at `http://localhost:9999` to create and manage agents.

### Development Setup

For development, clone the repository and use the provided `just` commands:

```sh
# Install dependencies (Rust crates and webui npm packages)
just install

# Run in development mode with hot-reload
just dev

# Build the webui
just build
```

See the [Justfile](Justfile) for all available commands.

### WebUI

The web interface is built with React and served automatically when the HTTP channel is enabled. After building (`just build`), it will be available at `http://localhost:9999` (or the port configured in your `config.yaml`).

## Update Installed Version

### Using Install Script

Simply re-run the install script to get the latest version:
```sh
curl -fFSL https://get.vizier.rs | sh
```

### Using Cargo (if installed via cargo)

1. Install `cargo-update` if you haven't already:
   ```sh
   cargo install cargo-update
   ```

2. Update the binary:
   ```sh
   cargo install-update vizier
   ```

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

## Development

### Project Structure

- `src/`: Rust source code
  - `agents/`: Agent process loop, LLM interaction, tools, hooks, skills
  - `channels/`: Discord, Telegram, HTTP (REST + WebSocket + WebUI serving)
  - `storage/`: Filesystem and SurrealDB storage backends
  - `config/`: YAML seed config deserialization
  - `schema/`: Shared types (responses, agent IDs, provider entries)
  - `mcp/`: MCP client + server integration
  - `embedding/`: Local embedding models (fastembed)
  - `shell/`: Shell execution abstraction (local + Docker)
  - `scheduler/`: Cron and one-time task scheduler
  - `transport/`: Command transport for agent/channel/global commands
- `webui/`: React-based web interface (React Router v7 + Tailwind v4 + TypeScript)
- `templates/`: Template files for agent configuration and identity
- `.vizier/`: Workspace directory for runtime data (config, database, agent workspaces)

### Available Commands

See the [`Justfile`](Justfile) for available commands:

| Command | Description |
|---------|-------------|
| `just install` | Install all dependencies (Rust crates + webui npm packages) |
| `just dev` | Run in development mode with hot-reload |
| `just run` | Run in attached mode |
| `just release` | Build release binary |
| `just docker` | Start Docker services (database, etc.) |
| `just build` | Build the webui frontend |

### CLI Commands

The `vizier` binary provides these subcommands:

- `vizier run [--config <path>]`: Start agents, server, and channels (daemonizes by default; use `-a` for attached mode). Works without a config file — see the [Docker](#docker) section for env-var configuration.
- `vizier shutdown [--config <path>]`: Stop a running daemonized instance
- `vizier onboard --path <path>`: Interactive wizard to generate seed config
- `vizier agent ps`: List running agents and their status

Agents are created and managed at runtime via the WebUI or HTTP API — there is no CLI subcommand for agent management beyond `ps`.

### Docker

The `ghcr.io/vizier-lab/vizier` image starts vizier with no config file. Configure via env vars (consumed by `docker-entrypoint.sh`):

| Env var | Purpose | Default |
|---|---|---|
| `VIZIER_CONFIG` | Path to a `.vizier.yaml` to load. If set, file is loaded first, then env-var overrides apply on top. | unset |
| `VIZIER_DATA_DIR` (or `VIZIER_WORKSPACE`) | Container data directory. | `$HOME/.vizier` (use a volume to persist) |
| `VIZIER_PORT` | HTTP server port. | `9999` |
| `VIZIER_STORAGE` | `filesystem` or `sqlite`. | `filesystem` |
| `VIZIER_WORKERS` | Tokio worker thread count. | `4` |
| `VIZIER_WS_IDLE_TIMEOUT` | WebSocket idle timeout (seconds). | `300` |
| `VIZIER_JWT_SECRET` | JWT signing secret. **Set to a strong value in production.** | `vizier-default-secret-change-me` |
| `VIZIER_EXTRA_ARGS` | Append arbitrary extra CLI args. | unset |

Examples:

```sh
# Config-less, port 8080
docker run -p 8080:8080 -e VIZIER_PORT=8080 ghcr.io/vizier-lab/vizier

# Persist data with a named volume
docker run -p 9999:9999 -v vizier-data:/data -e VIZIER_DATA_DIR=/data \
  ghcr.io/vizier-lab/vizier

# Pass a config file plus overrides
docker run -p 9999:9999 \
  -v $PWD/dev.vizier.yaml:/cfg.yaml \
  -e VIZIER_CONFIG=/cfg.yaml \
  -e VIZIER_PORT=8080 \
  ghcr.io/vizier-lab/vizier

# Subcommand passthrough (env vars skipped)
docker run ghcr.io/vizier-lab/vizier shutdown
```

### Adding New Features

1. **New Tools**: Add to `src/agents/tools/` and register in `src/agents/tools/mod.rs`
2. **New Channels**: Add to `src/channels/` and implement the `VizierChannel` trait
3. **New Models**: Extend the provider system in `src/agents/agent/model/`
4. **New Schedules**: Add to `src/scheduler/` and integrate with task database

## License

MIT License
