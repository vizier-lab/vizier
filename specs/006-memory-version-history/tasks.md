# Tasks: Version History for CORE.md and Memories

**Input**: Design documents from `/specs/006-memory-version-history/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (http-api.md, storage-trait.md, webui.md), quickstart.md

**Tests**: The plan commits to a small set of `cargo test` unit tests for the pure/sqlite helpers (plan.md → Testing). They are listed as normal tasks inside the story that owns the code, not as a TDD-first phase. Runtime behavior is verified manually per quickstart.md, as the constitution requires.

**Organization**: Tasks are grouped by user story. US1 (capture) is the MVP; US2 (browse/diff) and US3 (rollback) build on it but each is independently verifiable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

Single Rust project at repo root (`src/`), bundled WebUI in `webui/app/`. Per plan.md: CORE and memory history are **two independent implementations** — CORE on `AgentStorage` (`src/storage/agent.rs` ↔ `src/storage/sqlite/agent.rs` + `src/storage/sqlite/core_revision.rs`), memory on `MemoryStorage` (`src/storage/memory.rs` ↔ `src/storage/memory_bundle.rs` + `src/storage/sqlite/memory_revision.rs`). Only kind-agnostic value types (`src/schema/revision.rs`) and the line-diff engine (`src/storage/diff.rs`) are shared. One WebUI component (`webui/app/components/VersionHistory.tsx`) serves both.

---

## Phase 1: Setup

**Purpose**: New dependency and the shared value types every later task uses.

- [ ] T001 Add `similar = { version = "2", default-features = false, features = ["text"] }` to `[dependencies]` in `Cargo.toml` and run `cargo fetch` (research Decision 4; verify `cargo build` still succeeds on the default target)
- [ ] T002 [P] Create `src/schema/revision.rs` with the shared value types from data-model.md: `RevisionActor` (`Agent | User { user_id, username } | System`, `#[serde(tag = "type", rename_all = "snake_case")]`), `RevisionTrigger` (`Conversation | Dream | WebUi | Api | Import | Rollback { restored_from: i64 } | Baseline`, same serde attrs), `RevisionOrigin { actor, trigger }` with `system(trigger)` and `with_trigger(trigger)`, plus `RevisionDiff`, `DiffHunk`, `DiffLine` (`op: "equal" | "insert" | "delete"`), `RollbackResponse { no_change, new_seq, restored_from }`. All derive `Debug, Clone, Serialize, Deserialize`; API-facing ones also `utoipa::ToSchema`
- [ ] T003 [P] In the same `src/schema/revision.rs` add the CORE API types `CoreRevisionSummary { seq, actor, trigger, created_at: DateTime<Utc>, is_current, size_bytes }`, `CoreRevision` (summary fields + `content: String`), `PaginatedCoreRevisions { revisions, total, offset, limit }`, and the memory API types `MemoryRevisionSummary` (summary fields + `deleted: bool`), `MemoryRevision` (+ `content: Option<String>`, `title: Option<String>`, `tags: Vec<String>`), `PaginatedMemoryRevisions` — all `Serialize + Deserialize + Clone + Debug + utoipa::ToSchema`
- [ ] T004 Register the module: add `pub mod revision;` and `pub use revision::{…all public types…};` in `src/schema/mod.rs`
- [ ] T005 [P] Add `pub enum AuthMethod { Jwt, ApiKey }` and field `pub auth_method: AuthMethod` to `AuthenticatedUser` in `src/channels/http/auth/mod.rs`; set `auth_method: AuthMethod::Jwt` in the `"bearer"` branch and `AuthMethod::ApiKey` in the `"apikey"` branch of `src/channels/http/auth/middleware.rs`; fix any other `AuthenticatedUser { .. }` constructions the compiler reports
- [ ] T006 Add `RevisionOrigin::from_session(&VizierSession) -> Self` (actor `Agent`; trigger `Dream` when `session.1` is `VizierChannelId::Dream(..)`, else `Conversation`) and `RevisionOrigin::from_user(&AuthenticatedUser) -> Self` (actor `User { user_id, username }`; trigger `WebUi` for `AuthMethod::Jwt`, `Api` for `AuthMethod::ApiKey`) in `src/schema/revision.rs`, with a `#[cfg(test)]` test asserting the Dream/Conversation mapping (quickstart "Unit tests to expect")

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Tables, per-kind sqlite helper modules, and the shared diff engine. Nothing is recorded yet; this phase only makes recording possible.

