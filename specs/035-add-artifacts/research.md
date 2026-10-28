# Research: Permanent Artifacts

**Feature**: `035-add-artifacts` | **Date**: 2026-10-29

This records the decisions behind [plan.md](./plan.md). Each decision is grounded in the existing code and the spec's constraints: copy-on-promotion, `artifacts/` separate from `uploads/`, no automatic GC, artifact-only message and memory attachments, and legacy session-file references kept readable.

---

## R1 — Artifact metadata in SQLite, content in a versioned file on disk

**Decision**: One `artifact` row holds the identity, owning agent, display name, kind, MIME type, size, current version, the name of the current content file, origin conversation/source filename, creator and timestamps. The bytes live at:

```
<workspace>/artifacts/<agent_id>/<artifact_id>/content.v<version>
```

A promotion writes `content.v1`; a replacement writes `content.v2` (or the next version) as a new file, switches the row to it in a transaction with the event, then deletes the old version file. No live file is ever overwritten.

**Rationale**: Metadata queries (name search, kind/date filters, references, events) belong in SQLite. Content files can be large and are never served through SQLite. Version-named files make replacement atomic from a reader's point of view: the row determines which file is current, and an old file stays intact until the switch commits. They also avoid `rename`-over-existing differences on Windows and keep the artifact directory self-contained. The display name is not used as the on-disk path, so Unicode/Windows-reserved characters and name changes cannot become filesystem paths.

**Alternatives considered**:
- **Bytes in SQLite**: simplest transactions, but bloats the database, makes downloads copy the whole blob, and conflicts with the existing large-file uploads.
- **A stable `content` file replaced in place**: less metadata, but atomic replacement is platform-dependent, and a crash or failed rename can leave a partial or missing file.
- **Store under `uploads/` with an artifact flag**: explicitly rejected by the spec and would make clearing/two-tier separation ambiguous.
- **Use the display name as the file name**: user-facing names are not safe filenames on every platform and would require a separate sanitizer.

---

## R2 — Message attachments carry artifact identity, not bytes or a path

**Decision**: Extend `VizierAttachmentContent` with:

```rust
Artifact {
    agent_id: AgentId,
    artifact_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    available: Option<bool>,
}
```

`VizierAttachment.filename` remains the artifact's display name. History stores the identity; `available` is skipped when `None` on write and filled by history reads. `send_attachment` creates this variant. The WebUI builds the content URL from `agent_id` + `artifact_id`; a 404 or `available: false` renders a named tombstone.

**Rationale**: A copied URL or a snapshot would stop following replacement, the exact behaviour the spec forbids. Identity keeps every reference on the current version and lets the UI tell a deleted artifact from a missing session file. `available` is a read-time enrichment, not stored state, so replacement/deletion do not rewrite history.

**Alternatives considered**:
- **Store a `Local("/api/v1/artifacts/<id>")` URL**: looks like a session file to the existing code, cannot be distinguished for tombstones, and hides the identity from the model/UI.
- **Copy bytes into history**: duplicates storage and freezes the attachment at the old version.
- **Add a separate `artifact_id` field on `VizierAttachment`**: looser (a message could carry both a session file and an artifact) and requires every existing attachment consumer to branch on two content dimensions.

---

## R3 — One `AttachmentResolver` for model parts, channels and the UI

**Decision**: Add a small synchronous trait, implemented by `Artifacts`:

```rust
pub trait AttachmentResolver: Send + Sync {
    fn artifact_path(&self, agent_id: &str, artifact_id: &str) -> Result<Option<PathBuf>>;
}
```

`VizierAttachment::to_part` and `VizierResponse::to_tool_result` take `&dyn AttachmentResolver`; `VizierRequest::to_message` does too. The agent passes its `Artifacts` handle. `FileManager` holds an `Option<Arc<Artifacts>>` (set by `VizierDependencies` after construction) and uses the same resolver in `resolve`. `Artifacts` queries the current `content_file` through a synchronous `ArtifactStorage::artifact_content_file` lookup and returns `artifacts/<agent>/<id>/<content_file>`.

