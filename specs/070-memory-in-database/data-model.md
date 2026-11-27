# Data Model: Agent memory in the database

## memory_node (changed: one column added)

The existing derived graph table becomes the document's home. One row per document.

| Column | Type | Change | Meaning |
|---|---|---|---|
| `agent_id` | TEXT | — | owning agent (PK part) |
| `bundle` | TEXT | — | bundle name (PK part); `core` reserved |
| `path` | TEXT | — | normalised path within the bundle, no `.md` (PK part) |
| `slug` | TEXT | — | as today |
| `title` | TEXT | — | as today |
| `tags_json` | TEXT | — | JSON array of tags |
| `attachment_count` | INTEGER | — | as today (unread by current code; left alone) |
| `created_at` | TEXT | — | RFC 3339, set on first write |
| `updated_at` | TEXT | — | RFC 3339, set on every write |
| `read_count` | INTEGER | — | incremented in place by `memory_read` |
| **`content`** | TEXT NULL | **new** | the document body (markdown, no frontmatter). `NULL` = not migrated yet, or a file that could not be parsed during migration |

- SQLite: `add_column_if_missing("memory_node", "content", "TEXT")` in the memory schema init. Permanent, so not a #82 entry.
- PostgreSQL: `0019_memory_content.sql`: `ALTER TABLE memory_node ADD COLUMN IF NOT EXISTS content TEXT COLLATE "C";`.
- No new index. Reads are by primary key or `(agent_id, bundle)`, both of which are already indexed.

### Validation rules (unchanged, now enforced inside the write transaction)

1. `core` accepts only `soul`, except a pre-existing document already there (spec 043).
2. `create_only` refuses when a row with non-NULL `content` exists at the address.
3. Size ceiling and soft limit per `WritePolicy.limits_for` (identity pair for `core/soul`). The grandfathering rule compares against the stored `content` length.
4. A row with `content IS NULL` is **absent** to every reader: detail returns none, list/graph/bundles/search skip it, and `create_only` treats the address as free. A write fills it.

### State of one row

```text
           migration fills / write creates
 (no row) ────────────────────────────────▶ present (content NOT NULL)
    ▲                                           │  write → present (content replaced)
    │ delete / bundle delete                    │  read  → present (read_count + 1 only)
    └───────────────────────────────────────────┘
 migration: file unparseable ─▶ row kept, content NULL (absent; restorable by rollback)
 migration: file missing     ─▶ row deleted (with edges, passages)
```

## memory_edge, memory_passage, memory_revision (unchanged)

- `memory_edge`: still rewritten from `parse_relations(content)` on every write. It is now in the same transaction as the row.
- `memory_passage`: coordinates only, as before. Text is sliced from `memory_node.content`. Written after commit with vectors (research Decision 3). `content_hash` is kept: it still detects bodies indexed by an older chunker and embedding-dimension resets.
- `memory_revision`: unchanged schema. Every write, delete and rollback records one in the write transaction. The migration records none.

## state (one new key)

| Key | Value | Meaning |
|---|---|---|
| `migration.memory_in_database_v1` | RFC 3339 time it completed | the file→database migration finished for every agent (contract [migration.md](contracts/migration.md)). #82 |

## Removed stored artefacts

| Was | Now |
|---|---|
| `agents/{agent}/memory/{bundle}/{path}.md` (file store) | `memory_node.content` + columns; files left in place after migration, never written again |
| `agents/{agent}/memory/{bundle}/index.md` | generated into the export archive only |
| `agents/{agent}/memory/{bundle}/log.md` | generated into the export archive from `memory_revision` |

## Export archive (format kept)

```text
<bundle>.zip
├── index.md            # generated: table of path | title | tags | updated
├── log.md              # generated: one line per revision, oldest first
└── <path>.md           # one per present document: YAML frontmatter + body
    (frontmatter: slug, title, created_at, updated_at, agent_id, bundle, tags,
     keywords: [], relations (parsed from body), read_count)
```

Import accepts this shape and any archive from an earlier release. It skips `index.md`, `log.md` and non-`.md` entries, as today.
