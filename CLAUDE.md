# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Vizier is a Rust-based AI agent framework: a single binary that runs multiple concurrent AI agents, each with its own tools, memory, and provider, exposed over Discord, Telegram, and HTTP (REST + WebSocket + a bundled React WebUI). Storage is embedded SQLite — no external DB service required. Agent memory itself lives as human-readable markdown documents on disk (organized into per-agent **bundles**, see below), read/written through a pluggable `DocumentStore` abstraction; every other entity lives in SQLite.

## Commands

```sh
just install     # cargo fetch + npm i in webui/
just dev          # cargo watch -s "just run" (hot-reload)
just run          # cargo run -- run --config dev.vizier.yaml
just run-d        # same, detached (-d)
just shutdown     # cargo run -- shutdown --config dev.vizier.yaml
just build        # cd webui && npm run build
just release      # cargo build --release
just docker       # docker-compose down && docker-compose up -d

cargo test        # run tests (very few exist today — see below)
cargo clippy       # lint
```

There is no `just test` or `just lint` target — use `cargo test` / `cargo clippy` directly. Tests are sparse (`src/agents/agent/model/registry.rs`, `src/storage/memory.rs`, `src/skill/context.rs`); most correctness is exercised by running the binary, not by a test suite. For end-to-end checks, run the binary with an agent on the offline `dummyplug` provider (no keys needed): send `tools`, then a tool name for a sample request, then the JSON request to run the tool through the real loop — see `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`. The constitution requires this for any agent-observable change.

WebUI typecheck: `cd webui && npm run typecheck` (runs `react-router typegen && tsc`).

### Build gotcha: `build.rs` runs the WebUI build

`build.rs` shells out to `npm run build` in `webui/` on every `cargo build` **if** `webui/node_modules/` exists. If `node_modules/` is absent and `webui/build/client/` doesn't already exist, the build panics. Run `just install` first, or make sure `webui/build/client/` is pre-populated (this is how crates.io publishes work — see `include` in `Cargo.toml`).

## CLI subcommands

