# Contract: Agent Tools and Memory Attachments

**Feature**: `035-add-artifacts`

All tools are reached from `execute_python` code mode through the existing `ToolRouter`; a direct model tool call is refused as before. The names below are the dispatch keys and must be added to the `every_tool_is_in_its_category` table and the telemetry display labels where applicable.

---

## 1. Artifact tools (Files category)

### `promote_artifact`

```json
{ "filename": "report.pdf", "name": "Q3-report.pdf" }
```

- `filename`: a session file in the current conversation (required).
- `name`: optional artifact name; defaults to `filename`.
- Copies the bytes into `artifacts/<agent>/<id>/content.v1`; the source stays in `uploads/` and is not marked.
- Returns the artifact (id, name, kind, size, version 1, origin).
- Refuses: invalid name, duplicate `(agent, name)`, missing/unreadable/empty source, a source outside the current conversation, or the dream cycle (not offered).
- A failed attempt creates nothing.

### `update_artifact`

```json
{ "name": "Q3-report.pdf", "filename": "report-v2.pdf" }
```

- `name`: an existing artifact owned by the calling agent (required).
- `filename`: a session file in the current conversation (required).
- Writes `content.v<n+1>`, switches the row/event after the write, then removes the old version file.
- Keeps the artifact name and identity; increments the version; returns the updated artifact.
- Refuses a missing artifact, a missing/unreadable source, or another agent's artifact; a failed attempt leaves the old content/version intact.

### `list_artifacts`

```json
{ "kind": "pdf" }
```

- Optional `kind` filter.
- Returns the calling agent's artifacts with name, kind, MIME, size, version and last change. No pagination in the tool (the gallery has it); the list is bounded by the agent's workspace.

### `load_artifact_as_session_file`

```json
{ "name": "Q3-report.pdf" }
```

- Reads the artifact's current bytes, uploads them through the normal `FileManager::upload` path to a new `uploads/<file_id>/`, and saves a session file in the current conversation under a safe, server-derived name (`artifact-<artifact-id><ext>`, where `ext` comes from the artifact name/MIME). Returns the session filename, artifact name, MIME and size. Existing read tools (`read_document_file`, `read_image_file`) then work.
- This creates a transient copy; it does not change the artifact or create an attachment.
- Refuses a missing/deleted artifact.

### `send_attachment` (changed)

```json
{ "artifact": "Q3-report.pdf" }
```

- Resolves one artifact owned by the calling agent and queues a `VizierAttachment` whose content is `VizierAttachmentContent::Artifact { agent_id, artifact_id, available: None }` and whose `filename` is the artifact name.
- Refuses a session-file name with: "`<name>` is a session file, not an artifact. Promote it with `promote_artifact` first."
- Refuses a missing/deleted artifact, another agent's artifact, and a dream turn (not offered).
- The queued attachment is part of the reply; history stores the identity, so replacement/deletion are reflected later.

**Registration**: all four artifact tools and `send_attachment` are in the `Files` category on `default_toolset` (always on). None is in `DREAM_TOOL_NAMES`.

---

## 2. Memory tools (Memory category)

### `attach_artifact_to_memory`

```json
{ "bundle": "default", "path": "q3-planning", "artifact": "Q3-report.pdf" }
```

- `bundle` defaults to `default`; `path` is the concept path (no `.md`).
- `artifact`: one artifact name or stable identity owned by the calling agent.
- Requires the memory to exist (write it with `memory_write` first).
- Stores `MemoryAttachment::Artifact { artifact_id, name }`, updates `attachment_count`, the memory node, a revision and the reference row.
- Idempotent: attaching the same artifact again is a no-op returning the current list.
- Returns the memory's updated attachment list (display names).
- Refuses a session-file name, arbitrary upload or external URL with guidance to promote; refuses a missing memory or another agent's artifact.
- Not offered to the dream cycle.

### `detach_attachment_from_memory`

```json
{ "bundle": "default", "path": "q3-planning", "name": "Q3-report.pdf" }
```

- Removes one attachment by display name. It removes an artifact reference or a legacy session-file attachment.
- Idempotent-ish: a name that is not attached changes nothing and returns a clear result.
- Returns the updated list.
- Not offered to the dream cycle.

### `memory_write` (changed)

- The `attachments` argument is removed from its schema.
- An existing memory's attachments are read from its frontmatter and preserved on content update; a new memory starts with none.
- Its description points to `attach_artifact_to_memory`.
- `MemoryWriteArgs` uses `#[serde(deny_unknown_fields)]`: a script that still passes an `attachments` key gets a deserialization error, not a silent ignore. The tool description and the attach tool give the migration path.

### `memory_read` (changed)

- The output lists attachment display names from `MemoryAttachment` (artifact name or legacy filename).
- Artifact attachments are read from `artifacts/` and copied into the current conversation as session files under the same safe server-derived name used by `load_artifact_as_session_file`; the output's "added to session files" note lists those safe names so the agent can read them. The `attachments` metadata lists the display names.
- A missing/deleted artifact is reported by name as deleted; the rest of the memory still returns.
- Legacy session-file attachments keep the old copy-by-`file_id` path while their file exists.

---

## 3. Memory storage shape

```rust
#[serde(untagged)]
pub enum MemoryAttachment {
    Artifact { artifact_id: String, name: String },
    LegacySessionFile(VizierAttachment),
}
```

- `Memory.attachments` and generated `MemoryFrontMatter` use `Vec<MemoryAttachment>`.
- `RevisionFrontMatter.attachments` uses the same type; old revision snapshots parse as legacy.
- `attachment_count` is the list length.
- New attachments are written only as `Artifact`.
- Legacy entries are read-only; editing the memory does not migrate them. They can be detached by name.