**⚠️ CRITICAL**: US1–US3 all depend on this phase.

- [ ] T007 Add the `core_revision` and `memory_revision` `CREATE TABLE IF NOT EXISTS` statements and their indexes (`idx_core_rev_doc`, `idx_mem_rev_doc`, `idx_mem_rev_agent`) exactly as written in `contracts/storage-trait.md` to the `execute_batch` in `SqliteStorage::init_schema` in `src/storage/sqlite/mod.rs`
- [ ] T008 [P] Create `src/storage/sqlite/core_revision.rs` (CORE only) with `CoreRevisionRow { seq, content: String, actor: RevisionActor, trigger: RevisionTrigger, restored_from: Option<i64>, created_at: i64 }`, row ↔ column mapping helpers (`actor_kind`/`actor_id`/`actor_name` ↔ `RevisionActor`; `trigger`/`restored_from` ↔ `RevisionTrigger`), and synchronous `&Connection` functions `latest(conn, agent_id)`, `get(conn, agent_id, seq)`, `list(conn, agent_id, offset, limit) -> (Vec<CoreRevisionRow>, total)` (newest-first, `limit` clamped 1..=200), `ensure_baseline(conn, agent_id, current: Option<&str>)` (inserts seq 1 as `System`/`Baseline` when no rows exist and `current` is `Some`), `record(conn, agent_id, content: &str, origin, current_before_save: Option<&str>) -> Result<Option<i64>>` (calls `ensure_baseline`, returns `None` when `content == latest.content`, else inserts `max(seq)+1` with `created_at = Utc::now().timestamp_millis()`), and `delete_agent(conn, agent_id)`. Register `mod core_revision;` in `src/storage/sqlite/mod.rs`
- [ ] T009 [P] Create `src/storage/sqlite/memory_revision.rs` (memory only) with `MemoryRevisionRow { seq, content: Option<String>, deleted: bool, actor, trigger, restored_from, created_at }`, its own column mapping, and `latest(conn, agent_id, bundle, path)`, `get(conn, agent_id, bundle, path, seq)`, `list(conn, agent_id, bundle, path, offset, limit)`, `ensure_baseline(conn, agent_id, bundle, path, current: Option<&str>)`, `record(conn, agent_id, bundle, path, content: Option<&str>, origin, current_before_save: Option<&str>) -> Result<Option<i64>>` (`content = None` ⇒ `deleted = 1`; no-op when `(content, deleted)` equals latest; rejects empty `path`), and `delete_agent(conn, agent_id)`. Register `mod memory_revision;` in `src/storage/sqlite/mod.rs`
- [ ] T010 [P] In `src/storage/sqlite/memory_revision.rs` add `RevisionFrontMatter { title: String, #[serde(default)] tags: Vec<String>, #[serde(default)] attachments: Vec<VizierAttachment> }`, `pub(crate) fn memory_canonical(title, tags, attachments, body) -> Result<String>` and `pub(crate) fn parse_memory_canonical(text: &str) -> Result<(RevisionFrontMatter, String)>`; make `serialize_markdown` / `parse_markdown_bytes` in `src/storage/memory_bundle.rs` `pub(crate)` so they can be reused (research Decision 3)
- [ ] T011 [P] Create `src/storage/diff.rs` with `pub fn diff_lines(from: &str, to: &str) -> (Vec<DiffHunk>, usize, usize)` using `similar::TextDiff::from_lines` on `\r\n`→`\n`-normalized input, grouping with `grouped_ops(3)` into `DiffHunk { old_start, old_lines, new_start, new_lines, lines }` and `DiffLine { op, old_line, new_line, text }` (1-based line numbers, `text` without trailing newline); register `pub mod diff;` in `src/storage/mod.rs`
- [ ] T012 [P] Unit tests in `src/storage/diff.rs` (`#[cfg(test)]`): identical input ⇒ zero hunks and 0/0 counts; single insert; single delete; replace ⇒ delete+insert in one hunk; `\r\n` input equals `\n` input
- [ ] T013 [P] Unit tests in `src/storage/sqlite/core_revision.rs` against an in-memory `rusqlite::Connection` with `init_schema` applied: first `record` on empty history with `current_before_save = Some(old)` produces seq 1 baseline + seq 2; identical content ⇒ `Ok(None)`; `list` is newest-first with correct `total`; `delete_agent` empties the table
- [ ] T014 [P] Unit tests in `src/storage/sqlite/memory_revision.rs`: same suite as T013 plus deletion entry (`content = None`) then a content `record` afterwards; two consecutive deletion records ⇒ second is `Ok(None)`; `memory_canonical` ↔ `parse_memory_canonical` round-trip preserving title/tags/attachments/body