**Rationale**: The stored attachment variant has only identity; the resolver is the one place that turns it into bytes. The synchronous lookup is safe because the SQLite connection is a `parking_lot::Mutex<Connection>` and the read is a single indexed query; it avoids making `to_part`/`from_message` async. `FileManager` and `Artifacts` cannot own each other during construction, so `Artifacts` reads uploads through a free `read_upload(workspace, file_id)` helper and `FileManager` gets the resolver installed afterwards. All clones of `FileManager` share one inner handle, so the install is visible everywhere.

**Alternatives considered**:
- **Store `content_file` in the attachment**: becomes stale after a replacement; history would need rewriting.
- **A stable `current` path**: no resolver needed, but reintroduces in-place replacement and platform-specific atomicity (see R1).
- **Make `to_part` async and pass `Artifacts`**: touches many sync call sites for a trivial lookup; the SQLite read is already sync.
- **Let `Artifacts` own a `FileManager` and `FileManager` own `Artifacts`**: circular construction; the free upload reader breaks the cycle.

---

## R4 — A derived `artifact_reference` index for "used by" and delete counts

**Decision**: Add `artifact_reference` with one row per `(artifact_id, kind, target)`:
- message: `target = message:<session_slug>:<history_uid>`, plus `agent_id`, `session_slug`, `history_uid`, `channel`, `topic`, `label`;
- memory: `target = memory:<bundle>:<path>`, plus `agent_id`, `bundle`, `path`, `label` (the memory title).

Message rows are inserted in the same SQLite transaction as the history row by walking the saved `SessionHistoryContent` attachments. Memory rows are synced by the memory bundle write/attach/detach/delete paths and reconciled from frontmatter when an agent starts. Session deletion removes that session's message refs; memory deletion/detach removes its refs; artifact deletion removes all its refs; agent deletion removes all of them.

**Rationale**: The gallery must show where an artifact is used (FR-026) and the delete dialog must state how many references use it (FR-027). Scanning `session_history.data` JSON and every memory document on each detail view is slower and would need a second implementation of memory parsing. A derived table is cheap to write, indexed for the two reads, and can be rebuilt from its sources. It does not affect reference integrity: a message still holds the artifact id and shows an unavailable attachment, while a memory link is removed on artifact deletion. Event actor names resolve through one shared `utils` helper, the same display-name rule notifications use.

**Alternatives considered**:
- **Scan history JSON at read time**: works for messages but is O(history) per detail view and hard to index.
- **Scan memory documents at read time**: correct but adds file I/O and YAML parsing to every gallery detail.
- **Store references only in the artifact row**: loses multiple locations and cannot list "where used".
- **A separate table per kind**: two nearly identical implementations for no benefit.

---

## R5 — `MemoryAttachment` is a backward-compatible untagged enum

**Decision**: Replace `Memory.attachments: Vec<VizierAttachment>` with:

```rust
#[serde(untagged)]
pub enum MemoryAttachment {
    Artifact { artifact_id: String, name: String },
    LegacySessionFile(VizierAttachment),
}
```

`MemoryFrontMatter` is generated from `Memory`, so it follows. `RevisionFrontMatter` uses the same type. Old YAML `{filename, content: {...}}` deserializes as `LegacySessionFile`; new YAML `{artifact_id, name}` is an `Artifact`. No startup migration.

**Rationale**: The memory document is the source of truth and must keep parsing on existing installs. Untagged YAML is readable and keeps the old shape working without a new migration or a second document format. Revision snapshots and rollback keep working with the same type; a rollback can restore a legacy attachment or an artifact reference. The legacy read path is exactly what #82 tracks for removal.

