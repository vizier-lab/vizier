# AGENTS.md — Vizier

Rust AI agent framework. Multi-channel (Discord, Telegram, HTTP, WebUI).
Single binary, embedded SurrealDB (kv-rocksdb), async tokio runtime.

## Commands

```sh
just install          # cargo fetch + npm i in webui/
just dev              # cargo watch -s "just run"
just run              # cargo run -- run --config dev.vizier.yaml
just run-a            # cargo run -- run -a --config dev.vizier.yaml (attached mode)
just shutdown         # cargo run -- shutdown --config dev.vizier.yaml
just build            # cd webui && npm run build
just release          # cargo build --release
just docker           # docker-compose down && docker-compose up -d

cargo test            # run tests
cargo clippy          # lint (CI expectation: zero warnings)
```

**No `just test` or `just lint` targets exist.** Use `cargo test` and
`cargo clippy` directly.

## CLI subcommands

| Subcommand | Flags | Description |
|------------|-------|-------------|
| `run` | `-c/--config <PATH>`, `-a/--attached` | Run agents, server, and channels. Without `-a`, daemonizes (PID at `/tmp/vizier.pid`, logs to `.vizier/.runtime/logs/`). |
| `shutdown` | `-c/--config <PATH>` | Stop a running daemonized instance. |
| `onboard` | `-p/--path <PATH>` | Interactive wizard to generate `.vizier.yaml` seed config. |
| `skill` | `install`, `list`, `uninstall`, `update` | Manage skills — install from registry/git/local, list, uninstall, update. |

There is no `init`, `configure`, or `agent` subcommand. Agents are
created and managed at runtime via the WebUI or HTTP API.

## Build gotcha: `build.rs` auto-builds WebUI

`build.rs` runs `npm run build` in `webui/` during `cargo build` if
`webui/node_modules/` exists. If `node_modules/` is missing AND
`webui/build/client/` doesn't exist, cargo build **panics**.

Fix: run `just install` first, or ensure pre-built files exist in
`webui/build/client/`.

## Project structure

```
src/
├── main.rs              # entrypoint, log init, cli::start()
├── cli/                 # clap CLI: run, shutdown, onboard, skill
├── command/             # command handling
├── config/              # VizierConfig deserialized from YAML (seed config)
├── schema/              # shared types (VizierResponse, AgentId, ProviderEntry, etc.)
├── agents/
│   ├── agent/           # agent process loop, LLM interaction
│   │   ├── model/       # provider abstraction (ollama, openai, anthropic, gemini, deepseek, openrouter, mimo, llama_cpp)
│   │   └── system_prompt/ # system prompt construction, workspace init
│   ├── tools/           # all built-in tools (register here for new tools)
│   ├── hook/            # agent lifecycle hooks (debug, thinking, history, tool_calls)
│   ├── mcp/             # MCP client + server integration (rmcp crate)
│   ├── shell/           # shell execution abstraction
│   │   ├── local/       # local shell
│   │   └── docker/      # docker shell (bollard)
│   └── skill/           # reusable agent behaviors
├── channels/
│   ├── discord/         # serenity-based
│   ├── telegram/        # teloxide-based
│   └── http/            # axum REST + WebSocket + static WebUI serving
│       ├── api/v1/      # agents, providers, files, skills endpoints
│       ├── auth/        # JWT authentication, middleware
│       └── webui/       # serves built WebUI static files
├── storage/
│   ├── fs/              # filesystem storage backend
│   ├── surreal/         # SurrealDB storage backend
│   ├── indexer/         # document indexing (in-mem, surreal)
│   └── *.rs             # storage traits (MemoryStorage, TaskStorage, AgentStorage, ProviderStorage, etc.)
├── embedding/           # fastembed local embeddings
├── transport/           # command transport (agent, channel, global commands)
└── utils/               # utility functions
webui/                   # React Router v7 + Tailwind v4 + TypeScript
templates/               # agent.md / IDENTITY.md templates (include_str!'d)
skills/                  # seed skills (calculator, code-review, designer)
```

## Runtime config architecture

`.vizier.yaml` is **seed config** — it provides initial values that are
auto-migrated to storage on first run. After migration, most config is
managed at runtime via WebUI or HTTP API.

