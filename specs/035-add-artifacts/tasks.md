---
description: "Task list for Permanent Artifacts"
---

# Tasks: Permanent Artifacts

**Input**: Design documents from `/specs/035-add-artifacts/`

**Prerequisites**: `plan.md` (required), `spec.md` (required), `research.md`, `data-model.md`, `contracts/`, `quickstart.md`, and `.specify/memory/constitution.md`

**Tests**: Included because `plan.md`'s Testing section and implementation phases explicitly name the unit, HTTP, Dream-source, WebUI and quickstart checks required for this feature. Tests are colocated with the code they exercise unless a repository test file already exists.

**Organization**: Tasks are grouped by user story so each story can be implemented, tested and delivered independently after the shared foundation. The story priority order is P1 (US1, US2), P2 (US3, US4), then P3 (US5).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (`US1`–`US5`); setup, foundational and polish tasks have no story label
- Every task names the exact file or files to change

## Path Conventions

- Rust single binary: `src/`
- WebUI: `webui/app/`
- Design system: `design/src/`
- Documentation: `docs/src/`
- Feature documents: `specs/035-add-artifacts/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the feature's module skeletons and domain value types without implementing behavior.

- [x] T001 Create the new module skeletons and declare them: `src/artifacts/mod.rs`, `src/artifacts/paths.rs`, `src/artifacts/access.rs`, `src/session_files/mod.rs`, `src/schema/artifact.rs`, `src/storage/artifact.rs`, and `src/storage/sqlite/artifact.rs`; add `mod artifacts;` and `mod session_files;` to `src/main.rs`, add `pub mod artifact;` to `src/schema/mod.rs` and `src/storage/mod.rs`, add `pub mod artifact;` to `src/storage/sqlite/mod.rs`, and add `mod paths; pub mod access;` inside `src/artifacts/mod.rs`. No new crate or WebUI dependency; reuse the existing `axum`, `rusqlite`, `chrono`, `uuid`, `serde`, `serde_json`, `utoipa`, `tracing`, `tokio`, `parking_lot`, `arc-swap` and `@vizier/design` primitives named in `plan.md`.
- [x] T002 Define the artifact domain types in `src/schema/artifact.rs` and re-export them from `src/schema/mod.rs`: `Artifact`, `ArtifactId`, `ArtifactKind` (`image`, `pdf`, `text`, `markdown`, `csv`, `audio`, `video`, `other`), `ArtifactOrigin`, `ArtifactEvent`, `ArtifactEventAction` (`promoted`, `replaced`, `deleted`), `ArtifactReference`, `ArtifactPage`, `ArtifactStats`, `ArtifactFilters`, and `ArtifactCursor`. Use the fields and nullability in `specs/035-add-artifacts/data-model.md` §1: `Artifact` has `id` (UUID string), `agent_id`, `name` (1–255 characters after trim, unique per agent and case-sensitive), `kind`, `mime_type`, `size` (u64), `version` (starts at 1), `content_file`, nullable `origin_channel`/`origin_topic`/`source_filename`, `created_by: Requester`, `created_at` and `updated_at` in milliseconds; events snapshot the artifact name and carry a nullable source; references use the `message`/`memory` kinds and the target keys from data-model.md §1. Derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `JsonSchema` and `utoipa::ToSchema`, and add serde round-trip plus duplicate-name/case-sensitivity tests.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Storage, content store, attachment identity, permissions and shared services that every user story uses.

**⚠️ CRITICAL**: No user-story work can begin until this phase is complete.

- [x] T003 [P] Add the `ArtifactStorage` trait in `src/storage/artifact.rs` with the exact method surface in `contracts/artifact-storage.md` §1: create, get by id, get by name, list, stats, replace, delete, event listing, reference upsert/list/count/delete, message-reference cleanup by session, memory-reference cleanup by `(bundle, path)`, agent cleanup, and the synchronous `artifact_content_file(agent_id, id) -> Result<Option<String>>` lookup. Add `ArtifactStorage` to the `VizierStorageProvider` supertrait list in `src/storage/mod.rs`, implement `impl ArtifactStorage for VizierStorage` forwarding every method to `self.0`, and keep `create_artifact`, `replace_artifact` and `delete_artifact` taking the event so the row change and event share one transaction.
- [x] T004 [P] Add `init_artifact_schema(conn: &Connection)` in `src/storage/sqlite/mod.rs`, call it from `init_schema`, and create the `artifact`, `artifact_event` and `artifact_reference` tables and indexes from data-model.md §2 verbatim: `UNIQUE (agent_id, name)`, `idx_artifact_agent_updated`, `idx_artifact_agent_kind`, `idx_artifact_event_agent_time`, `idx_artifact_event_artifact`, `idx_artifact_ref_artifact`, `idx_artifact_ref_session` and `idx_artifact_ref_memory`. Events have no foreign-key cascade and survive artifact deletion. Add an in-memory test proving the schema is idempotent.
- [x] T005 Implement `ArtifactStorage` mutation methods in `src/storage/sqlite/artifact.rs`: `create_artifact` inserts the row and its `promoted` event in one `unchecked_transaction`, returns the duplicate-name error without overwriting; `replace_artifact` checks `expected_version`, writes the new `content_file`/size/MIME/kind/source/updated time and the `replaced` event in one transaction; `delete_artifact` deletes the row and all its references and inserts the `deleted` event in one transaction; `get_artifact`, `get_artifact_by_name`, `list_artifact_events`, and the sync `artifact_content_file` lookup read on the existing connection. Add tests for create/duplicate/get/replace version/delete/event rollback and the current-version lookup.
- [x] T006 Implement listing, filters, paging and reference queries in `src/storage/sqlite/artifact.rs`: `list_artifacts` filters by `q` (`name LIKE '%' || ? || '%'`, case-insensitive), `kind`, inclusive `from`/`to` on `updated_at`, and the opaque cursor condition `updated_at < at_ms OR (updated_at = at_ms AND id < id)`, ordered `updated_at DESC, id DESC` with `limit + 1`; return `total`/`total_bytes` over the filters before the cursor and `agent_total`/`agent_total_bytes` with no filters. Implement `artifact_stats`, `list_artifact_references`, `count_artifact_references`, `upsert_artifact_reference`, `delete_artifact_references`, `delete_message_references_for_session`, `delete_memory_references` and `delete_agent_artifact_data`. Add tests for tied timestamps, filters, cursor paging without duplicates/gaps, totals and reference CRUD.
- [x] T007 [P] Implement the versioned content store in `src/artifacts/paths.rs`: `artifact_dir(agent_id, artifact_id)`, `content_path(agent_id, artifact_id, file)`, `write_version` (write `content.v<version>.tmp-<uuid>`, flush/close, then same-directory rename to `content.v<version>`; never overwrite a live file), `read_current`, `delete_artifact` (remove the artifact directory), `delete_agent` (remove `artifacts/<agent_id>`), and `exists`. Add temp-workspace tests proving content lives only under `artifacts/<agent>/<id>/`, a new version never replaces a live file, and a failed write leaves no referenced file.
- [x] T008 [P] Add `VizierAttachmentContent::Artifact { agent_id: String, artifact_id: String, available: Option<bool> }` and the synchronous `AttachmentResolver` trait (`fn artifact_path(&self, agent_id: &str, artifact_id: &str) -> Result<Option<PathBuf>>`) in `src/schema/request.rs`; add `NoopAttachmentResolver` for tests/dummyplug. `available` must use `#[serde(default, skip_serializing_if = "Option::is_none")]` and never be stored on write. Change `VizierAttachment::to_part` to take `&dyn AttachmentResolver`, resolve an artifact's current bytes through it, and return a readable "no longer available" text part for a missing row/file instead of failing the turn. Tests cover the serde shape and both present/missing artifacts.
- [x] T009 Thread the `AttachmentResolver` through `VizierRequest::to_message`, `VizierResponse::to_tool_result` and every call site in `src/schema/request.rs`, `src/schema/response.rs`, `src/agents/agent/mod.rs` and `src/agents/agent/model/dummyplug.rs`; the agent passes its `Artifacts` handle (or `NoopAttachmentResolver` in dummyplug/unit contexts), and the existing `Bytes`/`Base64`/`Url`/`Local` branches stay byte-for-byte compatible.
- [x] T010 Extend `src/file_manager/mod.rs`: hold an optional shared artifact resolver installed after construction, resolve `VizierAttachmentContent::Artifact` to its current bytes in `FileManager::resolve`, add `delete_upload(file_id)` (`remove_dir_all(uploads/<file_id>)`, best effort) and a free `read_upload(workspace, file_id)` helper shared by `Artifacts` and tools. Add tests for artifact byte resolution, upload deletion and the shared reader.
- [x] T011 Implement the `Artifacts` service skeleton in `src/artifacts/mod.rs`: hold the workspace, `Arc<VizierStorage>`, `ArtifactPaths`, `FileManager` and one `tokio::sync::Mutex` mutation lock; implement `new`, `path_for(agent_id, artifact_id)`, `read_content`, and `impl AttachmentResolver for Artifacts` using `ArtifactStorage::artifact_content_file` plus `ArtifactPaths::exists`. No promote/replace/delete yet; this is the single resolver-backed service the next phases extend.
- [x] T012 [P] Add `src/artifacts/access.rs` with the four permission constants `all_agents:view_artifacts`, `owned_agents:view_artifacts`, `all_agents:manage_artifacts` and `owned_agents:manage_artifacts`, plus `can_view(caller, agent)` and `can_manage(caller, agent)` built on `crate::access::Caller`; `manage` implies `view`, `all` covers every agent, `owned` requires ownership, and the system role bypasses. Add all four strings to `AVAILABLE_PERMISSIONS` in `src/storage/user.rs` and tests for all/owned/owner/non-owner/system/view-only.
- [x] T013 [P] Add `MemoryAttachment` in `src/schema/storage.rs` as `#[serde(untagged)] pub enum MemoryAttachment { Artifact { artifact_id: String, name: String }, LegacySessionFile(VizierAttachment) }` with the same derives as `Memory`/`VizierAttachment`, and use it for `Memory.attachments`, `MemoryFrontMatter.attachments` and `RevisionFrontMatter.attachments` in `src/storage/sqlite/memory_revision.rs`; update `memory_canonical` to take `&[MemoryAttachment]`. Old YAML `{filename, content:{local}}` must parse as `LegacySessionFile` and new `{artifact_id, name}` as `Artifact`; add a round-trip and legacy-document test. No startup migration and no retroactive promotion.
- [x] T014 Change the memory write path so `MemoryStorage::write_memory` no longer takes an `attachments` argument and `MemoryOpRequest::Write` loses its attachments field: update `src/storage/memory.rs`, `src/storage/memory_bundle.rs`, `src/storage/sqlite/memory.rs`, `src/schema/commands.rs`, `src/agents/memory_ops.rs`, `src/agents/tools/vector_memory/mod.rs`, `src/channels/http/api/v1/agents/memory.rs` (pass no attachments for now; T059 adds the explicit `422` guard and attach/detach routes), and every `write_memory` call site. A content update reads the existing `MemoryAttachment` list and preserves it exactly; a new memory starts empty; the legacy filesystem migration's `write_migrated_memory` converts `Vec<VizierAttachment>` to `MemoryAttachment::LegacySessionFile`. Tests cover content update preserving artifact and legacy attachments.
- [x] T015 Add the memory-attachment storage operations in `src/storage/memory.rs`, `src/storage/memory_bundle.rs` and `src/storage/sqlite/memory.rs`: `attach_memory_attachment(agent_id, bundle, path, artifact_id, artifact_name, origin)` (idempotent by artifact id), `detach_memory_attachment(agent_id, bundle, path, name, origin)` (artifact or legacy by display name), `detach_artifact_from_memories(agent_id, artifact_id, origin)` and `reconcile_agent_artifact_references(agent_id)`. Add `MemoryOpRequest::AttachArtifact`/`DetachAttachment` in `src/schema/commands.rs` and dispatch both in `src/agents/memory_ops.rs`. Attach/detach rewrites only frontmatter, `updated_at`, `attachment_count`, the memory node, one `memory_revision` with the supplied `RevisionOrigin`, and the `artifact_reference` rows; it must not re-chunk, re-embed or change the body. `sync_artifact_references` runs from write/attach/detach/delete/import, and the reconcile scans the agent's concept documents at agent start. Add idempotency, detach-no-op, legacy detach and artifact-delete-link-removal tests.
- [x] T016 Create the `SessionFiles` service surface in `src/session_files/mod.rs` with `SessionFiles { storage, file_manager, workspace }`, `ClearResult { removed, bytes }`, `ClearError { AgentNotFound, TurnActive, Storage(String) }`, and `count`, `files_for_agent`, `delete_blobs`, `has_active_turn`; add `delete_session_files(session)`, `list_agent_files(agent_id)` and `session_file_reference_count(file_id)` to `SessionFileStorage` in `src/storage/session_file.rs`, implemented in `src/storage/sqlite/session_file.rs` with `json_extract(data, '$.file_id')`. `has_active_turn` matches the session against `TurnStorage::pending_turns(agent_id)`. Add unit tests for the reference count and active-turn detection; `clear`/`delete_conversation` are completed in US3.
- [x] T017 Persist message artifact references and fill availability in `src/storage/sqlite/history.rs` and `src/schema/history.rs`: add an attachment-iteration helper over `Request`, `Response` and `AudioReply`; `save_session_history` inserts the history row and one `artifact_reference` per `Artifact` attachment in one transaction with `kind = "message"`, `target = "message:<session_slug>:<uid>"`, `session_slug`, `history_uid`, `channel`, `topic` and `label`; history reads set `available = Some(row exists AND content file exists)` without mutating stored JSON and without re-locking `VizierStorage` inside the per-entry loop. Add tests for ref insertion, deleted artifacts, legacy rows and session deletion cleanup.
- [x] T018 Wire the shared services in `src/dependencies.rs`: construct `Artifacts` and `SessionFiles` once, install the `Artifacts` resolver on the `FileManager` clone, expose `pub artifacts` and `pub session_files` on `VizierDependencies`, and call `reconcile_agent_artifact_references` for each agent in `src/agents/mod.rs`'s startup path after its storage is available. Keep construction order non-circular by using the free upload reader from T010. Add a startup test or an assertion that the handles are present and the file manager resolves an artifact.

