# Data Model: Projects Mode Foundation

**Feature**: `040-projects-mode` | **Date**: 2026-10-31

All timestamps are Unix milliseconds (the existing storage convention). All
paths are canonical, absolute host paths. Types are Rust types in
`crates/vizier-core/src/schema/project.rs` unless stated otherwise.

---

## 1. Project

The long-lived entity: a folder mounted to one agent.

| Field | Type | Notes |
|-------|------|-------|
| `id` | `String` | Primary key. `nanoid` length 10, filesystem-safe. Never changes, including across removal. |
| `agent_id` | `AgentId` | The single agent this project belongs to. |
| `name` | `String` | Display name; defaults to the folder's final component; renameable. Two projects may share a name. |
| `folder` | `PathBuf` (stored as `TEXT`) | Canonical absolute path. The mount point. On Windows the canonical `\\?\` prefix is kept for comparison and stripped for display. |
| `owner_id` | `String` | `user_id` of the person who opened it. Used for management rights. |
| `shell_enabled` | `bool` (stored `INTEGER 0/1`) | Workspace-shell switch. Default `true`. |
| `created_at` | `i64` | When the project was first opened. |
| `removed_at` | `Option<i64>` | `NULL` while open; set by soft removal. |

**Derived, not stored**

| Field | Derivation |
|-------|------------|
| `available` | `folder.is_dir()` at read time. `false` means missing/unreadable → commands refused, history readable. |
| `channel_id` | `"project__" + id` (see §2). |
| `session_count` | Count of `session_detail` rows for the project channel, all owners (for the sidebar's project row). The caller's own session list is separate. |

### Table

```sql
CREATE TABLE IF NOT EXISTS project (
  id            TEXT PRIMARY KEY,
  agent_id      TEXT NOT NULL,
  name          TEXT NOT NULL,
  folder        TEXT NOT NULL,
  owner_id      TEXT NOT NULL,
  shell_enabled INTEGER NOT NULL DEFAULT 1,
  created_at    INTEGER NOT NULL,
  removed_at    INTEGER,
  UNIQUE(agent_id, folder)
);
CREATE INDEX IF NOT EXISTS idx_project_agent
  ON project(agent_id, removed_at);
```

`UNIQUE(agent_id, folder)` is the database enforcement of FR-003. The index
serves the sidebar's "projects of this agent that are not removed" query.

### Validation rules

| Rule | Source | On violation |
|------|--------|--------------|
| The folder exists and is a directory | FR-001 | `422 invalid_folder` |
| The folder is not the agent workspace, and is not inside it | FR-028 | `422 workspace_folder` |
| The folder is not empty/`.`/`..` after canonicalization | FR-001 | `422 invalid_folder` |
| The canonical folder is unique for the agent | FR-003 | reuse the existing row (clear `removed_at`) and return it, `200` |
| `name` is non-empty after trim, ≤ 120 chars | usability | `422 invalid_name` |
| `shell_enabled` is a bool | FR-017 | serde rejects |
| A project id is 10 alphanumeric characters | internal | `404` for malformed ids |

### State transitions

```text
                 open (create)                 remove (soft)
   (none) ──────────────────────► open ──────────────────────► removed
                                    ▲                             │
                                    │      open same folder       │
                                    └─────────────────────────────┘
                                          (clears removed_at)

   open/removed ── folder disappears ──► unavailable (derived, no row change)
   unavailable ── folder restored ─────► available (derived, no row change)
```

- **Open**: validate → canonicalize → find `(agent_id, folder)`.
  - absent → insert, title `"New session"` session is *not* created here (the
    UI calls `POST …/sessions` next);
  - present and `removed_at` is set → clear it, keep `id` and sessions;
  - present and open → return it (no duplicate).
- **Rename**: update `name` only.
- **Toggle shell**: update `shell_enabled` only. Takes effect on the next turn;
  no agent restart.
- **Remove**: refuse if any pending turn exists on the project channel
  (`409 turn_active`), else set `removed_at = now`. No files or history are
  touched.
- **Move the folder**: outside the model. The old project becomes unavailable;
  opening the new path creates a separate project (spec edge case).

### Agent deletion

`delete_agent` calls `delete_agent_projects(&conn, agent_id)` in the same
transaction as the rest of the agent's rows, so a deleted agent leaves no
project rows. Sessions/history are already removed by the existing agent
deletion path.

---

## 2. Project session (derived)

A conversation inside a project. It is **not** a new row type; it is the
existing session tuple with a project-scoped channel.

| Aspect | Value |
|--------|-------|
| Runtime identity | `VizierSession(agent_id, VizierChannelId::HTTP(owner_username, channel_id), Some(topic))` |
| `channel_id` | `"project__" + project_id` |
| Channel slug | `http__<owner_username>__project__<project_id>` |
| Detail id | `<agent_id>#http__<owner>__project__<project_id>#<topic>` |
| History key | `(agent_id, channel_slug, topic)` in `session_history` |
| Turn key | `(agent_id, channel_slug, topic)` in `session_turn` |
| Session files | `session_slug = <agent_id>__<channel_slug>__<topic>` |
| Usage | `channel = <channel_slug>`, `place = http:<channel_slug>:<topic>` |
| Notification | `audience_user_id` = the owner resolved from `<owner_username>` |
| Href | `/<agent_id>/projects/<project_id>/chat/<topic>` |

**Rules**

- The owner is always the authenticated username; it is never read from the
  request path or body.