| Config | File-based (seed) | Runtime (WebUI/API) | Migration |
|--------|-------------------|---------------------|-----------|
| `embedding` | YES (active) | No | — |
| `storage` | YES (active) | No | — |
| `channels.http` | YES (active) | No | — |
| `providers` | YES (seed) | YES — `/api/v1/providers` | Auto-migrate to providers table |
| `agents` | No | YES — `/api/v1/agents` | Created via API, stored in DB |
| `tools.brave_search` | YES (still there) | Per-agent setting via agents API | Not auto-migrated |

## Key patterns

**Error handling**: `VizierError(pub String)` in `src/error.rs`.
Project-wide type alias: `crate::Result<T> = Result<T, VizierError>`.
Use `throw_vizier_error()` for converting std errors. Avoid `unwrap()`
in library code — use `?` or explicit handling.

**Adding a new tool**:
1. Create module in `src/agents/tools/<name>.rs`
2. Implement `VizierTool` trait (associated types `Input`/`Output` with
   `JsonSchema + Deserialize + Serialize`, plus `name()`, `description()`,
   `call()`)
3. Add `mod <name>;` to `src/agents/tools/mod.rs`
4. Register in `VizierTools::new()` in the same file via `.tool(YourTool)`

**Adding a new channel**: Implement `VizierChannel` trait (`async fn run`)
in `src/channels/<name>/`, register spawn in `VizierChannels::run()`.

**Adding a storage backend**: Implement all traits composed by
`VizierStorageProvider` (MemoryStorage, TaskStorage, HistoryStorage,
SkillStorage, SessionStorage, StateStorage, DocumentIndexer, UserStorage,
), then implement `VizierStorageProvider` for it.

**Per-agent MCP/Shell**: Each agent owns its own MCP server configs
and shell config in `AgentToolsConfig`. When an agent starts,
`VizierTools::new()` creates MCP clients and shell instances directly
from the agent's config. No global MCP/shell singletons exist.

**Agent lifecycle**: `AgentCommand::Create/Update/Delete` flows from
HTTP API → `VizierTransport` → `VizierAgents` manager → aborts old
process, spawns new one → sends `ChannelCommand` to `VizierChannels`
for channel reconciliation.

## Config

- Seed config file: `.vizier.yaml` (YAML, top-level key `vizier:`)
- Supports `${ENV_VAR}` expansion via `shellexpand`
- Dev config: `dev.vizier.yaml` (committed, has real keys — don't mirror)
- `.vizier.yaml` uses `${VIZIER_JWT_SECRET}` placeholder
- Agents are managed at runtime via WebUI or API (no more `.agent.md` files)

## Logging

Uses `log` crate + `pretty_env_logger`. Default filters noisy crates
(rig, serenity, reqwest, hyper, surrealdb, etc.) to Error/Off.
Use `log::info!()`, `log::error!()`, etc. — never `println!` for
operational output.

## Conventional commits

Changelog generated by `git-cliff` (config in `cliff.toml`).
Prefixes: `feat:`, `fix:`, `doc:`, `perf:`, `refactor:`, `chore:`.
Breaking changes flagged with `[**breaking**]` in changelog.

## Release

Manual via GitHub Actions `workflow_dispatch` — input a version string.
Bundles cross-compiled binaries (linux x86_64 musl, aarch64 gnu).
Publishes to crates.io. Version lives in `Cargo.toml`.

## WebUI

React Router v7, React 19, TypeScript, Tailwind CSS v4, Zustand for state.
Motion (framer-motion successor) for animations, recharts for charts,
highlight.js for syntax highlighting, MDX editor for markdown editing.
Typecheck: `cd webui && npx react-router typegen && npx tsc` (or
`npm run typecheck`).
Built output goes to `webui/build/client/` — served by axum at runtime.

## What NOT to do

- Don't restructure working modules without explicit instruction
- Don't add `println!` — use `log` macros
- Don't `unwrap()` outside tests or main bootstrap
- Don't assume external services — SurrealDB is embedded (RocksDB),
  embeddings are local (fastembed)
- Don't modify `build.rs` without understanding the webui build chain
- Don't ignore the `cargo:rerun-if-changed` directives — they exist
  to avoid unnecessary rebuilds
