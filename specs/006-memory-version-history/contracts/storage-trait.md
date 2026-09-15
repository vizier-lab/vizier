# Contract: storage & transport changes

## No new storage trait — history methods live next to the documents they version

CORE history is part of `AgentStorage` (where `get_agent_core`/`set_agent_core` already live); memory history is part of `MemoryStorage` (where `write_memory`/`delete_memory` already live). Each has its own sqlite module and its own SQL. The only code shared between them is the kind-agnostic value types (`RevisionOrigin`, `RevisionActor`, `RevisionTrigger`, `RevisionDiff`, `DiffHunk`, `DiffLine`, `RollbackResponse`) and the pure `diff_lines` function in `src/storage/diff.rs`.

### `src/storage/agent.rs` — `AgentStorage` additions

```rust
/// Newest-first. Lazily inserts the baseline revision when a CORE exists but has no
/// history (Decision 5). `limit` clamped to 1..=200.
async fn list_core_revisions(&self, agent_id: &str, offset: usize, limit: usize)
    -> Result<PaginatedCoreRevisions>;
async fn get_core_revision(&self, agent_id: &str, seq: i64) -> Result<Option<CoreRevision>>;
/// `from == None` ⇒ `to - 1`. Errors if either seq is unknown.
async fn diff_core_revisions(&self, agent_id: &str, from: Option<i64>, to: i64) -> Result<RevisionDiff>;
/// Loads revision `seq` and re-saves it through `set_agent_core` with
/// `origin.trigger = Rollback { restored_from: seq }`. `no_change` when identical to current.
async fn rollback_core(&self, agent_id: &str, seq: i64, origin: &RevisionOrigin) -> Result<RollbackResponse>;
```

### `src/storage/memory.rs` — `MemoryStorage` additions

```rust
async fn list_memory_revisions(&self, agent_id: String, bundle: Option<String>, path: String,
                               offset: usize, limit: usize) -> Result<PaginatedMemoryRevisions>;
async fn get_memory_revision(&self, agent_id: String, bundle: Option<String>, path: String, seq: i64)
    -> Result<Option<MemoryRevision>>;
async fn diff_memory_revisions(&self, agent_id: String, bundle: Option<String>, path: String,
                               from: Option<i64>, to: i64) -> Result<RevisionDiff>;
/// Loads revision `seq`, parses the canonical text, re-saves via `write_memory` with
/// `origin.trigger = Rollback { restored_from: seq }`. Err if seq unknown or is a deletion entry.
async fn rollback_memory(&self, agent_id: String, bundle: Option<String>, path: String, seq: i64,
                         origin: &RevisionOrigin, indexer: &VizierIndexer) -> Result<RollbackResponse>;
```

Both sets are hand-forwarded on `VizierStorage` in `src/storage/mod.rs` like every other method.

### Sqlite — `src/storage/sqlite/core_revision.rs` (CORE only)

Synchronous helpers over `&rusqlite::Connection`, called from `SqliteStorage::set_agent_core` inside its transaction and from the `AgentStorage` history impls:

```rust
pub(crate) fn ensure_baseline(conn, agent_id, current: Option<&str>) -> Result<()>;
/// Inserts the next revision. Ok(None) when `content` equals the latest entry (no-op).
pub(crate) fn record(conn, agent_id, content: &str, origin: &RevisionOrigin,
                     current_before_save: Option<&str>) -> Result<Option<i64 /*seq*/>>;
pub(crate) fn latest(conn, agent_id) -> Result<Option<CoreRevisionRow>>;
pub(crate) fn get(conn, agent_id, seq) -> Result<Option<CoreRevisionRow>>;
pub(crate) fn list(conn, agent_id, offset, limit) -> Result<(Vec<CoreRevisionRow>, usize)>;
pub(crate) fn delete_agent(conn, agent_id) -> Result<()>;
```

