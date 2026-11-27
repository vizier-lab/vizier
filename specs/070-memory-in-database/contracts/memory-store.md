# Contract: memory store behaviour

`MemoryStorage` (implemented once by `BundleMemoryStore` for both SQL backends) keeps **every signature and every result shape**. The tools, the HTTP routes, the deprecated `/core` projection, the dream cycle and the WebUI see no change (SC-006). This contract pins what each operation reads and writes after the change. "FS" means the file store; every row of the last column must read **0**.

| Operation | Reads | Writes | FS calls |
|---|---|---|---|
| `write_memory` / `write_memory_with` | row at address (in txn) | **one txn**: row (content + columns), revision, edges, broken flags; **after commit**: passages + vectors | 0 |
| `rollback_memory` | revision row | through `write_memory_inner` | 0 |
| `delete_memory` | row (in txn) | **one txn**: revision (deletion), row delete, edges delete, broken flags; after commit: clear passages + vectors | 0 |
| `delete_bundle` (force) | rows of bundle | **one txn**: one deletion revision per document, rows/edges of the bundle, broken flags; after commit: passage rows + vectors | 0 |
| `import_bundle` | rows at each address | per document **one txn** (row, revision, edges); after each, passages; broken flags once at the end | 0 |
| `export_bundle` | rows of bundle, revisions of bundle | nothing | 0 |
| `get_memory_detail` | row | nothing | 0 |
| `increment_read_count` | — | `read_count = read_count + 1` on the row | 0 |
| `get_all_agent_memory` / `get_filtered_memories` | rows | nothing | 0 |
| `get_related_memories` | row, edges, target rows | nothing | 0 |
| `get_memory_graph` | rows, edges | nothing | 0 |
| `has_incoming_links` | edges | nothing | 0 |
| `list_bundles` | `DISTINCT bundle` of rows, plus `core` always | nothing | 0 |
| `query_memory` (search) | vector index, passage rows, hit rows (by primary key) | nothing (a document whose passage rows are missing or stale gets spans derived in memory; `reconcile_agent_passages` heals it) | 0 |
| `seed_identity` | any row at `core/soul` (NULL content included) | through `write_memory_inner` with the cap bypass, only when no row exists | 0 |
| `reconcile_agent_passages` | rows, passage rows | passages + vectors for docs with none or with drifted hash | 0 |
| `list/get/diff_memory_revisions` | revisions, row (baseline) | baseline revision if missing (as today) | 0 |

## Rules

1. **A row with `content IS NULL` is absent** to every read and to `create_only`. Only a write or the migration fills it.
2. **Reads write nothing** except `increment_read_count`'s counter and the revision baseline that `list_memory_revisions` already seeds (FR-004).
3. **One transaction per document change.** A failure at any step before commit leaves the row, its edges, its broken flags and its revision as they were. Passages may lag behind a committed change and are healed by `reconcile_agent_passages` at the next agent start. A search may return the previous passages in the meantime, which is the same as today's failure mode.
4. **No transaction is held across an `await` on the indexer or any network call.**
5. **Guards run inside the transaction** (reserved bundle, `create_only`, ceiling), so a refused write changes nothing (spec 043 contract rule 1 and SC-009 are kept).
6. **Relations come from the body on every write**. There is no read-time repair, because the body cannot change except through a write.
7. **Export is generated, never stored.** `index.md` uses the existing table format, and `log.md` is built from revisions: `- <created_at RFC 3339> — <created|updated|deleted> \`<path>\` "<title>"`, where the action is `created` for the document's first seq, `deleted` for a deletion entry and `updated` otherwise.
8. Concurrency: no fencing is added to memory writes. One agent's memory writes serialise on the transaction (SQLite `BEGIN IMMEDIATE`; PostgreSQL a per-agent `lock_key`, which also keeps `recompute_broken` from deadlocking between two writes), and the last commit wins.

## Tests that pin it

- `a_read_writes_nothing`: run detail, list, related, graph, bundles and search on a store whose file store is a recording fake (or none at all), and assert no FS calls and no change to any row except `read_count`.
- `a_write_that_fails_after_the_node_leaves_nothing_behind`: a fault injected at the edge step means the row, revision and edges are as before.
- `export_round_trips_through_import`: a bundle exported and imported into a fresh agent gives equal rows (content, title, tags) and an archive containing `index.md` and `log.md`.
- `an_archive_from_the_previous_release_imports`: a checked-in fixture zip from before this change.
- Existing size-limit, reserved-bundle, revision and passage tests pass unchanged against the row model.
