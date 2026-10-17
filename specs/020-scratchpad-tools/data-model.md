# Data Model: Scratchpad Tools

**Feature**: `020-scratchpad-tools` | **Date**: 2026-10-17

## 1. Table `scratch_file`

Created by `init_scratch_schema(conn)` in `src/storage/sqlite/mod.rs`, called from `init_schema` after `init_client_call_schema`.

```sql
CREATE TABLE IF NOT EXISTS scratch_file (
    agent_id   TEXT    NOT NULL,
    channel    TEXT    NOT NULL,            -- VizierChannelId::to_slug()
    topic      TEXT    NOT NULL DEFAULT '', -- '' for a session with no topic (research R2)
    name       TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    size       INTEGER NOT NULL,            -- UTF-8 bytes of content (research R3)
    updated_at TEXT    NOT NULL,            -- RFC 3339
    PRIMARY KEY (agent_id, channel, topic, name)
);
```

There is no extra index: the primary key's prefix `(agent_id, channel, topic)` serves the per-session count, listing and clear, and `agent_id` alone serves agent deletion.

| Field | Rule |
|---|---|
| `name` | 1–128 bytes of `[A-Za-z0-9._-]`, not `.` or `..`; case-sensitive (FR-010) |
| `size` | `0 ≤ size ≤ 1 048 576` (FR-011) |
| rows per `(agent_id, channel, topic)` | ≤ 64 (FR-011) |

## 2. Lifecycle

```text
            write_tmp / append_tmp (new name, under limits)
  (absent) ───────────────────────────────────────────────▶ present
     ▲                                                        │  write_tmp: content replaced
     │                                                        │  append_tmp: content extended
     │   session idle (process loop, research R4)             │
     │   /abort · agent instance stop with empty queue        │
     │   delete_session · delete_agent · server startup       │
     └────────────────────────────────────────────────────────┘
```

A row is never updated to an over-limit size. A refused write changes nothing.

## 3. Constants (`src/storage/scratch.rs`)

| Name | Value |
|---|---|
| `MAX_SCRATCH_FILE_BYTES` | `1_048_576` |
| `MAX_SCRATCH_FILES` | `64` |
| `MAX_SCRATCH_NAME_BYTES` | `128` |

## 4. Storage trait (`src/storage/scratch.rs`)

```rust
pub enum ScratchWrite {
    Written { size: u64 },
    TooLarge { limit: u64, current: u64 },        // current = the file's size now (0 if absent)
    TooMany { limit: usize, names: Vec<String> }, // only for a new name
}

#[async_trait::async_trait]
pub trait ScratchStorage {
    async fn write_scratch(&self, session: &VizierSession, name: &str, content: &str) -> Result<ScratchWrite>;
    async fn append_scratch(&self, session: &VizierSession, name: &str, content: &str) -> Result<ScratchWrite>;
    async fn read_scratch(&self, session: &VizierSession, name: &str) -> Result<Option<String>>;
    /// Names in the session, sorted.
    async fn scratch_names(&self, session: &VizierSession) -> Result<Vec<String>>;
    /// The session's scratch files, all of them. Called when it goes idle.
    async fn clear_scratch(&self, session: &VizierSession) -> Result<usize>;
    /// Startup sweep: nothing can be running yet, so every row is a leftover.
    async fn clear_all_scratch(&self) -> Result<usize>;
}
```

`ScratchStorage` joins the `VizierStorageProvider` supertrait list. `VizierStorage` forwards each method, and `SqliteStorage` implements it in `src/storage/sqlite/scratch.rs`. `delete_session` and `delete_agent` delete rows directly on their held connection, without going through the trait (as for `client_tool_call`).

## 5. Not stored

Scratch files are never written to session history (only the `execute_python` script that calls the tools is, as today), memory, revision tables, usage or the WebUI.
