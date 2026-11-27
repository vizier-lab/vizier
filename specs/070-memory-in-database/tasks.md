---

description: "Task list for spec 070: agent memory in the database"
---

# Tasks: Agent memory in the database

**Input**: Design documents from `specs/070-memory-in-database/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/memory-store.md](contracts/memory-store.md), [contracts/migration.md](contracts/migration.md), [quickstart.md](quickstart.md)

**Tests**: included. The contracts name the tests that pin them, the constitution requires a dummyplug e2e pass, and a storage migration requires a real-binary run.

**Organization**: grouped by user story. Almost all engine work is in one file (`crates/vizier-core/src/storage/memory_bundle.rs`), so `[P]` is rare inside a story. The migration (`dependencies.rs`) and docs can run beside it.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US5 from spec.md

Shorthand used below:
- `MB` = `crates/vizier-core/src/storage/memory_bundle.rs`
- `DEPS` = `crates/vizier-core/src/dependencies.rs`
- `SQLITE` = `crates/vizier-core/src/storage/sqlite/mod.rs`

---

## Phase 1: Setup

**Purpose**: branch everywhere, baseline numbers, and the two facts research left to verify.

- [ ] T001 Run `just checkout 070-memory-in-database` so the branch exists in `landing` and `dummyplug` too (constitution matching-branch rule; the spec-kit hook only made the workspace branch)
- [ ] T002 Record the **baseline** on `master` per quickstart §1: 1,000 generated documents in agent `bench`, median of 20 runs of memory list, bundle list, detail and a dummyplug `memory_search` turn, on local disk and optionally on MinIO. Save the numbers to `specs/070-memory-in-database/baseline.md`
- [ ] T003 [P] Verify research Decision 2. Search the workspace (`git grep -n "attachments" -- crates webui/app`) for any code that reads a memory document's frontmatter `attachments`/`attachment_count`. Write the result into `specs/070-memory-in-database/research.md` under Decision 2: "no reader, dropped and counted", or "reader at <file>, add a column" (in that case, add a task to Phase 2 before continuing)
- [ ] T004 [P] Verify there is no other reader of memory files. Run `git grep -n "DocumentStore\|document_store\|/memory/" -- crates app` and confirm that only `MB`, `DEPS`, `crates/vizier-core/src/encryption/migrate.rs` and test code use the memory file store. Note any other reader in `research.md` Decision 7

---

## Phase 2: Foundational (blocking)

**Purpose**: the column exists, the graph helpers can run inside one transaction, and every write fills `content`. Nothing below works until this phase is done.

- [ ] T005 [P] Add `content TEXT` to `memory_node` on SQLite: in the memory schema init in `SQLITE` (next to the `CREATE TABLE IF NOT EXISTS memory_node` at line ~83), call the existing `add_column_if_missing(conn, "memory_node", "content", "TEXT")` (defined at `SQLITE`:1398). Also add the column to the `CREATE TABLE` so fresh databases have it
- [ ] T006 [P] Add PostgreSQL migration `crates/vizier-core/src/storage/sql/postgres/migrations/0019_memory_content.sql` with `ALTER TABLE memory_node ADD COLUMN IF NOT EXISTS content TEXT COLLATE "C";`, and register it wherever the migrations are listed (check how `0018_platform_links.sql` is included)
- [ ] T007 In `MB`, turn the graph helpers into functions over a borrowed connection, so one operation can run them in one transaction:
  - `upsert_node`, `upsert_node_from_frontmatter`, `record_revision`, `delete_node`, `rewrite_edges`, `delete_edges_from`, `recompute_broken`, `resolve_relation`, `relations_for`, `list_nodes` each take `conn: &Connection` instead of calling `self.conn.lock()`
  - callers pass `&self.conn.lock()?` for now
  - no behaviour change
- [ ] T008 In `MB`, add the row-level document accessors:
  - `fn load_document(conn, agent, bundle, path) -> Result<Option<Memory>>`: one `SELECT … FROM memory_node WHERE … AND content IS NOT NULL`. Builds a `Memory` from the columns, with `relations` from `relations_for` and `keywords: vec![]`
  - `fn store_document(conn, agent, bundle, path, &MemoryFrontMatter, content)`: upserts every column including `content`
  - A row with `content IS NULL` is absent (contract memory-store rule 1)
- [ ] T009 In `MB`, make `write_memory_inner` read `existing` with `load_document` instead of `document_store.get` (used by the reserved-bundle guard, `create_only`, the grandfathered ceiling and the `created_at`/`read_count` carry-over), and write with `store_document` instead of `document_store.put`. Delete the `regenerate_index`/`append_log` calls from the write path (research Decision 4)
- [ ] T010 In `MB`, make `delete_memory` read the row with `load_document`, delete it through `delete_node` (which now removes `content` with the row), and drop `document_store.delete` and the index/log calls
- [ ] T011 In `MB`, make `write_migrated_memory` (the pre-bundle legacy migration) write the row with `store_document` instead of a file, keeping its baseline revision and no indexing. Make `finalize_bundle` only `recompute_broken` (no `regenerate_index`)
- [ ] T012 In `MB`, make `seed_identity` and `rollback_memory` work unchanged on top of T009 (they go through `get_memory_detail`/`write_memory_inner`). Run `cargo test -p vizier-core --lib storage::memory_bundle` and fix tests that write fixture files to disk so they go through `write_memory` instead (the reserved-bundle, size-limit, revision and identity tests must pass unchanged in intent)

**Checkpoint**: writes fill `content`. Reads still go through the file store until US2.

---

## Phase 3: User Story 1: existing memory survives the upgrade (P1) 🎯 MVP

**Goal**: on first start, every agent's documents move from the file store into `memory_node.content` once, losslessly, resumably, with no revisions, keeping the files (contract [migration.md](contracts/migration.md)).

**Independent test**: quickstart §2. Take a snapshot of a copied pre-upgrade workspace, start the new binary, and compare every memory response byte for byte. Kill it partway and restart, and the result is the same.

### Tests for US1

- [ ] T013 [P] [US1] In the `#[cfg(test)]` module of `DEPS`, add `memory_files_move_into_the_database`. Seed a `BlobDocumentStore::local` temp dir with two agents' files:
  - one nested path
  - one cross-bundle link
  - one file with an unknown `attachments` frontmatter key
  - one with broken YAML
  - one node row whose file is missing
  - one orphan `no-such-agent` folder

  Run the migration and assert:
  - `content`, title, tags, `created_at`, `updated_at`, `read_count` equal the files
  - the broken file's row stays with NULL content and its file is untouched
  - the missing-file row is deleted with its edges
  - nothing is read from `no-such-agent`
  - no `memory_revision` row was added
  - the marker is set