**Checkpoint**: `cargo test` passes with the new helper tests; no runtime behavior has changed yet.

---

## Phase 3: User Story 1 - Every save is captured as a version (Priority: P1) 🎯 MVP

**Goal**: Every successful CORE or memory save through Vizier (agent tools in conversation or dream, WebUI/API, import, migrations) appends a revision with actor + trigger; deletions are recorded; identical saves are skipped; history dies with the agent.

**Independent Test**: Follow quickstart.md → US1: save CORE via chat (`WRITE_CORE`) and via WebUI, write/edit/delete a memory via tool and WebUI, save CORE unchanged; then `SELECT` from `core_revision` / `memory_revision` (or hit the US2 endpoints once they exist) and confirm one row per save with the right actor/trigger, a `deleted` row for the delete, no row for the unchanged save, and a `baseline` seq 1 for pre-existing documents.

### Implementation for User Story 1

- [ ] T015 [US1] Change `AgentStorage::set_agent_core` signature to `(&self, agent_id: &str, core: &str, origin: &RevisionOrigin)` in `src/storage/agent.rs` (keep the config-backed default impl, just accept and ignore `origin` there) and update the forwarding impl in `src/storage/mod.rs`
- [ ] T016 [US1] Override `set_agent_core` in `src/storage/sqlite/agent.rs`: inside one `conn.unchecked_transaction()` read the current `agent_core.content` (if any), upsert the new content, call `core_revision::record(&tx, agent_id, core, origin, current.as_deref())`, commit; also call `core_revision::delete_agent` and `memory_revision::delete_agent` inside `delete_agent` (FR-019)
- [ ] T017 [US1] Change `MemoryStorage::{write_memory, delete_memory, delete_bundle, import_bundle}` in `src/storage/memory.rs` to take `origin: &RevisionOrigin` (placed before `indexer`), and update the forwarding impls in `src/storage/mod.rs`
- [ ] T018 [US1] In `src/storage/memory_bundle.rs::write_memory`, compute `canonical_before = existing.as_ref().and_then(parse → memory_canonical(...).ok())` and `canonical_new = memory_canonical(&title, &tags, &attachments, &content)?`, and after the `document_store.put` succeeds call `memory_revision::record(&conn, &agent_id, &bundle, &path, Some(&canonical_new), origin, canonical_before.as_deref())?` in the same `conn.lock()` scope as `upsert_node_from_frontmatter` (research Decision 8)
- [ ] T019 [US1] In `src/storage/memory_bundle.rs::delete_memory`, before `document_store.delete`, parse the existing document into its canonical text and call `memory_revision::record(.., None, origin, canonical_before)` (deletion entry); in `delete_bundle(force = true)` do the same per concept path inside the existing loop; non-force `delete_bundle` records nothing
- [ ] T020 [US1] In `src/storage/memory_bundle.rs::import_bundle`, for each successfully imported concept call `memory_revision::record(.., Some(&canonical), origin, None)` where `origin` is the caller's origin (the HTTP handler passes `.with_trigger(Import)`); in `write_migrated_memory` record with `RevisionOrigin::system(Baseline)`
- [ ] T021 [P] [US1] Add `origin: RevisionOrigin` to `MemoryOpRequest::{Write, Delete, DeleteBundle, ImportBundle}` in `src/schema/commands.rs` and forward it in `dispatch_memory_op` in `src/agents/memory_ops.rs`
- [ ] T022 [P] [US1] Update agent tools: `WriteCore::call` in `src/agents/tools/workspace/mod.rs` passes `&RevisionOrigin::from_session(&ctx.session)`; `memory_write`, `memory_delete`, `memory_delete_bundle` in `src/agents/tools/vector_memory/mod.rs` do the same (use the `ctx` parameter that `call` already receives)
- [ ] T023 [P] [US1] Update HTTP handlers in `src/channels/http/api/v1/agents/core.rs` (`update_core` → `RevisionOrigin::from_user(&user)`) and `src/channels/http/api/v1/agents/memory.rs` (`create_memory`, `do_update_memory` and its two callers, `delete_memory`/`delete_memory_scoped`, `delete_bundle_handler`, `import_bundle_handler` with `.with_trigger(RevisionTrigger::Import)`) — thread `user` into `do_update_memory` where it isn't already available
- [ ] T024 [P] [US1] Update the remaining call sites with `RevisionOrigin::system(RevisionTrigger::Baseline)`: `src/agents/mod.rs` (agent-creation CORE seed, ~line 217) and `src/dependencies.rs` (`migrate_filesystem_backend_to_sqlite` ~line 426, `migrate_agent_cores` ~line 745, and both `write_migrated_memory` calls ~lines 262/348)
- [ ] T025 [US1] Run `cargo clippy` and `cargo test`; fix every compile error from the signature changes until clean. Then `just run`, execute quickstart.md → US1 steps 1–5 and inspect the two tables (e.g. `sqlite3 <workspace>/vizier.db 'select seq,actor_kind,trigger,deleted from memory_revision'`), confirming SC-001 (every save captured, correct actor/trigger) and SC-007 (no row for the unchanged save)

