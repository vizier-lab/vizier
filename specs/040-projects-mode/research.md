# Research: Projects Mode Foundation

**Feature**: `040-projects-mode` | **Date**: 2026-10-31

Every `NEEDS CLARIFICATION` in the plan's Technical Context is resolved below.
Each decision is written as **Decision / Rationale / Alternatives considered**.

---

## R1. How a project session is identified

**Decision**: A project session is
`VizierSession(agent_id, VizierChannelId::HTTP(owner_username, "project__<project_id>"), Some(topic))`.
The channel id is built server-side from the authenticated username and the
project id. A shared helper module (`projects::channel`) owns the prefix,
building and parsing.

**Rationale**: This is the smallest change that satisfies every session-related
requirement:

- Personal ownership: the existing `HTTP(username, …)` convention already means
  "this person's conversation", and every write path builds the channel from
  the principal rather than from client input (`service/topics.rs:157`,
  `service/topics.rs:264`, `crates/vizier-http/src/topic.rs:68-72`).
- Remote access: the browser WebSocket route already constructs exactly this
  channel, so desktop and browser sessions share one identity.
- Notifications: `Notifications::chat_replied` already resolves the owner from
  `HTTP(username, _)` (`crates/vizier-core/src/notifications/mod.rs:224-260`).
- Persistence and restoration: the session key is stable across restarts and
  across removing/reopening a project, because the project row keeps its id.
- Storage needs no new session table: `session_detail`, `session_history`,
  `session_turn`, `session_file` and scratch files all key on the channel slug.

**Alternatives considered**:

- *New `VizierChannelId::Project(..)` variant.* Cleaner typing, but it forces
  changes to every exhaustive match (`usage_place`, `platform`,
  `usage_activity`, `to_slug`, `record_channel`, `is_non_user_channel`,
  notification audience, conversation kind) and to the WebSocket path parser,
  for no behaviour the HTTP channel does not already provide. Rejected as
  unjustified churn under Principle I.
- *Store `project_id` in `session_detail` only.* The channel would remain
  `vizier-webui`, so project and ordinary sessions would collide in the same
  channel and in the regular Sessions list, and `session_detail` is written
  lazily after the first turn. Rejected.
- *Encode the project in the topic.* Topic is a free-form client id; a
  malicious or stale client could address another project, and the history key
  would lose its meaning. Rejected.

**Constraint**: the id must be filesystem-safe because it becomes part of
`session_slug` (session files live under `uploads/<session_slug>/`). The
`project__` prefix and a nanoid/alphanumeric project id satisfy this; a colon
(e.g. `project:` ) would break Windows paths and is rejected.

---

## R2. Storage model for a project

**Decision**: One additive table, `project`, in the existing SQLite database,
with `UNIQUE(agent_id, folder)` and a nullable `removed_at`:

```text
project(
  id           TEXT PRIMARY KEY,
  agent_id     TEXT NOT NULL,
  name         TEXT NOT NULL,
  folder       TEXT NOT NULL,          -- canonical path
  owner_id     TEXT NOT NULL,          -- user who opened it
  shell_enabled INTEGER NOT NULL,      -- 0/1, default 1
  created_at   INTEGER NOT NULL,       -- Unix milliseconds
  removed_at   INTEGER,                -- NULL while open
  UNIQUE(agent_id, folder)
)
```

`available` is derived at read time (`folder` exists and is a directory), not
stored.

**Rationale**: The spec's identity rule is "at most one project per folder per
agent" (FR-003), which is a database constraint, not application logic.
Soft removal (FR-025) must keep the id so the channel — and therefore the
sessions — can be restored by reopening the folder. Explicit columns allow
listing, uniqueness and the removal filter to be indexed; a JSON blob would
force a table scan for the uniqueness rule.

**Alternatives considered**:

- *Store projects on `AgentConfig`.* Any config change restarts the agent and
  aborts its turns (`agents/mod.rs:471-493`); projects must be created and
  removed without touching the running agent or its shell switch. Rejected.
- *Delete the row on removal and recreate on reopen.* The new id would orphan
  the sessions; FR-025 and SC-006 require them to come back. Rejected.
- *Hard-code `folder` uniqueness only in the service.* Race-prone under two
  concurrent opens; the database constraint is the single source of truth.

---

## R3. Permission model

**Decision**: Add two strings to `AVAILABLE_PERMISSIONS`
(`crates/vizier-core/src/storage/user.rs:5-31`):

- `all_agents:projects` — use projects on every agent the person can view;
- `owned_agents:projects` — use projects on agents the person owns.

`projects/access.rs` owns the rules:

| Action | Rule |
|--------|------|
| See the Projects section and list projects | `can_view_agent` **and** (`all_agents:projects` **or** `owned_agents:projects` + ownership) |
| Open a project session / run turns / read its history | same as above |
| Create a project | same as above **and** the principal is `AuthMethod::Desktop` |
| Rename, remove, toggle the shell switch | project `owner_id` **or** `all_agents:projects` |

**Rationale**: This mirrors the existing `all_agents:*` / `owned_agents:*`
pattern (secrets, artifacts, integrations) and the spec's statement that the
permission exists "in a whole-workspace scope and an owned-agents scope"
(spec.md Key Entities). The system role is automatically extended with any new
`AVAILABLE_PERMISSIONS` entry on startup
(`crates/vizier-core/src/dependencies.rs:940-972`), satisfying FR-019 with no
migration.

**Alternatives considered**:

- *A single literal `"projects"` string.* Simpler, but it cannot express
  "projects of the agents I own" without a second check, and it breaks the
  permission catalogue's grouping conventions. Rejected.
- *Reuse `all_agents:view` as the gate.* That would give every viewer project
  command execution on the host, which is exactly the trust boundary the spec
  introduces. Rejected.

---

## R4. Desktop-only creation

**Decision**: `POST /agents/{agent_id}/projects` refuses any principal whose
`Principal.method` is not `AuthMethod::Desktop`. The check lives in the project
service, not the handler.

**Rationale**: `AuthMethod::Desktop` is minted only by the in-process
`embedded::Dispatcher` (`crates/vizier-http/src/embedded.rs:17-37`), and the
network auth middleware rejects a request that tries to supply it
(`crates/vizier-http/src/auth/middleware.rs:95-101`). This is an existing,
non-forgeable signal, so no new capability flag or loopback heuristic is
needed.

**Alternatives considered**:

- *Check `AuthMode` in the HTTP state.* Equivalent, but keeps the rule in the
  transport layer; the repo requires authorization in the service layer.
- *Hide the button in the WebUI only.* FR-021 requires a direct attempt to be
  refused; UI hiding alone is not a control.

---

## R5. Workspace shell execution contract

**Decision**: Add a richer local execution path used by the project tool:

- input: `command`, optional relative `workdir`, optional `timeout_secs`;
- output: `stdout`, `stderr`, `exit_code`, `duration_ms`, `timed_out`,
  `truncated`;
- default timeout **120 s**, per-call maximum **600 s**;
- each stream capped at **64 KiB** (UTF-8-safe cut), with `truncated = true`;
- a non-zero exit is a normal successful tool result (FR-010);
- on timeout the process tree is killed and partial output returned (FR-011);
- `workdir` is resolved under the canonical project folder and refused if it
  escapes (FR-009);
- commands run through the existing platform invocation (`sh -c` on Unix,
  PowerShell on Windows) with the host environment.

`LocalShell` gains the richer execution as an inherent method; the existing
`ShellProvider::exec` delegates to it and keeps its text-only contract, so the
shell addon's `shell_exec` behaviour is preserved. The platform-specific
process-tree kill lives in `agents/shell/process_tree.rs`:
`tokio::process` with `kill_on_drop`; on Unix the child is started in its own
process group (`CommandExt::process_group`) and the group is signalled with
`libc::killpg`; on Windows `taskkill /T /F /PID` is used.

**Rationale**: The current shell path returns stdout only, never stderr, exit
status or duration, cannot kill a timed-out child, and runs `Command::output()`
inside `spawn_blocking`, so dropping the future leaves the process alive
(`agents/shell/local/mod.rs:47-65`). The project shell must be a reliable tool,
but the Docker backend is not a project host and has no production caller
today; extending the shared `ShellProvider` with Docker-specific semantics
would be speculative work. Keeping the richer path in `LocalShell` avoids a
second shell backend and reuses the same invocation code.

**Alternatives considered**:

- *New independent `ProjectShell` with its own `std::process` code.* Would
  duplicate the platform invocation and the environment handling; rejected
  under Principle II.
- *Recompose the shell addon with the project folder.* The addon is per agent,
  so it would change every session's shell and violate FR-014.
- *Fire-and-forget kill of only the direct shell process.* Child processes
  survive; FR-011 explicitly requires stopping them.
- *Add `tokio`'s `process` feature plus a direct `libc` on Unix.* Accepted:
  no new crate enters the graph (`libc` is already in `Cargo.lock`), and the
  process-group call cannot be hand-rolled portably.

---

## R6. Making the shell available only in project sessions

**Decision**: Carry a per-turn `ToolContext.project: Option<Arc<ProjectTurnContext>>`
and a new `ToolTarget::Project`; append the tool's `CatalogueEntry` in
`turn_catalogue` and its category line in `turn_categories`; dispatch it in
`RouterBridge`; do not register it in `BaseTools` or the shell addon.

