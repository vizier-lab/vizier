# Research: Agent memory in the database

Findings from reading `crates/vizier-core/src/storage/memory_bundle.rs` (the one implementation behind `impl MemoryStorage for SqlStorage`), `storage/document/mod.rs`, `dependencies.rs`, `storage/migrate.rs` and the two schema sources (`storage/sqlite/mod.rs`, `storage/sql/postgres/migrations/0001_initial.sql`). Each decision lists what was chosen, why, and what else was considered.

## What touches the file store today

Every `DocumentStore` call in `BundleMemoryStore`, by operation:

| Operation | File-store calls | Notes |
|---|---|---|
| `get_all_agent_memory` / `get_filtered_memories` (memory list) | 1 LIST per bundle + 1 LIST to discover bundles, + 1 GET per document added behind Vizier's back | via `reconcile_all` → `discover_bundles` + `reconcile_bundle` |
| `list_bundles` | LIST × (bundles + 1), twice (`reconcile_all` then `discover_bundles` again) | |
| `get_memory_detail` / `memory_read` | 1 GET, + 1 PUT if relations drifted | `read_and_sync_document` read-repair |
| `increment_read_count` (every `memory_read`) | 1 GET + 1 PUT | **a full document rewrite on every read** |
| `get_related_memories` | LIST + GET (+PUT) | reconcile + read-repair |
| `get_memory_graph` | LIST × bundles | |
| `query_memory` (`memory_search`) | 1 GET per hit document | body fetched to slice passage text |
| identity load per turn | 1 GET (+PUT) | through `get_memory_detail` |
| `write_memory_inner` | GET + PUT (doc) + PUT (`index.md`) + GET + PUT (`log.md`) | 5 round trips per write |
| `delete_memory` | GET + DELETE + PUT (index) + GET + PUT (log) | |
| `delete_bundle`, `export_bundle`, `import_bundle`, `list_memory_revisions`, `reconcile_passages` | LIST/GET/PUT/DELETE | |

On local disk each is a syscall served from the page cache. On S3 each is an HTTPS round trip (LIST is paginated and slowest). A single `memory_read` on S3 currently costs at least four round trips (GET + optional PUT for detail, GET + PUT for the read count).

## Decision 1: where the content lives — a `content` column on `memory_node`

**Decision**: add one nullable column, `content TEXT`, to the existing `memory_node` table on both backends. The row already carries every frontmatter field that is read (`slug`/path, `title`, `tags_json`, `created_at`, `updated_at`, `read_count`); with the body beside them, the row *is* the document.

**Rationale**: the smallest change that makes the database the source (Principle I). Every existing query that lists, filters or counts nodes keeps working unchanged; `delete_agent`'s existing `DELETE FROM memory_node` already removes content with it (FR-008); `vizier storage migrate` copies tables generically by destination column list (`migrate.rs::copy_tables`), so the new column travels to PostgreSQL with no code (FR-018). `NULL` means "not migrated yet", which is exactly the resumability marker the migration needs (Decision 6).

**Alternatives considered**:
- *A new `memory_document` table keyed like `memory_node`*: duplicates the key and every timestamp/title column, or splits them and forces a join on every list. Rejected: two rows that must agree is the drift problem this feature removes.
- *Keep files, add a cache*: fixes local disk, leaves S3 slow and needs cross-node invalidation once Fleet runs several nodes. Rejected in the conversation that led to the spec.
- *Store the serialized markdown (frontmatter + body)*: makes the database hold a second copy of every frontmatter field already in columns. Rejected; markdown is produced on export only.

## Decision 2: frontmatter fields that are not columns

`Memory`/`MemoryFrontMatter` (`schema/storage.rs`, via `MarkdownDoc`) has: `slug`, `title`, `content`, `created_at`, `updated_at`, `agent_id`, `bundle`, `tags`, `keywords`, `relations`, `read_count`.

- `agent_id`, `bundle`, `slug` — already the row key.
- `relations` — derived from the body by `parse_relations` on every write and already stored as `memory_edge` rows; `relations_for` serves it.
- `keywords` — always written as `[]` by every write path. Not stored.