- `vizier run [-c <path>] [-d] [--port] [--workspace/--data-dir] [--storage sqlite] [--workers] [--ws-idle-timeout]` — start agents, scheduler, channels, and the command server. Works with **no config file** (config-less mode, see below). `sqlite` is the only supported `--storage` value; a deployment still configured with the legacy `filesystem` backend is migrated into sqlite automatically on first startup after upgrading (see `dependencies.rs`'s migrations below) — `--storage filesystem`/`VIZIER_STORAGE=filesystem` on a *new* invocation is rejected outright.
- `vizier shutdown [-c <path>]`
- `vizier onboard -p <path>` — interactive wizard that writes a seed `.vizier.yaml`.
- `vizier skill install|list|uninstall|update`
- `vizier agent ps` — list running agents. Agents themselves are created/managed at runtime via the WebUI or HTTP API — there's no `agent create` CLI.

### Config-less mode

Resolution order for the config file: explicit `-c` → `$VIZIER_CONFIG` → `./.vizier.yaml` → built-in defaults. In the no-file path, the workspace resolves to `$VIZIER_DATA_DIR` or `$HOME/.vizier`, and storage defaults to `sqlite`.

## Architecture

### Process startup (`src/cli/run.rs::run_server`)

A single tokio multi-thread runtime spawns five long-lived tasks off one shared `VizierDependencies`: `VizierScheduler`, `VizierChannels`, `VizierAgents`, `VizierCommandServer`, and the deps' own file-manager runner. They all communicate through `VizierTransport` (`src/transport.rs`), an in-process message bus built on `flume` channels — per-agent request channels, a memory-ops channel, agent lifecycle commands, generic command request/response, dream commands, and file commands. There's no shared mutable state beyond what's routed through this transport plus the shared `VizierStorage`.

### `VizierDependencies` (`src/dependencies.rs`)

Constructed once at startup: opens storage (sqlite connection or filesystem root), then runs one-time migrations (seed users → superadmin role, YAML providers → provider storage, per-agent MCP/shell config backfill, default CORE.md backfill for agents missing one). Cloned cheaply (`Arc` internals) into every subsystem.

### Agents (`src/agents/`)

- `VizierAgents` (`src/agents/mod.rs`) is the manager: on boot it loads every persisted agent config from storage and spawns one `agent_process` task per agent; afterwards it just services `AgentCommand::Create/Update/Delete/HealthCheck` from the transport, each doing storage write → old-process shutdown+unregister → respawn.
- `VizierAgent` (`src/agents/agent/mod.rs`) is the actual per-agent loop: builds its `VizierModel` (provider abstraction), `VizierTools`, `VizierSkills`, optional STT/TTS/image-gen, loads its CORE.md and owner profile, and runs the LLM request/response/tool-call cycle (`agents/process.rs` drives it against `VizierTransport`'s per-agent channel).
- Each agent owns its **own** MCP client set and shell instance, built fresh from its `AgentToolsConfig` in `VizierTools::new()`. There are no global MCP/shell singletons — this is deliberate (per-agent isolation).
- `hook/` — lifecycle hooks (debug logging, thinking-block handling, tool-call handling, handover).
- `shell/` — local and Docker (`bollard`) shell execution backends behind one abstraction.
- `skill/` — reusable agent behaviors, separate from the top-level `skill/` module (skill *packages*, install/registry).

### Tools (`src/agents/tools/mod.rs`)

Tools implement the `VizierTool` trait: associated `Input`/`Output` types (both `JsonSchema + Serialize + Deserialize`), `name()`, `description()`, `call()`.

**Memory tool names** (`tools/vector_memory/`): `memory_search` searches and returns addressed **passages** (it was `memory_read`), and `memory_read` returns one whole document by `(bundle, path)` (it was `memory_detail`; `memory_detail` is retired, not aliased). The two renames are a breaking change and landed together, because tool names are the dispatch key. The other six `memory_*` tools are untouched. A blanket impl turns any `VizierTool` into the dynamic `VizierToolDyn` used for dispatch. Two toolsets exist per agent — `default_toolset` (always-on: memory, workspace CORE read/write, scheduler, skills, subtasks, session files, consult/delegate other agents) and `user_toolset` (conditionally added per agent config: brave search, fetch, http client, TTS/STT/image-gen, webui messaging). MCP tools are dispatched separately, keyed by `mcp_<server>__<tool>`.

**Python sandbox / code mode** (`specs/007-code-mode-python-sandbox/`): dispatch over the three regular sets lives in `ToolRouter` (`VizierTools.router`), and `VizierTools` adds a `sandbox_toolset` plus a `ToolExposure` (`Direct` / `SandboxAdditive` / `CodeModeExclusive`, from `tools.python`) consulted only in `tools()`/`call()`. The tools are in `src/agents/tools/python/` (`execute_python`, `list_tool_functions`, `describe_tool_function`, plus `RouterBridge`, which runs script-made tool calls through `ToolRouter` + `ToolContext.hooks` + the per-tool timeout). The engine is `src/sandbox/`, which is agent-agnostic: a monty resume loop on `spawn_blocking`, the `SandboxBridge` trait, JSON⇄`MontyObject` conversion, and `ToolFunctionDoc` built from `ToolDefinition`. Script failures come back as an `ExecutionReport` (data), never `Err`. The sandbox tools are never in `DREAM_TOOL_NAMES`; `dream_tools`/`dream_call` bypass the exposure gating.

**Adding a new tool**: create `src/agents/tools/<name>.rs` implementing `VizierTool`, add `mod <name>;`, then `.tool(YourTool)` it onto `default_toolset` or `user_toolset` inside `VizierTools::new()`. If it should be available to the dream cycle, add its name to `VizierTools::DREAM_TOOL_NAMES`.

### Storage (`src/storage/`)

`VizierStorageProvider` is a supertrait composing every storage concern (`MemoryStorage`, `TaskStorage`, `HistoryStorage`, `SessionStorage`, `StateStorage`, `UserStorage`, `AgentStorage`, `ProviderStorage`, `GlobalConfigStorage`, `DreamJournalStorage`, `DreamStorage`, `SessionFileStorage`). `VizierStorage` type-erases the concrete backend (`storage/sqlite` — the sole `VizierStorageProvider` implementation) behind `Arc<Box<dyn VizierStorageProvider>>` and hand-forwards every trait method. **Adding a storage backend** means implementing every one of those traits for the new type, then `impl VizierStorageProvider for it`. The old `storage/fs` (`FileSystemStorage`) backend has been removed as a runtime option; its non-memory trait impls survive only as a read source for the one-time `migrate_filesystem_backend_to_sqlite` startup migration in `dependencies.rs`, for deployments upgrading from a pre-existing `--storage filesystem` install.

`MemoryStorage` is the one exception to "storage backend owns the bytes": memory concept documents are markdown files (YAML frontmatter + body) addressed by `(agent_id, bundle, path)`, read/written through the pluggable `storage::document::DocumentStore` trait (default: `LocalDocumentStore`, rooted at `{workspace}/agents/{agent_id}/memory/...`), with `storage::memory_bundle::BundleMemoryStore` as the single implementation of bundle/concept logic (link parsing, `index.md`/`log.md` maintenance, bundle export/import as `.zip`) and the sole `impl MemoryStorage for SqliteStorage`. SQLite still caches a derived, reconcilable **Memory Graph Index** (`memory_node`/`memory_edge` tables) so listing/graph/related-memory queries never need to read the documents themselves — only `memory_detail`, semantic search, and export ever call into `DocumentStore`. See `specs/004-memory-open-format/` for the full design.

**Passage-level retrieval** (`specs/009-memory-semantic-chunking/`): queries match **passages**, not whole documents. `storage/chunk.rs` splits a document body into structure-aware passages — `scan_blocks` (markdown blocks) then `pack_blocks` (size-packing under `ChunkLimits`, default 1200/400/2400 bytes); both are pure, sync and unit-tested, and `chunk_markdown` consults no embedder (the embedding-seam path was measured out, research Decision 1b). A third derived table, `memory_passage`, holds each passage's coordinates (ordinal, line span, byte span, `continues`, `content_hash`) — **never its text**, which is sliced from the document at query time. The `document_index` vec0 table keeps its DDL and changes only row granularity: one row per passage, keyed `{agent_id}/{bundle}/{path}#{ordinal}`. Absence of `memory_passage` rows for a document is the only marker that it is unconverted, which is what makes the per-agent conversion task resumable; drift is detected by comparing `content_hash`, not by re-chunking. `write_memory`/`delete_memory`/`delete_bundle`/`import_bundle` keep passages in step, and deletion is driven by *stored* ordinals so a shrinking document cannot orphan vectors. Passages are indexed in one batched `DocumentIndexer::add_document_indexes` call per save, with title/tags/heading-breadcrumb prefixed to each passage's index text (never to its returned text). Chunking or indexing failure is logged and never fails the save.

The one-time conversion of an existing corpus runs **per agent in `VizierAgents::spawn_agent`**, not in `dependencies.rs` — the indexer is built per agent from that agent's embedding config and does not exist where the other migrations run.

**Version history** (`specs/006-memory-version-history/`): CORE history lives in the `core_revision` table behind `AgentStorage` (`set_agent_core` upserts `agent_core` and records the revision in one transaction — `agent_core` is the sole home of CORE; the legacy `AgentConfig.core` field is migrated out on startup), memory history in `memory_revision` behind `MemoryStorage` (recorded inside `BundleMemoryStore::{write_memory, delete_memory, delete_bundle, import_bundle}`). Both are recorded *inside* the write path, so every caller only supplies a `RevisionOrigin` (who/why); the two kinds are deliberately separate implementations sharing only `schema/revision.rs` value types and `storage/diff.rs`. Direct on-disk edits to memory files are not versioned.

### Channels (`src/channels/`)

`VizierChannel` trait: `async fn run(&self)`. Implementations: `discord/` (twilight), `telegram/` (teloxide), `http/` (axum — REST under `api/v1/`, WebSocket, JWT auth in `auth/`, and it also serves the built WebUI static files from `webui/build/client/`). `VizierChannels::run()` is where new channel spawns get registered.

### Providers / models (`src/agents/agent/model/`)

Provider abstraction (`VizierModel`) over the many backends declared in `config/provider.rs` (ollama, openai, anthropic, deepseek, openrouter, gemini, mimo, llama_cpp, elevenlabs, and a growing long tail — groq, mistral, xai, perplexity, moonshot, zai, minimax, together, cohere, huggingface, hyperbolic, voyageai, galadriel, mira, chatgpt, copilot, azure, opencode, custom), plus `dummyplug` — an offline test provider (`model/dummyplug.rs`) that needs no keys and lets you list/sample/run tools by hand. Runs on `rig-core`.

### Scheduler (`src/scheduler/`)

Cron (`croner`) and one-time task execution, plus `scheduler/dream/` — a separate periodic "dream" cycle per agent (see `dream_interval` in agent config) that runs a restricted tool subset (`VizierTools::DREAM_TOOL_NAMES`) for unattended reflection/journaling.

### Config layering

- `.vizier.yaml` (top-level key `vizier:`) is **seed config**, loaded once via `VizierConfig::load`. Supports `${ENV_VAR}` expansion (`shellexpand`). `dev.vizier.yaml` is the local dev config (already has working keys — don't treat it as a template to copy secrets from).
- On first run, seed `providers` are migrated into provider storage (`dependencies.rs::migrate_providers`) and become runtime-editable via `/api/v1/providers`. Agents are **never** defined in YAML — they're created/updated only through the WebUI/HTTP API and persisted to storage.
- Per-agent tool config includes `tools.python.{enabled, code_mode}` (both default `false`; `code_mode` requires `enabled`, validated in `VizierAgents` on create/update → 400).
- `AgentConfig` also carries `chunking` (`ChunkLimits`: `target_size`/`min_size`/`max_size`) and `auto_context` (`chat_passages` default 5, `silent_read_passages` default **0**, `threshold` default 0.6, `size_cap`, `per_document`). `SilentRead` fires for every non-mention Discord guild message and every Telegram group message, so its budget is separate and off by default — its cost would otherwise scale with channel traffic. The automatic-context threshold is provisional and meant to be derived by replaying stored session history.
- CLI flags (`--port`, `--workspace`/`--data-dir`, `--storage`, `--workers`, `--ws-idle-timeout`) override whatever the config file loaded, applied via `VizierConfig::apply_overrides`.
- Docker env vars (`VIZIER_CONFIG`, `VIZIER_DATA_DIR`/`VIZIER_WORKSPACE`, `VIZIER_PORT`, `VIZIER_STORAGE`, `VIZIER_WORKERS`, `VIZIER_WS_IDLE_TIMEOUT`, `VIZIER_JWT_SECRET`, `VIZIER_EXTRA_ARGS`) are translated to CLI flags by `docker-entrypoint.sh`, which then `exec`s the binary so signals propagate.

### Agent identity: CORE.md, not `.agent.md` files

Each agent's persistent "self" document is `CORE.md`, stored in storage (not the filesystem) and seeded from `templates/CORE.md` (`constant::CORE_MD`) on agent creation, with a startup backfill migration for any agent missing one. `templates/agent.template.md` / `vizier.agent.md` at the repo root are a legacy/reference identity template format — current agents read/write CORE via the `READ_CORE`/`WRITE_CORE` tools against storage, not a file on disk.

## Key conventions

- **Errors**: `VizierError(pub String)` (`src/error.rs`); the crate-wide alias is `crate::Result<T> = Result<T, VizierError>`. Convert external errors with `throw_vizier_error(prefix, err)`. Avoid `unwrap()`/`expect()` outside tests and `main`'s bootstrap.
- **Logging**: `tracing` only (never `println!`) — `tracing::info!`, `warn!`, `error!`, etc. `main.rs` sets per-crate directive overrides (rig, twilight, sqlite, reqwest, hyper, bollard, rmcp, etc. quieted to `error`/`off` by default unless `RUST_LOG` is set).
- **Extensibility is trait-based**: new tools/channels/storage backends/providers get added by implementing the relevant trait and registering the impl in its module constructor — not by branching on type inside existing dispatch code (see `.specify/memory/constitution.md` for the fuller rationale if present).
- **Conventional commits**: `feat:`, `fix:`, `doc:`, `perf:`, `refactor:`, `chore:` — changelog is generated by `git-cliff` (`cliff.toml`). Breaking changes flagged `[**breaking**]`.
- **Per-agent isolation**: MCP servers and shell config live on `AgentToolsConfig` per agent, not globally — don't reintroduce a global MCP/shell singleton.

## WebUI (`webui/`)

React Router v7 + React 19 + TypeScript + Tailwind v4. State via Zustand-style stores in `app/hooks/*Store.tsx`. Recharts for charts, highlight.js for syntax highlighting, MDX editor for markdown. Build output (`webui/build/client/`) is what the Rust HTTP channel serves at runtime — see the `build.rs` gotcha above for why you generally need `npm install` done before `cargo build` will succeed.

<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/011-task-completion-reports/plan.md`
<!-- SPECKIT END -->