**Rationale**: This is the established per-turn extension seam. Client tools
(spec 019) already do exactly this because recomposing `VizierTools` per turn
copies every toolset and replays integration/UI concerns
(`specs/019-client-tools/research.md:88-92`). A `ToolContext` field also gives
the tool the project folder without another storage read. Because the tool is
only in the turn catalogue, `list_tool_functions`, `describe_tool_function`
and script dispatch all see it consistently.

**Alternatives considered**:

- *Add it to `default_toolset`.* Would offer it in ordinary conversations and
  give it no project folder; violates FR-014.
- *Recompose `VizierTools` per turn.* Correct but expensive and contrary to
  two prior design decisions; rejected.
- *Key on `VizierRequest.metadata`.* Metadata is client-supplied per request
  and not durable; the session must be the source of truth.

---

## R7. Project context and instruction document

**Decision**: `projects/instructions.rs` reads, in order, `AGENTS.md` then
`CLAUDE.md` from the project root, caps the content at **32 KiB**, and reports
whether it was truncated. `system_prompt/project.rs` adds a `## Project`
section (name, folder, shell state) and, when present, a
`## Project Instructions` section, injected after CORE and before configured
documents. A missing document is harmless.

**Rationale**: `AGENTS.md` is the convention already used by this repository
and by coding agents generally; `CLAUDE.md` is a common fallback. Re-reading
per turn means edits are picked up on the next turn without a restart. The
system prompt (rather than the user-message context block) keeps project
identity stable for prompt caching and makes the framing apply to every turn,
including a checkpoint rebuild.

**Alternatives considered**:

- *Only `AGENTS.md`.* Misses repositories that use `CLAUDE.md`.
- *Only the user-message context block.* It is rebuilt per request and
  deliberately kept out of the system prefix for caching; project identity is
  not a per-request attachment.
- *Reuse CORE.md or `config.documents`.* Those are agent-level, not
  folder-level, and would leak project instructions into ordinary sessions.
- *No cap.* FR-016 requires a bounded size and a truncation notice.

---

## R8. Project context when the folder is missing or the shell is off

**Decision**: `ProjectTurnContext` carries `available` and `shell_enabled`.
When the folder is missing/unreadable, the prompt says so and the workspace
tool is **not** offered; when the switch is off, the tool is not offered and
the prompt says it is off. History remains readable in both cases.

**Rationale**: FR-017 and FR-026 both require that the agent is not handed a
tool that cannot work. Omitting the tool is clearer to the model than a
guaranteed failure, and the prompt still explains the state so the agent can
answer the person. Availability is evaluated on each turn, so a folder that
comes back (or a switch toggled) takes effect without restarting the agent.

**Alternatives considered**: offering the tool and refusing at call time. That
produces a tool failure the model may retry; the spec's language ("not
offered") points to omission.

---

## R9. Excluding project sessions from the regular Sessions list

**Decision**: `projects::channel::is_project_slug` is checked in
`storage/sqlite/session.rs::conversation_rows` and
`service/conversations.rs::kind_of`. Project sessions are listed only by the
project sessions endpoint (`GET …/projects/{id}/sessions`), which queries only
the caller's project channel.

**Rationale**: `ConversationRow` is derived from `session_history` grouped by
channel; any `http__` channel currently becomes a WebUI conversation
(`storage/sqlite/session.rs:131-151`, `service/conversations.rs:33-43`). A
single prefix check in both places keeps FR-006 true for the sidebar, the
conversation picker, search and counts, without changing the session table.

**Alternatives considered**: adding a `project_id` column to `session_history`.
Redundant — the channel already carries it — and would require a backfill plus
index changes.

---

## R10. Creating a project session

**Decision**: `POST …/projects/{id}/sessions` generates a topic id server-side
and writes a `session_detail` row with title `"New session"` before returning
the topic. `refresh_session_detail` treats that placeholder as untitled and
replaces it with the generated 60-character title after the first request.

**Rationale**: FR / US2 requires a new empty session to be "listed under that
project" immediately. Today a session only appears after its first history
row, so an empty one would be invisible. Writing the detail row at creation
makes the list truthful. Using a recognizable placeholder avoids changing the
existing "title equals topic slug means untitled" rule for all other channels.

**Alternatives considered**: keeping the new session only in client state until
the first message. It disappears on reload and breaks multi-tab consistency.

---

## R11. Attribution: notifications and usage

**Decision**: Reuse the HTTP paths. `chat_replied` gains a project-aware href
(`/{agent}/projects/{id}/chat/{topic}`); the audience remains the owner resolved
from the username. Usage keeps the existing `channel` slug (which contains the
owner and project id) and `place`; no usage schema or aggregation change.

**Rationale**: FR-029 asks for attribution, not a new report. The existing rows
already encode person (`http__<user>__…`) and project (`project__<id>`), so the
data is attributable without a migration. The one thing that is wrong without a
change is the notification link, which would point at the ordinary chat route.

