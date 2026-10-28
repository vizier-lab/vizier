# Implementation Plan: Permanent Artifacts

**Branch**: `035-add-artifacts` | **Date**: 2026-10-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/035-add-artifacts/spec.md`

## Summary

Add a permanent file tier — **artifacts** — beside transient session files. An agent or a person promotes a session file into a named, agent-owned artifact; promotion **copies** the bytes into `<workspace>/artifacts/<agent_id>/<artifact_id>/` so the source can be cleared without touching the artifact. Artifacts can be replaced (version counter, overwrite-in-place semantics), listed/downloaded/previewed/deleted in a new **Artifacts** gallery, attached to outgoing messages by identity, and attached to memories through dedicated `attach_artifact_to_memory` / `detach_attachment_from_memory` tools and API operations. `memory_write` loses its attachments parameter and never changes attachments. Session files remain transient and are removed only by an explicit **Clear session files** action, `/clear-files` (`/clear_files` on Telegram), conversation deletion or agent deletion; there is **no automatic GC** in this release. Message and memory attachments made before this feature keep working as legacy references, are tracked for removal in #82, and are not migrated.

The plan's decisions are detailed in [research.md](./research.md) (R1–R15). The four that shape the implementation:

1. **Artifact metadata in SQLite, content on disk** (R1/R2). `artifact`, `artifact_event` and `artifact_reference` tables; each artifact's bytes are a versioned file (`content.v<n>`) under its own `artifacts/` directory. A replacement writes the next version file and switches the row to it, so no live file is overwritten and Windows does not need replace-on-rename.
2. **One resolver for artifact bytes** (R3). `VizierAttachmentContent::Artifact { agent_id, artifact_id, available }` carries identity, not a content path. `VizierAttachment::to_part` and `FileManager::resolve` use an `AttachmentResolver` backed by `Artifacts`; history reads fill `available` so the WebUI can show a tombstone.
3. **A reference index, not a scan** (R4). `artifact_reference` rows are written with message history and memory attachments, so the gallery can show "used by" and the delete dialog can count references without scanning history JSON or every memory document. Message refs are removed with their session; memory refs are reconciled when an agent starts.
4. **Memory attachments are a separate operation** (R5–R7). `MemoryAttachment` is an untagged `Artifact` / `LegacySessionFile` enum; `memory_write` preserves whatever is already attached. Attach/detach tools and API operations rewrite only the attachment list, record a memory revision, and leave the body and embeddings alone.

## Technical Context

**Language/Version**: Rust edition 2024 (single `vizier` binary); TypeScript 5.9 / React 19 / React Router 7 (SPA) / Tailwind v4 in `webui/`, on the `design/` submodule.

**Primary Dependencies**: No new Rust crate and no new WebUI dependency. Reuse `axum`, `rusqlite` (bundled SQLite with JSON1), `chrono`, `uuid`, `serde`/`serde_json`/`serde_yaml`, `utoipa`, `tracing`, `tokio`, `parking_lot`, `arc-swap`, and the existing `@vizier/design` primitives (`Sidebar`, `PageFrame`, `FilterTabs`, `Table`, `FileRow`, `FilePreview`, `Modal`, `ConfirmDialog`, `EmptyState`, `LoadMore`, `Skeleton`, `TextLink`).

**Storage**: Embedded SQLite plus the workspace filesystem. Three new tables — `artifact`, `artifact_event`, `artifact_reference` — and no changes to existing tables. Content lives under `<workspace>/artifacts/<agent_id>/<artifact_id>/content.v<version>`; session files stay under `<workspace>/uploads/<file_id>/`. Memory concept frontmatter changes from `Vec<VizierAttachment>` to a backward-compatible `Vec<MemoryAttachment>`; old YAML parses as the legacy variant and is not migrated. Shared session-file blobs are detected with SQLite `json_extract` over `session_file.data`; no column is added.

**Testing**: `cargo test` and `cargo clippy` (artifact storage CRUD/filters/cursors/references/events, content paths and version switching, memory frontmatter compatibility and attach/detach, reference sync/reconcile, shared-blob clear, command confirmation, attachment resolver, history availability fill); `cd design && npm run typecheck && npm run build-storybook` for the sidebar story; `cd webui && npm test && npm run typecheck` for gallery/message helpers; `cd webui && npm run e2e` for the gallery, side-panel actions and message tombstone when a browser is available. End-to-end: dummyplug and HTTP/DB steps in [quickstart.md](./quickstart.md) §1–§10.

**Target Platform**: Linux, macOS and Windows via the single binary; modern evergreen browsers for the WebUI. Paths use `PathBuf`/the workspace; no OS-specific code. Versioned content files avoid replace-on-rename differences; `artifacts/` and `uploads/` are both under the configured workspace.

**Project Type**: Web application inside the existing single-binary Rust service (Rust backend + `webui` and `design` submodules).

**Performance Goals**: the gallery's first page under 1 s p95 with 500 artifacts (FR/SC-011); indexed keyset list by `(updated_at, id)`; search/filter counts over the agent's artifacts only; availability is one `metadata` check per returned page item, not per stored row; history availability fill is one artifact lookup per attachment on the page; attach/detach rewrites one concept document and records one revision; no new background jobs, and the memory reference reconcile is folded into the existing per-agent memory pass rather than adding a second startup scan.

**Constraints**: No new external service, queue, cache or required network; no automatic session-file GC; no artifact version history or restore; no quotas; no public links; no generated-UI artifact media; no retroactive promotion; artifacts never under `uploads/`; legacy message and memory attachments keep working and are not migrated; the memory attachment tool/API shape is a deliberate pre-1.0 breaking change; legacy paths are tracked in #82; design → webui → vizier commit order for submodule changes.

**Scale/Scope**: Three SQLite tables, one artifact content store, one `Artifacts` service, one `SessionFiles` service, one `AttachmentResolver`, four artifact tools plus two memory attachment tools, one memory storage extension, roughly nine HTTP routes (gallery list/detail/content/activity/delete, session promote/clear, memory attach/detach), one WebUI gallery page, two chat side-panel actions, one channel command, and docs/OpenAPI updates.

## Constitution Check

*GATE: passes before Phase 0 research; re-checked after design below.*

| Principle | Assessment |
|---|---|
| **I. Lean by default** | PASS. No new crate, external service or daemon. Three tables and two services are the smallest shape that satisfies permanence, usage/audit and safe clearing. No artifact version history, quota or automatic GC. The versioned content file avoids a cross-platform atomic-replace abstraction. The reference index is justified by FR-026/FR-027 (where used, delete count); scanning all history and memory documents per detail view would be slower and more code. |
| **II. DRY via traits** | PASS. `ArtifactStorage` is one storage concern implemented once by `SqliteStorage` and composed into `VizierStorageProvider`; **one** `Artifacts` service owns promote/replace/delete/read/list/resolve; **one** `SessionFiles` service is shared by the HTTP side panel, both channel readers, conversation deletion and agent deletion; **one** `MemoryAttachment` type is used by the memory document, revisions and API; **one** `AttachmentResolver` is used by model parts and channels; command confirmation is one shared helper, not duplicated per platform; the file range parser is shared; actor display names come from one `utils` helper reused by notifications and artifacts. No kind/type `match` is added to dispatch. |
| **III. Self-contained runtime** | PASS. Metadata is in embedded SQLite and content is in the workspace `artifacts/` directory; no external database, object store, broker or network is required. The gallery is served by the same binary. |
| **IV. Portability** | PASS. Content filenames are version-based (`content.v<n>`) and never replace a live file, avoiding platform-specific rename-over-existing behavior. No OS-specific path or process calls; names are metadata and never user-controlled filesystem paths. No native dependency is added. |
| **V. Unified errors & observability** | PASS. Storage follows `anyhow::Result`; services/tools return `crate::Result`/`VizierError`; all logging uses `tracing`; no `unwrap` outside tests. Cleanup/disk failures are logged without failing a completed artifact operation. |
| **Workflow gates** | PASS. `cargo fmt`, `cargo clippy`, `cargo test`, design Storybook, WebUI tests/typecheck, the dummyplug/HTTP quickstart and docs/OpenAPI updates are phases below. The memory tool/API attachment change is a deliberate pre-1.0 breaking change and is called out in the commit and docs. Legacy session-file attachment read paths are tracked in #82. Design changes are made in `design/` first, then `webui/`, then the vizier pins. |

**Post-design re-check**: no violations. Watch items: (a) an artifact row must never point at a content version that was not fully written; (b) a clear must not delete a shared `uploads/` blob still used by another conversation; (c) memory content updates must preserve attachments while attach/detach must not re-chunk or re-embed the body; (d) the message attachment tombstone must not disappear from the UI when its content 404s; (e) channel size/read failures must be reported without dropping the reply text; (f) legacy session-file attachment paths stay read-only and are not silently migrated.

## Project Structure

### Documentation (this feature)

```text
specs/035-add-artifacts/
├── plan.md                         # this file
├── research.md                     # R1–R15 decisions and alternatives
├── data-model.md                   # tables, entities, content layout, state transitions
├── quickstart.md                   # dummyplug/API/DB/WebUI validation
├── contracts/
│   ├── artifacts-api.md            # gallery + promote/clear HTTP contracts
│   ├── artifact-storage.md         # trait, DDL, content paths, references, deletion
│   ├── agent-tools-and-memory.md   # artifact tools, send_attachment, memory attach/detach
│   ├── session-files.md            # clear/conversation/agent deletion + channel command
│   └── webui.md                    # gallery, side panel, message attachments, a11y
├── checklists/requirements.md
└── tasks.md                        # /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
src/
├── schema/
│   ├── artifact.rs                 # ArtifactId, Artifact, ArtifactKind, ArtifactOrigin,
│   │                               #   ArtifactEvent, ArtifactReference, filters/page/cursor
│   ├── mod.rs                      # re-exports
│   ├── request.rs                  # VizierAttachmentContent::Artifact + AttachmentResolver
│   ├── response.rs                 # to_tool_result takes the resolver
│   ├── storage.rs                  # MemoryAttachment (Artifact | LegacySessionFile); Memory uses it
│   ├── commands.rs                 # MemoryOpRequest::AttachArtifact/DetachAttachment; Write loses attachments
│   └── history.rs                  # attachment iteration helpers for refs/availability
├── artifacts/
│   ├── mod.rs                      # Artifacts service: promote/replace/list/detail/delete/
│   │                               #   activity/content/resolve/delete_agent
│   └── paths.rs                    # artifacts/<agent>/<id>/content.v<n>, version writes, deletes
├── session_files/
│   └── mod.rs                      # SessionFiles: count/clear/delete_conversation/clear_agent,
│                                   #   active-turn check, shared-blob safety
├── storage/
│   ├── artifact.rs                 # ArtifactStorage trait + VizierStorage forwarding
│   ├── mod.rs                      # add ArtifactStorage to VizierStorageProvider
│   ├── session_file.rs             # list_agent_files, delete records, reference count helpers
│   ├── sqlite/
│   │   ├── artifact.rs             # DDL, CRUD, keyset list, refs/events, sync content lookup
│   │   ├── mod.rs                  # init_artifact_schema; delete_session removes records/refs
│   │   ├── agent.rs                # delete agent artifacts/events/refs/session_file rows
│   │   ├── history.rs              # save refs with history; fill attachment availability
│   │   ├── session_file.rs         # agent listing/records/reference count
│   │   └── memory.rs               # MemoryStorage attach/detach forwarding
│   ├── memory.rs                   # MemoryStorage write signature loses attachments; attach/detach
│   └── memory_bundle.rs            # attach/detach, preserve attachments, revision, refs, reconcile
├── agents/
│   ├── agent/mod.rs                # AttachmentResolver on model/tool parts
│   ├── memory_ops.rs               # dispatch AttachArtifact/DetachAttachment
│   ├── mod.rs                      # agent delete clears session blobs + artifacts/ dir
│   └── tools/
│       ├── mod.rs                  # Files category + tool registration/table/telemetry labels
│       ├── artifacts/mod.rs        # promote_artifact, update_artifact, list_artifacts,
│       │                           #   load_artifact_as_session_file
│       ├── session_files/mod.rs    # send_attachment now takes an artifact
│       ├── session_files/send_attachment.rs
│       └── vector_memory/
│           ├── mod.rs              # memory_write loses attachments; memory_read resolves artifacts
│           └── attachments.rs      # attach_artifact_to_memory / detach_attachment_from_memory
├── channels/
│   ├── commands.rs                 # ClearConfirmations helper + builtin command entries
│   ├── discord/mod.rs              # /clear-files handler + attachment failure notice
│   ├── telegram/mod.rs             # /clear_files handler + attachment failure notice
│   └── http/
│       ├── mod.rs                  # OpenAPI paths/schemas
│       └── api/v1/
│           ├── mod.rs              # nest artifact routes
│           ├── range.rs            # shared Range parser (moved from files.rs)
│           ├── files.rs            # use shared range parser
│           └── agents/
│               ├── artifacts.rs    # gallery routes
│               ├── channel.rs      # promote/clear session files routes
│               ├── memory.rs       # remove attachment fields; attach/detach routes
│               └── mod.rs          # expose edit-permission helper
├── file_manager/mod.rs             # resolve Artifact via the installed resolver; delete upload
├── dependencies.rs                 # construct Artifacts + SessionFiles; FileManager resolver
├── utils/mod.rs                    # shared Requester actor-name helper (notifications + artifacts)
└── storage/sqlite/mod.rs           # init_artifact_schema