- [ ] T014 [P] [US1] In `DEPS` tests, add `an_interrupted_memory_migration_finishes_without_duplicates` (fill half the rows, rerun, same result as one pass; `already` count right) and `a_store_failure_leaves_the_marker_unset` (a `DocumentStore` fake whose `list` errors for one agent: other agents are migrated, the marker is absent, and a rerun with a working store completes)

### Implementation for US1

- [ ] T015 [US1] In `DEPS`, add the constant `MEMORY_IN_DATABASE: &str = "migration.memory_in_database_v1"` next to `ONE_TIME_TASK_LOCAL` and `WORKSPACES_DROPPED`, and `async fn migrate_memory_into_database(document_store, storage: &VizierStorage, sql: &SqlStore) -> Result<()>`. It does the following (contract migration.md §2–§4):
  - skip when `storage.get_state(marker)` is set
  - for each stored agent, `list("{agent}/memory")`, then go through sorted `*.md` candidates (not `index`/`log`)
  - skip rows whose content is already set
  - GET + `parse_markdown_bytes::<MemoryFrontMatter>`
  - `store_document` + `rewrite_edges` in one transaction per document
  - delete NULL-content rows whose file is absent
  - `recompute_broken` once
  - log one summary line per agent
  - set the marker only if no store failure happened
  - on a store failure, log at `error` and return `Ok` so startup continues
- [ ] T016 [US1] Expose what the migration needs from `MB` as `pub(crate)`: `store_document`, `rewrite_edges`, `delete_node`, `delete_edges_from`, `recompute_broken`, `normalize_path`, `parse_markdown_bytes`. Alternatively, add one `pub(crate) async fn migrate_document(&self, agent, bundle, path, bytes) -> MigrateOutcome` on `BundleMemoryStore` so the SQL stays in `MB`; this is preferred, because it keeps memory SQL in one file
- [ ] T017 [US1] Call `migrate_memory_into_database` in `VizierDependencies::new` (`DEPS` ~line 185). It goes after `migrate_filesystem_backend_to_sqlite` and **after** `storage` is built (it needs `StateStorage`), and before `migrate_agent_cores`. Pass the `BlobDocumentStore` built at `DEPS`:155
- [ ] T018 [US1] Count documents whose frontmatter had keys outside the known set, using a second lenient parse into `serde_yaml::Mapping`, and include `unknown_keys=<n>` in the summary line. Follow the outcome of T003
- [ ] T019 [US1] On completion, log one `info` line saying `agents/*/memory/` is no longer read and may be removed after a backup