**Checkpoint**: History is being recorded for every save path. US2 can now expose it.

---

## Phase 4: User Story 2 - A user browses history and sees what changed between versions (Priority: P2)

**Goal**: Users can list a document's versions (newest-first, paginated), open any version read-only, and see a line diff against the previous version or between any two versions — in the WebUI and the HTTP API, under the same access rules as the document.

**Independent Test**: quickstart.md → US2: open *Core* → History, confirm list/current badge/actor/trigger; open a version; view "Changes" on a known edit; compare oldest ↔ newest; repeat from a memory's slide-over; curl `…/core/history?to=3` and `?from=1&to=3`; confirm a non-authorized user gets 403/404.

### Implementation for User Story 2

- [ ] T026 [P] [US2] Add `list_core_revisions(agent_id, offset, limit) -> PaginatedCoreRevisions`, `get_core_revision(agent_id, seq) -> Option<CoreRevision>`, `diff_core_revisions(agent_id, from: Option<i64>, to: i64) -> RevisionDiff` to `AgentStorage` in `src/storage/agent.rs` (default impls return `Err("not supported")`), forward them in `src/storage/mod.rs`, and implement them in `src/storage/sqlite/agent.rs` using `core_revision::{list, get}` + `diff::diff_lines`; `list_core_revisions` calls `core_revision::ensure_baseline` with the current `agent_core.content` first (FR-018); `is_current = seq == latest.seq`; `size_bytes = content.len()`; unknown seq in diff ⇒ `Err`
- [ ] T027 [P] [US2] Add `list_memory_revisions(agent_id, bundle: Option<String>, path, offset, limit) -> PaginatedMemoryRevisions`, `get_memory_revision(.., seq) -> Option<MemoryRevision>` (populate `title`/`tags` via `parse_memory_canonical` when `content` is `Some`), `diff_memory_revisions(.., from, to) -> RevisionDiff` to `MemoryStorage` in `src/storage/memory.rs`, forward in `src/storage/mod.rs`, implement in `src/storage/memory_bundle.rs` using `memory_revision::{list, get}` + `diff::diff_lines`; `list_memory_revisions` calls `ensure_baseline` with the canonical text of the on-disk document if it exists; diffing a deletion entry treats its content as `""`
- [ ] T028 [US2] Add `MemoryOpRequest::{ListRevisions { bundle, path, offset, limit }, GetRevision { bundle, path, seq }, DiffRevisions { bundle, path, from, to }}` and `MemoryOpResponse::{Revisions(PaginatedMemoryRevisions), Revision(Option<MemoryRevision>), Diff(RevisionDiff)}` in `src/schema/commands.rs`; dispatch them in `src/agents/memory_ops.rs`
- [ ] T029 [P] [US2] In `src/channels/http/api/v1/agents/core.rs` add `#[derive(Deserialize)] struct CoreHistoryQuery { offset: Option<usize>, limit: Option<usize>, seq: Option<i64>, from: Option<i64>, to: Option<i64> }` and a `get_core_history` handler (`GET /history`) that checks `require_agent`/`user_can_view_agent` exactly like `get_core`, then dispatches: `seq` ⇒ `get_core_revision` (404 if `None`); `to` ⇒ `diff_core_revisions` (404 on unknown seq); else `list_core_revisions` (404 if the agent has no CORE at all). Register `.route("/history", get(get_core_history))` in `core()` and add `#[utoipa::path]` annotations for the three response shapes
- [ ] T030 [P] [US2] In `src/channels/http/api/v1/agents/memory.rs` add `struct MemoryHistoryQuery` (same fields) and a `get_memory_history` handler on `GET /history/{bundle}/{*path}` that strips a trailing `.md`, normalizes the path like the existing `get_memory_detail_scoped`, and sends `ListRevisions`/`GetRevision`/`DiffRevisions` over `state.transport.send_memory_op`, mapping errors with the existing `error_status_for`; 404 when the memory neither exists nor has history. Register the route in `memory()` **before** the `/{slug}` routes and add `#[utoipa::path]`
- [ ] T031 [P] [US2] Add the TypeScript types from `contracts/webui.md` (`RevisionActor`, `RevisionTrigger`, `CoreRevisionSummary`, `CoreRevision`, `PaginatedCoreRevisions`, `MemoryRevisionSummary`, `MemoryRevision`, `PaginatedMemoryRevisions`, `DiffLine`, `DiffHunk`, `RevisionDiff`, `RollbackResponse`, `HistoryRow`, `HistoryVersion`) to `webui/app/interfaces/types.ts`
- [ ] T032 [P] [US2] Add service functions to `webui/app/services/vizier.tsx`: `getCoreHistory(agentId, offset?, limit?)`, `getCoreRevision(agentId, seq)`, `diffCoreRevisions(agentId, to, from?)`, `getMemoryHistory(agentId, bundle, path, offset?, limit?)`, `getMemoryRevision(agentId, bundle, path, seq)`, `diffMemoryRevisions(agentId, bundle, path, to, from?)` — memory URLs built as `/agents/${agentId}/memory/history/${encodeURIComponent(bundle)}/${encodeMemoryPath(path)}` with query params
- [ ] T033 [US2] Create `webui/app/components/VersionHistory.tsx` per `contracts/webui.md`: props `{ source: { list, get, diff, rollback }, label, onRolledBack? }`; internal states `list | version | diff`; newest-first rows with `v{seq}`, actor label (`Agent` / `@username` / `System`), trigger label (`conversation` / `dream cycle` / `WebUI` / `API` / `import` / `restored from v{n}` / `baseline`), relative + absolute time (`title` attr), `Current` and `Deleted` badges, `View` / `Changes` buttons (`Changes` hidden on seq 1 and deletion rows), "Load more" while `offset + limit < total`; version pane shows `title`/`tags` header when present and a "records a deletion" notice for deleted rows; diff pane renders hunks with `@@ -a,b +c,d @@` headers, old/new line numbers, and `op`-based CSS classes; "Compare two" toggle with checkboxes and a `Compare vA → vB` action (newer as `to`); errors via `useToastStore`. Leave the `rollback`/Restore wiring as a stub that US3 fills in
- [ ] T034 [P] [US2] Add diff styles to `webui/app/app.css`: `--diff-insert-bg` / `--diff-delete-bg` tokens defined for both the light and dark theme blocks, plus `.diff-line`, `.diff-line.insert`, `.diff-line.delete`, `.diff-hunk-header`, `.diff-gutter` classes (mono font, `overflow-x: auto` on the container, min 16px side padding preserved)
- [ ] T035 [US2] Wire the CORE page: in `webui/app/routes/agent-core.tsx` add a `History` `btn btn-ghost` in `.main-header` (always visible), a `historyOpen` state, and a `SlideOver` titled `History: CORE` rendering `<VersionHistory label="CORE" source={{ list: (o,l) => getCoreHistory(agentId,o,l).then(map to {rows,total} with deleted:false), get: seq => getCoreRevision(...).then(map to HistoryVersion), diff: (to,from) => diffCoreRevisions(...), rollback: async () => { throw } }} />`
- [ ] T036 [US2] Wire the memory page: in `webui/app/routes/memory.tsx` add a `History` button beside Edit/Delete in the `modalMode === 'view'` slide-over, a `showHistory` state that swaps the slide-over body to `<VersionHistory label={selectedMemory.title} source={{ list/get/diff bound to agentId, selectedMemory.bundle, selectedMemory.path (memory rows pass `deleted` through), rollback: stub }} />` with a `← Back` control; reset `showHistory` on `closeModal`
- [ ] T037 [US2] Run `cd webui && npm run typecheck`, `cargo clippy`, `cargo test`; then `just run` and execute quickstart.md → US2 steps 1–7, verifying SC-002 (find a changed line via history+diff), SC-005 (history of a large document opens within 2 s — seed via a loop of `PUT /core` calls), and the 403/404 access check