design/src/components/Sidebar/
├── Sidebar.tsx                     # "artifacts" page under the agent's file/memory group
└── Sidebar.stories.tsx             # story option

webui/app/
├── api/endpoints/artifacts.ts      # list/detail/content URL/activity/delete/promote/clear
├── api/endpoints/memory.ts         # artifact attach/detach calls
├── api/frames.ts                   # VizierAttachment artifact content + tombstone state
├── api/schema.d.ts                 # regenerated from OpenAPI
├── routes.ts                       # :agentId/artifacts
├── routes/artifacts.tsx            # gallery page
├── routes/chat.tsx                 # pass session actions to the side panel
├── screens/artifacts/
│   ├── ArtifactsGallery.tsx        # list, filters, activity tab, states
│   ├── ArtifactDetail.tsx          # preview, provenance, references, delete
│   └── ArtifactPreviewDialog.tsx   # shared preview body for artifact files
├── screens/chat/SessionFilesPanel.tsx # Promote + Clear session files
├── screens/chat/ReplyMedia.tsx     # artifact URL + deleted tombstone
├── screens/chat/FilePreviewContent.tsx # extracted preview body shared by both dialogs
├── layouts/AppShell.tsx            # artifacts page path + agent page list
├── lib/frames.ts                   # files clear action (if needed)
├── lib/artifacts.ts                # kind/filter/URL helpers + tests
└── e2e/artifacts.spec.ts           # gallery, side panel, message tombstone, mobile

