# Contract: Artifact Storage and Content

**Feature**: `035-add-artifacts`

`ArtifactStorage` is one storage concern beside `SessionFileStorage`, `MemoryStorage`, etc. It is implemented once by `SqliteStorage`, forwarded by `VizierStorage`, and composed into `VizierStorageProvider`. Content is on disk; metadata/references/events are in SQLite. The DDL is in [data-model.md](../data-model.md) §2.

---

## 1. Trait surface

```rust
#[async_trait]
pub trait ArtifactStorage {
    async fn create_artifact(&self, artifact: &Artifact, event: &ArtifactEvent) -> Result<()>;
    async fn get_artifact(&self, agent_id: &str, id: &str) -> Result<Option<Artifact>>;
    async fn get_artifact_by_name(&self, agent_id: &str, name: &str) -> Result<Option<Artifact>>;
    async fn list_artifacts(&self, filters: &ArtifactFilters) -> Result<ArtifactPage>;
    async fn artifact_stats(&self, agent_id: &str) -> Result<ArtifactStats>;
    async fn replace_artifact(
        &self,
        agent_id: &str,
        id: &str,
        expected_version: u32,
        content_file: &str,
        size: u64,
        mime_type: &str,
        kind: ArtifactKind,
        source_filename: &str,
        updated_at: DateTime<Utc>,
        event: &ArtifactEvent,
    ) -> Result<Artifact>;
    async fn delete_artifact(
        &self,
        agent_id: &str,
        id: &str,
        event: &ArtifactEvent,
    ) -> Result<()>;
    async fn list_artifact_events(
        &self,
        agent_id: &str,
        before: Option<ArtifactCursor>,
        limit: usize,
    ) -> Result<Vec<ArtifactEvent>>;
    async fn list_artifact_references(
        &self,
        agent_id: &str,
        artifact_id: &str,
    ) -> Result<Vec<ArtifactReference>>;
    async fn count_artifact_references(&self, agent_id: &str, artifact_id: &str) -> Result<usize>;
    async fn upsert_artifact_reference(&self, reference: &ArtifactReference) -> Result<()>;
    async fn delete_artifact_references(&self, agent_id: &str, artifact_id: &str) -> Result<()>;
    async fn delete_message_references_for_session(
        &self,
        agent_id: &str,
        session_slug: &str,
    ) -> Result<()>;
    async fn delete_memory_references(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<()>;
    async fn delete_agent_artifact_data(&self, agent_id: &str) -> Result<()>;

    /// Synchronous because model-part conversion (`to_part`) and `FileManager::resolve`
    /// are synchronous. One indexed read under the existing connection lock.
    fn artifact_content_file(&self, agent_id: &str, id: &str) -> Result<Option<String>>;
}
```

`create_artifact`, `replace_artifact` and `delete_artifact` insert their event in the **same transaction** as the row change; a delete also removes the artifact's references in that transaction. This is what makes the event log and the row state agree. `VizierStorage` forwards every method. `SqliteStorage::init_artifact_schema` creates the three tables from `data-model.md`.

---

## 2. Content store (`src/artifacts/paths.rs`)

```rust
pub struct ArtifactPaths { workspace: PathBuf }

impl ArtifactPaths {
    pub fn artifact_dir(&self, agent_id: &str, artifact_id: &str) -> PathBuf;
    pub fn content_path(&self, agent_id: &str, artifact_id: &str, file: &str) -> PathBuf;
    pub async fn write_version(
        &self, agent_id: &str, artifact_id: &str, version: u32, bytes: &[u8],
    ) -> Result<String>;          // returns "content.v<n>"
    pub async fn read_current(
        &self, agent_id: &str, artifact_id: &str, file: &str,
    ) -> Result<Vec<u8>>;
    pub async fn delete_artifact(&self, agent_id: &str, artifact_id: &str) -> Result<()>;
    pub async fn delete_agent(&self, agent_id: &str) -> Result<()>;
    pub fn exists(&self, agent_id: &str, artifact_id: &str, file: &str) -> bool;
}
```

**Write protocol**:

1. Validate the name/arguments and read the source bytes before touching the content directory.
2. `create_dir_all(artifacts/<agent>/<id>)`.
3. Write the bytes to `content.v<version>.tmp-<uuid>` in that directory; flush and close.
4. Rename the temp file to `content.v<version>`. Because it is a new name, this is a same-directory rename that does not replace a live file and works on Windows.
5. Insert/update the artifact row in one transaction with its event, setting `content_file = "content.v<version>"`.
6. Delete the previous `content.v<n>` (for a replacement) after the commit. A failure is logged; it is retried on the next replacement/deletion.

If step 5 fails, the new version file is removed and the old row/content stays. If the process crashes between 4 and 5, the new file is unreferenced and is removed on the artifact's next mutation.

**Deletion**:
- Artifact delete: delete rows/refs/event in SQLite, then `remove_dir_all(artifacts/<agent>/<id>)`; a failure is logged.
- Agent delete: `remove_dir_all(artifacts/<agent>)`; a failure is logged.
- No artifact content is ever placed under `uploads/`.

**Name safety**: content filenames are server-generated (`content.v<n>`), never the display name. The display name is validated and used only in metadata/download headers.

---