**Checkpoint**: an upgraded instance has all its memory in `memory_node.content`. Reads still also work through the files.

---

## Phase 4: User Story 2: memory reads never wait on the file store (P1)

**Goal**: every read in contract memory-store.md makes zero file-store calls and writes nothing but `read_count`.

**Independent test**: quickstart §3, last step. Delete the agent's `agents/<id>/memory/` folder while the server runs, and every read and search still answers the same. Plus `a_read_writes_nothing`.

### Tests for US2

- [ ] T020 [P] [US2] In `MB` tests, add a recording `DocumentStore` fake (counts every call), and test `a_read_writes_nothing`:
  - setup: write 3 documents across 2 bundles with links
  - reset the counter
  - call `get_memory_detail`, `get_all_agent_memory(None)`, `get_filtered_memories`, `get_related_memories`, `get_memory_graph`, `list_bundles`, `query_memory`, `increment_read_count`, `list_memory_revisions`
  - assert zero calls, and that every column except `read_count` is unchanged

  Then delete the old self-heal/reconcile tests that pin removed behaviour: `stale_relations_self_heal_on_read`, `get_related_memories_self_heals_without_a_prior_detail_call`, `reconciliation_picks_up_documents_copied_from_elsewhere`, `index_and_log_documents_reflect_bundle_contents` (the last one is replaced by T031)

### Implementation for US2

- [ ] T021 [US2] In `MB`, delete `reconcile_bundle` and `reconcile_all`, and rewrite `discover_bundles` as `SELECT DISTINCT bundle FROM memory_node WHERE agent_id = ?1 AND content IS NOT NULL`. Remove every call (`get_all_agent_memory`, `get_related_memories`, `get_memory_graph`, `list_bundles`, `delete_bundle`, `export_bundle`, `reconcile_passages`)
- [ ] T022 [US2] In `MB`, replace `read_and_sync_document` with `load_document` everywhere (`get_memory_detail`, `get_related_memories`, `seed_identity`), with no relation repair and no `recompute_broken` on read. `get_memory_detail` becomes one query
- [ ] T023 [US2] In `MB`, rewrite `increment_read_count` as one `UPDATE memory_node SET read_count = read_count + 1 WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3 AND content IS NOT NULL`
- [ ] T024 [US2] In `MB`, make `query_memory` load the `content` of every hit document with one `SELECT path, bundle, content, … FROM memory_node WHERE agent_id = ? AND (bundle, path) IN (…)` (or one query per hit document if the dialect lacks row-value `IN`; check the `Dialect` trait), and slice passages from it with `slice_passage`. No `DocumentStore` call
- [ ] T025 [US2] In `MB`, make `list_nodes`, `node_to_memory`, `list_bundles`, `get_memory_graph` and `has_incoming_links` ignore rows with `content IS NULL` (rule 1). In `list_memory_revisions`, read the current canonical from `load_document` (via `memory_canonical`) instead of the file
- [ ] T026 [US2] In `MB`, make `reconcile_passages` read bodies through `load_document` (drop its `reconcile_bundle` call and its `document_store.get`)

**Checkpoint**: P1 complete (MVP). Memory is served from the database only, and existing memory is migrated.

---

## Phase 5: User Story 3: writes are all-or-nothing (P2)

**Goal**: content, node, revision, edges and broken flags change in one transaction per document (contract memory-store.md rules 3–5). Passages follow after commit.

**Independent test**: `a_write_that_fails_after_the_node_leaves_nothing_behind`.

### Tests for US3

- [ ] T027 [P] [US3] In `MB` tests, add `a_write_that_fails_after_the_node_leaves_nothing_behind`. Make the edge step fail, using a test-only failpoint (a `#[cfg(test)]` `AtomicBool` checked at the top of `rewrite_edges`), and assert the row content, revision count and edges equal the state before the call. Add the same check for `delete_memory`

### Implementation for US3

