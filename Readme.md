# Vizier

> [!WARNING]
> **Disclaimer:** this project currently on high-speed development mode; Readmes and Documentations may not properly updated yet

> 21st Century Digital Steward; Right-hand agent for you majesty

Vizier is a Rust-based AI agent framework that provides a unified interface for AI assistants across multiple communication channels (Discord, HTTP, etc.) with memory, tool usage, and extensible architecture.

## Features

- **Multi-Channel Support**: Connect to Discord, HTTP (REST API & WebSocket), and WebUI
- **AI Model Integration**: Support for multiple AI providers (DeepSeek, OpenRouter, Ollama, etc.)
- **Memory System**: Session-based short-term memory, configurable recall depth, and vector-based long-term memory
- **Tool System**: Extensible tool framework including CLI access, web search (Brave Search), Python interpreter (opt-in), scheduler (cron & one-time tasks), vector memory, and workspace document management
- **Scheduler**: Built-in task scheduler for automated agent execution
- **WebUI**: Modern React-based web interface for interaction and management
- **TUI Interface**: Built-in terminal user interface for local interaction (WIP)
- **Configuration Driven**: Flexible configuration via YAML files with environment-specific overrides

## Installation and Configuration

### Prerequisites

No prerequisites required for standard installation. The install script handles everything automatically.

#### For Custom Installation (Building from Source)

- [Rust and Cargo](https://rust-lang.org/) installed

#### Optional: Python Support

**Python is NOT included by default.** Vizier can be built with or without Python interpreter support:

- **Default build**: No Python dependency required
- **With Python**: Add `--features python` flag to enable Python interpreter tool

If you want to use the Python interpreter tool, you need:
1. Python 3.9+ installed on your system
2. Build Vizier with Python feature enabled

**Installing Python 3.9+ (only if you need Python support):**

**macOS:**
```sh
brew install python@3.9
```

**Ubuntu/Debian:**
```sh
sudo apt-get install python3.9 python3.9-dev
```

**Windows:**
Download from [python.org](https://www.python.org/downloads/)

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
   vizier init
   ```
   This will create a minimal config and sample agent to run in your directory.

3. **Run the agent:**
   ```sh
   vizier run
   ```

#### Quick Start with Python

If you need the Python interpreter tool:

```sh
# Install with Python feature
cargo install vizier --features python

# Or from source
cargo build --release --features python
```

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

### Building with Python Support

To enable the Python interpreter tool, build Vizier with the `python` feature:

```sh
# Using cargo install
cargo install vizier --features python

# Building from source
cargo build --release --features python

# Development mode
just dev-python  # Run with Python feature
just release-python  # Build release with Python
```

**Requirements for Python support:**
- Python 3.9+ installed on your system
- Build with `--features python` flag

**What Python support adds:**
- Python interpreter tool for agents
- Programmatic tool calling within Python scripts
- Ability to execute Python code for data processing, automation, etc.

**Note:** Without Python support, any agents with `python_interpreter` enabled will simply skip that tool without error.

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

## Troubleshooting

### Python-Related Issues

#### Python Interpreter Not Available

If your agent configuration includes `python_interpreter: true` but the tool is not working:

**This is expected behavior!** By default, Vizier is built without Python support to minimize dependencies.

**Solution: Rebuild with Python feature**
```sh
# Reinstall with Python support
cargo install vizier --features python

# Or from source
cargo build --release --features python
```

#### Python Library Not Loaded Error

If you see an error like:
```
dyld[...]: Library not loaded: /Library/Frameworks/Python.framework/Versions/3.14/Python
```

This means you built Vizier with Python support, but the binary was linked against a different Python version than what's on your system.

**Solution: Build from source against your Python version**
```sh
# Clone the repository
git clone https://github.com/vizier-lab/vizier
cd vizier

# Build with your Python version
PYO3_PYTHON=$(which python3.9) cargo build --release --features python
```

#### Wrong Python Version

If you see:
```
Python version X.X detected, but Python 3.9 or higher is required
```

Install Python 3.9 or higher following the installation instructions above, then rebuild with Python feature.


## Planned Features (V1.0.0)

- [x] Web UI (React-based interface)
- [x] Scheduler and task system (cron & one-time tasks)
- [x] Vector memory for long-term retention
- [x] Python interpreter tool
    - [x] Programmatic Tool Calling
- [x] Brave Search integration
- [x] Local embedding model support
- [x] Docker Sandbox
- [x] Simple TUI (terminal user interface)
- [x] Additional AI providers (Google Gemini, OpenAI, Anthropic, etc.)
- [ ] WASM based plugin
- [ ] Model Context Protocol (MCP) integration
- [ ] Built-in HTTP client tool
- [ ] Skill system for reusable agent behaviors

## Development

### Project Structure

- `src/`: Rust source code (agents, channels, tools, scheduler, database, etc.)
- `webui/`: React-based web interface (built with Vite + React Router)
- `templates/`: Template files for agent configuration and identity
- `.vizier/`: Workspace directory for runtime data (config, database, agent workspaces)
- `migrations/`: Database migrations (SurrealDB schemas)

### Available Commands

See the [`Justfile`](Justfile) for available commands:

| Command | Description |
|---------|-------------|
| `just install` | Install all dependencies (Rust crates + webui npm packages) |
| `just dev` | Run in development mode with hot-reload |
| `just dev-python` | Run in development mode with Python support |
| `just run` | Run in release mode |
| `just run-python` | Run in release mode with Python support |
| `just release` | Build release binary |
| `just release-python` | Build release binary with Python support |
| `just tui` | Start the terminal user interface (WIP) |
| `just docker` | Start Docker services (database, etc.) |
| `just build` | Build the webui frontend |

### CLI Commands

The `vizier` binary provides these subcommands:

- `vizier run --config <path>`: Start the agent with given config
- `vizier tui`: Launch the TUI client (requires running agent)
- `vizier init`: Initialize a new vizier workspace
- `vizier configure`: Generate a new config non-interactively

### Adding New Features

1. **New Tools**: Add to `src/agent/tools/` and register in `src/agent/tools/mod.rs`
2. **New Channels**: Add to `src/channels/` and implement the `Channel` trait
3. **New Models**: Extend the provider system in `src/agent/agent_impl/provider.rs`
4. **New Schedules**: Add to `src/scheduler/` and integrate with task database

## License

MIT License
