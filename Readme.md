# Vizier

> 21st Century Digital Steward; Right-hand agent for you majesty

Vizier is a Rust-based AI agent framework: a single binary that runs multiple concurrent AI agents — each with its own provider, tools, memory, and identity — exposed over Discord, Telegram, and HTTP (REST + WebSocket + a bundled React WebUI). Storage is embedded SQLite; no external services required.

📖 **Docs:** `docs/` (mdBook) — start with `docs/src/introduction.md`.

## Features

- **Multi-channel** — Discord, Telegram, REST, WebSocket, WebUI. Each agent gets its own bot token per platform; slash commands for sessions, checkpoints, and aborts.
- **29 providers** — Ollama, llama.cpp, OpenAI, Anthropic, Gemini, DeepSeek, OpenRouter, Xiaomi MiMo, Groq, Mistral, xAI, Perplexity, Moonshot, Z.ai, MiniMax, Together, Cohere, Hugging Face, Hyperbolic, Voyage AI, Galadriel, Mira, ChatGPT (OAuth), GitHub Copilot, Azure OpenAI, OpenCode Zen/Go, and any OpenAI-compatible `custom` endpoint (via [rig](https://github.com/0xPlaygrounds/rig)).
- **Open-format memory** — long-term memory is plain markdown on disk, organized into per-agent bundles with a link graph (`[label](concept.md)`, `[[bundle/slug]]`), semantic search via a per-agent embedding model (local fastembed or cloud), and `.zip` export/import.
- **Tools** — shell (local or Docker sandbox), Brave web/news search, web fetch, HTTP client, cron & one-time scheduler, memory graph tools, skills, session files (PDF/DOCX/XLSX/images), TTS / STT / image generation, parallel sub-tasks, inter-agent consult/delegate, per-agent MCP servers.
- **Skills** — reusable `SKILL.md` packages with resources and scripts; global or per-agent; installable from a registry, git, or a local path; recommended to the agent by embedding similarity.
- **Dream cycle** — optional cron-scheduled reflection that extracts insights from recent sessions and consolidates them into memory and the agent's `CORE.md`.
- **Checkpoints** — automatic context handover when a session nears the model's context window, plus `/checkpoint`, `/lobotomy`, `/abort`.
- **Multi-user** — JWT + API keys, roles with granular permissions, agent ownership and sharing.
- **Config-less** — runs with no config file; everything is managed in the WebUI/API.

## Quick Start

```sh
# 1. Install
curl -fsSL https://get.vizier.rs | sh          # or: cargo install vizier / cargo binstall vizier

# 2. (optional) seed a .vizier.yaml
vizier onboard

# 3. Run
vizier run                                     # config-less needs VIZIER_JWT_SECRET set
```

Open `http://localhost:9999`, create the first user, add a provider key under **Settings → Providers**, and create an agent.

### Docker

```sh
docker run -p 9999:9999 -v vizier-data:/data \
  -e VIZIER_DATA_DIR=/data -e VIZIER_JWT_SECRET=$(openssl rand -hex 32) \
  blinfoldking/vizier
```

Images: `blinfoldking/vizier` (Docker Hub, recommended) and `ghcr.io/vizier-lab/vizier` (identical). A sample `docker-compose.yaml` is included.

| Env var | Purpose | Default |
|---|---|---|
| `VIZIER_CONFIG` | Path to a `.vizier.yaml` to load (env overrides still apply on top) | unset |
| `VIZIER_DATA_DIR` / `VIZIER_WORKSPACE` | Data directory — mount a volume | `$HOME/.vizier` |
| `VIZIER_PORT` | HTTP port | `9999` |
| `VIZIER_STORAGE` | `sqlite` (only value; a legacy `filesystem` deployment is auto-migrated on first start) | `sqlite` |
| `VIZIER_WORKERS` | Tokio worker threads | `4` |
| `VIZIER_WS_IDLE_TIMEOUT` | WebSocket idle timeout (s) | `300` |
| `VIZIER_JWT_SECRET` | JWT signing secret — **set a strong value** | `vizier-default-secret-change-me` |
| `VIZIER_EXTRA_ARGS` | Extra CLI args appended to `vizier run` | unset |

Any first argument other than `run` (`shutdown`, `agent ps`, `skill …`) is passed straight through.

## Configuration model

`.vizier.yaml` is optional and small — only `providers` (seed, migrated to storage on first run), `storage`, `channels.http`, and `worker_threads`. Everything else is runtime state managed via the WebUI / HTTP API: providers, users/roles, and all agent configuration (model, tools, shell, MCP servers, embedding, channel tokens, dream schedule). Agents are never defined in YAML.

```yaml
vizier:
  providers:
    ollama: { base_url: "http://localhost:11434" }
    anthropic: { api_key: "${ANTHROPIC_API_KEY}" }
  storage: { type: sqlite }
  channels:
    http: { port: 9999, jwt_secret: "${VIZIER_JWT_SECRET}" }
```

Resolution order: `-c <path>` → `$VIZIER_CONFIG` → `./.vizier.yaml` → built-in defaults (workspace `$VIZIER_DATA_DIR` or `~/.vizier`).

## CLI

| Command | Description |
|---------|-------------|
| `vizier run [-c <path>] [-d] [--port] [--data-dir] [--storage sqlite] [--workers] [--ws-idle-timeout]` | Start agents, scheduler, channels, and the API/WebUI (`-d` daemonizes) |
| `vizier shutdown [-c <path>]` | Stop a running instance via its command socket |
| `vizier onboard [-p <path>]` | Interactive wizard that writes a seed `.vizier.yaml` |
| `vizier agent ps` | List running agents and their status |
| `vizier skill install <slug\|owner/repo\|git-url\|./path> [-a <agent>]` | Install a skill (needs `git`) |
| `vizier skill list \| uninstall <slug> \| update <slug>` | Manage installed skills |

## API

- REST under `/api/v1` — Swagger UI at `/swagger`.
- `POST /api/v1/agents/{id}/chat` for synchronous chat; `ws://…/api/v1/agents/{id}/channel/{channel}/topic/{topic}/chat?token=<jwt>` for streaming (thinking, tool calls, checkpoints, final message).
- Auth: `Authorization: Bearer <jwt>` or `Authorization: ApiKey vk_…`.

See `docs/src/api-integration/`.

## Development

```sh
just install     # cargo fetch + npm install in webui/ (+ cargo-watch)
just dev         # cargo watch -s "just run" — hot reload with dev.vizier.yaml
just run         # cargo run -- run --config dev.vizier.yaml
just run-d       # same, detached
just shutdown    # stop the dev instance
just build       # build the WebUI (npm run build)
just release     # cargo build --release
just docker      # docker-compose down && up -d

cargo test       # tests are sparse; most behavior is exercised by running the binary
cargo clippy
cd webui && npm run typecheck
```

> **Build gotcha:** `build.rs` runs `npm run build` in `webui/` on every `cargo build` if `webui/node_modules/` exists. If `node_modules/` is absent and `webui/build/client/` doesn't exist, the build panics — run `just install` first.

### Project structure

```
src/
  cli/              run / shutdown / onboard / skill / agent subcommands
  config/           .vizier.yaml schema (providers, storage, http channel, shell & MCP types)
  dependencies.rs   VizierDependencies: opens storage, runs one-time migrations
  transport.rs      in-process message bus (flume) tying agents, channels, scheduler together
  agents/           per-agent process loop, model/provider abstraction, tools, hooks, shell (local/docker), MCP, skills runtime
  channels/         discord (serenity), telegram (teloxide), http (axum: REST, WS, JWT/API-key auth, WebUI static)
  scheduler/        cron + one-time tasks, dream cycle
  storage/          VizierStorage over SQLite; document store + BundleMemoryStore for markdown memory
  indexer/          sqlite-vec vector index
  embedding/ tts/ stt/ image_generation/   per-provider adapters
  skill/            skill packages: manifest, install (registry/git/local)
  schema/           shared types (agent config, requests/responses, sessions, history)
  command/          Unix-socket command server (shutdown, health) for the CLI
webui/              React Router v7 + React 19 + Tailwind v4; built output served by the HTTP channel
templates/          CORE.md seed for new agents
skills/             skill registry (installed with `vizier skill install <slug>`)
vizier-derive/      #[derive(MarkdownDoc)] for frontmatter+body documents
docs/               mdBook documentation
specs/              design specs (Spec Kit)
```

### Extending

Everything is trait-based — implement the trait and register it in the module constructor:

- **Tool**: implement `VizierTool` in `src/agents/tools/<name>.rs`, add it to `default_toolset` or `user_toolset` in `VizierTools::new()`; add to `DREAM_TOOL_NAMES` if the dream cycle should have it.
- **Channel**: implement `VizierChannel` in `src/channels/`, spawn it in `VizierChannels::run()`.
- **Provider**: add a variant to `ProviderVariant` + config struct in `src/config/provider.rs`, resolve credentials in `provider_keys.rs`, build the client in `src/agents/agent/model/`.
- **Storage backend**: implement every `*Storage` trait and `VizierStorageProvider`.

Conventions: `tracing` for logs (never `println!`), `crate::Result` / `VizierError`, conventional commits (`feat:`, `fix:`, …; changelog by `git-cliff`).

## License

MIT
