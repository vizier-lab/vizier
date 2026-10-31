# Data Model: Project Coding Workspace

**Feature**: `041-project-coding-workspace`

Two additive SQLite tables, one data migration, one in-memory ledger, and changes to existing
value types. The 040 `project` table is unchanged.

---

## 1. Channel: `VizierChannelId::Project`

| Field | Type | Notes |
|---|---|---|
| `0` | `String` | the person's username (server-supplied from the principal, never from a client) |
| `1` | `String` | project id (`[A-Za-z0-9_-]+`, as 040) |

- Slug: `project__{username}__{project_id}`.
- Serialised (serde, externally tagged): `{"Project":["alice","p3xY9aQ2mZ"]}`.
- Web wire form (URLs, socket path, `channel_id` in requests and responses):
  `project__{project_id}`, unchanged from 040.
- `usage_activity()` → `UsageActivity::Project`; `usage_place()` →
  `project:{slug}:{topic}`; `platform()` → `None`; `web_user()` → `Some(username)`.
- Included in the dream cycle: `is_non_user_channel` does not match `project__`.

See [contracts/channel-and-migration.md](./contracts/channel-and-migration.md).

## 2. `project_chat` (new table)

The saved mode of one project chat.

```sql
CREATE TABLE IF NOT EXISTS project_chat (
    agent_id   TEXT    NOT NULL,
    channel    TEXT    NOT NULL,   -- project__{user}__{project_id}
    topic      TEXT    NOT NULL,
    mode       TEXT    NOT NULL CHECK (mode IN ('plan', 'build')),
    updated_at INTEGER NOT NULL,   -- Unix ms
    PRIMARY KEY (agent_id, channel, topic)
);
```

- No row ⇒ `plan` (FR-009). Rows are written only by `PUT …/mode`.
- Deleted with the topic (`delete_session`) and with the agent (`delete_agent`). Kept on project
  soft removal, so reopening restores modes.

`ProjectMode` (`schema/project.rs`): `enum { Plan, Build }`, `serde(rename_all = "snake_case")`,
`Default = Plan`.

## 3. `project_worktree` (new table)

```sql
CREATE TABLE IF NOT EXISTS project_worktree (
    id          TEXT    PRIMARY KEY,          -- nanoid(10)
    project_id  TEXT    NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    agent_id    TEXT    NOT NULL,
    owner_id    TEXT    NOT NULL,             -- user_id of the creator
    owner_name  TEXT    NOT NULL,             -- username at creation: the chat's channel user
    topic       TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    branch      TEXT    NOT NULL,
    base_ref    TEXT    NOT NULL,             -- what it started from, for "merged?"
    path        TEXT    NOT NULL,             -- canonical {root}/.vizier/{agent_id}/{name}
    created_at  INTEGER NOT NULL,             -- Unix ms
    UNIQUE (project_id, name),
    UNIQUE (project_id, topic)
);
CREATE INDEX IF NOT EXISTS idx_worktree_project ON project_worktree(project_id);
```

`Worktree` (`schema/project.rs`) mirrors the row plus derived, never stored:

| Derived | Source |
|---|---|
| `available` | `path.is_dir()` and listed by `git worktree list` |
| `changed`, `untracked` | `git status` in `path` (only on detail/confirmation and sidebar refresh) |
| `branch_merged` | `git merge-base --is-ancestor branch base_ref` |

Validation (on create, R13): name `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, not `*.lock`, no `..`;
branch passes `git check-ref-format --branch`; name unique per project; path absent on disk.

State:

```text
(create) ──► active ──(folder deleted outside)──► unavailable
               │                                      │
               └──────────(delete: chat + folder)─────┴──► (row gone)