**Checkpoint**: Users can browse and diff history for both document kinds; nothing can be changed from the history UI yet.

---

## Phase 5: User Story 3 - A user rolls a document back to a previous version (Priority: P3)

**Goal**: Users can restore any earlier content version as a new, attributed revision (never rewriting history), including restoring a deleted memory, with a confirmation step; the agent sees the restored content on its next read.

**Independent Test**: quickstart.md → US3: restore an older CORE version → new top row `restored from vK`, editor shows restored text, `READ_CORE` returns it; restore the current version → "nothing changed", no new row; delete a memory, `POST …/memory/history/{bundle}/{path} {"seq": N}` → memory reappears; delete the agent → both tables empty for it.

### Implementation for User Story 3

- [ ] T038 [P] [US3] Add `rollback_core(agent_id, seq, origin: &RevisionOrigin) -> RollbackResponse` to `AgentStorage` in `src/storage/agent.rs` (default `Err`), forward in `src/storage/mod.rs`, implement in `src/storage/sqlite/agent.rs`: `core_revision::get` (Err "unknown version" if `None`) → `self.set_agent_core(agent_id, &row.content, &origin.clone().with_trigger(Rollback { restored_from: seq }))` → return `no_change = true` when the new latest seq did not advance, else `new_seq = Some(latest.seq)`
- [ ] T039 [P] [US3] Add `rollback_memory(agent_id, bundle: Option<String>, path, seq, origin, indexer) -> RollbackResponse` to `MemoryStorage` in `src/storage/memory.rs`, forward in `src/storage/mod.rs`, implement in `src/storage/memory_bundle.rs`: `memory_revision::get` → Err if `None` or `deleted` ("cannot restore a deletion entry") → `parse_memory_canonical` → `self.write_memory(agent_id, Some(bundle), Some(path), false, fm.title, body, fm.tags, fm.attachments, &origin.with_trigger(Rollback{..}), indexer)` → compare latest seq before/after to fill `no_change`/`new_seq`
- [ ] T040 [US3] Add `MemoryOpRequest::Rollback { bundle, path, seq, origin }` and `MemoryOpResponse::Rollback(RollbackResponse)` in `src/schema/commands.rs`; dispatch in `src/agents/memory_ops.rs`
- [ ] T041 [P] [US3] In `src/channels/http/api/v1/agents/core.rs` add `struct RollbackRequest { seq: i64 }` and a `rollback_core` handler on `POST /history` (same access check as `update_core`) calling `state.storage.rollback_core(&agent_id, body.seq, &RevisionOrigin::from_user(&user))`; 404 on unknown seq; register `.route("/history", get(get_core_history).post(rollback_core))`; `#[utoipa::path]`
- [ ] T042 [P] [US3] In `src/channels/http/api/v1/agents/memory.rs` add a `rollback_memory` handler on `POST /history/{bundle}/{*path}` with body `{ seq }` sending `MemoryOpRequest::Rollback { .., origin: RevisionOrigin::from_user(&user) }`; map "unknown version" ⇒ 404 and "deletion entry" ⇒ 400 via `error_status_for` (extend it if needed); register alongside the GET route; `#[utoipa::path]`
- [ ] T043 [P] [US3] Add `rollbackCore(agentId, seq)` (`POST /agents/${agentId}/core/history`) and `rollbackMemory(agentId, bundle, path, seq)` (`POST …/memory/history/${bundle}/${path}`) to `webui/app/services/vizier.tsx`
- [ ] T044 [US3] Complete the restore flow in `webui/app/components/VersionHistory.tsx`: `Restore` button on non-current, non-deleted rows (list, version pane, and diff pane); confirmation dialog with copy from `contracts/webui.md` ("v{seq}'s content will be saved as a new version v{next}. Nothing is deleted…"), an optional `extraWarning` prop rendered inside it; on confirm call `source.rollback(seq)`, toast `Restored v{seq} as v{new_seq}` or `Already identical to current — nothing changed`, refetch page 0, then `onRolledBack?.(res)`
- [ ] T045 [US3] Wire CORE rollback in `webui/app/routes/agent-core.tsx`: `rollback: seq => rollbackCore(agentId, seq)`, `extraWarning={hasChanges ? 'Your unsaved editor changes will be discarded.' : undefined}`, `onRolledBack` re-runs the existing `load()` so `content`/`original` reflect the restored version
- [ ] T046 [US3] Wire memory rollback in `webui/app/routes/memory.tsx`: `rollback: seq => rollbackMemory(agentId, bundle, path, seq)`; `onRolledBack` re-fetches the memory via `getMemory` into `selectedMemory` and calls the existing list/graph reload so a restored (previously deleted) memory reappears
- [ ] T047 [US3] Run `cd webui && npm run typecheck`, `cargo clippy`, `cargo test`; then `just run` and execute quickstart.md → US3 steps 1–6 and the "Import / export unchanged" check, verifying SC-003 (≤ 3 interactions), SC-004 (`READ_CORE` returns restored content, no restart), SC-006 (older rows untouched after rollback)

