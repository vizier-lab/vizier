# Implementation Plan: Projects Mode Foundation

**Branch**: `040-projects-mode` | **Date**: 2026-10-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/040-projects-mode/spec.md`

## Summary

A **project** is a host folder mounted to exactly one agent. It has a name, an
owner, a workspace-shell switch, a soft-removed state, and personal sessions.
Project sessions are ordinary conversations addressed through a
project-scoped HTTP channel (`project__<project_id>`), so the existing session,
history, notification, usage and WebSocket machinery is reused unchanged
instead of adding a new session kind.

The foundation adds:

1. one additive SQLite table (`project`) with a `ProjectStorage` concern;
2. the `all_agents:projects` / `owned_agents:projects` permissions;
3. a per-turn **workspace shell** tool available only in project sessions,
   with exit status, stderr, duration, timeout + process-tree kill, capped
   output and a project-rooted working directory;
4. a project section in the system prompt with the folder and bounded
   `AGENTS.md` / `CLAUDE.md` instructions;
5. project HTTP routes, desktop-only creation with a trust disclosure, the
   WebUI Projects section above Sessions, and a host-execution indicator for
   browser users.

Git integration, diff review, a file editor, per-session worktrees, a terminal
UI and sandboxing remain out of scope (FR-030).

## Technical Context

**Language/Version**: Rust (workspace edition 2024), TypeScript + React Router 7 for the WebUI, Tauri 2 for the desktop shell.

**Primary Dependencies**: `axum`, `tokio` (add the `process` feature), `rusqlite` (bundled SQLite), `serde`/`schemars`/`utoipa`, `nanoid`, `tracing`, `parking_lot`; WebUI React + Zustand; `tauri-plugin-dialog` (already installed). `libc` is already in `Cargo.lock` transitively and is declared as a Unix-only direct dependency for process-group signalling. No new crates.

**Storage**: Embedded SQLite at `{workspace}/.runtime/vizier.db` (additive `project` table, `CREATE TABLE IF NOT EXISTS`) plus the host filesystem for the project folder itself. No external service, no new runtime dependency.

**Testing**: `cargo test` and `cargo clippy` for the Rust workspace, `cd app && cargo test` for the Tauri shell, `cd webui && npm run typecheck && npm test` for the WebUI; end-to-end via the dummyplug agent steps in [quickstart.md](./quickstart.md), per the constitution's e2e gate. Project creation itself is exercised through the embedded desktop `Dispatcher` in the app test suite because creation is desktop-only.

**Target Platform**: Vizier desktop app on macOS, Windows and Linux (the project host), with the opt-in browser WebUI served by the same process. The server-only install is not a project host.

**Project Type**: Single Rust binary + embedded WebUI + Tauri desktop shell (existing monorepo layout).

**Performance Goals**: A desktop user opens a folder, starts a session and receives a first shell result in under 2 minutes (SC-001); two sessions of one project run commands concurrently and are never serialised (FR-027 / SC-008); shell output and command duration are reported without blocking the agent runtime.

**Constraints**: Commands run as the host operating-system account with no sandbox (FR-002); the default command limit is 120 s (max 600 s per call), output is capped at 64 KiB per stream, and the project instruction document is capped at 32 KiB. Project creation requires a desktop principal (FR-021). Project folders that are the application's own workspace (or inside it) are refused (FR-028). UI changes must respect the existing design system and pass WebUI typecheck/tests.

**Scale/Scope**: Dozens of projects per agent and dozens of personal sessions per project; several sessions may run commands at once. No cross-agent sharing of a project; the same folder opened from another agent is a separate project (FR-003).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. Lean by default | The smallest change that satisfies the spec: one table, one storage concern, one service module, one per-turn tool. Project sessions reuse the existing HTTP channel/session machinery rather than introducing a new `VizierChannelId` variant and touching every exhaustive match. No new crate and no new config surface beyond the two permission strings. | PASS |
| II. DRY via trait-based extensibility | Authorization lives in one `projects/access.rs`; the project shell is the existing `LocalShell` execution path with a richer outcome, not a second shell implementation; per-turn tool injection follows the established client-tool pattern (`ToolContext` + `turn_catalogue` + `ToolTarget`) instead of recomposing toolsets; project context is resolved once and reused by the prompt and the tool. | PASS |
| III. Self-contained, zero-dependency runtime | The project list lives in the existing bundled SQLite database; the WebUI stays embedded; project folders are local paths and are not fetched from a service. No required external process or network resource is introduced. | PASS |
| IV. Portability by default | Paths use `Path`/`PathBuf` and `canonicalize`; command invocation (`sh -c` vs PowerShell) stays in `LocalShell`; the platform-specific process-tree kill is isolated in one shell helper with per-OS implementations, not spread through business logic; Windows reserved-name/path handling follows `LocalDocumentStore::resolve`. | PASS |
| V. Unified errors & observability | Storage returns `crate::Result<T>`; the service maps to `ServiceError`; HTTP handlers keep the `APIResponse` envelope; project/shell operations use `tracing` spans and `warn!`/`info!` (no `println!`). No `unwrap()`/`expect()` outside tests and bootstrap. | PASS |

No violations. Complexity Tracking stays empty.

**Post-design re-check (after Phase 1)**: the design keeps sessions inside the
existing `HTTP` channel kind, introduces no new runtime dependency, and
isolates all platform code in the shell module. The gates still pass.

## Project Structure

### Documentation (this feature)

```text
specs/040-projects-mode/
├── plan.md                        # This file
├── research.md                    # Phase 0: decisions and rejected alternatives
├── data-model.md                  # Phase 1: entities, table, validation, state
├── contracts/
│   ├── http-api.md                # Phase 1: project endpoints and errors
│   ├── session-and-permissions.md # Phase 1: channel identity, listing, auth matrix
│   ├── workspace-shell.md         # Phase 1: tool contract and execution semantics
│   └── ui-and-desktop.md          # Phase 1: sidebar, routes, trust flow, host notice
├── quickstart.md                  # Phase 1: dummyplug + desktop validation
└── tasks.md                       # Phase 2 output (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── schema/
│   ├── project.rs                 # Project, ProjectSummary/Detail, session rows
│   └── session.rs                 # project-channel helpers applied here
├── storage/
│   ├── project.rs                 # ProjectStorage trait + VizierStorage forwarding
│   ├── mod.rs                     # add ProjectStorage to the provider bound
│   └── sqlite/
│       ├── project.rs             # SqliteStorage implementation, delete helpers, tests
│       ├── session.rs             # project channel excluded from conversation rows
│       └── mod.rs                 # init_project_schema + registration
├── projects/
│   ├── mod.rs                     # ProjectService (open/list/rename/remove/sessions)
│   ├── access.rs                  # permission + ownership rules (single home)
│   ├── channel.rs                 # project__<id> channel id helpers
│   ├── context.rs                 # ProjectTurnContext resolution (folder, switch, instructions)
│   ├── instructions.rs            # AGENTS.md → CLAUDE.md, bounded read
│   └── shell.rs                   # WorkspaceShell tool (VizierTool impl)
├── service/
│   ├── projects.rs                # DTOs + authorization + error mapping
│   └── topics.rs                  # project-channel authorization for read routes
├── agents/
│   ├── shell/
│   │   ├── local/mod.rs           # richer local execution: cwd, timeout, exit, caps
│   │   ├── process_tree.rs        # per-OS child-process-tree kill helper
│   │   └── outcome.rs             # ShellCommand / ShellOutcome value types
│   ├── tools/mod.rs               # ToolContext.project, ToolTarget::Project, turn_catalogue
│   ├── tools/python/bridge.rs     # dispatch ToolTarget::Project
│   ├── agent/mod.rs               # resolve project context per turn, prompt section
│   └── process.rs                 # project-aware notification href
├── system_prompt/
│   └── project.rs                 # ## Project / ## Project Instructions section
├── notifications/mod.rs           # project href, audience unchanged (HTTP)
└── storage/user.rs                # add the two project permissions