**Alternatives considered**: adding a `project_id` dimension to `usage_event`.
A schema and query change for a display feature the spec does not require.

---

## R12. Constants and defaults

**Decision**: Put the foundation's limits in one `projects` constants module:

| Constant | Value | Used by |
|----------|-------|---------|
| `DEFAULT_COMMAND_TIMEOUT` | 120 s | workspace shell |
| `MAX_COMMAND_TIMEOUT` | 600 s | workspace shell input validation |
| `OUTPUT_CAP_BYTES` | 64 KiB per stream | workspace shell |
| `INSTRUCTIONS_CAP_BYTES` | 32 KiB | instruction reader |
| `PROJECT_ID_LEN` | 10 | project id generation (same shape as WebUI topic ids) |

They are compile-time defaults, not a new configuration surface. The spec calls
the exact values implementation choices (Assumptions).

**Alternatives considered**: exposing them in the global config. That adds a
config surface with no current caller; Principle I says no.

---

## R13. Folder validation and refusing the application's workspace

**Decision**: `open_project` canonicalizes the chosen folder and requires it to
exist and be a directory. It then canonicalizes `VizierConfig.workspace` and
refuses the folder when it is equal to, or inside, that workspace. The
containment check uses canonical paths (symlinks resolved) and `Path::starts_with`.

**Rationale**: FR-001 and FR-028. Canonicalization makes `.`/`..`, symlinks and
case differences on Windows resolve to one comparable form; comparing raw
strings would be bypassable. Refusing descendants protects `.runtime` (the
database, encryption key, lock), `agents/` (memory/CORE) and `uploads/`.

**Alternatives considered**: refusing only exact equality. The workspace's
children are the most dangerous targets (the database and memory documents), so
containment is required.

---

## R14. Removing a project while a turn is running

**Decision**: Removal first checks `TurnStorage::pending_turns(agent_id)` for
any turn whose channel is the project channel; if one exists, return `409` with
a stable code and change nothing. Otherwise set `removed_at`.

**Rationale**: FR / edge cases require refusal until the turn ends. Pending
turns are already the durable source of truth used by session-file clearing and
conversation deletion, so checking them avoids a second in-memory registry.

**Alternatives considered**: refusing only if the *caller's* session is busy.
Another person's running session would still be hidden mid-turn and its tool
output would reference a project that is gone; refusing on any project turn is
safer.

---

## R15. WebUI routing and channel threading

**Decision**: Project sessions use
`/:agentId/projects/:projectId/chat/:topicId`. The chat screen, topic list and
socket open take a `channel` argument (ordinary sessions keep
`vizier-webui`). `AppShell`/`pages.ts` learn the project segment, and the
sidebar's current-session check matches both routes.

**Rationale**: Keeping the route under `:agentId` preserves `AppShell`'s agent
resolution and every agent-scoped permission check. Putting the project in the
path makes a project session shareable and bookmarkable, and it makes the host
indicator and project menu derivable without extra state. The only alternative
(project in a query string) would leave two URLs for one session and complicate
the sidebar's active-row logic.

**Alternatives considered**: a top-level `/projects/...` route. `AppShell`
treats the first segment as the agent unless it is `settings`/`agents`, so a
top-level project route needs a new shell mode; rejected as larger.

---

## R16. Desktop folder picker

**Decision**: Add a Tauri command `desktop_pick_folder` that calls the existing
`tauri-plugin-dialog` blocking folder picker, register it in both the debug and
release command lists, expose it as `DesktopShell.pickFolder` in the transport
seam, and call it only when `transport.authMode === 'desktop'`.

**Rationale**: The dialog plugin and its capability already exist and are used
by the workspace picker (`app/src-tauri/src/shell.rs:181-198`,
`app/src-tauri/capabilities/main.json`). Reusing `desktop_open_workspace` is
wrong: it shuts down and restarts the whole engine to change the app workspace
(`app/src-tauri/src/engine.rs:355-359`). A folder pick is a read-only native
dialog, so creation can stay a normal authenticated API call.

**Alternatives considered**: adding `@tauri-apps/plugin-dialog` to the WebUI.
That duplicates a Rust-side capability and widens the JS surface; the existing
pattern is Rust-side.

---

## R17. Concurrency

**Decision**: Do not add any lock at the project level. Each session keeps the
existing per-session turn queue; different topics in one project run in
parallel. The project shell is constructed per turn and holds no shared mutable
state.

**Rationale**: FR-027/SC-008 require two sessions not to wait on each other.
The per-session queue already provides this, and adding a project mutex would
regress it. The only shared state is the SQLite project row, read once per turn.

**Alternatives considered**: serialising project commands to avoid file races.
The spec explicitly accepts races and makes the person responsible; serialising
would violate FR-027.