docs/src/
├── SUMMARY.md                      # Artifacts page entry
├── configuration/artifacts.md      # storage, promotion, replacement, clearing, permissions
├── configuration/index.md          # workspace layout adds artifacts/
├── configuration/memory.md         # artifact-only attachments, legacy note
├── configuration/tools-embedding.md# artifact tools, send_attachment
├── configuration/channels.md       # /clear-files
└── api-integration/rest-api.md     # artifact + memory attachment APIs, clear, attachment shape

CLAUDE.md                           # architecture notes for artifacts, session clearing, memory
```

**Structure Decision**: the feature stays inside the existing single binary and the existing WebUI-on-design layering. The backend adds one domain service (`src/artifacts/`) and one transient-file service (`src/session_files/`), a storage concern, a filesystem content store, and a small extension to memory. The design system gains one sidebar page (and its story); the WebUI gains one gallery page and two chat side-panel actions. There is no new service, crate, route namespace or external process. The `artifacts/` directory is separate from `uploads/` by construction.

## Phases

### Phase 0 — Schema, storage and reference index (R1, R2, R4, R10)

1. `schema/artifact.rs`: `Artifact`, `ArtifactKind`, `ArtifactOrigin`, `ArtifactEvent`, `ArtifactEventAction`, `ArtifactReference`, `ArtifactPage`, `ArtifactFilters`, cursor types; validation helpers for names and kinds; serde/JsonSchema/ToSchema derives and round-trip tests.
2. `storage/artifact.rs`: `ArtifactStorage` with create/replace/delete (each taking its event in the same transaction), get-by-id/get-by-name/list/stats, event list, reference upsert/list/count/delete, message-reference cleanup, agent cleanup, and the sync `artifact_content_file` lookup. Add it to `VizierStorageProvider` and forward every method from `VizierStorage`.
3. `storage/sqlite/artifact.rs`: DDL for `artifact`, `artifact_event`, `artifact_reference` with indexes; keyset list by `(updated_at, id)`; kind/date/search filters; `agent_total`/bytes; event and reference queries; JSON serialized `Requester`; tests for ordering, ties, filters, cursor, stats, referential cleanup.
4. `storage/sqlite/mod.rs`: `init_artifact_schema` from `init_schema`; `delete_session` also deletes the session's `session_file` rows and message references; `delete_agent` deletes artifacts, events, references and session-file rows.
5. `storage/sqlite/history.rs`: extract artifact attachments from saved request/response content and upsert reference rows in the same transaction; fill `available` on every history read with one artifact lookup per attachment; tests for refs, deleted artifacts and legacy rows.
6. `storage/session_file.rs` / `sqlite/session_file.rs`: `list_agent_files`, record deletion helpers, and a reference-count lookup by `file_id` (`json_extract`) for shared-blob safety; tests.

### Phase 1 — Content store, resolver and message attachments (R1–R3, R12)

1. `artifacts/paths.rs`: `artifacts/<agent_id>/<artifact_id>/content.v<n>` layout; create/write/read/delete; version file writes are new names, never an overwrite; `delete_agent` removes the whole agent directory; tests with temp workspaces.
2. `schema/request.rs`: add `VizierAttachmentContent::Artifact { agent_id, artifact_id, available }` and the `AttachmentResolver` trait; `to_part` resolves artifact bytes or a readable "no longer available" note; `NoopAttachmentResolver` for tests.
3. `schema/response.rs` / `agents/agent/mod.rs` / tests: thread the resolver through `to_message` and `to_tool_result`; all call sites pass the agent's `Artifacts` resolver; dummyplug/unit tests pass a no-op.
4. `file_manager/mod.rs`: install an optional `Arc<Artifacts>` after construction; `resolve` handles `Artifact`; add `delete_upload(file_id)`; add `read_upload(workspace, file_id)` as the shared upload reader used by `Artifacts`.
5. `dependencies.rs`: construct `FileManager`, `Artifacts`, `SessionFiles`; install the resolver on the file manager; expose the handles on `VizierDependencies`.
6. Tests: resolver current-version lookup, deleted artifact note, `FileManager` artifact bytes, version file switch.

### Phase 2 — Artifacts service, tools and memory attachments (R1, R5–R7, R13)

1. `artifacts/mod.rs`: promote (copy source bytes, validate, insert row/event), replace (new version file, row/event switch, old file removal), list (filters/cursor/stats), detail (references/events), delete (rows/refs/event/dir), activity, content path/read, availability, agent file cleanup; one mutation lock for artifact writes; resolve event actor names through the shared `utils` helper; tests.
2. `agents/tools/artifacts/mod.rs`: `promote_artifact {filename, name?}`, `update_artifact {name, filename}`, `list_artifacts`, `load_artifact_as_session_file {name}` in the **Files** category; register in `BaseTools::new`; add names to the category table/telemetry labels.
3. `send_attachment`: take an artifact name, resolve it, push an `Artifact` attachment; refuse a session-file name with promotion guidance; update descriptions and `utils` labels.
4. `schema/storage.rs`: `MemoryAttachment` untagged enum (`Artifact { artifact_id, name }` | `LegacySessionFile(VizierAttachment)`); `Memory.attachments` and generated frontmatter use it; `RevisionFrontMatter` follows; old YAML parses as legacy; tests.
5. `schema/commands.rs` / `agents/memory_ops.rs`: `MemoryOpRequest::Write` loses `attachments`; add `AttachArtifact` and `DetachAttachment` with `RevisionOrigin`; dispatch them.
6. `storage/memory.rs` / `memory_bundle.rs`: `write_memory` loses the attachments argument and preserves existing attachments on update; implement `attach_memory_attachment` (idempotent by artifact id) and `detach_memory_attachment` (artifact or legacy name); update `attachment_count`, node, revision, and `artifact_reference` rows; no re-chunk/re-embed; reconcile memory refs at agent start; convert the legacy filesystem migration's `Vec<VizierAttachment>` to `MemoryAttachment::LegacySessionFile` in `write_migrated_memory`; tests.
7. `agents/tools/vector_memory/attachments.rs`: `attach_artifact_to_memory` (one artifact, own agent, existing memory, idempotent, Memory category) and `detach_attachment_from_memory` (artifact or legacy, clear no-op); register both; exclude from `DREAM_TOOL_NAMES`; update `memory_write`/`memory_read` descriptions and output (artifact copies use the safe `artifact-<id><ext>` session name); `MemoryWriteArgs` denies unknown fields so an old `attachments` key is rejected instead of silently ignored.
8. Tests: promote/replace/delete, duplicate names, missing source, version/event/ref rows, memory preservation, attach/detach/idempotency, legacy YAML, dream exclusion.

### Phase 3 — Session clearing and deletion orchestration (R8, R9)

1. `session_files/mod.rs`: `SessionFiles::{count, clear, delete_conversation, files_for_agent, delete_blobs}`; active-turn check via `TurnStorage::pending_turns`; clear records first, then delete a blob only when no other `session_file` row references its `file_id`; conversation deletion clears files and calls `storage.delete_session`; agent cleanup collects ids, calls `storage.delete_agent`, then deletes blobs and `artifacts/<agent_id>`.
2. `channels/http/api/v1/agents/channel.rs`: `DELETE …/files` clear route (view + platform-chat permission; refuses active turns; returns count/bytes); `POST …/files/promote` (same permission; body `{filename, name?}`; records the person as actor).
3. `agents/mod.rs`: `handle_delete` collects the agent's session files before storage deletion, then removes blobs and the artifact directory after a successful storage delete; logs cleanup failures.
4. Tests: clear count, shared blob retained/removed, active turn refused, session deletion, agent deletion, promote endpoint permission.

### Phase 4 — HTTP artifacts API, memory endpoints and docs (R9–R11)

1. `api/v1/agents/artifacts.rs`: list/detail/content/activity/delete; `require_auth`, `user_can_view_agent` for reads, `user_can_edit_agent` for delete; keyset cursor; `available`; `Content-Disposition`/`Content-Type`; Range download via the shared parser; tests.
2. `api/v1/agents/memory.rs`: remove `attachments` from create/update requests and reject a present attachments list with `422 use_attach_artifact`; add `POST /doc/{bundle}/{*path}/attachments` (`{artifact}`) and `DELETE …/attachments?name=…`; map the updated memory to a `MemoryAttachmentView` (tagged `artifact`/`legacy_session_file`, with read-time availability); tests.
3. `channels/http/mod.rs` + `api/v1/mod.rs`: register routes and OpenAPI schemas; add `VizierAttachmentContent::Artifact`, `MemoryAttachment`, artifact types; run `scripts/openapi.mjs` to refresh `webui/openapi.json` and `schema.d.ts`.
4. Docs: `configuration/artifacts.md`, `SUMMARY.md`, `index.md` (`artifacts/`), `memory.md` (artifact-only attachments, legacy note, attach/detach), `tools-embedding.md`, `channels.md` (`/clear-files`), `rest-api.md` (artifacts, memory attach/detach, clear, attachment shape), and `CLAUDE.md`.
5. #82: confirm the existing legacy session-file attachment entry names the implemented types/paths; add anything new (e.g., the `MemoryAttachment::LegacySessionFile` read path and the `422 use_attach_artifact` refusal) without removing anything.
6. Request-attachment guard: in `POST /agents/{agent_id}/chat` and the WebSocket request path, inspect incoming attachments before `send_file_resolve`; a client-supplied `VizierAttachmentContent::Artifact` is refused (or must match the conversation's agent and the caller's access). A person's uploads remain session files; only the agent/tool response path creates artifact references. Add tests for the cross-agent id case.

### Phase 5 — Design and WebUI (R13)

1. `design/`: add `artifacts` to `SidebarPage` and its group (icon `folder`), update the Sidebar story; run design typecheck/storybook and commit the design submodule first.
2. `webui/app/routes.ts` / `layouts/AppShell.tsx`: `/:agentId/artifacts` route, `AGENT_PAGES`, `PAGE_PATH`, sidebar navigation.
3. `screens/artifacts/`: `ArtifactsGallery` (search, kind/date filters, artifact/activity tabs, list/table, total count/bytes, loading/empty/error, paging), `ArtifactDetail` (preview, origin, version, events, references, delete confirmation), `ArtifactPreviewDialog` sharing the extracted preview body with `FilePreviewDialog`.
4. `SessionFilesPanel`: add per-row **Promote** (name dialog defaulting to the filename; call the promote endpoint; toast) and **Clear session files** (ConfirmDialog with count, artifact/legacy warning; call clear; dispatch files empty; disabled while the turn is running).
5. `ReplyMedia` / `frames.ts`: build the artifact content URL, render the existing image/audio/video/file previews for available artifacts, and show a named tombstone when `available === false` or the content request 404s; keep session-file attachments unchanged.
6. Tests: `lib/artifacts` kind/filter/URL/tombstone helpers; WebUI typecheck/test; Playwright gallery (filter/search/preview/detail/delete/permission), side panel (promote/clear/confirm), message tombstone and mobile layout. Update the agent config Tools "Always on" copy to mention artifacts alongside session files.

### Phase 6 — Channel command and delivery (R8, R12)

1. `channels/commands.rs`: add the builtin entry in the Discord/Telegram rules (`clear-files` / `clear_files`) and a shared `ClearConfirmations` two-step token with a 60 s TTL and tests.
2. Discord: handle `/clear-files` (count + prompt, then `confirm`), call `SessionFiles::clear`, reply with the count or the active-turn refusal; update the command list.
3. Telegram: handle `/clear_files` and `/clear_files confirm`, same prompt/count/refusal; update `setMyCommands` through the existing builder.
4. Discord/Telegram send loops: when `FileManager::resolve` or the platform send fails for an artifact (deleted, too large, unavailable), send a short explicit notice naming the file after the reply text; never silently drop it.
5. Tests: command list entries/descriptions, confirmation TTL, clear via both command routes with a fake session, oversized attachment notice (unit-level where the platform call is mockable; live channel check noted in quickstart).

### Phase 7 — Verification and cleanup (R14, R15)

1. Run the quickstart: dummyplug promote/replace/clear, message attachments, memory attach/detach and legacy detach, conversation/agent deletion, filesystem and SQLite checks, gallery and phone checks; mark Discord/Telegram live checks as live-only.
2. `cargo fmt`, `cargo clippy --all-targets`, `cargo test`; design typecheck/storybook; `cd webui && npm test && npm run typecheck && npm run e2e` (if a browser is available).
3. Verify zero automatic session-file deletion (no retention code/jobs), zero artifact content under `uploads/`, no new network at startup, and that a clear/delete never breaks a shared blob.
4. Commit order: design submodule → webui submodule → vizier root (backend, docs, gitlinks). Flag the memory attachment tool/API change and `send_attachment` argument change as `[**breaking**]`.

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| A crash leaves an unreferenced `content.v<n>` after a replacement. | The row switch is the commit point; the previous file is deleted after it. Unreferenced version files are removed on the next replace/delete of that artifact, and the whole artifact directory is removed on delete. Log the cleanup failure rather than losing the artifact. |
| A shared `uploads/` blob is deleted with one conversation and breaks another. | `SessionFiles` deletes a blob only when no other `session_file` row has the same `file_id` (`json_extract`); test the shared and last-reference cases. |
| Memory reference rows drift from the frontmatter after a crash or import. | Each write/attach/detach syncs rows; agent startup reconciles memory references from the concept documents; tests cover import and recovery. |
| `memory_write` drops attachments while the user edits content. | The write path reads and preserves existing `MemoryAttachment`s; tests cover content updates with artifact and legacy attachments. |
| Attaching to memory triggers a re-chunk/re-embed or a content revision that says the body changed. | Attach/detach only rewrite frontmatter, node count, references and a revision; they do not call the chunker/indexer. |
| The WebUI loses the tombstone when an artifact's content 404s. | History fill sets `available`, and `ReplyMedia` treats `available === false` or a failed content load as a named unavailable file; e2e covers it. |
| A channel silently drops a too-large/deleted artifact. | The send loops retain the reply text and send an explicit notice on resolve/send failure; the artifact remains in the gallery. |
| The memory API silently ignores an old `attachments` field. | Create/update explicitly reject a present attachments list with `422 use_attach_artifact`; the tool schema simply has no attachments argument. |
| Artifact names are used as filesystem paths. | Names are validated (no separators/control characters) and content files use `content.v<n>` under an id directory; the display name is metadata. |
| Concurrent replacements produce mixed content. | One in-process mutation lock per artifact; each version is a new file, and the row switch is a single SQL update in a transaction with its event. |
| The gallery's "used by" list shows deleted conversations. | Message refs are deleted with their session; memory refs are deleted with the memory or detach; the artifact delete removes all remaining refs. |

## Complexity Tracking

No Constitution Check violations. No new dependency, external service, background daemon, automatic GC, retention setting or artifact version history. The reference index and the separate memory attach/detach operations are feature requirements (usage/delete counts and non-clobbering content updates), not speculative abstractions. The versioned content file is the smallest portable way to replace artifact bytes safely. The legacy session-file attachment paths are intentionally kept for pre-035 data and are tracked for removal in #82.
