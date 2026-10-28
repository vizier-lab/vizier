# Contract: Session Files, Clear and Deletion

**Feature**: `035-add-artifacts`

Session files remain the transient tier. This contract covers the one clear service shared by the WebUI, Discord, Telegram, conversation deletion and agent deletion. There is **no automatic retention or garbage collector** in this release.

---

## 1. `SessionFiles` service (`src/session_files/mod.rs`)

```rust
pub struct SessionFiles {
    storage: Arc<VizierStorage>,
    file_manager: FileManager,
    workspace: PathBuf,
}

pub struct ClearResult { pub removed: usize, pub bytes: u64 }

pub enum ClearError {
    AgentNotFound,
    TurnActive,
    Storage(String),
}

impl SessionFiles {
    pub async fn count(&self, session: &VizierSession) -> Result<usize>;
    pub async fn clear(&self, session: &VizierSession) -> Result<ClearResult, ClearError>;
    pub async fn delete_conversation(&self, session: &VizierSession) -> Result<ClearResult, ClearError>;
    pub async fn files_for_agent(&self, agent_id: &str) -> Result<Vec<SessionFileRecord>>;
    pub async fn delete_blobs(&self, files: &[SessionFileRecord]) -> usize; // best effort
    pub async fn has_active_turn(&self, session: &VizierSession) -> bool;
}
```

`has_active_turn` calls `TurnStorage::pending_turns(agent_id)` and matches the session. A running or queued turn refuses the clear/delete.

`clear`:
1. Refuse if `has_active_turn`.
2. `storage.list_session_files(session)`.
3. Delete the session's `session_file` records (the existing per-file delete or a batch helper).
4. For each distinct `file_id`, if `storage.session_file_reference_count(file_id) == 0`, call `FileManager::delete_upload(file_id)` (`remove_dir_all(uploads/<file_id>)`).
5. Return the count and the sum of record sizes.
6. Failures in step 4 are logged and do not roll back the records; the file is already unreachable from the session and a future operation can retry.

`delete_conversation`:
1. Refuse if active turn.
2. Call `clear` for the session's files.
3. Call `storage.delete_session` (which also deletes `session_file` rows and `artifact_reference` rows for the session as a second line of defence).
4. Return the clear result.

`delete_blobs` for agent deletion:
- takes the records collected before `storage.delete_agent`;
- after the storage delete, checks each `file_id`'s remaining reference count (zero because the agent's rows are gone, unless another agent shares the blob) and removes the upload directory.

---

## 2. Shared blob rule

A `file_id` can be referenced by more than one `session_file` row (a memory read can re-save the same upload into another conversation; the normal upload path creates a fresh id). A blob is deleted only when:

```sql
SELECT COUNT(*) FROM session_file
WHERE json_extract(data, '$.file_id') = ?1
```

returns `0`. The session-file record(s) for the conversation are removed first, so the count is the remaining references. The last conversation to clear/delete removes the blob. This is the only reference-counting in the session-file tier; artifact content never shares an upload because promotion copies.

---

## 3. WebUI side panel