**Alternatives considered**:
- **Migrate every legacy attachment to an artifact at startup**: preserves data but adds naming/collision/actor rules, a one-time migration, a startup cost and a new artifact per attachment — contrary to the spec's explicit no-retroactive-promotion decision.
- **Drop legacy attachments**: data loss.
- **A separate `memory_attachment` table**: memories are documents on disk; a table would become a second source of truth and complicate export/import.
- **A tagged enum**: old YAML has no tag; untagged is the only shape that reads both without a wrapper.

---

## R6 — `memory_write` is content-only; attach and detach are separate operations

**Decision**: Remove `attachments` from `MemoryWriteArgs`, `MemoryOpRequest::Write`, `MemoryStorage::write_memory` and the HTTP create/update body. `write_memory` reads the existing frontmatter and preserves its attachments. Add:
- `attach_artifact_to_memory { bundle?, path, artifact }` (one artifact, own agent, existing memory, idempotent by artifact id);
- `detach_attachment_from_memory { bundle?, path, name }` (removes an artifact or a legacy session-file attachment by display name);
- HTTP `POST /memory/doc/{bundle}/{*path}/attachments` and `DELETE /memory/doc/{bundle}/{*path}/attachments?name=…`;
- `MemoryOpRequest::AttachArtifact` / `DetachAttachment` and storage methods.

The attach/detach storage path rewrites frontmatter, `attachment_count`, the memory node, a revision and the reference rows; it does **not** re-chunk, re-embed or change the body.

**Rationale**: A content update must not clobber attachments. A dedicated operation can validate the artifact, be idempotent, return the updated list, and support legacy detach. It also lets the API reject a stray `attachments` field with a clear `422 use_attach_artifact` instead of silently ignoring it. The spec's memory API/tool shape is a deliberate pre-1.0 breaking change.

**Alternatives considered**:
- **Keep `attachments` on `memory_write`**: every content save replaces the list; the agent must resend it or lose it, which is the problem the user identified. `MemoryWriteArgs` instead denies unknown fields, so an old key is a clear error; the HTTP create/update body explicitly refuses a non-empty attachments list.
- **One attach tool that also writes content**: re-couples the two concerns and makes validation ambiguous.
- **Only `attach`, no `detach`**: legacy attachments could never be removed without editing the document, and the spec requires them detachable.
- **Directly edit frontmatter in the tool**: bypasses the revision and reference index.

---

## R7 — Artifact tools live in the existing Files category

**Decision**: Add `promote_artifact`, `update_artifact`, `list_artifacts` and `load_artifact_as_session_file` to the `Files` category (no 17th category). Change `send_attachment` to take an artifact name. Add `attach_artifact_to_memory` / `detach_attachment_from_memory` to the `Memory` category. None is added to `DREAM_TOOL_NAMES`.

**Rationale**: Artifacts are files; the existing `Files` category already owns session-file listing/reading and `send_attachment`. A new category would require a 17th category, a new sandbox category block, tool-table updates and user-facing docs for no dispatch benefit. The memory attachment tools belong with the other memory tools and should be discovered there. The dream cycle cannot create permanent content and is excluded consistently.

**Alternatives considered**:
- **A new `Artifacts` category**: clearer grouping, but more surface for two tools that are fundamentally file operations.
- **Put promote/update on `send_attachment`**: conflates watching a file and sending it.
- **Let `memory_write` attach in the same call**: rejected in R6.

---

## R8 — One `SessionFiles` service for clear, conversation deletion and agent deletion

**Decision**: Add `src/session_files/mod.rs`:
- `count(session)`, `clear(session) -> {removed, bytes}`, `delete_conversation(session)`, `files_for_agent(agent_id)`, `delete_blobs(files)`;
- active-turn check through `TurnStorage::pending_turns(agent_id)` filtered to the session;
- delete the session's `session_file` rows, then remove each `uploads/<file_id>/` directory only when no other `session_file` row has the same `file_id` (SQLite `json_extract(data, '$.file_id')`, JSON1 is already used elsewhere);
- the side panel calls `DELETE …/files`; the channel command calls the same service; conversation deletion clears files then deletes the session metadata; agent deletion collects the agent's files before `storage.delete_agent` and deletes remaining blobs plus `artifacts/<agent_id>` after it.