```sql
CREATE TABLE IF NOT EXISTS core_revision (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    content TEXT NOT NULL,
    actor_kind TEXT NOT NULL,
    actor_id TEXT,
    actor_name TEXT,
    trigger TEXT NOT NULL,
    restored_from INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE(agent_id, seq)
);
CREATE INDEX IF NOT EXISTS idx_core_rev_doc ON core_revision(agent_id, seq DESC);
```

### Sqlite — `src/storage/sqlite/memory_revision.rs` (memory only)

Called from `BundleMemoryStore` (which already holds the `Arc<Mutex<Connection>>`) inside `write_memory` / `delete_memory` / `delete_bundle` / `import_bundle` / `write_migrated_memory`, and from the `MemoryStorage` history impls:

```rust
pub(crate) fn ensure_baseline(conn, agent_id, bundle, path, current: Option<&str>) -> Result<()>;
/// `content = None` records a deletion entry. Ok(None) when `(content, deleted)` equals the latest (no-op).
pub(crate) fn record(conn, agent_id, bundle, path, content: Option<&str>, origin: &RevisionOrigin,
                     current_before_save: Option<&str>) -> Result<Option<i64 /*seq*/>>;
pub(crate) fn latest(conn, agent_id, bundle, path) -> Result<Option<MemoryRevisionRow>>;
pub(crate) fn get(conn, agent_id, bundle, path, seq) -> Result<Option<MemoryRevisionRow>>;
pub(crate) fn list(conn, agent_id, bundle, path, offset, limit) -> Result<(Vec<MemoryRevisionRow>, usize)>;
pub(crate) fn delete_agent(conn, agent_id) -> Result<()>;
```

Also in this module: `memory_canonical(title, tags, attachments, body) -> Result<String>` and `parse_memory_canonical(text) -> Result<(RevisionFrontMatter, String)>` (memory-specific snapshot format, Decision 3).

```sql
CREATE TABLE IF NOT EXISTS memory_revision (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL,
    bundle TEXT NOT NULL,
    path TEXT NOT NULL,
    seq INTEGER NOT NULL,
    content TEXT,
    deleted INTEGER NOT NULL DEFAULT 0,
    actor_kind TEXT NOT NULL,
    actor_id TEXT,
    actor_name TEXT,
    trigger TEXT NOT NULL,
    restored_from INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE(agent_id, bundle, path, seq)
);
CREATE INDEX IF NOT EXISTS idx_mem_rev_doc ON memory_revision(agent_id, bundle, path, seq DESC);
CREATE INDEX IF NOT EXISTS idx_mem_rev_agent ON memory_revision(agent_id);
```

Both `CREATE TABLE` blocks are added to `SqliteStorage::init_schema` (`src/storage/sqlite/mod.rs`). In each module, `record` calls `ensure_baseline(current_before_save)` first, then compares against `latest`, then inserts `max(seq)+1`.

### Shared — `src/storage/diff.rs`

`pub fn diff_lines(from: &str, to: &str) -> RevisionDiff`-shaped output (hunks + add/delete counts) using `similar`. Kind-agnostic; used by both `diff_core_revisions` and `diff_memory_revisions`.

## Modified trait signatures

`src/storage/agent.rs` — `AgentStorage`:
```rust
async fn set_agent_core(&self, agent_id: &str, core: &str, origin: &RevisionOrigin) -> Result<()>;
```
The default impl (config-based) is kept for signature compatibility but `SqliteStorage` overrides it: upsert `agent_core` + `core_revision::record` in one transaction.

`src/storage/memory.rs` — `MemoryStorage`:
```rust
async fn write_memory(&self, agent_id, bundle, path, create_only, title, content, tags, attachments,
                      origin: &RevisionOrigin, indexer) -> Result<Memory>;
async fn delete_memory(&self, agent_id, bundle, path, origin: &RevisionOrigin, indexer) -> Result<()>;
async fn delete_bundle(&self, agent_id, bundle, force, origin: &RevisionOrigin, indexer) -> Result<()>;
async fn import_bundle(&self, agent_id, bundle, zip_bytes, origin: &RevisionOrigin, indexer) -> Result<ImportReport>;
```
`write_migrated_memory` (migration-only, on `BundleMemoryStore`) records with `origin = system/baseline`.