---

## 4. Memory storage methods

Add to `MemoryStorage` (implemented by `SqliteStorage` via `BundleMemoryStore`):

```rust
async fn attach_memory_attachment(
    &self, agent_id: String, bundle: String, path: String,
    artifact_id: String, artifact_name: String, origin: &RevisionOrigin,
) -> Result<Memory>;

async fn detach_memory_attachment(
    &self, agent_id: String, bundle: String, path: String,
    name: String, origin: &RevisionOrigin,
) -> Result<Memory>;

async fn reconcile_agent_artifact_references(&self, agent_id: &str) -> Result<usize>;
```

`write_memory` loses its `attachments: Vec<VizierAttachment>` argument. It reads existing `MemoryAttachment`s and preserves them. `import_bundle` parses the document's existing `MemoryAttachment`s; the legacy filesystem migration's `write_migrated_memory` converts its `Vec<VizierAttachment>` to `MemoryAttachment::LegacySessionFile` before writing. Both sync references.

Attach/detach:
- rewrite the frontmatter, `updated_at`, `attachment_count`;
- upsert the memory node count;
- record one `memory_revision` with `canonical_new` (title/tags/attachments/body) using the supplied `RevisionOrigin`;
- sync the `artifact_reference` rows;
- do **not** re-chunk, re-embed or change the body.

---

## 5. HTTP memory attachment endpoints

All under `/api/v1/agents/{agent_id}/memory`, behind `require_auth` and the existing `require_agent` (agent visibility):

### Attach

```http
POST /doc/{bundle}/{*path}/attachments
Content-Type: application/json

{ "artifact": "Q3-report.pdf" }
```

- Resolves the artifact by name for this agent; `404` if missing.
- Sends `MemoryOpRequest::AttachArtifact` through the agent's memory-op channel (the same path as memory writes).
- Response `200` with the memory detail (or `{ attachments: [...] }`); the UI/agent sees the updated list.
- `404` memory or artifact; `403` visibility; `422` invalid name.

### Detach

```http
DELETE /doc/{bundle}/{*path}/attachments?name=Q3-report.pdf
```

- Sends `MemoryOpRequest::DetachAttachment`; removes one attachment by display name (artifact or legacy).
- Response `200` with the memory detail; `404` memory or name not attached; `403` visibility.

### Create/update legacy guard

`POST /` and `PUT /doc/{bundle}/{*path}` do **not** accept an attachments list. A request that still sends a non-empty `attachments` field is refused with:

```json
{ "status": 422, "message": "memory attachments must be artifacts; use POST …/attachments" }
```

The field is kept only as a compatibility guard so old clients get a clear error instead of silent data loss; it is tracked in #82.

### Memory detail response

`attachments` is `Vec<MemoryAttachmentView>`, tagged by `kind`:
- artifact: `{ "kind": "artifact", "artifact_id": "...", "name": "Q3-report.pdf", "mime_type": "application/pdf", "size": 184320, "available": true }`
- legacy: `{ "kind": "legacy_session_file", "filename": "...", "content": { "local": "/api/v1/files/..." }, "available": true }`

The view is built from the stored `MemoryAttachment`; `available` is read-time only. `memory_read` reports a missing artifact by name. The stored document remains the two-variant enum with no `kind` field.

---

## 6. Message attachment shape

`VizierAttachmentContent` gains:

```json
{
  "filename": "Q3-report.pdf",
  "content": {
    "artifact": {
      "agent_id": "dp-a",
      "artifact_id": "550e8400-e29b-41d4-a716-446655440000",
      "available": true
    }
  }
}
```

- `available` is omitted on write and filled on history reads; `false` means the artifact row or content file is gone.
- A person's request attachments are session-file uploads. The HTTP/WebSocket request path rejects a client-supplied `Artifact` content (or validates its `agent_id` against the conversation's agent and the caller's access) before resolving it; only the agent/tool response path creates artifact attachments. This prevents a request to agent A from reading agent B's artifact by id.
- `VizierAttachment::to_part` resolves the current content through `AttachmentResolver`; a missing artifact becomes a readable note, not a failed turn.
- `FileManager::resolve` resolves it for Discord/Telegram delivery.
- The WebUI builds `/api/v1/agents/{agent_id}/artifacts/{artifact_id}/content` (with the auth token) and shows a tombstone on `available: false` or a 404.
- Existing `Local`/`Url`/`Bytes`/`Base64` attachments are untouched legacy data.

---

## 7. Dream cycle and scheduling

- `promote_artifact`, `update_artifact`, `list_artifacts`, `load_artifact_as_session_file`, `send_attachment`, `attach_artifact_to_memory` and `detach_attachment_from_memory` are not in `DREAM_TOOL_NAMES`.
- Ordinary, scheduled-task and background turns get the artifact tools and may promote/attach; the tools use the turn's `ToolContext.session` to scope promotion to the current conversation.
- The dream cycle can still read memory text and see attachment counts; it cannot create or change artifact attachments.

---

## 8. Tool error rules

- Invalid input, a missing memory/artifact, a cross-agent name and a deleted artifact return readable errors (`crate::Result`/`VizierError`), never a silent success.
- `send_attachment` and `load_artifact_as_session_file` do not mutate the artifact.
- `promote_artifact`/`update_artifact` create a row/event only after the content file is written; a failed copy creates none.
- No agent tool deletes an artifact.
