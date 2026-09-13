# Vizier

> 21st Century Digital Steward

Vizier is a Rust-based AI agent framework: a single binary that runs multiple
concurrent AI agents over Discord, Telegram, and HTTP (REST + WebSocket +
bundled WebUI), with open-format markdown memory, extensible tools, per-agent
MCP servers, skills, and a built-in scheduler. Storage is embedded SQLite.

## Features

- Multi-channel: Discord, Telegram, HTTP (REST + WebSocket), WebUI
- 29 providers: Ollama, llama.cpp, OpenAI, Anthropic, Gemini, DeepSeek,
  OpenRouter, Groq, Mistral, xAI, Moonshot, MiniMax, Together, Cohere,
  Azure, OpenCode, custom OpenAI-compatible endpoints, and more
- Memory: markdown concept documents in per-agent bundles with a link graph
  and semantic search (local fastembed or cloud embeddings)
- Tools: shell (local/Docker), web search & fetch, HTTP client, scheduler,
  session files, TTS/STT/image generation, sub-tasks, inter-agent
  consult/delegate, MCP
- Skills, dream cycle, checkpoints, multi-user roles & API keys
- WebUI on port 9999; no external database

## Quick start

Run with no config file:

```sh
docker run --rm -p 9999:9999 -e VIZIER_JWT_SECRET=change-me blinfoldking/vizier
```

Persist data and run on a custom port:

```sh
docker run -p 8080:8080 \
  -v vizier-data:/data \
  -e VIZIER_DATA_DIR=/data \
  -e VIZIER_PORT=8080 \
  -e VIZIER_JWT_SECRET=$(openssl rand -hex 32) \
  blinfoldking/vizier
```

Pass a YAML config:

```sh
docker run -p 9999:9999 \
  -v $PWD/.vizier.yaml:/cfg.yaml \
  -e VIZIER_CONFIG=/cfg.yaml \
  blinfoldking/vizier
```

Open http://localhost:9999, create the first user, add a provider key under
Settings → Providers, and create an agent.

## Environment variables

| Variable | Purpose | Default |
|---|---|---|
| `VIZIER_CONFIG` | Path to a `.vizier.yaml` (loaded first, then env overrides). | unset |
| `VIZIER_DATA_DIR` / `VIZIER_WORKSPACE` | Container data directory. Use a volume to persist. | `$HOME/.vizier` |
| `VIZIER_PORT` | HTTP server port. | `9999` |
| `VIZIER_STORAGE` | `sqlite` (only supported value). | `sqlite` |
| `VIZIER_WORKERS` | Tokio worker thread count. | `4` |
| `VIZIER_WS_IDLE_TIMEOUT` | WebSocket idle timeout (seconds). | `300` |
| `VIZIER_JWT_SECRET` | JWT signing secret. **Set to a strong value in production.** | `vizier-default-secret-change-me` |
| `VIZIER_EXTRA_ARGS` | Append arbitrary extra CLI args. | unset |
| `RUST_LOG` | Log filter, e.g. `vizier=debug`. | unset |

Provider API keys can also be supplied as env vars (`OPENAI_API_KEY`,
`ANTHROPIC_API_KEY`, `OPENROUTER_API_KEY`, `GEMINI_API_KEY`, …) — they are
used as a fallback when no key is set in the WebUI.

## CLI passthrough

Any subcommand other than `run` is passed through unchanged (env vars skipped):

```sh
docker exec vizier vizier agent ps
docker run --rm blinfoldking/vizier --version
```

## Tags

- `blinfoldking/vizier:<version>` — pinned to a specific release
- `blinfoldking/vizier:latest` — latest stable release
- Multi-arch manifest list: `linux/amd64`, `linux/arm64`
- Also published to `ghcr.io/vizier-lab/vizier` (same tags, identical images)

## Source

- GitHub: https://github.com/vizier-lab/vizier
- Docs: https://github.com/vizier-lab/vizier/tree/master/docs/src

## License

MIT