crates/vizier-http/src/
├── api/v1/agents/projects.rs      # agent-scoped project router
├── api/v1/agents/mod.rs           # nest /{agent_id}/projects
├── api/v1/agents/channel.rs       # project channel accepted by existing routes
├── topic.rs                       # authorize project channel + re-check per inbound turn
└── lib.rs                         # OpenAPI paths/schemas

webui/app/
├── api/endpoints/projects.ts      # client calls
├── stores/projects.ts             # list/detail/selection + 15 s refresh
├── screens/projects/              # ProjectsSection, ProjectRow, OpenProjectDialog, TrustDialog
├── screens/shell/AgentSidebar.tsx # Projects section above <SessionList>
├── screens/chat/*                 # channel-aware chat screen, host notice, greeting CTA
├── routes/projects.tsx            # project session route
├── routes.ts                      # route registration
├── lib/access.ts / permissions.ts # project permission helpers and labels
└── api/transport/{common,tauri,http}.ts # desktop folder picker

app/src-tauri/src/
├── shell.rs                       # desktop_pick_folder command (dialog plugin)
└── ipc.rs / engine.rs             # unchanged creation path (embedded dispatcher)

crates/vizier-core/
└── Cargo.toml                     # tokio "process"; libc on Unix
```

**Structure Decision**: The project entity is a core storage/service concern,
the workspace shell is an agent tool, and all remote access goes through the
existing agent-scoped HTTP router. Browser and desktop share one WebUI; only
the folder picker is desktop-specific.

## Design Overview

### 1. Identity: a project session is an HTTP session with a project channel

`VizierSession(agent, VizierChannelId::HTTP(owner_username, "project__<project_id>"), topic)`
is the whole session identity. This gives, without new code paths:

- personal ownership (the username is server-supplied from the principal);
- a distinct `session_detail` / `session_history` / `session_turn` key;
- existing notifications (`chat_replied` already handles HTTP) and usage rows;
- remote browser access through the existing WebSocket route.

A project's sessions are therefore not a new channel kind and do not require
changes to `VizierChannelId` or its exhaustive matches. The only required
carve-outs are:

- `storage/sqlite/session.rs::conversation_rows` and
  `service/conversations.rs::kind_of` skip `project__` channels so FR-006 holds;
- `notifications::chat_replied` builds the project route href for a project
  channel;
- all project authorization is layered on top of the existing agent visibility
  check.

### 2. Storage: one additive table

`project` stores `id`, `agent_id`, `name`, `folder` (canonical), `owner_id`,
`shell_enabled`, `created_at`, `removed_at`. `UNIQUE(agent_id, folder)` gives
FR-003 and lets removal be soft (FR-025): the row and its id survive, so
reopening the same folder clears `removed_at` and the old sessions are
addressable again because their channel id is unchanged.

`available` is derived on read from the folder's existence/directory-ness
(FR-026); it is not stored. `ProjectStorage` is added to the
`VizierStorageProvider` bound and forwarded by `VizierStorage`. See
[data-model.md](./data-model.md).

### 3. Permissions: two strings, one access module

- `all_agents:projects` — use projects on any agent the person can view.
- `owned_agents:projects` — use projects on agents they own.

`projects/access.rs` is the only place that answers: can use, can manage
(project owner or whole-workspace permission), can create (desktop + can use).
The system role gains both strings automatically through the existing
`migrate_users` extension, satisfying FR-019 without a migration.

### 4. Turn context: resolved once, used by prompt and tool

`projects::context::for_session(deps, session)` returns
`Option<ProjectTurnContext>` only for a project channel: project id/name,
canonical folder, `available`, `shell_enabled`, and bounded instructions.

`chat_turn` resolves it once, stores it on `ToolContext.project`, and passes it
to `prepare_system_prompts`. The prompt gains a `## Project` section after CORE
and before configured documents, stating the folder, whether the shell is on,
and (when present) the instruction document with an explicit truncation note
(FR-015/FR-016). Ordinary, task, dream, subagent and inter-agent sessions have
a different channel, so they resolve to `None` automatically (FR-014). A
background report waking inside a project session inherits the context because
it reuses the origin session. Because the resolver also re-reads the session
owner's current permission, revocation disables the project section and the
shell on the next turn even for an internally woken turn, while history stays
readable.

### 5. Workspace shell: per-turn tool, richer local outcome

`workspace_shell` is a native `VizierTool` in the existing `shell` category. It
is **not** registered in `BaseTools` or the shell addon. It is appended by
`turn_catalogue`/`turn_categories` when `ToolContext.project` exists and the
switch is on, and dispatched by `RouterBridge` through a new
`ToolTarget::Project` (mirroring `ToolTarget::Client`). This keeps it out of
ordinary conversations, tasks, dreams and subagent pieces, and avoids
recomposing the agent's toolsets per turn.

Execution reuses the existing local shell invocation (`sh -c` / PowerShell) and
`LocalShell`, extended with a richer outcome: stdout, stderr, exit code,
duration, timed-out and truncated flags. It uses `tokio::process` with
`kill_on_drop`, puts the child in its own process group on Unix / uses
`taskkill /T` on Windows on timeout, resolves an optional relative working
directory under the canonical project root and refuses escapes, and caps each
stream. The existing addon `shell_exec` keeps its current text contract by
rendering the same outcome. See [workspace-shell.md](./contracts/workspace-shell.md).

### 6. HTTP API

Agent-scoped routes under `/api/v1/agents/{agent_id}/projects`:

- `GET /` — list the agent's non-removed projects, with `available` and session counts.
- `POST /` — desktop-only open/create; validates folder, refuses the app workspace, reuses an existing row.
- `GET /{project_id}` — project detail visible to a permitted viewer.
- `PATCH /{project_id}` — rename and/or shell switch (owner or whole-workspace).
- `DELETE /{project_id}` — soft remove (owner or whole-workspace), refused while any project turn is pending.
- `GET /{project_id}/sessions` — the caller's sessions in the project.
- `POST /{project_id}/sessions` — create a titled empty session and return its topic id.

History, files and chat reuse the existing channel routes with
`channel_id = project__<project_id>`, with project authorization added to
`authorize_topic`/`TopicConnection::open` and re-checked per inbound turn so
revocation is immediate (FR-018). The same helper is applied to the REST chat
and `/btw` routes that accept a channel id, so no entry point bypasses it. See
[http-api.md](./contracts/http-api.md).

### 7. WebUI and desktop

- `ProjectsSection` renders above `SessionList` in `AgentSidebar`, only when
  the person has a project permission and the agent has projects (FR-005).
  Rows expand to the caller's sessions with the existing live marker.
- "Open a Project" appears in the desktop empty conversation and the project
  section. It calls a new Tauri `desktop_pick_folder` command (dialog plugin is
  already installed), then shows a danger-tone trust disclosure before POSTing
  the folder. The WebUI shows no creation affordance and the server refuses a
  non-desktop principal (FR-021).
- Project sessions use `/:agentId/projects/:projectId/chat/:topicId`; the chat
  screen takes its channel from the route instead of the hard-coded
  `vizier-webui`. Browser users see a persistent "Commands run on the host
  machine" indicator (FR-023).
- Rename, remove and the shell switch are in the project row's menu, disabled
  unless the person owns the project or has `all_agents:projects` (FR-022).

### 8. Removal, restoration and concurrency

Removal sets `removed_at` after checking `pending_turns` for every topic in the
project channel; no file or history row is deleted. Reopening the same folder
clears `removed_at` and the prior sessions reappear. Two sessions in one
project run through the existing per-session turn queues, so they are
independent (FR-027), while a single session keeps its existing serialisation.

## Phase 0: Outline & Research

All unknowns are resolved in [research.md](./research.md). No
`NEEDS CLARIFICATION` remains. Decisions cover session identity, the storage
model, the permission model, desktop-only creation, the shell contract and
process-tree kill, per-turn tool injection, prompt/instruction handling,
listing exclusions, session creation/titling, attribution, constants, folder
validation, removal checks, WebUI routing and concurrency.

## Phase 1: Design & Contracts

Generated:

- [data-model.md](./data-model.md) — `project` table, derived session key,
  state transitions, validation, DTOs and storage operations.
- [contracts/http-api.md](./contracts/http-api.md) — endpoints, bodies,
  responses, status codes and permission rules.
- [contracts/session-and-permissions.md](./contracts/session-and-permissions.md)
  — channel namespace, slugs, listing exclusion, authorization matrix.
- [contracts/workspace-shell.md](./contracts/workspace-shell.md) — tool input
  and output schema, timeout/kill/output rules, prompt contract.
- [contracts/ui-and-desktop.md](./contracts/ui-and-desktop.md) — sidebar,
  routes, trust flow, host indicator, desktop IPC.
- [quickstart.md](./quickstart.md) — dummyplug and desktop validation steps.

## Verification Plan

- **Rust**: `cargo test` and `cargo clippy` at the workspace root. New unit
  tests cover channel helpers, canonical path/workspace refusal, instruction
  resolution and truncation, shell timeout/kill/cap/cwd escape, storage
  uniqueness and soft removal, and the permission matrix. HTTP conformance
  gains a `projects` group; `tests/conformance/pending.txt` only shrinks.
- **Desktop**: `cd app && cargo test`, including a smoke test that creates a
  project through the embedded `Dispatcher` (desktop principal) and checks that
  the network router refuses creation.
- **WebUI**: `cd webui && npm run typecheck && npm test` for the projects
  store, permission helpers, channel-aware routes and host notice.
- **End to end**: the dummyplug steps in [quickstart.md](./quickstart.md)
  prove shell output/exit/duration, a failing command as a normal result,
  timeout, cwd refusal, project context/instructions, listing isolation,
  remote access and the host indicator, removal/restoration, and permission
  refusal.

## Complexity Tracking

> No constitution violations. This table is intentionally empty.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| — | — | — |