## 3. Sync content lookup

`SqliteStorage::artifact_content_file(agent_id, id)` locks `self.conn` and runs:

```sql
SELECT content_file FROM artifact WHERE agent_id = ?1 AND id = ?2
```

It returns `None` when the row is gone. `Artifacts` joins it with `ArtifactPaths::content_path`; `ArtifactResolver::artifact_path` returns `Some(path)` only when both the row and the file exist. This is the one sync storage method; it exists because `VizierAttachment::to_part` is synchronous and runs on the model/tool path.

---

## 4. Reference syncing

### Message references

`SqliteStorage::save_session_history`:
1. Builds the history entry and serializes it.
2. Opens one `unchecked_transaction`.
3. Inserts the history row.
4. Walks the content's attachments (`Request.attachments`, `Response.attachments`, `AudioReply`), keeps `Artifact` variants, and `INSERT OR REPLACE`s one `artifact_reference` per artifact:
   - `kind = "message"`
   - `target = format!("message:{}:{}", session.to_slug(), uid)`
   - `session_slug`, `history_uid`, `channel`, `topic`, `label`
5. Commits.

If the reference insert fails, the history insert rolls back with it. Historical rows without references (pre-feature) are not backfilled; they have no artifact attachments anyway.

### Memory references

`BundleMemoryStore::{write_memory, attach_memory_attachment, detach_memory_attachment, delete_memory, delete_bundle, import_bundle}` calls a private `sync_artifact_references(agent_id, bundle, path, title, attachments)`:
- deletes `kind='memory'` rows for `(agent, bundle, path)`;
- inserts one row per `MemoryAttachment::Artifact`, with `target = format!("memory:{bundle}:{path}")` and `label = title`.

If the document write succeeds and the reference sync fails, the failure is logged and `reconcile_agent_artifact_references(agent_id)` fixes it on the next agent start. That reconcile scans the agent's concept documents (the same source as the passage reconcile) and replaces all `memory` reference rows for the agent. Imported bundles therefore become visible in the gallery's "used by" list after the reconcile.

### Deletion

- Session deletion: `DELETE FROM artifact_reference WHERE kind='message' AND agent_id=? AND session_slug=?`.
- Memory deletion/bundle deletion: matching `kind='memory'` rows.
- Artifact deletion: all rows for the artifact.
- Agent deletion: all rows for the agent.
- Artifact rows are not cascaded into events; deletion events survive.

---

## 5. Listing, filters and cursors

`ArtifactFilters` contains `agent_id`, `q`, `kind`, `from`, `to`, `cursor`, `limit`. The SQL:
- optional `name LIKE '%' || ? || '%'`;
- optional `kind = ?`;
- optional `updated_at BETWEEN ? AND ?`;
- cursor `updated_at < ? OR (updated_at = ? AND id < ?)`;
- order `updated_at DESC, id DESC`;
- `limit + 1` to decide `next`.

`total`/`total_bytes` run the same filters without the cursor. `agent_total`/`agent_total_bytes` run without any filter. The next cursor is the last returned item's `(updated_at_ms, id)` base64url-encoded. Tests cover tied timestamps and a new artifact arriving between pages.

The activity query is the same shape over `artifact_event(created_at, id)`, with no filters beyond agent and cursor.

---

## 6. Availability and history fill

`ArtifactStorage` can return the row and `content_file`; the storage's history reads fill availability:
- After deserializing each `SessionHistory`, walk its attachments.
- For `Artifact { agent_id, artifact_id, .. }`, set `available = Some(row exists AND content_file exists)`.
- This is read-time only; `available` is skipped when `None` on write.

The gallery list/detail checks availability for the returned page items only (`ArtifactPaths::exists`), not every row in the table. A missing content file makes `available: false`; a missing row is a deletion (not listed). The history fill runs on the history read's existing connection; it must not re-lock `VizierStorage` from inside the per-entry loop (query the artifact table on the same connection, or release the lock before filling) to avoid a self-deadlock.

---

## 7. Concurrency and failure rules

- One `Artifacts` mutation lock (`tokio::sync::Mutex`) serializes promote/replace/delete; writes are rare and this avoids per-artifact lock bookkeeping.
- A replacement takes the current version from the locked row; it never trusts a client-supplied version.
- Promotion reads the source session file before creating any files; a missing/unreadable source creates nothing.
- The source session file is not moved, marked or referenced by the artifact row; clearing it cannot affect the artifact's content.
- A disk failure on the content write fails the operation; a disk failure on old-version cleanup is logged.
- A failed event insert rolls back the row change and removes the new content file.

---

## 8. Tests

- Name validation, duplicate create, case sensitivity, missing/empty source, kind derivation.
- Promote → row/event/content file; replace → version+1, new file, old file removed, event; failed replace leaves old row/file.
- Keyset list with tied `updated_at`, kind/date/search filters, `next`, totals, agent totals.
- Availability true/false/missing row.
- Reference insert on history save, session delete cleanup, memory sync/reconcile, artifact delete cleanup, agent delete cleanup.
- Content path under `artifacts/` (never `uploads/`), agent directory removal, version temp cleanup.
- Shared `file_id` reference counting via `json_extract`.
- Sync content lookup returns the current version, not a stale one.