**Rationale**: The spec has one clear semantic and three entry points. A service prevents the HTTP handler, Discord, Telegram and the agent manager from each reinventing the active-turn/shared-blob rules. `json_extract` avoids a schema change; the table is small and clear is rare. The two-step channel confirmation is UI-level and lives in the channel reader, not the service.

**Alternatives considered**:
- **Delete blobs unconditionally**: breaks a blob shared by another conversation (memory-read copies and re-saves can share a `file_id`).
- **Add a `file_id` column and index**: faster reference counting, but a schema migration for a rare operation; `json_extract` is already used in this codebase.
- **Automatically collect by age**: explicitly deferred by the spec.
- **Per-platform clear implementations**: duplicates the rules and the count.

---

## R9 — Conversation and agent deletion are explicit cleanup paths

**Decision**: `DELETE …/topic/{topic_id}` is refused while a turn is running or queued; otherwise it calls `SessionFiles::delete_conversation` (clear session files and their unshared blobs, then `storage.delete_session`). The existing `storage.delete_session` also deletes the session's `session_file` rows and message reference rows. `VizierAgents::handle_delete` collects the agent's session files before `storage.delete_agent` (which deletes artifact rows/events/refs and session-file rows), then deletes the remaining blobs and the whole `artifacts/<agent_id>/` directory. Failure to remove a file after the metadata commit is logged; the database never points at a live artifact that was not removed.

**Rationale**: With no automatic GC, conversation deletion is a real cleanup path (user direction). Deleting rows in SQLite first gives a clear commit point; file cleanup is best-effort. Artifacts are copies, so clearing session files cannot affect them. The same service covers agent deletion so a deleted agent leaves no content.

**Alternatives considered**:
- **Delete files before the metadata**: a metadata failure leaves a live conversation with no files.
- **Leave session files on conversation/agent deletion**: leaves the growth the feature is meant to make manageable.
- **Delete the history table too**: not required by the spec and would change existing history retention; message reference rows are removed instead.

---

## R10 — Keyset paging, kind/date filters and one stats query

**Decision**: Gallery list order is `updated_at DESC, id DESC`. The cursor is an opaque base64 encoding of `(updated_at_ms, id)`; the next page uses `updated_at < at OR (updated_at = at AND id < id)`. Filters: `q` on name (`LIKE`), `kind` (the stored `ArtifactKind`), `from`/`to` on `updated_at` (inclusive date bounds). The response carries filtered `total`/`total_bytes` plus unfiltered `agent_total`/`agent_total_bytes`. The activity view uses `created_at DESC, id DESC` and its own cursor. Availability is checked per returned page item (`content.v<n>` exists), not per stored row.

**Rationale**: The spec requires paging without duplicates or gaps (FR-024); a keyset cursor is stable while new artifacts arrive, unlike offset paging. Reusing the same cursor shape as task runs/notifications keeps the UI simple. The gallery header needs the agent's total even when a filter is active, hence both totals. Checking up to 50 content files per page is bounded; checking all 500 would not be.

**Alternatives considered**:
- **Offset paging**: a new artifact at the head shifts the page and can duplicate/skip rows.
- **Sort by `created_at`**: replacements wouldn't surface recently changed artifacts; the spec shows last change time.
- **FTS for name search**: overkill for a few hundred short names; `LIKE` with the agent index is enough.
- **Compute agent totals in the UI**: would need a second full list or a separate stats call; one query is simpler.

---

## R11 — Artifact access uses four explicit permissions

