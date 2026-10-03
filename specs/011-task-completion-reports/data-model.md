# Data Model: Task Run Results, Requester and Framing

**Feature**: `011-task-completion-reports` · **Date**: 2026-10-04

---

## `Requester` (new value type)

`src/schema/task.rs`

```rust
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Requester {
    /// Someone asked for this task, identified as the channel they reached the agent on
    /// knows them — "@dani (DiscordId: 182…)", a web username. Deliberately not an account:
    /// most people reaching an agent have none (research Decision 7).
    User(String),
    /// The agent set this task up itself, with no one asking.
    Agent(AgentId),
}
```

**Rendering for the request frontmatter**: `User(id)` → that identity; `Agent(_)` → `self`.

**Validation**: none on the payload. The enum tag is the only thing enforced, and it is enforced
by construction — an HTTP caller never supplies it (FR-028), and an agent chooses between two
variants rather than writing a string.

---

## `Task` (changed)

`src/schema/task.rs`

| Field | Change |
|---|---|
| `user: String` | **replaced** by `requester: Requester` |
| everything else | unchanged |

`is_active` keeps its meaning and gains a use: a fired one-time task is set `false` rather than
deleted (FR-001, FR-002), which is also what stops the scheduler re-picking it since
`get_task_list(None, Some(true))` already filters on it (`scheduler/mod.rs:78`).

**Persistence**: `task.data` is a JSON blob, so the field swap needs no DDL. The `is_active`
column is already real and indexed.

**Migration** (`dependencies.rs`, beside the existing one-time migrations): for every stored task,
`user: "x"` → `requester: { "user": "x" }`. Every pre-existing task becomes person-attributed;
nothing recorded today can identify an agent-initiated one, and the approximation is stated
rather than guessed (research Decision 7).

---

## `TaskRun` (new entity)

`src/schema/task.rs`

```rust
pub struct TaskRun {
    pub id: i64,                        // monotonic; the low half of the page cursor
    pub agent_id: AgentId,
    pub task_slug: String,
    pub ran_at: DateTime<Utc>,          // the run's address, and the high half of the cursor
    pub finished_at: Option<DateTime<Utc>>,
    pub session_key: String,            // the conversation this run wrote into
    pub state: TaskRunState,
}

#[derive(Serialize, Deserialize, Clone, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskRunState {
    Running,
    Answered,
    NoResponse,
    Interrupted,
}
```

**No response text.** The row points at a conversation; the response is read from
`session_history` on demand. A second copy would drift and would contradict the spec's stated
reason for removing previews (research Decision 1).

### State transitions

```
                  ┌──────────────────────────── scheduler fires
                  ▼
            ┌───────────┐
            │  running  │ ── final response with content ──▶ │ answered │
            └───────────┘ ── channel closed, no content ───▶ │ no response │
                  │       ── dispatch failed (agent gone) ──▶ │ no response │
                  │
                  └──────── process stopped, swept at next startup ──▶ │ interrupted │
```

Terminal states are terminal — nothing re-opens a closed run. *Not yet run* is the **absence** of
rows for a task, not a state (FR-005).

`Answered` means the agent produced a response, not that the news was good: a run whose response
reports a failure is `answered`. That distinction lives in the prose a person reads, by design —
this feature introduces no success/failure taxonomy.

### Invariants

- At most one `running` row per `(agent_id, task_slug)`. This is the overlap lock (FR-010,
  research Decision 5), not merely a consistency wish.
- A row exists for every firing the scheduler attempted, including one that never reached an
  agent — that is what makes "produced nothing" distinguishable from "never ran" (FR-024).
- `finished_at` is `NULL` exactly while `state = running`.
- Deleting a task deletes its runs (FR-012), which also satisfies FR-013: a new task reusing a
  freed slug finds no rows.

---

## `task_run` table

`src/storage/sqlite/mod.rs`, in the same `CREATE TABLE IF NOT EXISTS` batch as the rest

```sql
CREATE TABLE IF NOT EXISTS task_run (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id     TEXT NOT NULL,
    task_slug    TEXT NOT NULL,
    ran_at       INTEGER NOT NULL,          -- ms since epoch, as session_history stores time
    finished_at  INTEGER,
    session_key  TEXT NOT NULL,
    state        TEXT NOT NULL
);

-- the run list: newest-first within one task, which is the only ordering anything asks for
CREATE INDEX IF NOT EXISTS idx_task_run_task ON task_run(agent_id, task_slug, ran_at DESC, id DESC);
-- the startup sweep, and the overlap check
CREATE INDEX IF NOT EXISTS idx_task_run_state ON task_run(state);
```

`AUTOINCREMENT` matters: `id` must be monotonic for the cursor to be stable, and a plain
`INTEGER PRIMARY KEY` can reuse a rowid freed by a delete.