- [ ] T028 [US3] In `MB` `write_memory_inner`:
  - take one `let conn = self.conn.lock()?; let tx = conn.transaction()?;`
  - run `load_document`, the three guards, `store_document`, `record_revision`, `rewrite_edges` and `recompute_broken` on it
  - `tx.commit()`, then drop the connection
  - only then call `write_passages_lenient`

  No `.await` happens while the connection is held. Check the `Transaction` type in `crates/vizier-core/src/storage/sql/mod.rs`:589 for commit and rollback-on-drop behaviour
- [ ] T029 [US3] Apply the same one-transaction shape to `delete_memory`, `delete_bundle` (all deletion revisions and row/edge/passage-row deletes in one transaction, then clear vectors after commit) and `import_bundle` (one transaction per imported document, then passages, then `recompute_broken` once)

**Checkpoint**: no partial writes.

---

## Phase 6: User Story 4: export and import keep the markdown archive (P3)

**Goal**: export generates `index.md`, `log.md` and one `.md` per document from rows. Import is unchanged in behaviour and accepts archives from earlier releases.

**Independent test**: quickstart §4.

### Tests for US4

- [ ] T030 [P] [US4] Check in a fixture `crates/vizier-core/tests/fixtures/memory-bundle-pre070.zip`, exported from `master` before this change, with 3 documents, a link, `index.md` and `log.md`. In `MB` tests, add `an_archive_from_the_previous_release_imports`
- [ ] T031 [US4] In `MB` tests, add `export_round_trips_through_import`: write, update and delete in a bundle, export, import into a fresh agent, and assert equal content, title and tags. Also assert the archive has `index.md` (table rows) and a `log.md` line per revision in the contract rule 7 format

### Implementation for US4

- [ ] T032 [US4] In `MB`, change `regenerate_index` into `fn render_index(conn, agent, bundle) -> String` (same body, no store write). Add `fn render_log(conn, agent, bundle) -> String` from `memory_revision` rows of the bundle, oldest first: `created` for each path's first seq, `deleted` for deletion entries, `updated` otherwise; title from the revision's canonical. Use the helpers in `crates/vizier-core/src/storage/sqlite/memory_revision.rs`, or add `list_bundle(conn, agent, bundle)` there
- [ ] T033 [US4] In `MB`, rewrite `export_bundle`:
  - list present rows of the bundle
  - serialise each with `serialize_markdown` (frontmatter fields as in data-model.md, `relations` from `relations_for`, `keywords: []`)
  - add `index.md` and `log.md`
  - zip
  - error `bundle '<b>' does not exist or is empty` when there are no rows
- [ ] T034 [US4] In `MB`, make `import_bundle` check for an existing document with `load_document` instead of `document_store.get`, and write with `store_document` (T029 already provides the transaction)
- [ ] T035 [US4] Delete `append_log`, `index_key`, `log_key` and `bundle_prefix`, if they are now unused, from `MB`

---

## Phase 7: User Story 5: moving to PostgreSQL carries memory (P3)

**Goal**: `vizier storage migrate` copies `memory_node.content` and the marker. No code change is expected (research Decision 8).

**Independent test**: quickstart §5.

- [ ] T036 [P] [US5] In `crates/vizier-core/src/storage/migrate.rs` tests, add `memory_content_is_a_copied_column`: a current SQLite schema lists `content` in `sqlite_columns(conn, "memory_node")`, so `copy_tables` carries it. If a PostgreSQL test harness exists in `crates/vizier-core/src/storage/sql/tests.rs`, also assert `0019` adds it
- [ ] T037 [US5] Run quickstart §5 against a local PostgreSQL with pgvector (e.g. `docker run -e POSTGRES_PASSWORD=… pgvector/pgvector:pg16`) and record the result in the PR description

---

## Phase 8: Polish and cross-cutting

- [ ] T038 Drop the `DocumentStore` from `BundleMemoryStore` (`MB` struct and `BundleMemoryStore::new`) and from `SqlStorage` (the `pub document_store` field and the `new`/`with_secrets` parameter in `SQLITE`:245–280, plus `bundle_store()`). Update every call site; `git grep -n "SqlStorage::new(\|SqlStorage::with_secrets(\|BundleMemoryStore::new(" -- crates app` lists about 67, mostly tests. In `DEPS`, keep building the `BlobDocumentStore` only for the three memory migrations, and do the same for `crates/vizier-core/src/encryption/migrate.rs`:326
- [ ] T039 Regenerate `crates/vizier-core/public-api.txt` with `just public-api` (the `document_store` field and constructor parameter are gone) and review the diff
- [ ] T040 [P] Update docs that describe memory as files on disk: `docs/src/` (search `git grep -n -i "markdown\|on disk\|memory/" -- docs/src`). Say memory lives in the database, can be exported as markdown, and that hand-edited files are not picked up. Add the "old `agents/*/memory/` folders can be removed after the upgrade" note to the storage/upgrade page
- [ ] T041 [P] Update `CLAUDE.md`:
  - the opening "What this is" paragraph ("agent memory itself lives as human-readable markdown documents on disk")
  - the **Storage** section (`DocumentStore adapts memory documents onto that blob store`; "Database rows hold … never object content"; passage text "sliced from the document")
  - **Agent identity**