Old files written by earlier releases may carry keys outside this set (for example `attachments`/`attachment_count`, present in test fixtures and in `public-api.txt`). Serde already ignores them on every read today, so nothing currently visible reads them. **Decision**: the migration does not carry unknown keys, and logs a per-agent count of documents that had any, so an operator can tell. *Verify during implementation* (task): grep for any reader of memory `attachments` before relying on this; if one exists, add a column for it rather than dropping it.

## Decision 3: atomic writes — one transaction for content and graph, passages after commit

**Decision**: `write_memory_inner`, `delete_memory`, `delete_bundle`, `import_bundle` (per document) and `rollback_memory` (through `write_memory_inner`) run their database steps on **one connection inside one `Connection::transaction()`**: read the existing row, the guards (reserved bundle, `create_only`, size ceiling), upsert node + content, record revision, rewrite edges, recompute broken flags. Passage rows and their vectors are written after commit, as today, through `write_passages_lenient`.

**Rationale**: the helpers (`upsert_node`, `record_revision`, `rewrite_edges`, `delete_node`, `delete_edges_from`, `recompute_broken`, `resolve_relation`) each call `self.conn.lock()` themselves today, so a write spans several connections and can stop halfway. They become functions over a `&Connection`, called once per operation inside the transaction (`Connection::transaction` already nests by depth, `sql/mod.rs:589`). Embedding is an async network call to a provider; holding a database transaction (SQLite takes its write lock with `BEGIN IMMEDIATE`) across it would stall every other writer. FR-005 allows the semantic index to follow the commit, and its failure is already logged and swallowed (FR-036 of spec 009). A document with no passage rows is picked up by the per-agent `reconcile_passages` at the next start, so a failed index heals itself.

**Alternatives considered**: writing passage *rows* in the transaction and vectors after — leaves rows without vectors that `reconcile_passages` would not recognise as unconverted (it keys on "no rows"). Rejected; passages stay one unit after commit.

## Decision 4: what happens to `index.md` and `log.md`

Both are written per bundle on every write and delete (`regenerate_index`, `append_log`), cost three file-store round trips per write, and are read by nothing except `export_bundle` (import skips them).