New table, so `add_column_if_missing` is not needed — but note it exists
(`storage/sqlite/mod.rs:373`) for any column added to this table later, since
`CREATE TABLE IF NOT EXISTS` does nothing to a table that already exists.

---

## `TaskRunStorage` (new trait)

`src/storage/task_run.rs`, composed into `VizierStorageProvider` and hand-forwarded by
`VizierStorage` like every other storage concern.

```rust
#[async_trait::async_trait]
pub trait TaskRunStorage {
    /// Open a run. Called before the agent starts, so an interrupted run is still accounted for.
    async fn open_task_run(&self, agent_id: AgentId, task_slug: String, ran_at: DateTime<Utc>, session_key: String) -> Result<i64>;

    /// Close it in a terminal state.
    async fn close_task_run(&self, id: i64, state: TaskRunState, finished_at: DateTime<Utc>) -> Result<()>;

    /// The overlap lock (FR-010).
    async fn running_task_run(&self, agent_id: AgentId, task_slug: String) -> Result<Option<TaskRun>>;

    /// Newest first. `before`/`before_id` continue a previous page; `limit` bounds it.
    async fn list_task_runs(&self, agent_id: AgentId, task_slug: String, before: Option<DateTime<Utc>>, before_id: Option<i64>, limit: usize) -> Result<Vec<TaskRun>>;

    async fn get_task_run(&self, agent_id: AgentId, task_slug: String, ran_at: DateTime<Utc>) -> Result<Option<TaskRun>>;

    /// Startup sweep: every `running` row becomes `interrupted` (FR-018).
    async fn interrupt_open_task_runs(&self) -> Result<usize>;

    /// Called from task deletion (FR-012).
    async fn delete_task_runs(&self, agent_id: AgentId, task_slug: String) -> Result<()>;
}
```

Adding a storage concern means implementing the trait for `SqliteStorage` and registering it in
`VizierStorageProvider` — not branching inside existing dispatch (Principle II).

---

## `VizierRequestContent::Task` → `Unattended` (renamed) — **breaking**

`src/schema/request.rs:66`

```rust
// before
Task(String),

// after — what its three construction sites actually share: a machine wrote this prompt
// and nobody is waiting on the answer. Two of those three are the dream cycle, which is
// why "Task" was the wrong name and "Scheduled" would be too.
Unattended(String),
```

No `#[serde(alias = "task")]`, by decision: this ships in a breaking release. The enum is
externally tagged, so the persisted key changes from `"task"` to `"unattended"` and legacy rows no
longer deserialize. `parse_history_row` drops an unparseable row silently
(`storage/sqlite/history.rs:32`), so the effect is quiet: the opening `Request` row of each legacy
task run and each legacy dream request disappears from history. Responses in those sessions are
unaffected (research Decision 12).

Call sites to update: `scheduler/mod.rs:175`, `scheduler/dream/mod.rs:195` and `:343`,
`process.rs:1065`, `request.rs:85`.

---

## `VizierSession::is_scheduled_task()` (new)

`src/schema/session.rs`

```rust
impl VizierSession {
    /// A scheduled task run — not a dream cycle, not an interactive turn.
    ///
    /// The question the framing needs answered, with one home. The content kind cannot answer
    /// it: the dream cycle sends `Unattended` too (research Decision 8).
    pub fn is_scheduled_task(&self) -> bool {
        matches!(self.1, VizierChannelId::Task(..))
    }
}
```

This, not the rename, is the structural guard. The rename removes the temptation to check the
content kind; this removes the opportunity.

---

## `VizierChannelId::Task` (changed rendering)

`src/schema/session.rs:62`

```rust
// before — sub-second nanos of a second-truncated time is always 0, so every run of a task
// collided into one conversation
Self::Task(id, datetime) => format!("task__{}__{}", id, datetime.timestamp_subsec_nanos()),

// after — one conversation per firing
Self::Task(id, datetime) => format!("task__{}__{}", id, datetime.timestamp_millis()),
```

The variant's shape is unchanged; only its rendering is. Legacy `task__{slug}__0` conversations
stay addressable and surface as one pre-existing run per task (research Decision 2).

**This is the change with the widest blast radius in the feature.** `to_slug()` is the storage
identity for sessions, history and session files (`storage/sqlite/{session,history,session_file}.rs`),
so it is also what makes per-run separation possible at all.

---

## Reading a run's response

Not a stored field. Given a `TaskRun`, the response is the last `SessionHistoryContent::Response`
with `VizierResponseContent::Message { content, .. }` in that run's session. `list_session_history`
already takes any `VizierSession` (`storage/history.rs`), so a `Task(slug, ran_at)` session needs
no new storage call — only the HTTP and tool layers that address it.

A run with no such entry is why `TaskRunState::NoResponse` exists: the state is decided when the
run closes, not re-derived at read time.