- [ ] T042 [P] Amend `.specify/memory/constitution.md` to v2.3.1 (PATCH): reword the Distribution constraint sentence on memory documents using `DocumentStore`/`BlobStore`, and add a Sync Impact Report entry
- [ ] T043 Add the four checklist entries from contract migration.md §6 to issue #82. Show the exact text to the person and wait for a yes before posting, since it is outward-facing (`gh issue view 82`, then `gh issue edit 82` or a comment, whichever the issue uses)
- [ ] T044 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, and `cd app; cargo test` (the desktop app compiles against the changed constructors)
- [ ] T045 Real-binary upgrade run, quickstart §2 (local; S3 on MinIO optional): snapshot comparison, broken-file handling, untouched sources, restart idempotence, interrupted run (constitution manual-verification gate)
- [ ] T046 Dummyplug end-to-end, quickstart §3 (write, read, link, search, identity, delete, rollback, reads with the folder deleted) (constitution e2e gate)
- [ ] T047 After-measurement, quickstart §6. Compare against `baseline.md` (T002) for SC-003 and SC-005, and put the numbers in the PR description

---

## Dependencies and execution order

### Phases

- **Setup (T001–T004)**: none. T003 can add a Phase 2 task.
- **Foundational (T005–T012)**: after Setup; blocks every story. T005 and T006 are parallel. T007 → T008 → T009–T011 → T012 run in order (one file).
- **US1 (T013–T019)**: after Foundational. Its migration lives in `DEPS`, so it can run **in parallel with US2**, which works in `MB`. T016 touches `MB`; coordinate it with US2 (do T016 first).
- **US2 (T020–T026)**: after Foundational. Ship it **with US1**: reading only from rows without the migration would hide every pre-upgrade document.
- **US3 (T027–T029)**: after US2 (same functions in `MB`).
- **US4 (T030–T035)**: after US3 (T034 relies on T029's transaction). T030 can be made any time.
- **US5 (T036–T037)**: after US1 (needs the column and the marker). Independent of US2–US4.
- **Polish (T038–T047)**: T038 after every `MB` task. T039 after T038. T040–T042 any time. T043 at the end, with approval. T044–T047 last.

### Parallel opportunities

- T003 ∥ T004, and T005 ∥ T006.
- After Foundational: US1 (`DEPS`) ∥ US2 (`MB`).
- Tests marked [P] can be written ahead of the implementation in their story.
- T040 ∥ T041 ∥ T042 at any point.

### Parallel example: after Phase 2

```text
Stream A (DEPS):  T013, T014 → T015 → T017 → T018 → T019
Stream B (MB):    T016 (first, shared) → T020 → T021 → T022 → T023 → T024 → T025 → T026
Stream C (docs):  T040, T041, T042
```

## Implementation strategy

### MVP (P1 = US1 + US2, released together)

1. Phase 1, then Phase 2.
2. US1 and US2 in parallel.
3. **Stop and validate**:
   - quickstart §2 (upgrade) and §3 (dummyplug, including reads with the folder deleted)
   - T020 (zero file-store calls)
4. This alone delivers the speed-up and keeps every existing memory.

### Incremental

5. US3: atomic writes (removes the partial-write class).
6. US4: export/import from rows, no stored `index.md`/`log.md`.
7. US5: PostgreSQL move check.
8. Polish: constructor cleanup, public API, docs, constitution, #82, final runs and measurements.

## Notes

- Commit after each task or logical group in conventional-commit format, e.g.:
  - `feat(memory): store memory content in the database`
  - `feat(memory)!: …` if a public API change warrants it (the `SqlStorage` constructors are public)
- Never run `cargo fmt` (the workspace is not rustfmt-clean).
- Keep every migration and legacy shim until #82 removes it, and add the #82 entries (T043) before the PR is merged.