**Checkpoint**: Foundation ready — artifact rows, content paths, references, permissions, memory attachment storage, resolver and session-file counting exist. User-story work can now begin.

---

## Phase 3: User Story 1 — Promote a file into an artifact that outlives the conversation (Priority: P1) 🎯 MVP

**Goal**: An agent (or a dream working from one of its source conversations) copies a session file into a permanent artifact, and can replace its content while keeping its identity and name.

**Independent Test**: On an offline `dummyplug` agent, create a session file, promote it, clear the source conversation's session files, and download the artifact with matching bytes; replace it from a different session file and confirm the name stays, the version increments and only the new `content.v<n>` remains; attempt a replacement from a missing file and confirm the artifact is unchanged; run the Dream-source unit tests and confirm a source promotion records the source conversation as origin and leaves source files byte-identical.

### Implementation for User Story 1

- [x] T019 [US1] Implement `Artifacts::promote` in `src/artifacts/mod.rs`: resolve the source session file through `storage.get_session_file`/the shared upload reader, reject a missing, unreadable or zero-length source, validate the artifact name (trimmed 1–255 characters, non-empty, no `/`, `\`, ASCII control characters, `.` or `..`), default an omitted name to the source filename, reject a duplicate `(agent_id, name)` with a message pointing to replacement, copy the bytes to `content.v1`, compute size/MIME/kind, and insert the row plus a `promoted` event with `created_by`, origin channel/topic and source filename. Serialize mutations with the service lock; a failure creates no row and removes any temp/version file. Tests cover success, duplicate, invalid name, missing/empty source and source-stays-in-uploads.
- [x] T020 [US1] Implement `Artifacts::replace` in `src/artifacts/mod.rs`: find the calling agent's artifact by name, read the source session file, write `content.v<current+1>` through `ArtifactPaths::write_version`, update the row and insert a `replaced` event in one transaction, then delete the previous version file after commit; keep the artifact id/name, increment the version exactly once, update size/MIME/kind/source/updated time, and log (without failing) old-file cleanup errors. A failed source read or transaction leaves the old content, version and metadata intact. Tests cover version switch, old-file removal, failed replacement and identical-content replacement.
- [x] T021 [US1] Centralize the validation and kind logic used by T019/T020 in `src/schema/artifact.rs`: `validate_artifact_name` implements the exact name rule from data-model.md §1, and `ArtifactKind::from_mime` implements the mapping in data-model.md §6 (`image/*` → image; `application/pdf`/`.pdf` → pdf; `text/markdown`/`.md` → markdown; `text/csv`/`.csv` → csv; `text/*`, `application/json`, `application/xml`, `application/yaml`, `application/x-yaml`, `application/toml` → text; `audio/*` → audio; `video/*` → video; anything else → other). Refactor T019/T020 to call the helpers; add table-driven tests including Unicode names, control characters, `.`/`..`, `.md`/`.csv` extensions and unknown MIME types.
- [x] T022 [US1] Create `src/agents/tools/artifacts/mod.rs` with `promote_artifact` and `update_artifact` implementing `VizierTool` in the `Files` category. Inputs are exactly `promote_artifact { filename, name?, source_session? }` and `update_artifact { name, filename, source_session? }`; start with the current-session rule (an omitted source means the current session), call `Artifacts::promote`/`replace` with the calling agent, and return the artifact id, name, kind, MIME, size, version and origin. T023 then replaces the inline current-session resolution with the shared Dream-aware helper. Register both in `BaseTools::new`'s `default_toolset` in `src/agents/tools/mod.rs`, add them to the `every_tool_is_in_its_category` table and the telemetry display labels, and add tests for schema/name/category.
- [x] T023 [US1] Add `dream_sources: Vec<VizierSession>` to `ToolContext` in `src/agents/tools/mod.rs` (default empty) and update every `ToolContext` literal in `src/agents/tools/mod.rs` and `src/agents/agent/mod.rs` tests. Add one shared `resolve_source_session(ctx, source_session: Option<&str>) -> Result<VizierSession>` helper in `src/agents/tools/artifacts/mod.rs`: an ordinary turn accepts the current session only (or an omitted source meaning current); a dream accepts an omitted source only when exactly one `dream_sources` entry exists, otherwise the slug must match an entry; any other session is refused with a clear error. Tests cover ordinary/current, dream allow-list, arbitrary-session refusal and the one-source default.
- [x] T024 [US1] Add `list_artifacts` (`kind?`) and `load_artifact_as_session_file` (`name`) to `src/agents/tools/artifacts/mod.rs`, register them in `BaseTools::new` and the Files category table. `list_artifacts` returns the calling agent's artifacts with name, kind, MIME, size, version and last change, optionally filtered by kind and with no gallery pagination. `load_artifact_as_session_file` reads the artifact's current bytes, uploads them through `FileManager::upload` under the safe server-derived name `artifact-<artifact-id><ext>`, saves a session file in the current conversation, and returns the session filename, artifact name, MIME and size; it creates no attachment and never mutates the artifact. Refuse a missing/deleted artifact and keep both tools out of the dream toolset.
- [x] T025 [US1] Extend `list_session_files`, `read_document_file` and `read_image_file` to accept an optional `session` source: add `session: Option<String>` to the args in `src/agents/tools/session_files/mod.rs` and `src/agents/tools/read_image/mod.rs`, resolve it through T023's helper, include the source slug in `SessionFileSummary`/output, and refuse a source outside the ordinary-current/dream allow-list. A dream must be able to list all its sources and read a named file from one; an ordinary turn must not read another conversation. Add tests for both scopes and the refusal.
- [x] T026 [US1] Carry Dream source sessions through the scheduler and turn setup: in `src/scheduler/dream/mod.rs` include the cycle's actual source `VizierSession`s (serialized) in the extraction and consolidation request metadata alongside the existing slug list, and in `src/agents/agent/mod.rs` parse that metadata into `ToolContext.dream_sources` for a dream turn (extraction uses the inner source of `VizierChannelId::Dream(source, Extraction)`; consolidation uses the metadata list). Update `src/scheduler/dream/prompts.rs` to mention the source-file read tools, `promote_artifact(source_session=...)`, and the source slug. Tests cover extraction, consolidation, a non-source refusal and no source file being created/changed/cleared.
- [x] T027 [US1] Extend `VizierTools::DREAM_TOOL_NAMES` in `src/agents/tools/mod.rs` with `list_session_files`, `read_document_file`, `read_image_file`, `promote_artifact` and `list_artifacts`; do **not** add `update_artifact`, `load_artifact_as_session_file`, `send_attachment` (added later only for the ordinary toolset) or any clear/delete tool. Update the exact-list assertions around `DREAM_TOOL_NAMES` and add a test that every dream tool exists on `default_toolset`.
- [x] T028 [US1] Add the person-facing promotion route `POST /api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/files/promote` in `src/channels/http/api/v1/agents/channel.rs`: body `{filename, name?}`, resolve the conversation exactly like `GET …/files`, require agent visibility plus the existing `show_<platform>_chats` rule for Discord/Telegram and `ArtifactsAccess::can_manage`, call `Artifacts::promote` with `Requester::User(user.user_id)` and the conversation origin, return `201` with the artifact detail summary, and map errors to `404` (agent/conversation/file), `409` duplicate name, `422` invalid/empty/unreadable source and `403` permission/conversation access. Add route tests for the success/duplicate/forbidden cases.

**Checkpoint**: US1 is complete when promotion/replacement and Dream-source promotion work independently on `dummyplug`, with byte-identical artifacts after a source clear.

---

## Phase 4: User Story 2 — Browse and manage artifacts in the gallery (Priority: P1)

**Goal**: A permitted person can browse, search, filter, preview, download, inspect and delete an agent's artifacts; the page and its content are gated by the four artifact permissions.

**Independent Test**: With several artifacts of different kinds, open the gallery as a view user, verify newest-first ordering, stats, search/filter/paging, previews and downloads; open detail for origin/provenance/references; delete one as a manager and confirm the dialog/count and tombstone; repeat as a view-only person (no delete/promote, direct requests refused), a no-permission person (no entry, `403`), an `owned_agents` owner (allowed) and a non-owner (refused). Verify empty/loading/error states and phone/keyboard use.

### Implementation for User Story 2

- [x] T029 [US2] Extract the single `Range` parser from `src/channels/http/api/v1/files.rs` into `src/channels/http/api/v1/range.rs` (`RangeRequest`, `parse_range`, and the existing tests), declare it in `src/channels/http/api/v1/mod.rs`, and make `files::download_file` use the shared parser. This parser is reused by artifact content downloads in T032.
- [x] T030 [US2] Implement the read side of `Artifacts` in `src/artifacts/mod.rs`: `list(ArtifactFilters)` returning the keyset page and stats from T006, `detail(agent_id, id)` returning the artifact, its references and newest-first events, `activity(agent_id, cursor, limit)` over `artifact_event`, `content_path`/`read_content` using the current `content_file`, and a per-returned-item `available` check (`row exists AND content file exists`). Do not scan history JSON or memory documents at read time; use `artifact_reference`. Add tests for ordering, availability true/false, empty references and deletion records still appearing in activity.
- [x] T031 [US2] Implement `Artifacts::delete(agent_id, id, actor)` in `src/artifacts/mod.rs`: collect the artifact's memory references, delete the artifact row/refs and insert a `deleted` event in one transaction, call `MemoryStorage::detach_artifact_from_memories` with the deleting person's `RevisionOrigin`, then remove the artifact's `artifacts/<agent>/<id>/` directory; message references remain and become unavailable. Return `{removed, references, memory_links_removed}`; a memory cleanup or disk failure is logged and leaves a stale link that reads as deleted, never a partial body loss. Add tests for message refs, memory link removal/revision and content-directory removal.
- [x] T032 [US2] Add `src/channels/http/api/v1/agents/artifacts.rs` with `GET /agents/{agent_id}/artifacts` (query `q`, `kind`, `from`, `to`, `cursor`, `limit` default 50/max 100), `GET …/artifacts/{artifact_id}` (detail with origin, references, events), `GET …/artifacts/{artifact_id}/content` (current bytes, stored MIME, `Content-Disposition: inline; filename*=UTF-8''<percent-encoded name>`, shared Range parser; `404` missing row and `404` “artifact content is unavailable” for a missing file), `GET …/artifacts/activity?cursor=&limit=`, and `DELETE …/artifacts/{artifact_id}` (`can_manage`; returns removed/references/memory_links_removed). All routes use `require_auth`; gallery/detail/content/activity use `ArtifactsAccess::can_view`, delete uses `can_manage`, a missing agent is `404`, no permission is `403`, and a malformed cursor is `400 malformed cursor`. Register the module/nest in `src/channels/http/api/v1/agents/mod.rs`.
- [x] T033 [US2] Register the artifact paths and schemas in the OpenAPI document in `src/channels/http/mod.rs` (and any path/schema lists in `src/channels/http/api/v1/agents/mod.rs`): artifact list/detail/activity/content/delete, promote, clear, the `Artifact`/`ArtifactEvent`/`ArtifactReference`/`ArtifactFilters`/`ArtifactPage` types, and the `VizierAttachmentContent::Artifact` shape. Keep the generated document in sync with the contracts in `specs/035-add-artifacts/contracts/`.
- [x] T034 [P] [US2] Add `artifacts` to the design `SidebarPage` union and the agent-pages group in `design/src/components/Sidebar/Sidebar.tsx` (icon `folder`, accessible label `Artifacts`) and update `design/src/components/Sidebar/Sidebar.stories.tsx`; run the design typecheck/storybook check and commit the `design` submodule change first per the cascade rule.
- [x] T035 [P] [US2] Add `canViewArtifacts(agent)` and `canManageArtifacts(agent)` to `webui/app/lib/access.ts` mirroring `src/artifacts/access.rs` (all/owned, owner for owned, manage implies view, superadmin/system bypass), plus the Artifacts permission group with readable labels in `webui/app/lib/permissions.ts` and tests in `webui/app/lib/access.test.ts`/`permissions.test.ts`.
- [x] T036 [US2] Add the `/:agentId/artifacts` route in `webui/app/routes.ts`, add `artifacts` to `AGENT_PAGES` and `PAGE_PATH` (`/artifacts`) in `webui/app/layouts/AppShell.tsx`, and hide the sidebar entry when `!canViewArtifacts(agent)` while keeping the agent reachable in the switcher.
- [ ] T037 [P] [US2] Add `webui/app/api/endpoints/artifacts.ts` for list/detail/content URL/activity/delete/promote/clear, regenerate `webui/openapi.json` and `webui/app/api/schema.d.ts` with `VIZIER_URL=http://localhost:9999 node scripts/openapi.mjs` against a running dev server (or the repository's documented command), and typecheck the new endpoint module.
- [x] T038 [P] [US2] Add `webui/app/lib/artifacts.ts` with kind/filter query building, artifact content URL construction, the `available: false` tombstone helper and reference label helpers, plus `webui/app/lib/artifacts.test.ts` covering each. Do not add a second permission rule; import the T035 helpers.
- [x] T039 [US2] Implement `webui/app/screens/artifacts/ArtifactsGallery.tsx`, `ArtifactDetail.tsx` and `ArtifactPreviewDialog.tsx`: `PageFrame` title/description, `FilterTabs` Artifacts/Activity, search/kind/date filters that persist while paging, `LoadMore` with the cursor, total count/bytes, table on desktop and stacked cards on phone, row actions Open/Preview/Download/Delete (delete only with manage), detail with preview/metadata/origin/provenance/references and a `ConfirmDialog` naming the artifact and reference count, activity newest-first with deleted records, and the loading/empty/filter-empty/activity-empty/error/retry/unavailable states. Extract the shared preview body (`kind`, text read, CSV parse, body switch) from `webui/app/screens/chat/FilePreviewDialog.tsx` into `webui/app/screens/chat/FilePreviewContent.tsx` and use it from both dialogs.
- [ ] T040 [US2] Add `webui/app/e2e/artifacts.spec.ts` (desktop and phone) covering the sidebar entry, gallery list/stats/search/kind/date/paging, image/PDF/CSV/markdown/text previews and unsupported download, detail provenance/references/activity, delete as manager vs view-only, no-view/no-permission and owner/non-owner cases, and the role editor's four artifact labels. If a browser is unavailable, record the manual checks in `specs/035-add-artifacts/quickstart.md`.
- [ ] T041 [US2] Add backend tests for the gallery/API in `src/channels/http/api/v1/agents/artifacts.rs` and `src/artifacts/mod.rs`: all/owned/owner/non-owner/view-only/system access, `403`/`404`/`400`, content bytes/Range/Content-Disposition, delete's message refs and memory-link revision, and activity retention after artifact deletion.
- [ ] T042 [US2] Finish the gallery's cross-cutting UI behavior in `webui/app/screens/artifacts/` and `webui/app/layouts/AppShell.tsx`: loading/empty/error announcements, keyboard-operable filters/rows, focus return after dialogs, text/icon (not colour-only) unavailable/action warnings, a full-screen phone detail, wrapped filters and no horizontal scroll. Add the agent-config Tools “Always on” copy mention of artifacts alongside session files in `webui/app/screens/agents/ToolsFields.tsx`.

**Checkpoint**: US2 is complete when the gallery, preview/download, detail, activity and delete work for a permitted person, and view-only/no-permission/owned behavior matches the matrix.

---

## Phase 5: User Story 3 — Clear a conversation's session files explicitly (Priority: P2)

**Goal**: A person can clear one conversation's transient files from the WebUI side panel or `/clear-files` (`/clear_files` on Telegram), while artifacts survive; conversation and agent deletion clean up the transient tier.

**Independent Test**: Create files in two conversations, promote one, attach an artifact to a message and a memory, clear the first conversation from the side panel and the second with the channel command; confirm counts, no cross-conversation effect, artifact downloads still work, old references show unavailable, an empty clear changes nothing, a cancelled confirmation removes nothing, and a running/queued turn refuses. Delete a conversation and an agent and confirm the transient records/blobs are gone while artifacts promoted from them survive.

### Implementation for User Story 3

- [x] T043 [US3] Implement `SessionFiles::clear` and `SessionFiles::delete_conversation` in `src/session_files/mod.rs`: refuse `TurnActive` when `has_active_turn` is true; list the conversation's records; delete the records first; for each distinct `file_id` call `session_file_reference_count` and delete `uploads/<file_id>/` only when the count is zero; return `{removed, bytes}`; `delete_conversation` clears then calls `storage.delete_session`. A blob-deletion failure is logged after the records are gone and does not roll back. Add tests for zero files, count/bytes, active turn, shared `file_id` and last-reference deletion.
- [x] T044 [US3] Add `DELETE /api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/files` in `src/channels/http/api/v1/agents/channel.rs` using the same conversation visibility as `GET …/files`; artifact permissions do not gate session-file clear. Refuse an active turn with `409` and the message “a turn is running or queued in this conversation; wait for it to finish”; return `200 {removed, bytes}`; `404` for a missing agent/conversation. Add tests for permission, empty clear, active refusal and count.
- [x] T045 [US3] Make conversation deletion use the shared service in `src/channels/http/api/v1/agents/channel.rs`: `DELETE …/topic/{topic_id}` refuses `409` while a turn is running or queued, calls `SessionFiles::delete_conversation` (clear + `storage.delete_session`), leaves `artifacts/` untouched, and logs a metadata failure after files were cleared so a retry can finish. Add tests that the session-file records/blobs and message references are gone while a promoted artifact remains downloadable.
- [x] T046 [US3] Update `VizierAgents::handle_delete` in `src/agents/mod.rs` to collect `SessionFiles::files_for_agent(agent_id)` **before** `storage.delete_agent`, then after a successful delete call `SessionFiles::delete_blobs` for the collected records (best effort, log failures); do not delete a blob still referenced by another agent. Add a test around the collect/delete ordering and a shared-blob case.
- [x] T047 [US3] Add `ClearConfirmations` to `src/channels/commands.rs` with `request(session)`, `take(session) -> bool` (true only within the 60 s TTL), and `cancel(session)`, backed by a mutex map; add the builtin command entries `clear-files` (Discord, canonical) and `clear_files` (Telegram, valid name) to the respective `PlatformRules`/`builtin_commands` in `src/channels/discord/mod.rs` and `src/channels/telegram/mod.rs`. Add tests for request/confirm, expiry, cancel, confirm-without-request and the builtin name clash with a skill.
- [x] T048 [P] [US3] Handle `/clear-files` in `src/channels/discord/mod.rs`: count the current conversation's files, reply “There are no session files to clear in this conversation.” when zero, reply with the refusal when a turn is active, otherwise store the pending confirmation and reply with the count/artifact warning and “Send /clear-files confirm within 60 seconds to continue.”; on `confirm`, take the token, re-check the active turn, call `SessionFiles::clear`, and reply “Cleared N session files. Artifacts are untouched.” Replace the pending token on a new request and cancel on another message. Update the command list/help text.
- [x] T049 [P] [US3] Handle `/clear_files` and `/clear_files confirm` in `src/channels/telegram/mod.rs` with the same count/prompt/confirm/refusal semantics as T048, using the Telegram-valid command name and the existing `setMyCommands` builder. Add unit tests for the command entries and the shared confirmation behavior.
- [x] T050 [US3] Add the chat side-panel actions in `webui/app/screens/chat/SessionFilesPanel.tsx` and wire them from `webui/app/routes/chat.tsx`: a per-row **Promote** action only when `canManageArtifacts(agent)` opens a name dialog prefilled with the filename, calls `POST …/files/promote`, shows `Promoted “<name>” to artifacts.` with a gallery link, and leaves the source in the list; a **Clear session files** button when the list is non-empty and the turn is idle opens the required `ConfirmDialog` (title `Clear session files?`, body naming N, “Artifacts are unaffected.” and the older-reference warning), calls `DELETE …/files`, reports the count, dispatches the `files: []` action and leaves other topic state intact. A `409` explains the active turn and leaves the list. The panel must not list the artifact library.
- [ ] T051 [US3] Add WebUI tests in `webui/app/lib/frames.test.ts` and/or `webui/app/screens/chat/SessionFilesPanel` test coverage for the clear action emptying `files` while preserving other topic state, a new session-file frame prepending normally after a clear, and promote/clear visibility for view-only vs manager. Add typecheck coverage for the new props.
- [x] T052 [US3] Add focused `SessionFiles` tests in `src/session_files/mod.rs`: count/clear/zero, two conversations sharing a `file_id` (one clear keeps the blob, the last removes it), conversation deletion leaving artifacts, agent deletion collecting files before storage deletion, and active queued/running refusal. Keep the tests deterministic with an in-memory SQLite connection where possible.
- [ ] T053 [US3] Add channel-command tests in `src/channels/commands.rs`, `src/channels/discord/mod.rs` and `src/channels/telegram/mod.rs` for the builtin entries/descriptions, the 60 s TTL, cancel, confirm-without-request, and a clear through both readers with a fake session; mark live Discord/Telegram checks as LIVE in `specs/035-add-artifacts/quickstart.md` rather than requiring credentials in CI.

**Checkpoint**: US3 is complete when both clear entry points and both deletion paths remove only transient files, keep artifacts, and respect active turns and shared blobs.

---

## Phase 6: User Story 4 — Attach only artifacts to replies and memories (Priority: P2)

**Goal**: New outgoing message attachments and new memory attachments reference artifacts by identity; `memory_write` is content-only and preserves existing attachments; replacements/deletions flow through references; channel and WebUI delivery show the current file or a named unavailable state.

**Independent Test**: On `dummyplug`, promote an image/artifact, attach it with `send_attachment`, confirm the reply carries it; replace it and confirm the same message serves new bytes; delete it and confirm a named tombstone. Write a memory, attach with `attach_artifact_to_memory`, update its content, clear the source conversation, read the memory and confirm the attachment still resolves; detach and confirm it is gone; try a session-file name and confirm the promotion guidance; send an oversized artifact to a channel and confirm the text arrives with an explicit failure notice.

### Implementation for User Story 4

- [x] T054 [P] [US4] Change `send_attachment` in `src/agents/tools/session_files/send_attachment.rs` to take `{artifact}` and resolve one artifact owned by the calling agent; push a `VizierAttachment { filename: artifact.name, content: Artifact { agent_id, artifact_id, available: None } }` into `pending_attachments`; refuse a session-file name with “`<name>` is a session file, not an artifact. Promote it with `promote_artifact` first.”, refuse a missing/deleted/cross-agent artifact, and update the tool description. Keep it out of the dream toolset. Add tests for success, session-file refusal, missing artifact and cross-agent name.
- [x] T055 [US4] Update `FileManager::resolve` callers and the Discord/Telegram send loops in `src/channels/discord/mod.rs` and `src/channels/telegram/mod.rs`: when resolving or sending an artifact fails (deleted, too large, unreadable), still deliver the reply text and send one short explicit notice naming the file after the text; never silently drop the attachment. Reuse one shared failure-notice helper rather than duplicating platform text. Add unit-level tests where the platform call is mockable and record the live oversized check in `quickstart.md`.
- [x] T056 [US4] Extend `webui/app/api/frames.ts` with `content: { artifact: { agent_id, artifact_id, available? } }`; update `webui/app/screens/chat/ReplyMedia.tsx` so `attachmentUrl` builds `/api/v1/agents/{agent_id}/artifacts/{artifact_id}/content` with the auth token, available images/audio/video use the existing media components, other kinds use the existing FilePeek/download path, and `available === false`, a content `403`/`404`, or `!canViewArtifacts(agent)` renders a named “<name> — artifact unavailable” state without a download. Keep legacy `local`/`url`/`bytes`/`base64` rendering unchanged. Use the shared `FilePreviewContent` from T039 for artifact previews.
- [x] T057 [P] [US4] Add `attach_artifact_to_memory` and `detach_attachment_from_memory` in `src/agents/tools/vector_memory/attachments.rs`: attach takes `{bundle? = "default", path, artifact}` and one artifact name/identity owned by the calling agent, requires an existing memory, calls `MemoryStorage::attach_memory_attachment` and returns the updated attachment display-name list; an already-attached artifact is a no-op. Detach takes `{bundle?, path, name}` and removes an artifact or legacy attachment by display name, with a clear no-op result. Both are in the `Memory` category, registered in `BaseTools::new`, and available to dreams for the agent's own artifacts. Refuse a session-file name/external URL with promotion guidance, a missing memory and another agent's artifact.
- [x] T058 [P] [US4] Update `memory_write` and `memory_read` in `src/agents/tools/vector_memory/mod.rs`: remove `attachments` from `MemoryWriteArgs`, add `#[serde(deny_unknown_fields)]` so an old key is a deserialization error, update the description to point at `attach_artifact_to_memory`; `memory_read` lists `MemoryAttachment` display names, copies artifact attachments into the current conversation under the same safe `artifact-<id><ext>` name used by `load_artifact_as_session_file`, reports a missing/deleted artifact by name without failing the rest of the memory, and keeps the legacy session-file copy path while its file exists.
- [ ] T059 [P] [US4] Update `src/channels/http/api/v1/agents/memory.rs`: remove `attachments` from `CreateMemoryRequest`/`UpdateMemoryRequest` and reject a present non-empty list with `422 use_attach_artifact`; add `POST /doc/{bundle}/{*path}/attachments` (`{artifact}`) and `DELETE /doc/{bundle}/{*path}/attachments?name=…` using `MemoryOpRequest::AttachArtifact`/`DetachAttachment`; require the existing `require_agent` memory visibility plus artifact view permission; return the memory detail with `Vec<MemoryAttachmentView>` tagged `kind: artifact` (`artifact_id`, `name`, `mime_type`, `size`, `available`) or `kind: legacy_session_file` (`filename`, `content`, `available`). Add tests for attach/detach, legacy detach, 404 memory/artifact/name, 403 and 422.
- [ ] T060 [P] [US4] Guard incoming request attachments in `src/channels/http/api/v1/agents/mod.rs` (REST `POST /chat`, `ChatRequest`/`AttachmentChatRequest`) and the WebSocket request path in `src/channels/http/mod.rs` before `send_file_resolve`: reject a client-supplied `VizierAttachmentContent::Artifact` with a clear client error, or validate its `agent_id` against the conversation's agent and caller access, so agent A cannot read agent B's artifact by id. A person's uploads remain session-file uploads; only the agent/tool response path creates artifact references. Add a cross-agent test.
- [x] T061 [US4] Extend `DREAM_TOOL_NAMES` in `src/agents/tools/mod.rs` with `attach_artifact_to_memory` and `detach_attachment_from_memory`, and add tests that a dream can attach/detach only the agent's own artifacts and still cannot see `update_artifact`, `load_artifact_as_session_file`, `send_attachment` or any clear/delete tool. Keep `memory_write`/`memory_read` behavior from T058 in the dream toolset.
- [ ] T062 [US4] Add memory-attachment tests in `src/storage/memory_bundle.rs` and `src/agents/tools/vector_memory/` for attach idempotency, detach artifact vs legacy by name, detach-not-attached no-op, missing memory, cross-agent artifact refusal, content update preserving attachments, and `memory_read` reporting a deleted artifact without failing the rest. Add the legacy YAML fixture check from `quickstart.md` §5b at unit level.
- [ ] T063 [US4] Add `send_attachment` and channel-delivery tests in `src/agents/tools/session_files/send_attachment.rs` and `src/channels/discord/mod.rs`/`src/channels/telegram/mod.rs`: artifact attachment shape, session-file refusal text, deleted/missing artifact, oversized/read-failure notice after text delivery. Keep live platform credentials out of the tests.
- [ ] T064 [US4] Add WebUI tests in `webui/app/screens/chat/ReplyMedia`/`webui/app/lib/frames.test.ts` for artifact content URL construction, `available: false` tombstone, a 404/403 content response, no-view permission hiding the request, replacement showing new bytes on reload, and legacy `local`/`url` attachments still rendering.
- [ ] T065 [US4] Register the memory attachment paths/schemas in `src/channels/http/mod.rs` and refresh `webui/openapi.json` plus `webui/app/api/schema.d.ts` for the `MemoryAttachmentView`/attach/detach shapes; run `cd webui && npm run typecheck` for the generated client. Ensure no response carries artifact bytes in a JSON body.

**Checkpoint**: US4 is complete when artifacts are the only new message/memory attachment path, memory content updates preserve attachments, replacement/deletion behavior reaches every reference, and channel/WebUI failures are explicit.

---

## Phase 7: User Story 5 — Provenance is recorded and boundaries hold (Priority: P3)

**Goal**: Every promotion/replacement/deletion is auditable, agents cannot reach another agent's artifacts, agents cannot delete artifacts, permissions are enforced on every operation, and agent deletion removes artifact data without touching other agents.

**Independent Test**: Promote as the agent, replace as a manager, and confirm both events show actor/action/time/source; attempt cross-agent list/read/replace/attach/delete and confirm refusal; attempt view-only delete/promote and no-permission gallery/content and confirm `403`; revoke a permission while the gallery is open and confirm the next request is refused; delete an agent and confirm its artifacts, content, references and events are gone while another agent is untouched.

### Implementation for User Story 5

- [x] T066 [US5] Add one shared actor-display helper in `src/utils/mod.rs` (resolve `Requester::User` to the username and `Requester::Agent` to the agent name, falling back to the id) and reuse it from `src/notifications/mod.rs` and `src/artifacts/mod.rs`; add `actor_name` to artifact event/reference API views and detail/activity responses without storing it in the event row. Add tests for user/agent/missing actors and a deleted artifact's event still resolving.
- [x] T067 [US5] Enforce cross-agent isolation in `src/artifacts/mod.rs` and the artifact tools: every operation filters by the calling `agent_id`; a name/identity that exists only on another agent returns the same “not found” result as a missing one; `send_attachment`, `promote_artifact`, `update_artifact`, `load_artifact_as_session_file`, `list_artifacts`, memory attach/detach and every service lookup refuse cross-agent access. Add service-level and tool-level tests that cannot distinguish “exists on B” from “does not exist”.
- [x] T068 [P] [US5] Add toolset-boundary assertions in `src/agents/tools/mod.rs`: no ordinary, scheduled, background or dream toolset contains a delete-artifact tool; no dream toolset contains `update_artifact`, `load_artifact_as_session_file`, `send_attachment` or any session-file clear/delete tool; the artifact tools present in dreams are exactly the set in T027/T061. Add the `every_tool_is_in_its_category` entries and telemetry labels for every new tool.
- [x] T069 [US5] Complete agent-deletion artifact cleanup: in `src/storage/sqlite/agent.rs` delete the agent's `artifact`, `artifact_event` and `artifact_reference` rows inside the existing `delete_agent` transaction (alongside session-file rows), and in `src/agents/mod.rs` call `Artifacts::delete_agent_files(agent_id)` (new method in `src/artifacts/mod.rs` that removes `artifacts/<agent_id>`) after a successful storage delete, logging disk failures. Do not remove another agent's directory or rows. Add a test with two agents sharing no artifacts.
- [ ] T070 [P] [US5] Add the permission-matrix tests for `src/artifacts/access.rs` and `src/channels/http/api/v1/agents/artifacts.rs`: `all_agents:view_artifacts` any agent, `owned_agents:view_artifacts` only owned, `all_agents:manage_artifacts`/`owned_agents:manage_artifacts` imply view, system bypass, view-only delete/promote refused, no-permission gallery/content refused, and a revoked permission absent from the next request. Verify the WebUI mirror in `webui/app/lib/access.test.ts` agrees with the server rule.
- [ ] T071 [P] [US5] Add stale-link and cleanup-failure coverage in `src/storage/memory_bundle.rs` and `src/agents/tools/vector_memory/`: a memory whose artifact link cleanup failed reads the artifact as deleted by name without breaking the body, a later detach/reconcile can remove it, and a failed cleanup never removes the memory body. Add a test that the artifact row is deleted before memory links are detached so a concurrent attach cannot validate a deleted artifact.
- [ ] T072 [US5] Add event-retention coverage in `src/storage/sqlite/artifact.rs` and `src/artifacts/mod.rs`: `deleted` events survive the artifact row and content removal, activity lists them newest first with actor/action/name/time/source, deleting the agent removes them, and no event ever stores content. Add a detail/activity test after deletion.
- [x] T073 [US5] Record the pre-1.0 compatibility paths for #82 in `specs/035-add-artifacts/quickstart.md` (Recorded run) and in the issue #82 checklist: `MemoryAttachment::LegacySessionFile` and its `memory_read` copy/legacy detach branch, the legacy message/reply `Local`/`Url`/`Bytes`/`Base64` paths, and the `422 use_attach_artifact` guard. State that no legacy data is migrated and that each path must be migrated or intentionally dropped before removal.

**Checkpoint**: US5 is complete when provenance is visible, every boundary/permission refusal is tested, and agent deletion leaves no artifact data behind.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, verification, the constitution's dummyplug e2e gate and release hygiene.

- [x] T074 [P] Add `docs/src/configuration/artifacts.md` covering artifact storage under `artifacts/`, promotion/copy semantics, replacement/version behavior, explicit clearing and no automatic GC, the four permissions, memory attachments, reference behavior after replace/delete, Dream source access and the legacy note; add the page to `docs/src/SUMMARY.md` and mention the `artifacts/` directory in `docs/src/configuration/index.md`.
- [x] T075 [P] Update `docs/src/configuration/memory.md` (artifact-only memory attachments, legacy session-file note, attach/detach), `docs/src/configuration/tools-embedding.md` (artifact tools and `send_attachment`), `docs/src/configuration/channels.md` (`/clear-files`/`/clear_files`), and `docs/src/api-integration/rest-api.md` (artifact routes, memory attach/detach, clear, `VizierAttachmentContent::Artifact`, four permissions). Keep the API examples aligned with `contracts/`.
- [x] T076 [P] Update `CLAUDE.md`'s architecture notes for `src/artifacts/`, `src/session_files/`, `ArtifactStorage`, the resolver, `MemoryAttachment`, the four permissions, Dream source access and the no-automatic-GC decision.
- [x] T077 Run `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test` across `src/` and `Cargo.toml`; fix every failure and any `unwrap()`/`expect()` outside tests or `main` bootstrap.
- [ ] T078 Run `cd design && npm run typecheck && npm run build-storybook` and `cd webui && npm test && npm run typecheck`; fix the sidebar story, generated client and gallery/message helper failures.
- [ ] T079 Run `cd webui && npm run e2e -- artifacts.spec.ts` when a browser is available; record the manual desktop/phone checks in `specs/035-add-artifacts/quickstart.md` if it is not.
- [ ] T080 Walk `specs/035-add-artifacts/quickstart.md` §1–§13 against a running binary with an offline `dummyplug` agent: promote/replace/clear, message attachments, memory attach/detach and legacy detach, artifact-delete memory-link removal, conversation/agent deletion, the four-permission matrix, Dream-source tests and the SQLite/filesystem checks; record commands, observed values and LIVE-only steps in the “Recorded run” section.
- [x] T081 Verify the no-automatic-collection and storage-separation invariants: `grep -R "retention\|garbage collect\|GC" src/session_files src/artifacts` finds no collection code, `find <workspace>/uploads -path '*/artifacts/*'` returns nothing, artifact content is only under `artifacts/<agent_id>/<artifact_id>/content.v<n>`, and a clear/conversation/agent deletion never removes a shared blob still referenced elsewhere.
- [x] T082 Commit in cascade order `design/` submodule → `webui/` submodule → vizier root (update the `design`/`webui` gitlinks with `git add design webui`, plus backend/docs); flag the memory attachment tool/API change and the `send_attachment` argument change as `[**breaking**]` in the conventional commit, and confirm the `#82` checklist has the T073 entries.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies; T001–T002 can start immediately.
- **Foundational (Phase 2)**: Depends on Setup; T003–T018 block every user story. T003/T004 can run in parallel, then T005–T006 depend on both; T007/T008 are independent of the SQLite work but T009/T010 depend on T008; T011 depends on T005/T007; T012/T013 are independent; T014–T015 depend on T013; T016 depends on T010; T017 depends on T003/T005/T008; T018 depends on T011/T016.
- **User Stories (Phases 3–7)**: All start after Foundational. They can proceed in parallel by staff, but the integration points below must be respected.
- **Polish (Phase 8)**: Depends on the desired stories being complete; T074/T075/T076 can run in parallel with late code work, but T077–T082 need the code final.

### User Story Dependencies

- **US1 (P1)**: Foundational only. It creates the artifact rows/content every other story uses; T028 (person promotion endpoint) is the backend hook for US3's side-panel Promote.
- **US2 (P1)**: Foundational; its list/detail/activity can be developed against seeded artifact rows. Integration with real artifacts needs US1's T019/T020; delete uses T031 and the foundational memory link detach (T015).
- **US3 (P2)**: Foundational; its clear/delete paths are independent of artifacts. T050's Promote action needs US1's T028; the Clear action does not.
- **US4 (P2)**: Foundational; T054/T056 need US1's artifacts, and T062's artifact-delete link behavior uses T013/T015. T060's request guard is independent of US1.
- **US5 (P3)**: T066/T072 need US2's detail/activity (T030) and US1/US2's events; T069 needs the agent-deletion hook from US3 T046; T067/T068/T070 are independent after the relevant story code exists.

### Within Each User Story

- Models/value types before services, services before endpoints/tools, backend before UI integration.
- Tests colocated with their implementation must fail before the implementation is considered done; the checkpoints name the independent verification.
- US1: T019–T021 before T022/T024; T023 before T025/T026; T028 after T019.
- US2: T029–T032 before T041; T033 after T032; T034–T038 can run in parallel, then T039, then T040/T042.
- US3: T043 before T044–T046; T047 before T048/T049; T050 after T044 and T028; T051–T053 after the respective code.
- US4: T054–T058 before T059/T063; T060 independent; T061 after T057/T058; T064 after T056; T065 after T059.
- US5: T066–T068 before T070/T072; T069 after T046; T071 after T015/T031; T073 last.

### Parallel Opportunities

- Setup: T001 and T002 touch different files, but T002's types are referenced by T001 only as declarations; run T001 then T002.
- Foundational: T003/T004 in parallel; T007/T008 in parallel with T005/T006; T012/T013 in parallel; T014/T015 after T013; T016 after T010; T017 after T003/T005/T008; T018 after T011/T016.
- US1: T021/T023/T026/T028 touch separate files and can run in parallel; T022/T024 share `src/agents/tools/artifacts/mod.rs` so sequence them; T025 and T026 both touch dream plumbing, sequence T023 before either.
- US2: T034 (design) is independent of the Rust work; T035–T038 touch separate WebUI files and can run in parallel; T039 follows T035–T038; T040/T042 follow T039; T041 follows T029–T032.
- US3: T048 and T049 (Discord/Telegram) are separate files after T047; T050 (WebUI) can run beside T044 (HTTP); T052/T053 are test-only and can run after their code.
- US4: T054, T055, T057, T058, T059, T060 touch mostly separate files and can run in parallel after their prerequisites; T061 after T057; T064 after T056.
- US5: T066/T067/T068/T070/T071 can run in parallel after their story code; T069 depends on T046; T072 depends on T030.
- Polish: T074/T075/T076 are independent docs edits; run them in parallel, then T077–T082.

### Parallel Example: US1

```text
# After Foundational:
Task: "T019 Implement Artifacts::promote in src/artifacts/mod.rs"
Task: "T023 Add dream_sources and resolve_source_session in src/agents/tools/mod.rs"
Task: "T026 Carry Dream source sessions in src/scheduler/dream/mod.rs and src/agents/agent/mod.rs"
Task: "T028 Add POST .../files/promote in src/channels/http/api/v1/agents/channel.rs"
```

### Parallel Example: US2

```text
# After T029–T033:
Task: "T034 Add artifacts to the design Sidebar"
Task: "T035 Add canViewArtifacts/canManageArtifacts in webui/app/lib/access.ts"
Task: "T037 Add webui/app/api/endpoints/artifacts.ts and regenerate schema.d.ts"
Task: "T038 Add webui/app/lib/artifacts.ts and its tests"
```

### Parallel Example: US3

```text
# After T047:
Task: "T048 Handle /clear-files in src/channels/discord/mod.rs"
Task: "T049 Handle /clear_files in src/channels/telegram/mod.rs"
Task: "T050 Add Promote and Clear session files to webui/app/screens/chat/SessionFilesPanel.tsx"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (T001–T002).
2. Complete Phase 2 (T003–T018); do not start a story until the artifact storage/content/resolver/permission foundation is green.
3. Complete US1 (T019–T028): agent promotion/replacement, the Files tools, Dream-source promotion and the person-facing promote endpoint.
4. **STOP and validate**: run the US1 portion of `quickstart.md` §1–§2 and the Dream-source tests; confirm bytes after a clear, version switching, failed replacement and no source mutation.
5. Deploy/demo the promotion foundation if ready.

### Incremental Delivery

1. Setup + Foundational → artifact tier exists and is permissioned.
2. US1 → durable artifacts with promotion/replacement (MVP).
3. US2 → gallery/browse/download/detail/activity/delete.
4. US3 → explicit clear, conversation/agent cleanup and the channel command.
5. US4 → artifact-only message/memory attachments and delivery behavior.
6. US5 → audit, cross-agent boundaries, permission matrix and final deletion guarantees.
7. Polish → docs, full `cargo`/design/webui gates and the recorded dummyplug run.

### Parallel Team Strategy

- One developer owns Phase 1–2 (shared storage/resolver foundation).
- Once Foundational is done:
  - Developer A: US1 (artifact service/tools/Dream source).
  - Developer B: US2 backend/HTTP (T029–T033, T041) and the design sidebar (T034).
  - Developer C: US2 WebUI (T035–T039, T042) after the endpoint shape is agreed.
  - Developer D: US3 clear/channel/WebUI.
  - Developer E: US4 memory/message attachments.
  - Developer F: US5 audit/boundaries/tests, then Polish docs.
- Rebase on the shared `src/artifacts/mod.rs`, `src/agents/tools/mod.rs` and `src/channels/http/api/v1/agents/mod.rs` before merging; those are the high-conflict files.

---

## Notes

- [P] means different files and no dependency on an incomplete task; shared-file tasks are deliberately sequential.
- Tests are included because `plan.md`'s Testing field and each phase explicitly require the listed Rust, HTTP, WebUI, Storybook and quickstart checks; the constitution's dummyplug gate is T080.
- No automatic session-file GC, quota, artifact version history/restore, public links, generated-UI artifact media, retroactive promotion or legacy-memory migration is in scope. The legacy paths are read-only and tracked for #82 in T073.
- The memory attachment tool/API shape and `send_attachment` argument change are deliberate pre-1.0 breaking changes; flag them in commit messages and docs.
- Design changes are made in `design/` first, then `webui/`, then the vizier gitlinks (T082). The landing site is not touched.
- A task is done only when its colocated tests pass and the story checkpoint's independent verification has been run.
