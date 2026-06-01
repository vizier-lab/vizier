# 1.2 Quick Start

## Prerequisites

### Ollama (Optional — for Local Models)

If you want to use local models, install Ollama:

**macOS:**
```sh
brew install ollama
ollama serve
```

**Linux:**
```sh
curl -fsSL https://ollama.com/install.sh | sh
ollama serve
```

**Windows:**
Download from [ollama.com](https://ollama.com/download/windows)

> **Note:** You can use any supported provider (OpenRouter, DeepSeek, Anthropic, etc.) instead of Ollama. Configure your preferred provider during onboard or later in the WebUI.

## Initialize Your Workspace

Run the interactive onboard wizard:

```sh
vizier onboard
```

This will walk you through:
- Workspace path
- Username and primary user details
- HTTP port and JWT secret
- Provider selection (Ollama, OpenRouter, DeepSeek, Anthropic, OpenAI, Gemini, MiMo, Llama.cpp)
- Embedding model selection
- Storage backend (Filesystem or SurrealDB)

## Run Your Agent

```sh
vizier run
```

Then open `http://localhost:9999` in your browser to access the WebUI.

## Create Your First Agent

1. Open the WebUI at `http://localhost:9999`
2. Navigate to Agents and click "Create Agent"
3. Configure your agent's name, provider, model, and system prompt
4. Start chatting!

## Development Quick Start

For development, clone the repository and use the provided `just` commands:

```sh
# Install dependencies (Rust crates and webui npm packages)
just install

# Run in development mode with hot-reload
just dev

# Build the webui
just build
```

### Available Just Commands

| Command | Description |
|---------|-------------|
| `just install` | Install all dependencies (Rust crates + webui npm packages) |
| `just dev` | Run in development mode with hot-reload |
| `just run` | Run in attached mode |
| `just shutdown` | Stop a running daemonized instance |
| `just release` | Build release binary |
| `just docker` | Start Docker services (database, etc.) |
| `just build` | Build the webui frontend |

## Next Steps

- Configure your agent: See [2. Configuration](../configuration/index.md)
- [API Integration](../api-integration/rest-api.md) - Connect programmatically