**Where `memory_revision::record` is called inside `BundleMemoryStore`:**
| Method | `content` | `current_before_save` |
|--------|-----------|------------------------|
| `write_memory` | `Some(memory_canonical(new))` | canonical of `existing` (if any) |
| `delete_memory` | `None` (deleted=1) | canonical of existing |
| `delete_bundle(force)` | `None` per concept | canonical of each |
| `import_bundle` | `Some(canonical(imported))` per concept | `None` (import skips existing paths) |
| `rollback_memory` | via `write_memory` | — |

## Transport — `src/schema/commands.rs`

```rust
pub enum MemoryOpRequest {
    Write { .., origin: RevisionOrigin },
    Delete { .., origin: RevisionOrigin },
    DeleteBundle { .., origin: RevisionOrigin },
    ImportBundle { .., origin: RevisionOrigin },
    // NEW
    Rollback { bundle: Option<String>, path: String, seq: i64, origin: RevisionOrigin },
    ListRevisions { bundle: Option<String>, path: String, offset: usize, limit: usize },
    GetRevision { bundle: Option<String>, path: String, seq: i64 },
    DiffRevisions { bundle: Option<String>, path: String, from: Option<i64>, to: i64 },
    ..existing read variants unchanged..
}
pub enum MemoryOpResponse { .., Rollback(RollbackResponse), Revisions(PaginatedMemoryRevisions),
                            Revision(Option<MemoryRevision>), Diff(RevisionDiff) }
```
Dispatched in `src/agents/memory_ops.rs::dispatch_memory_op`.

## Schema — `src/schema/revision.rs` (new)

Shared: `RevisionActor`, `RevisionTrigger`, `RevisionOrigin` (+ `from_session`, `from_user`, `system`, `with_trigger`), `RevisionDiff`, `DiffHunk`, `DiffLine`, `RollbackResponse`. CORE: `CoreRevisionSummary`, `CoreRevision`, `PaginatedCoreRevisions`. Memory: `MemoryRevisionSummary`, `MemoryRevision`, `PaginatedMemoryRevisions`. All `Serialize + Deserialize + Clone + Debug`, API-facing ones also `utoipa::ToSchema`. See `data-model.md`.

## Auth — `src/channels/http/auth/`

`AuthenticatedUser` gains `pub auth_method: AuthMethod` (`Jwt | ApiKey`), set in `middleware.rs`'s two existing branches. No behavior change elsewhere.

## Call-site updates (all mechanical)

| File:line (today) | Change |
|-------------------|--------|
| `src/agents/tools/workspace/mod.rs:49` (`WriteCore`) | `set_agent_core(.., &RevisionOrigin::from_session(&ctx.session))` |
| `src/agents/tools/vector_memory/mod.rs:351/818/865` | pass `&RevisionOrigin::from_session(&ctx.session)` |
| `src/agents/memory_ops.rs:39/85/93/101` | forward `origin` from the request |
| `src/agents/mod.rs:217` (create seeds CORE) | `RevisionOrigin::system(Baseline)` |
| `src/dependencies.rs:426/745` + `write_migrated_memory` callers | `RevisionOrigin::system(Baseline)` |
| `src/channels/http/api/v1/agents/core.rs:105` | `from_user(&user)` |
| `src/channels/http/api/v1/agents/memory.rs` create/update/delete/delete_bundle/import | `from_user(&user)` (+ `.with_trigger(Import)` for import) |
| `src/storage/sqlite/agent.rs::delete_agent` | also `core_revision::delete_agent(conn, agent_id)` and `memory_revision::delete_agent(conn, agent_id)` |