- The chat details panel (`SessionFilesPanel`) lists only the current conversation's session files; it does not show the agent's artifact library (that is the gallery).
- **Promote** on each row opens a small name dialog (default: the file's name) and calls `POST …/files/promote`; on success, show a toast; the source stays in the list (copy, no marker).
- **Clear session files** is a panel action with a `ConfirmDialog`:
  - title: `Clear session files?`
  - body: `This removes N files from this conversation. Artifacts are unaffected. Older messages or memories that reference these files will no longer open them.`
  - confirm calls `DELETE …/files`.
  - on success, report the removed count and dispatch the chat store's `files` action with `[]`.
  - disabled/hidden while the turn is running (`state.phase !== 'idle'`); the server refuses anyway.
- There is no "artifact" marker on a promoted source; the gallery is the permanent view.

---

## 4. HTTP route

```http
DELETE /api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/files
```

- Visibility: same as `GET …/files` (`user_can_view_agent` and the existing `show_<platform>_chats` rule for Discord/Telegram).
- `409` with `"a turn is running or queued in this conversation; wait for it to finish"` when active.
- `200 { "removed": n, "bytes": m }`.
- The client confirmation is not a server token; the `DELETE` is the confirmed action.

`POST …/files/promote` is documented in [artifacts-api.md](./artifacts-api.md) §6. It reads the conversation's session file; it does not clear or mark it.

---

## 5. Channel command

Canonical: `/clear-files` (Discord); Telegram registers `/clear_files` because hyphens are not valid in its command names. Both use the same two-step flow:

1. **Request**: `/clear-files` (or `/clear_files`) — count the current conversation's session files. If zero, reply "There are no session files to clear in this conversation." If a turn is active, reply with the refusal. Otherwise reply:
   > `This will remove N session files from this conversation. Artifacts are unaffected. Older messages or memories that reference them will no longer open. Send /clear-files confirm within 60 seconds to continue.`
   Store a pending confirmation for the session in memory.
2. **Confirm**: `/clear-files confirm` (Telegram `/clear_files confirm`) — consume the pending token (if absent/expired: "No clear is waiting; send /clear-files first."). Re-check the active turn, call `SessionFiles::clear`, then reply with the count:
   > `Cleared N session files. Artifacts are untouched.`
3. **Cancel**: any other message or a later request replaces the pending token; the token expires after 60 seconds. No files are removed.
4. The command is a builtin, so it wins a name clash with a skill named `clear-files`/`clear_files`; the skill is still reachable through `/skill`.
5. Anyone who can participate in the conversation can run it. The reply states that artifacts are unaffected.

`channels/commands.rs` owns `ClearConfirmations`:

```rust
pub struct ClearConfirmations { /* Mutex<HashMap<String, Instant>> */ }
impl ClearConfirmations {
    pub fn request(&self, session: &VizierSession) -> Instant;
    pub fn take(&self, session: &VizierSession) -> bool; // true only within TTL
    pub fn cancel(&self, session: &VizierSession);
}
```

Both readers hold one instance (or one shared instance on `VizierDependencies`); tests cover TTL, cancel and a confirm without a request.

---

## 6. Conversation deletion

```http
DELETE /api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}
```

- Refused while a turn is running or queued (`409`).
- Calls `SessionFiles::delete_conversation`: clears the conversation's session files and unshared blobs, then `storage.delete_session`.
- The storage delete removes the session detail, background jobs/pieces, client calls, scratch files, pending turns, session-file rows and message `artifact_reference` rows.
- Artifacts promoted from the conversation remain; the artifact's origin metadata still records the channel/topic. A legacy memory referencing a removed session file shows it unavailable.
- If the metadata delete fails after the files were cleared, the conversation remains without files; the next retry completes the deletion. This is logged.

---

## 7. Agent deletion

`VizierAgents::handle_delete`:
1. Collect `session_files.files_for_agent(agent_id)` before the storage delete.
2. `storage.delete_agent` — deletes artifact rows/events/references, session-file rows and the existing agent tables; no blob deletion in SQLite.
3. Delete the collected upload blobs that have no remaining references (best effort, log failures).
4. `Artifacts::delete_agent_files(agent_id)` — `remove_dir_all(artifacts/<agent_id>)` (best effort, log failures).
5. Existing behavior continues: stop integrations/processes, unregister channels, remove the agent workspace if requested.

This leaves no artifact content, events, references or session-file records for the deleted agent. The artifact directory is removed regardless of the `delete_workspace` flag (artifacts are agent data, not the optional agent workspace).

---

## 8. What clear does not do

- It does not delete artifacts.
- It does not delete another conversation's files.
- It does not delete a blob still referenced by another session-file row.
- It does not run on a timer or at startup.
- It does not rewrite legacy message/memory references; they render unavailable after the file is gone.
- It does not delete a conversation's history rows in this release; message reference rows are removed and the conversation metadata is deleted.

---

## 9. Tests

- Count/clear/zero files; count and bytes.
- Active turn (queued and running) refuses; a finished turn allows.
- Two conversations sharing a `file_id`: clearing one keeps the blob; clearing the last removes it.
- Conversation deletion removes records/blobs and leaves artifacts.
- Agent deletion removes session-file records/blobs and the artifact directory, including a shared blob still referenced by another agent (kept).
- Clear endpoint permission (view + platform chat), missing agent/file.
- Channel confirmation: request, confirm, expiry, cancel, confirm-without-request, builtin name clash.
- No automatic deletion: no scheduler/startup code removes session files.