**Decision**: Add `all_agents:view_artifacts`, `owned_agents:view_artifacts`, `all_agents:manage_artifacts` and `owned_agents:manage_artifacts` to `AVAILABLE_PERMISSIONS` and the role editor. `view` allows the gallery, preview, content download and activity; `manage` additionally allows promotion and deletion; `manage` implies `view`. The all/owned scope plus ownership selects the agent set; the system role bypasses. The gallery/content/activity endpoints use `ArtifactsAccess::can_view` and promote/delete use `can_manage`; promotion additionally requires access to the source conversation (agent visibility + the platform-chat rule). The agent tools act only on their own agent; a client-supplied artifact reference is validated against the conversation's agent. A message/memory attachment's bytes use the view permission, so a person without it sees the attachment as unavailable while the surrounding message/memory renders. These permissions do not grant the agent's other pages.

**Rationale**: The user explicitly asked for `view_artifacts`/`manage_artifacts` for both `all_agents` and `owned_agents`, matching the existing secrets/integrations permission pattern. Separating view from manage lets a role browse and download without being able to promote/delete. Agent visibility is not a substitute — the new permission is the gallery gate — and the permissions are additive, so they never widen access to the agent's other surfaces. One server helper is mirrored by one WebUI helper; direct requests remain authoritative.

**Alternatives considered**:
- **Keep agent visibility + `all_agents:edit`**: rejected by the user; it conflates artifact access with agent configuration and cannot express view-only.
- **One `artifacts` permission**: cannot separate browse/download from promote/delete.
- **Let view imply manage**: defeats the distinction.
- **Let agent visibility alone grant artifact access**: lets any sharee read permanent artifacts without an explicit grant.
- **Trust an artifact id in a request body**: would let a request to agent A read agent B's artifact; reject/validate it in the HTTP/WebSocket path.

---

## R12 — Artifact kinds, previews and channel limits

**Decision**: Store an `ArtifactKind` derived from the MIME type/extension: `image`, `pdf`, `text`, `markdown`, `csv`, `audio`, `video`, `other`. The gallery filter uses these kinds. Preview uses the existing preview body for images, PDF, CSV, markdown and text; audio/video/other offer download (the gallery may reuse the message media player where it already exists, but it is not a requirement). Channel delivery attempts the platform send and, on a size/read error, sends a short explicit notice after the reply text; the artifact is not modified.

**Rationale**: The spec names those preview types and the existing `FilePreviewDialog`/`FilePreview` already implement them. A stored kind makes the filter an index-friendly column and avoids MIME-prefix logic in SQL. Reporting a platform error is more reliable than hard-coded size constants that drift; the user sees the failure and the artifact remains in the gallery.

**Alternatives considered**:
- **Filter by raw MIME**: many subtypes and awkward UI options.
- **Hard-code Discord/Telegram limits**: platform limits change by boost/tier; the send API is authoritative.
- **Silently log the send failure**: violates FR-053/SC-012.

---

## R13 — WebUI composition and the design submodule

**Decision**: Add `artifacts` to the design `SidebarPage`/group (icon `folder`), with a Storybook story. Add `/:agentId/artifacts` and an `ArtifactsGallery` page using `PageFrame`, `FilterTabs`, `Table`/`FileRow`, `FilePreview`, `Modal`, `ConfirmDialog`, `EmptyState`, `LoadMore`, `Skeleton`. The chat side panel gains per-row **Promote** and a **Clear session files** action (ConfirmDialog). Message attachments build an artifact content URL and render a named tombstone on `available: false`/404. Extract the preview body from `FilePreviewDialog` so artifact and session-file previews share it. No memory-screen attachment picker is added; attach/detach is an agent/API operation in this release.

**Rationale**: The existing design system already has every primitive the gallery needs. A new component set would be unused surface. The sidebar page is the only design change, and it must be made in `design/` first per the repo's cascade. The memory screen has no attachment UI today, and the spec's requirement is the agent tool/API path.

