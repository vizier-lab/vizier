# 1.2 Quick Start

## 1. (Optional) Pick a model provider

Vizier needs at least one LLM provider. If you have an API key for OpenAI, Anthropic, OpenRouter, etc., you can add it in the WebUI after startup and skip this section.

For fully local models, install Ollama:

**macOS**
```sh
brew install ollama
ollama serve
ollama pull qwen3
```

**Linux**
```sh
curl -fsSL https://ollama.com/install.sh | sh
ollama serve
```

**Windows** — download from [ollama.com](https://ollama.com/download/windows).

The built-in defaults already include `ollama` (`http://localhost:11434`) and `llama_cpp` (`http://localhost:8080`) providers, so a local setup needs no configuration at all.

## 2. (Optional) Generate a seed config

```sh
vizier onboard
```

The wizard asks for:

- Workspace path (where `.vizier.yaml` and the `.vizier/` data directory are created)
- HTTP port (default `9999`) and JWT secret (a random one is generated)
- One primary provider and its API key / base URL
- Storage type — choose **SQLite** (`Filesystem` is legacy and is migrated into SQLite on first run)
- Worker threads and WebSocket idle timeout

Skip this step entirely if you're happy with defaults: with no config file, Vizier uses `$VIZIER_DATA_DIR` or `$HOME/.vizier` as its workspace and SQLite storage. See [Config-less mode](../configuration/index.md#config-less-mode).

## 3. Run

```sh
vizier run
```

Resolution order for the config file: `-c <path>` → `$VIZIER_CONFIG` → `./.vizier.yaml` → built-in defaults.

Add `-d` to daemonize; stop it later with `vizier shutdown`.

## 4. Create the first user

Open `http://localhost:9999`. On a fresh install the WebUI redirects to an onboarding page that creates the first user (this user gets the `superadmin` system role). After that, log in normally.

The same thing via API: `GET /api/v1/auth/setup-status` → `POST /api/v1/auth/setup` with `{ "username", "password" }`.

## 5. Add a provider key

**Settings → Providers** — paste an API key for any provider (or leave `ollama` as-is). Providers from `.vizier.yaml` are already there.

## 6. Create your first agent

**Agents → New Agent**: pick a name, provider, model, and enable whatever tools you want (shell, web fetch, Brave search, MCP servers…). Save and start chatting.

Each agent has:

- **Core** — its persistent `CORE.md` identity/operating document (editable in the WebUI)
- **Memory** — a browsable knowledge graph of markdown concept documents
- **Tasks** — scheduled cron / one-time tasks
- **Skills** — per-agent skill packages
- **Dream** — an optional scheduled reflection cycle
- **Usage** — token usage stats

See [Agent Configuration](../configuration/agents.md).

## Development quick start

```sh
git clone https://github.com/vizier-lab/vizier && cd vizier
just install     # cargo fetch + npm install in webui/
just dev         # cargo watch -s "just run" (hot reload, uses dev.vizier.yaml)
```

| Command | Description |
|---------|-------------|
| `just install` | Install Rust crates + WebUI npm packages (also installs `cargo-watch`) |
| `just dev` | Run with hot-reload (`cargo watch`) |
| `just run` | `cargo run -- run --config dev.vizier.yaml` |
| `just run-d` | Same, detached |
| `just shutdown` | Stop the dev instance |
| `just build` | Build the WebUI (`npm run build`) |
| `just release` | `cargo build --release` |
| `just docker` | `docker-compose down && docker-compose up -d` |

There is no `just test`/`just lint` — use `cargo test` and `cargo clippy` directly. WebUI typecheck: `cd webui && npm run typecheck`.

## Next Steps

- [Configuration](../configuration/index.md)
- [REST API](../api-integration/rest-api.md)
