# Vizier

> **A digital steward for Your Majesty.**\
> AI agents that remember who they are, and who you are. Hosted by you.

Vizier is a Rust-based AI agent framework: a single binary that runs multiple concurrent AI agents — each with its own provider, tools, memory, and identity — exposed over Discord, Telegram, and HTTP (REST + WebSocket + a bundled React WebUI). Storage is embedded SQLite; no external services required.

🌐 **Website:** https://www.vizier.rs — landing page, blog and documentation.
📖 **Docs source:** `docs/src/` — rendered by the `landing/` site at https://www.vizier.rs/docs.

## Features

- **Multi-channel** — Discord, Telegram, REST, WebSocket, WebUI. Each agent gets its own bot token per platform; slash commands for sessions, checkpoints, and aborts.
- **29 providers** — Ollama, llama.cpp, OpenAI, Anthropic, Gemini, DeepSeek, OpenRouter, Xiaomi MiMo, Groq, Mistral, xAI, Perplexity, Moonshot, Z.ai, MiniMax, Together, Cohere, Hugging Face, Hyperbolic, Voyage AI, Galadriel, Mira, ChatGPT (OAuth), GitHub Copilot, Azure OpenAI, OpenCode Zen/Go, and any OpenAI-compatible `custom` endpoint.
- **Open-format memory** — long-term memory is plain markdown on disk, organized into per-agent bundles with a link graph (`[label](concept.md)`, `[[bundle/slug]]`), semantic search via a per-agent embedding model (local fastembed or cloud), and `.zip` export/import.
- **Tools** — shell (local or Docker sandbox), Brave web/news search, web fetch, HTTP client, cron & one-time scheduler, memory graph tools, skills, session files (PDF/DOCX/XLSX/images), TTS / STT / image generation, parallel sub-tasks, inter-agent consult/delegate, per-agent MCP servers.
- **Python sandbox & code mode** — opt-in `execute_python` running an in-process, isolated Python interpreter (no files, network or OS; bounded by the agent's tool timeout). With **code mode** on, the agent's tools become functions inside scripts and are hidden from the model, so one script can loop over many tool calls and only its final result enters the context. Deferred: memory ceiling, tool-call cap, output truncation.
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
just install     # submodules, npm install in design/ and webui/, cargo fetch (+ cargo-watch)
just dev         # cargo watch -s "just run" — hot reload with dev.vizier.yaml
just run         # cargo run -- run --config dev.vizier.yaml
just run-d       # same, detached
just shutdown    # stop the dev instance
just build       # build the WebUI (npm run build)
just landing     # the website (landing, blog, docs) on http://localhost:4321
just landing-build  # build the website
just release     # cargo build --release
just docker      # docker-compose down && up -d
just jaeger      # local Jaeger for traces, UI at http://localhost:16686
just run-otel    # `just run` exporting traces to it (also dev-otel; jaeger-down to stop)

cargo test       # tests are sparse; most behavior is exercised by running the binary
cargo clippy
cd webui && npm run typecheck
cd webui && npm run dev   # the WebUI on http://localhost:5173, proxying /api to a running `just run`
```

### Try the dummyplug integration locally

This branch includes the standalone Node.js dummyplug server as a submodule. To try the branch
from a clean checkout, install Rust, Node.js 22+, and `just`, then run:

```sh
git clone --recurse-submodules --branch 030-standalone-dummyplug-server \
  https://github.com/vizier-lab/vizier.git
cd vizier
just install
```

Start dummyplug and Vizier in separate terminals from the repository root:

```sh
just dummyplug  # OpenAI-compatible test server on 127.0.0.1:9998
```

```sh
just run        # Vizier API and WebUI on http://localhost:9999
```

On a fresh database, open the Vizier URL and create the first account. Create an agent with the
**dummyplug (testing)** provider and any model name. The default dummyplug URL is
`http://127.0.0.1:9998/v1`; no model API key is needed. Send `tools` to list the agent's tools,
or send a tool name such as `read_core` to see its arguments and an example request.

Run the standalone server tests and Vizier's test-harness tests manually with:

```sh
just dummyplug-test
just test-scripts
```

Tracing and metrics are exported over OpenTelemetry when `OTEL_EXPORTER_OTLP_ENDPOINT` is set; see [Observability](docs/src/configuration/observability.md).

> **Build gotcha:** the WebUI (`webui/`), its component library (`design/`) and the website (`landing/`) are git submodules. Clone with `--recurse-submodules`, or run `git submodule update --init --recursive`. `build.rs` runs `npm run build` in `webui/` on every `cargo build` if `webui/node_modules/` exists; if `node_modules/` is absent and `webui/build/client/` doesn't exist, the build panics — run `just install` first. `landing/` is not part of the binary build. `legacy/` holds the previous WebUI for reference; it is never built, served or packaged.

### Design library, WebUI and landing submodules

`design/` is a git submodule: [vizier-lab/design](https://github.com/vizier-lab/design), the UI component library with a Storybook catalogue that is the source of truth for the design. `webui/` is a submodule too: [vizier-lab/webui](https://github.com/vizier-lab/webui), the WebUI, built from that library (`"@vizier/design": "file:../design"`). `landing/` is [vizier-lab/landing](https://github.com/vizier-lab/landing), the website (Astro + Starlight). It reads `docs/` and links `design/` from this checkout, so docs and design stay in one place.

```sh
git clone --recurse-submodules https://github.com/vizier-lab/vizier.git   # new clone
git submodule update --init --recursive                                    # existing clone

cd design && npm install && npm run storybook   # catalogue on http://localhost:6006
just landing                                   # website on http://localhost:4321
```

To move vizier to a newer library commit, bump the pin like any other change:

```sh
cd design && git pull origin main && cd ..
git add design && git commit -m "chore: bump design library"
```

### Project structure

```
src/
  cli/              run / shutdown / onboard / skill / agent subcommands
  config/           .vizier.yaml schema (providers, storage, http channel, shell & MCP types)
  dependencies.rs   VizierDependencies: opens storage, runs one-time migrations
  transport.rs      in-process message bus (flume) tying agents, channels, scheduler together
  agents/           per-agent process loop, model/provider abstraction, tools, hooks, shell (local/docker), MCP, skills runtime
  channels/         discord (twilight), telegram (teloxide), http (axum: REST, WS, JWT/API-key auth, WebUI static)
  scheduler/        cron + one-time tasks, dream cycle
  sandbox/          sandboxed Python engine (monty): run loop, JSON⇄Python values, tool docs
  storage/          VizierStorage over SQLite; document store + BundleMemoryStore for markdown memory
  indexer/          sqlite-vec vector index
  embedding/ tts/ stt/ image_generation/   per-provider adapters
  skill/            skill packages: manifest, install (registry/git/local)
  schema/           shared types (agent config, requests/responses, sessions, history)
  command/          Unix-socket command server (shutdown, health) for the CLI
webui/              submodule: vizier-lab/webui (React Router v7 + React 19 + Tailwind v4); built output served by the HTTP channel
design/             submodule: vizier-lab/design, the UI library + Storybook catalogue
landing/            submodule: vizier-lab/landing, the website (landing page, blog, docs)
legacy/             the previous WebUI, kept for reference; never built or served
templates/          CORE.md seed for new agents
skills/             skill registry (installed with `vizier skill install <slug>`)
vizier-derive/      #[derive(MarkdownDoc)] for frontmatter+body documents
docs/               documentation source, rendered by landing/ as www.vizier.rs/docs
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
