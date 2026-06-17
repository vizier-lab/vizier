# Vizier

> 21st Century Digital Steward

Vizier is a Rust-based AI agent framework providing a unified interface for AI
assistants across Discord, Telegram, HTTP, and WebUI channels — with memory,
extensible tools, MCP integration, and a built-in scheduler.

## Features

- Multi-channel: Discord, Telegram, HTTP (REST + WebSocket), WebUI
- AI providers: DeepSeek, OpenRouter, Ollama, Anthropic, OpenAI, Gemini,
  Xiaomi MiMo, Llama.cpp
- Memory: session short-term + vector long-term (local fastembed)
- Tool system: shell, web fetch, HTTP client, scheduler, vector memory,
  Python sandbox, sub-agents, MCP
- WebUI: React-based management UI on port 9999
- Embedded storage: filesystem or sqlite (no external DB needed)

## Quick start

Run with no config file — uses sensible defaults:

```sh
docker run --rm -p 9999:9999 blinfoldking/vizier
```

Persist data and run on a custom port:

```sh
docker run -p 8080:8080 \
  -v vizier-data:/data \
  -e VIZIER_DATA_DIR=/data \
  -e VIZIER_PORT=8080 \
  blinfoldking/vizier
```

Pass a YAML config:

```sh
docker run -p 9999:9999 \
  -v $PWD/dev.vizier.yaml:/cfg.yaml \
  -e VIZIER_CONFIG=/cfg.yaml \
  blinfoldking/vizier
```

Open http://localhost:9999 to manage agents.

## Environment variables

| Variable | Purpose | Default |
|---|---|---|
| `VIZIER_CONFIG` | Path to a `.vizier.yaml` (loaded first, then env overrides). | unset |
| `VIZIER_DATA_DIR` / `VIZIER_WORKSPACE` | Container data directory. Use a volume to persist. | `$HOME/.vizier` |
| `VIZIER_PORT` | HTTP server port. | `9999` |
| `VIZIER_STORAGE` | `filesystem` or `sqlite`. | `sqlite` |
| `VIZIER_WORKERS` | Tokio worker thread count. | `4` |
| `VIZIER_WS_IDLE_TIMEOUT` | WebSocket idle timeout (seconds). | `300` |
| `VIZIER_JWT_SECRET` | JWT signing secret. **Set to a strong value in production.** | placeholder |
| `VIZIER_EXTRA_ARGS` | Append arbitrary extra CLI args. | unset |

## CLI passthrough

Any subcommand other than `run` is passed through unchanged (env vars skipped):

```sh
docker run --rm blinfoldking/vizier shutdown
docker run --rm blinfoldking/vizier agent ps
```

## Tags

- `blinfoldking/vizier:<version>` — pinned to a specific release
- `blinfoldking/vizier:latest` — latest stable release
- Multi-arch manifest list: `linux/amd64`, `linux/arm64`
- Also published to `ghcr.io/vizier-lab/vizier` (same tags, identical images)

## Source

- GitHub: https://github.com/vizier-lab/vizier
- Full docs: https://github.com/vizier-lab/vizier/blob/main/Readme.md

## License

MIT
