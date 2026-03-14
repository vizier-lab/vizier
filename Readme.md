# Vizier

> **Disclaimer:** this project currently on high-speed development mode; Readmes and Documentations may not properly updated yet

> 21st Century Digital Steward; Right-hand agent for you majesty

Vizier is a Rust-based AI agent framework that provides a unified interface for AI assistants across multiple communication channels (Discord, HTTP, etc.) with memory, tool usage, and extensible architecture.

## Features

- **Multi-Channel Support**: Connect to Discord, HTTP, and other communication platforms
- **AI Model Integration**: Support for multiple AI providers (DeepSeek, OpenRouter, Ollama, etc.)
- **Memory System**: Session-based memory with configurable recall depth
- **Tool System**: Extensible tool framework with CLI access, web search, and vector memory
- **TUI Interface**: Built-in terminal user interface for local interaction (WIP)
- **Configuration Driven**: Flexible configuration via TOML files

## Installation and Configuration

**you need to have [cargo](https://rust-lang.org/) installed**

to setup your initial config, run this command:
```sh
# install vizier
cargo install vizier-ai

# generate config and workspace
vizier-ai onboard

## run the agent
vizier-ai run --config "PATH_TO_YOUR_CONFIG"
```

## Update Installed Version

1. install `cargo-update` if you haven't installed it
```sh
cargo install cargo-update
```

2. update the binary
```sh
cargo install-update vizier-ai
```


## Planned Features

- [ ] additional channels and client
    - [ ] web ui
    - [ ] tui (on progress)
- [ ] additional providers support
    - [ ] google
    - [ ] openai
    - [ ] etc
- [ ] support openai embedding model
- [ ] additional tools
    - [ ] mcp
    - [ ] built-in http client
- [ ] scheduler and task system
- [ ] skill system
- [ ] misc
    - [ ] webpage
    - [ ] logo

## Development

### Project Structure

- `src/`: Rust source code
- `templates/`: Template files for agent configuration
- `migrations/`: Database migrations (if using SQL)
- `.vizier/`: Workspace directory for runtime data

### Available Commands

See the `Justfile` for available commands:
- `just dev`: Run in development mode
- `just run`: Run in release mode  
- `just tui`: Start TUI interface
- `just docker`: Start Docker services

### Adding New Features

1. **New Tools**: Add to `src/agent/tools/`
2. **New Channels**: Add to `src/channels/`
3. **New Models**: Extend the model provider system in `src/agent/mod.rs`

## License

MIT License