**Checkpoint**: All three user stories are functional end-to-end.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T048 [P] Document the new endpoints (`GET/POST /agents/{id}/core/history`, `GET/POST /agents/{id}/memory/history/{bundle}/{path}`, query-param dispatch, response shapes) in `docs/src/api-integration/rest-api.md`
- [ ] T049 [P] Update `CLAUDE.md` → *Storage* section with two sentences: CORE history lives in `core_revision` via `AgentStorage`, memory history in `memory_revision` via `MemoryStorage`; both recorded inside the write path; direct on-disk edits are not versioned
- [ ] T050 [P] Confirm `cross build` still succeeds for one musl target in `Cross.toml` with the `similar` dependency (constitution Principle IV), or note the result in the PR description
- [ ] T051 Final pass: `cargo clippy` (no warnings in touched files), `cargo test`, `cd webui && npm run typecheck`, and a full run-through of quickstart.md; verify `tracing` is used for any new logging and no `unwrap()`/`expect()` was introduced outside tests

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001 first (dependency), then T002–T006; T004 after T002/T003; T006 after T002 and T005.
- **Foundational (Phase 2)**: depends on Phase 1. T007 first (tables); T008/T009/T010/T011 in parallel after T007 (T010 is in the same file as T009 — same author, sequential); tests T012–T014 after their modules.
- **US1 (Phase 3)**: depends on Phase 2. T015 → T016; T017 → T018 → T019 → T020 (all in `memory_bundle.rs`, sequential); T021–T024 in parallel once T015/T017 signatures exist; T025 last.
- **US2 (Phase 4)**: depends on US1 (needs rows to show). T026 ∥ T027; T028 after T027; T029 after T026; T030 after T028; T031 → T032 → T033; T034 ∥ T033; T035/T036 after T033; T037 last.
- **US3 (Phase 5)**: depends on US2 (UI panel + GET endpoints). T038 ∥ T039; T040 after T039; T041 after T038; T042 after T040; T043 ∥ backend; T044 after T033 + T043; T045/T046 after T044; T047 last.
- **Polish (Phase 6)**: after US3.

