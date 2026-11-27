# Contract: memory file → database migration

`VizierDependencies::migrate_memory_into_database(document_store, storage, sql)`, in `crates/vizier-core/src/dependencies.rs`.

## 1. When it runs

- In `VizierDependencies::new`, after `migrate_memory_to_bundles` and `migrate_filesystem_backend_to_sqlite`, before `migrate_agent_cores` and before anything can start an agent or serve a memory route (FR-014).
- On SQLite and PostgreSQL alike (written over `SqlStore`/`StateStorage`, no `on_sqlite`).
- Skipped entirely when `state["migration.memory_in_database_v1"]` exists.

## 2. What it reads

- The agent list from storage. Folders with no agent row are never read.
- For each agent, one `DocumentStore::list("{agent}/memory")`, then one `get` per candidate document. The file store is the configured one (local or S3), through the existing `BlobDocumentStore` with the `agents` prefix.
- Candidates are `*.md` entries whose leaf is not `index.md` or `log.md`, sorted, with paths normalised by `normalize_path`.

## 3. What it writes, per candidate

| Situation | Action | Counted as |
|---|---|---|
| row exists with non-NULL `content` | nothing | `already` |
| GET fails (I/O, network) | **abort the agent's pass**, marker not set | store failure |
| parse fails | log `agent, bundle, path, error`; file untouched | `failed` |
| second candidate normalising to an address already filled in this pass | log as duplicate; file untouched | `failed` |
| parsed | upsert row: `title`, `tags`, `created_at`, `updated_at`, `read_count` from the file, `content` = body; rewrite edges from body. **No revision.** One transaction per document. | `migrated` |

After the scan, for the agent:

| Row state | Action |
|---|---|
| `content IS NULL` and no file at its address | delete row, its edges, its passage rows (the vectors go when `reconcile_agent_passages` runs at agent start; a passage key whose row is gone is never returned) |
| `content IS NULL`, file present but unparseable | keep (absent to reads; revisions remain for rollback) |

Then `recompute_broken(agent)` runs once, and one log line is written per agent: `memory migrated into the database: agent=<id> migrated=<n> already=<n> failed=<n> unknown_keys=<n>`. `unknown_keys` counts documents whose frontmatter had keys outside the known set (research Decision 2).

## 4. Completion

- Every agent finished without a store failure: set `state["migration.memory_in_database_v1"] = <now>`, and log once that source files under `agents/*/memory/` are no longer read and may be removed after a backup.
- Any store failure: no marker. Log at `error` with the agent and cause, saying the migration retries on the next start, then continue startup. Rows already filled stay filled, and the next run counts them as `already`.

## 5. Guarantees

1. **Idempotent and resumable**: a document is filled once and never overwritten by a rerun (FR-011, SC-004).
2. **Lossless for every known field** (FR-012, SC-001). Byte-identical body (no trimming, no re-serialisation of the body).
3. **No revisions** (FR-013). `memory_revision` is not touched.
4. **Source files are never deleted or rewritten** (FR-017).
5. **Passages are not built here.** There is no embedder at this point, so existing passage rows of a filled document stay valid (same body, same `content_hash`), and documents with none are converted by the per-agent `reconcile_agent_passages` at agent start, exactly as the spec 009 conversion works.

## 6. Issue #82 entries (FR-019)

To add to the pre-1.0 removal checklist, each naming its dependants:

- `VizierDependencies::migrate_memory_into_database` and the marker `migration.memory_in_database_v1`.
- `storage::document::{DocumentStore, BlobDocumentStore}`: used only by the three memory migrations after this change (`migrate_memory_to_bundles`, `migrate_filesystem_backend_to_sqlite`, `migrate_memory_into_database`).
- `parse_markdown_bytes` / `MemoryFrontMatter` parsing of stored files. Still used by import (permanent), so only the migration's call is listed.
- The `content IS NULL` = absent rule for rows left by a failed parse. Once the migration is removed, a cleanup can delete such rows.
- Docs paragraph explaining that old `agents/*/memory/` folders can be deleted.