**Alternatives considered**:
- **A new gallery component family**: duplicates `PageFrame`, `FilePreview`, `FilterTabs` and `EmptyState`.
- **Put artifacts only in the chat side panel**: cannot browse across conversations, contrary to the gallery requirement.
- **Add a memory attachment picker**: useful later, but not required for the agent path and would add a second UI surface.

---

## R14 — Verification through dummyplug and HTTP/DB checks

**Decision**: The quickstart uses an offline dummyplug agent and `execute_python` to call `load_path_as_session_file`, `promote_artifact`, `update_artifact`, `send_attachment`, `memory_write`, `attach_artifact_to_memory` and `detach_attachment_from_memory`; HTTP/`curl` for the gallery, promote/clear and memory endpoints; SQLite/filesystem checks for rows and `artifacts/` vs `uploads/`; the four-permission matrix and dream source tests; Playwright for the gallery/side panel/message unavailable state when a browser is available. Discord/Telegram delivery and the two-step command are checked by unit tests plus a manual/live channel step where credentials exist.

**Rationale**: The constitution requires an end-to-end dummyplug path for agent-observable changes. The tool calls go through the real code-mode sandbox and router, so the test covers more than a direct storage call. The HTTP/DB checks make deletion and storage separation observable. Channel credentials are not available in CI, so the shared command/clear logic is unit-tested and the live step is marked.

**Alternatives considered**:
- **Only unit tests**: misses the sandbox/router/memory/history integration.
- **Only a live provider**: nondeterministic and needs keys; dummyplug is the default harness.
- **A mock Discord/Telegram server**: more infrastructure than the command/clear path warrants.

---

## R15 — Deferred and legacy scope

**Decision**: This release has no automatic session-file GC/retention, no artifact version history/restore, no retroactive promotion, no quota enforcement, no public links, no generated-UI artifact media, and no migration of legacy session-file memory attachments. The legacy read paths are:
- `MemoryAttachment::LegacySessionFile` in memory documents, `memory_read`'s copy path, `detach_attachment_from_memory`'s legacy branch, and the memory-detail response;
- `VizierAttachmentContent::Local` in older message/reply history, with the human-facing unavailable placeholder;
- the `422 use_attach_artifact` refusal on memory create/update.

These are recorded in #82 (comment: "Planned pre-1.0 removal (spec 035, artifacts)") and must be migrated to artifact references or intentionally dropped before removal.

**Rationale**: The user explicitly deferred GC and chose copy-on-promotion to avoid marking session files. Version history, quotas and generated-UI media are separate product decisions. A one-time migration of legacy memory attachments was rejected (R5) because it would create artifacts without the user's promotion step and add naming/collision rules. Keeping the legacy read paths makes old data usable while #82 tracks the removal.

**Alternatives considered**:
- **Backfill every session file**: violates the explicit-promotion model and could create a large artifact gallery overnight.
- **Drop legacy attachments**: data loss and a broken upgrade.
- **Add a `keep_everything` retention option**: the spec intentionally makes session files transient.

---

## R16 — Dreams read source sessions and may promote

**Decision**: A dream turn gets a `ToolContext.dream_sources: Vec<VizierSession>` allow-list. Extraction uses the inner source session of `VizierChannelId::Dream(source, Extraction)`; consolidation receives the cycle's actual source sessions in the request metadata (the existing slug list stays for the prompt/journal). `list_session_files`, `read_document_file` and `read_image_file` accept an optional source identifier and resolve only to the current session in an ordinary turn or to one of `dream_sources` in a dream. `promote_artifact` and `update_artifact` use the same source resolver, recording the source conversation as the artifact origin. `DREAM_TOOL_NAMES` gains those read tools plus `promote_artifact`, `list_artifacts`, `attach_artifact_to_memory` and `detach_attachment_from_memory`; it does not gain `update_artifact`, `load_artifact_as_session_file`, `send_attachment`, or any clear/delete tool. The source sessions' files are read-only; a dream promotion copies bytes exactly like an ordinary promotion and never writes the source.