- The project id must belong to the same agent; a project id from another agent
  yields `404`.
- The project must be non-removed and the caller must hold the project
  permission for the agent (see the authorization matrix).
- Project sessions never appear in `GET /agents/{id}/conversations` or in the
  regular Sessions list.

### Project session row (API projection)

| Field | Type | Notes |
|-------|------|-------|
| `topic` | `String` | Topic id; the session's address within the project. |
| `title` | `String` | From `session_detail`; `"New session"` until the first reply titles it. |
| `is_thinking` | `bool` | Live/running marker, already used by the sidebar. |
| `last_activity` | `Option<i64>` | `MAX(session_history.timestamp)` for that topic; `None` for a brand-new empty session. |

Query shape: left-join `session_detail` to per-topic `MAX(timestamp)` in
`session_history`, filtered by `agent_id` and the exact project channel slug,
ordered by `last_activity DESC NULLS LAST, title`.

### Storage operations (`ProjectStorage`)

| Method | Purpose |
|--------|---------|
| `list_projects(agent_id, include_removed: bool)` | Sidebar list; removed rows excluded normally. |
| `get_project(agent_id, project_id)` | Detail / turn context. Returns removed rows too (for restore checks). |
| `open_project(Project) -> (Project, bool created)` | Insert or restore; the `bool` drives `201` vs `200`. |
| `rename_project(id, name)` | Owner/whole-workspace only (checked in the service). |
| `set_project_shell(id, enabled)` | Same. |
| `remove_project(id, removed_at)` | Soft removal. |
| `project_sessions(agent_id, channel_slug)` | The session row projection above. |
| `delete_agent_projects(conn, agent_id)` | Cascade helper used by agent deletion. |

The service layer performs authorization and folder validation; storage is
deliberately unaware of permissions.

---

## 3. Project instructions

A file the agent reads on every project turn.

| Aspect | Value |
|--------|-------|
| Candidate files | `AGENTS.md`, then `CLAUDE.md` (first one that exists) |
| Location | direct child of the project folder (not recursive) |
| Encoding | UTF-8, lossy on invalid bytes |
| Cap | 32 KiB (`INSTRUCTIONS_CAP_BYTES`) |
| Truncation | cut on a UTF-8 boundary; the prompt states it was truncated |
| Missing/unreadable | no section, no error, no tool failure |
| Re-read | every turn (edits apply on the next turn) |

This is not a new stored entity; it is read-through state on
`ProjectTurnContext`.

---

## 4. Project permission

| Permission string | Meaning |
|-------------------|---------|
| `all_agents:projects` | See and use projects of every agent the person can view; manage every project. |
| `owned_agents:projects` | See and use projects of agents the person owns. |

Both are added to `AVAILABLE_PERMISSIONS` and therefore to the system role on
startup. The WebUI permission catalogue gains a "Projects" group with both
entries. Management additionally allows the project's `owner_id`.

---

## 5. Workspace shell call (value object)

One command run inside a project. Not stored as a row; it lives in the turn's
history as a normal tool call/result.

| Field | Type | Notes |
|-------|------|-------|
| `command` | `String` | The command line, run by `sh -c` / PowerShell. |
| `workdir` | `Option<String>` | Optional relative directory under the project root; default is the root. |
| `timeout_secs` | `Option<u64>` | Default 120, minimum 1, maximum 600. |
| `stdout` | `String` | Captured standard output, capped. |
| `stderr` | `String` | Captured standard error, capped. |
| `exit_code` | `Option<i32>` | `None` when killed/timed out. |
| `duration_ms` | `u64` | Wall-clock time from spawn to exit/kill. |
| `timed_out` | `bool` | True when the limit was hit and the tree was killed. |
| `truncated` | `bool` | True when either stream was cut at the cap. |

**Invariants**

- A non-zero `exit_code` is a successful tool result; the tool only errors when
  the project is unavailable, the shell is disabled, or `workdir` escapes.
- `timed_out = true` implies `exit_code = None` and partial output is returned.
- The command is never sandboxed (FR-002); the tool description says so.

---

## 6. API DTOs (`service/projects.rs`)

| DTO | Fields |
|-----|--------|
| `ProjectSummary` | `id`, `agent_id`, `name`, `folder`, `available`, `shell_enabled`, `owner_id`, `is_owner`, `session_count`, `created_at` |
| `ProjectDetail` | summary + `instructions_present: bool`, `instructions_truncated: bool` (no instruction text in API responses) |
| `OpenProjectRequest` | `folder: String` |
| `UpdateProjectRequest` | `name: Option<String>`, `shell_enabled: Option<bool>` |
| `ProjectSession` | `topic`, `title`, `is_thinking`, `last_activity` |
| `CreateProjectSessionResponse` | `topic`, `title`, `channel_id` |

`ProjectDetail` never returns the instruction text; the text is for the agent
prompt only. The API returns `folder` because the UI must display which folder
is mounted and the trust disclosure has already shown it.

---

## 7. Invariants summary

1. One project row per `(agent_id, canonical folder)`, forever (soft removal
   keeps the id).
2. A project belongs to exactly one agent; a folder opened for another agent is
   a different project.
3. A project session belongs to exactly one person (its channel username) and
   one project (its channel id).
4. Removing a project deletes no file and no history row; it only sets
   `removed_at`.
5. A missing folder changes only the derived `available` flag.
6. Project context and the workspace shell exist only for project sessions.
7. The project folder is never the workspace or inside it.
8. The shell switch gates tool availability, not storage or history access.