### User Story Dependencies

- **US1** is self-contained after Phase 2 and is the MVP: history is captured even if no UI exists yet (verifiable via sqlite).
- **US2** requires US1's rows but is otherwise independent of US3.
- **US3** requires US2's endpoints/panel; the backend rollback methods (T038–T040) could be built in parallel with US2 if staffed.

### Parallel Opportunities

- Phase 2: T008, T009(+T010), T011 are three separate files — three parallel streams; their tests T012–T014 follow each.
- US1: once T015/T017 land, T021 (transport), T022 (tools), T023 (HTTP), T024 (migrations/creation) touch disjoint files.
- US2: backend CORE (T026, T029) and backend memory (T027, T028, T030) are disjoint; the WebUI stream (T031→T032→T033, T034) can proceed against the contracts before the backend is finished.
- US3: T038/T041 (CORE) vs T039/T040/T042 (memory) vs T043 (WebUI service) are disjoint.

---

## Parallel Example: Phase 2

```
# after T007 (tables) — three independent streams:
T008 src/storage/sqlite/core_revision.rs    → T013 tests
T009 + T010 src/storage/sqlite/memory_revision.rs → T014 tests
T011 src/storage/diff.rs                    → T012 tests
```

## Parallel Example: User Story 2

```
Backend CORE:   T026 → T029
Backend memory: T027 → T028 → T030
WebUI:          T031 → T032 → T033 (+ T034) → T035, T036
Join:           T037
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 + Phase 2 (types, tables, helpers, tests).
2. Phase 3 (thread `RevisionOrigin` through every save path, record in the write implementations).
3. **STOP and validate**: every save shows up in `core_revision` / `memory_revision` with the right actor/trigger; unchanged saves add nothing; deletes are recorded. This alone already protects against data loss (rows can be read with sqlite even before the UI exists).

### Incremental Delivery

1. US1 → history is captured (silent, safe to ship).
2. US2 → history is visible and diffable in the WebUI/API.
3. US3 → history is actionable (rollback).
4. Polish → docs, cross-build check, final lint/test/typecheck.

### Notes

- Do not introduce a shared "revision" trait or table-selecting abstraction; CORE and memory are intentionally separate implementations (research Decision 15). Only `src/schema/revision.rs` value types and `src/storage/diff.rs` are shared.
- Keep `VersionHistory.tsx` a single presentational component fed by `HistoryRow`; each route maps its own API types (user decision).
- No history tools for the agent (FR-022); do **not** add `DREAM_TOOL_NAMES` or `default_toolset` entries.
- Commit messages: `feat(memory): …` / `feat(core): …` / `feat(webui): …` conventional commits.