**Decision**: stop storing them. `export_bundle` **generates** both into the archive: `index.md` from `memory_node` (the existing `regenerate_index` body, returned as bytes instead of stored), `log.md` from `memory_revision` rows of the bundle (oldest first, one line per revision in the existing `- <time> — <action> \`<path>\` "<title>"` shape, with `created`/`updated`/`deleted` from the revision's seq and `deleted` flag). The archive keeps its structure (FR-007). Old `log.md` files are not migrated: everything they say is in `memory_revision`, which is richer (actor, trigger).

**Alternative considered**: a `memory_bundle_log` table. Rejected — it would duplicate `memory_revision`.

## Decision 5: what is removed from the read paths

- `reconcile_bundle`, `reconcile_all`, `discover_bundles`' LIST: removed. Bundles are `SELECT DISTINCT bundle FROM memory_node`, plus `core` (already added when absent).
- `read_and_sync_document`'s relation repair: removed. Relations are parsed on every write, so they cannot drift without a hand edit, which is no longer possible. Detail reads the row.
- `increment_read_count`: one `UPDATE memory_node SET read_count = read_count + 1`, no document rewrite.
- `query_memory`: slices passage text from the row's `content` (loaded with one query for the hit documents), never from the file store.
- `list_memory_revisions`' baseline: reads current content from the row.
- `reconcile_passages`: kept (it converts documents with no passages and rebuilds on embedding-dimension change) but reads the body from the row; its first line, the call to `reconcile_bundle`, goes.

`stale_relations_self_heal_on_read`, `get_related_memories_self_heals_without_a_prior_detail_call` and `reconciliation_picks_up_documents_copied_from_elsewhere` test behaviour that no longer exists; they are deleted, and replaced by tests that a read writes nothing (FR-004).

## Decision 6: the migration

**Where**: a new `migrate_memory_into_database` in `dependencies.rs`, run in `VizierDependencies::new` right after the two existing memory migrations (`migrate_memory_to_bundles`, `migrate_filesystem_backend_to_sqlite`) and before `migrate_agent_cores`/any agent start (FR-014). It is written over the SQL connection, so it runs on SQLite and PostgreSQL alike (FR-010), and keyed by a `state` row `migration.memory_in_database_v1` read and written through `StateStorage`, the cross-backend marker pattern of `migrate_drop_workspaces` (FR-011).

**Steps**, per agent row in `agent` (orphan folders with no agent are not read — spec edge case):
1. `list("{agent}/memory")` once. A failed LIST is a store failure: stop, do not set the marker (FR-016), log, and continue startup with memory as it is in the database. (Startup is not refused: an S3 outage must not take the whole instance down; the next start retries.)
2. For each `*.md` path whose leaf is not `index`/`log`: skip if its `memory_node` row already has non-NULL `content` (resumable, never overwrites — FR-011). Otherwise GET and parse. On a parse error: log agent/bundle/path/reason, count it as failed, leave the file (FR-015). On success: upsert the row with the file's own `title`, `tags`, `created_at`, `updated_at`, `read_count` and body (FR-012), rewrite its edges from the body; **no revision** (FR-013).
3. Rows of this agent that still have `content IS NULL` after the scan are documents whose file is gone or unreadable. A *missing* file means the node was stale (today's reconcile would have deleted it): delete the node, its edges and passages. An *unparseable* file keeps its row with NULL content; reads treat NULL content as absent (detail returns `None`, list skips it), and its revisions remain, so it can be restored with rollback.
4. `recompute_broken(agent)` once.
5. Log `migrated / skipped (already in db) / failed` per agent.

When every agent finished step 1–4 without a store failure, set the marker. Failed parses do not hold the marker back: retrying them cannot succeed, and they are logged with their path.

Two files that normalise to one path (`a.md` and `a/.md`, a trailing separator): the first wins by sorted order, the second is logged as a duplicate. Source files are never deleted (FR-017).

**Write after migrate**: `write_migrated_memory` (the pre-bundle legacy migration) writes the row directly instead of a file, so an instance upgrading from a pre-bundle release goes legacy → database in one start.

**Alternatives considered**: running per agent in `AgentHost::start`, like the passage conversion. Rejected: a gateway or the HTTP memory routes may read an agent's memory before its host starts, and the passage conversion is per agent only because it needs that agent's embedder, which this migration does not.

## Decision 7: `DocumentStore` after the change

After the move, the only reader of memory files is the migration (and the two older migrations). `BundleMemoryStore` stops holding a `DocumentStore`; `SqlStorage::with_secrets`/`SqlStorage::new` lose the parameter; `dependencies.rs` builds a `BlobDocumentStore` only for the three migrations. The trait and `BlobDocumentStore` stay (the migration uses them) and are listed on #82 with the migration (FR-019). No new trait is added (Principle I).

## Decision 8: `vizier storage migrate` (SQLite → PostgreSQL)

No code change. `copy_tables` copies every destination table by its column list, so `memory_node.content` travels once PostgreSQL migration `0019` adds the column. The command refuses a source whose schema is not current (`schema_mismatch`), which means the source ran its startup migrations; if the marker was already set on the source, its `state` row is copied and the destination does not rerun. If it was not (the source failed to reach a store), `copy_blobs` still copies the memory files and the destination migrates them on first start. Covered by a quickstart step rather than a new test.

## Decision 9: performance baseline

SC-003/SC-005 are targets. The plan records a baseline before the change: 1,000 generated documents in one agent, median of 20 `memory_search` and `GET …/memory` calls, on local disk and on MinIO (`docker run minio/minio`, optional, marked as such in the quickstart). The same script runs after. If the S3 gain is under 5×, the cause is investigated before the target is adjusted.

## Out of scope (confirmed)

- Uploads, artifacts, UI pages and other file-store bytes (spec Assumptions).
- Full-text search on PostgreSQL. Possible later with `content` in a column; not added here.
- Deleting source files. A documented manual step (FR-017).