```

There is no soft state: deleting a worktree removes the row, the folder (via git) and the chat.
Project soft removal does not touch worktrees.

## 4. `ProjectStorage` additions

```rust
async fn project_chat_mode(&self, agent_id: &str, channel: &str, topic: &str) -> Result<ProjectMode>;
async fn set_project_chat_mode(&self, agent_id: &str, channel: &str, topic: &str, mode: ProjectMode) -> Result<()>;
async fn insert_worktree(&self, worktree: &Worktree, detail: VizierSessionDetail) -> Result<()>; // one transaction
async fn worktree(&self, project_id: &str, id: &str) -> Result<Option<Worktree>>;
async fn worktree_by_topic(&self, project_id: &str, topic: &str) -> Result<Option<Worktree>>;
async fn worktrees(&self, project_id: &str) -> Result<Vec<Worktree>>;
async fn delete_worktree(&self, id: &str) -> Result<()>;
async fn project_has_main_turn(&self, agent_id: &str, project_id: &str) -> Result<bool>; // R14
```

`project_sessions` (040) returns, per row, its `mode` and `worktree` (id, name, branch) so the
sidebar needs one call.

## 5. `ProjectTurnContext` (changed)

| Field | Status | Notes |
|---|---|---|
| `project_id`, `name`, `available`, `shell_enabled`, `instructions`, `instructions_truncated` | unchanged | |
| `folder` | unchanged | the project root |
| `cwd` | **new** | worktree path when the topic has one, else `folder` |
| `worktree` | **new** | `Option<WorktreeRef { id, name, branch }>` |
| `mode` | **new** | `ProjectMode`, read once at turn start |
| `git` | **new** | `Option<GitInfo { branch: Option<String>, toplevel: PathBuf }>` — `None` when git is missing or `cwd` is not in a repository |

`available` is now "`cwd` is a directory". `shell_offered()` becomes
`available && shell_enabled && mode == Build`. `AGENTS.md`/`CLAUDE.md` are read from `cwd`.

## 6. `VizierRequest` and history (changed)

- `VizierRequest.mentions: Vec<String>` — `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
  Set only from a project chat's socket frame; ignored for any other channel.
- The request's history entry stores `metadata.mentions` (the list as sent). The resolved
  contents are never stored (FR-019).
- `VizierRequest.project_scope: Option<ProjectScope { project_id, owner, topic, mode }>` —
  `#[serde(default, skip_serializing_if = "Option::is_none")]`, set only by
  `BackgroundJobs::dispatch_piece` from the launching turn's `ProjectTurnContext`; persisted with
  the durable `session_turn` row so a resumed piece keeps it (FR-007a, R11).

## 7. Stale-read ledger (in memory)

`ProjectReadLedger` on `VizierDependencies`:

| Key | Value |
|---|---|
| `(session slug, canonical path)` | sha256 of the file's bytes when last read or written in that chat |

Bounded to 1,024 entries per session (oldest evicted); a session's entries are dropped on
`delete_session`. Not persisted (R8).

## 8. `UsageActivity` (changed)

Adds `Project` (serialised `project`). The migration sets it on rewritten rows; new rows get it
from `usage_activity()`. The usage page lists it as **Projects**.

## 9. `ToolCategory` (changed)

Adds `Coding` (name `coding`, description "Read, search and change files in the project folder,
see its git state, and run commands"), the 17th category, placed after `Shell`. Because
`ToolCategory::ALL` reserves built-in names, an MCP server keyed `coding` becomes `coding_mcp`
and a client category `coding` becomes `coding_client`. `workspace_shell` moves from `Shell` to
`Coding`.

## 10. Wire types (new DTOs, `service/projects.rs`)

| DTO | Fields |
|---|---|
| `ProjectSession` (changed) | `+ mode`, `+ worktree: Option<WorktreeSummary>` |
| `WorktreeSummary` | `id, name, branch, available` |
| `WorktreeDetail` | `WorktreeSummary + base_ref, path (display), created_at, changed, untracked, branch_merged` |
| `CreateWorktreeRequest` | `name, base, branch?` |
| `CreateWorktreeResponse` | `worktree: WorktreeSummary, topic, channel_id` |
| `SetModeRequest` | `mode` |
| `FileSuggestion` | `path, kind (file|dir)` |
| `GitSummary` | `branch, detached, ahead, behind, changed (files), available` |
| `GitChange` | `path, old_path?, kind (modified|added|deleted|renamed|untracked|conflicted), added, removed` |
| `GitFileDiff` | `path, diff (unified text), truncated, binary` |
| `GitBranch` | `name, current, checked_out_in?: {worktree_id?, name}` |