**Rationale**: The user wants dreams to be able to keep something they found while reflecting, but not to mutate the conversations they are reviewing. The existing dream session is its own session, so without a source scope its file tools see nothing useful. A typed allow-list is safer than a free-form session id: a dream can only read/promote from the sessions the scheduler selected. Keeping `update_artifact`/`send_attachment`/`load_artifact_as_session_file` out avoids replacing permanent files or writing session files from an unattended cycle. `memory_read` may still copy an artifact into the dream's own transient session; that is not a source write.

**Alternatives considered**:
- **Keep artifacts out of dreams**: contradicts the user's requirement; the dream could reflect on a valuable file but never keep it.
- **Let the dream use its own session only**: sees no source files, the current limitation.
- **Let a dream pass any session slug**: a compromised/prompted dream could read another conversation.
- **Let dreams update/delete artifacts or send attachments**: unattended replacement/deletion is too destructive and write-like for the source.

---

## R17 — Deletion is reference-only in both directions

**Decision**: Deleting a conversation or a memory leaves the artifact row, its events and its content intact; only the reference row and the message/memory are removed. Deleting an artifact leaves message and memory bodies intact: message attachments remain with `available: false`, while memory attachments are removed by calling `detach_artifact_from_memories(artifact_id, origin)` for every memory reference, recording a memory revision; the artifact row/event/refs and content are then removed. A memory cleanup failure is logged and leaves a stale link that reads as deleted; a later detach/reconcile can remove it. The attach path verifies the artifact row at write time, and the delete removes the row before detaching memory links so a concurrent attach cannot recreate a validated link.

**Rationale**: The user wants the artifact to persist when its referencing object is deleted, and the memory to persist when the artifact is deleted. Messages are immutable conversation history, so a named unavailable attachment is the correct record; a memory is editable knowledge, so keeping a knowingly dead link is worse than removing it and recording why. A memory revision preserves the change without touching the body. The row-first delete prevents a concurrent attach from validating a deleted artifact.

**Alternatives considered**:
- **Delete the artifact with its session/memory**: data loss and contradicts the user.
- **Keep a memory tombstone**: the user explicitly asked to delete the memory link.
- **Delete the message attachment row**: rewrites immutable history; the message should still show what was sent.
- **Ignore a failed memory cleanup**: acceptable only as a logged stale link that reads as deleted.

---

## Open questions resolved by the plan

| Spec question | Decision |
|---|---|
| Artifact scope: agent or conversation | Agent-owned (R1/R11); the origin conversation is provenance only. |
| Version history or overwrite | Overwrite semantics with a version counter and event log; no content history/restore (R1). |
| Promotion trigger | Agent-initiated, no confirmation; people with manage permission can promote; only people with manage permission delete; dreams may promote from source sessions (R6/R11/R16). |
| Text or binary | Any file type; kind is metadata and preview is best-effort (R12). |
| Retention default/configurable | None in this release; explicit clear only (R8/R15). |
| References pin or follow latest | Follow the current artifact (R2/R5). |
| Gallery view/delete permissions | `all_agents`/`owned_agents` × `view_artifacts`/`manage_artifacts`; manage implies view; system bypass; agent visibility is not a substitute (R11). |
| Clear command on all platforms | `/clear-files` (Discord) and `/clear_files` (Telegram), two-step confirmation; the WebUI uses the side-panel action (R8). |
| Memory attachments | Artifact-only for new writes; dedicated attach/detach; legacy session-file attachments readable/detachable but not migrated (R5/R6). |
| Dream access | Read-only source session files; promote/list/attach/detach allowed; update/delete/send/load/clear not (R16). |
| Session/memory deletion | Artifact persists; only the reference goes. |
| Artifact deletion | Message body persists with an unavailable attachment; memory body persists with the link removed and a revision; content/refs/event go (R17). |
| Artifact storage | `<workspace>/artifacts/<agent_id>/<artifact_id>/content.v<n>`, never `uploads/` (R1). |
