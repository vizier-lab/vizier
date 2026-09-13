# Contract: startup migration to bundle storage + sqlite-only backend

Location: `src/dependencies.rs`, alongside the existing `migrate_*` one-time migrations (seed
users → superadmin role, YAML providers → provider storage, per-agent MCP/shell config backfill,
default CORE.md backfill). Add two migrations, both run once at startup, each gated by its own
marker so neither re-runs (consistent with existing migrations' pattern), and both run **before**
`FileSystemStorage` is removed from the codebase (i.e., this migration code is the last thing
that still needs to read from it).

## Part A: `migrate_memory_to_bundles` — every deployment

### Preconditions / detection

- **sqlite backend**: legacy `memory` table (schema in `src/storage/sqlite/mod.rs::init_schema`)
  has rows.
- **filesystem backend** (pre-removal): `agents/{id}/memory/*.md` exist directly under the
  memory root (flat, not yet inside a bundle subdirectory).

If neither is true (fresh install, or already migrated), this part is a no-op.

### Behavior (FR-014, FR-015, FR-024)

For every existing memory, regardless of source:

1. Determine the owning agent: its `agent_id` field as stored (for a `global`-visibility memory,
   this is the special `_global` pseudo-agent directory/row — see below).
2. Write it as a concept document into that agent's **default** bundle
   (`{agent_id}/memory/default/{slug}.md`) via `BundleMemoryStore`, which writes the bytes
   through `DocumentStore` **and** upserts the corresponding `memory_node`/`memory_edge` rows
   (data-model.md) in the same call — so it participates in index/log generation and is
   immediately servable from the Memory Graph Index, not just written to disk.
3. Drop `visibility` and `shared_to` from the frontmatter entirely (FR-015) — the document
   becomes private to `agent_id` with no further meaning attached to its old visibility.
4. Do **not** rewrite the document's `content` body — any embedded `[[slug]]` wikilinks are left
   exactly as written (FR-015, Edge Cases: accepted breaking change to link resolution, not a
   content-mutation step).
5. Preserve `read_count` and the existing timestamp (mapped to both `created_at` and
   `updated_at`, since no prior "created" timestamp exists to recover — see data-model.md).

### Special case: `_global`/shared memories

Today's `_global` pseudo-agent directory (filesystem backend) and rows with `visibility =
"global"` (sqlite backend) represent one physical memory visible to every agent. Under the new
all-private model there is no equivalent — the spec's resolution (FR-015, Edge Cases) is: the
memory becomes private to **its recorded `agent_id`** only. A memory stored under `_global` with
no other `agent_id` recorded is migrated to a single designated agent (the migration must pick
one deterministically — e.g. the first agent found, or a reserved system agent id — and this
choice must be logged via `tracing::warn!` since it is a lossy, one-time judgment call, not
silently dropped).

## Part B: `migrate_filesystem_backend_to_sqlite` — `filesystem`-backend deployments only (FR-025)

### Preconditions / detection

`config.storage == StorageConfig::Filesystem` (checked before the `filesystem` variant is
removed from `StorageConfig` — this migration must ship in the same release that removes it, so
there is exactly one release where both the migration and the old config variant coexist).

### Behavior

For every entity `FileSystemStorage` currently implements (`AgentStorage`, `TaskStorage`,
`HistoryStorage`, `SessionStorage`, `StateStorage`, `UserStorage`, `ProviderStorage`,
`GlobalConfigStorage`, `DreamJournalStorage`, `DreamStorage`, `SessionFileStorage` — everything
except `MemoryStorage`, handled by Part A): read every record via `FileSystemStorage`'s existing
trait methods and write it via the equivalent already-existing `SqliteStorage` trait methods.
No new storage logic is written for this part — it is a data-copy loop over methods that already
exist on both sides today.

### Ordering

Part A runs first (so memory is already off the filesystem's flat layout), then Part B copies
everything else, then (in the same startup, before agents spawn) the config is treated as
effectively `Sqlite` for the rest of this process's lifetime. The operator's `.vizier.yaml`/
`VIZIER_STORAGE=filesystem` setting itself is not silently rewritten on disk by the migration —
startup logs a `tracing::warn!` telling the operator to update their config to `sqlite` (or drop
`--storage`/`VIZIER_STORAGE` to use the new sole default), since the `filesystem` value stops
being accepted once this ships (FR-025).

## Failure handling (both parts)

- A migration failure on one record (e.g. malformed legacy data) must not abort the whole
  migration — log via `tracing::error!` and continue with the rest, consistent with "no existing
  memory should become unreadable or vanish" being a best-effort guarantee for *recoverable*
  data, while still surfacing what couldn't be recovered.
- Both migrations run before agents are spawned (same ordering as the other `dependencies.rs`
  migrations), so no agent process observes a partially-migrated state.

## Post-migration state

- Index (`index.md`) and log (`log.md`) documents are generated for every bundle touched by
  Part A as part of the same pass (FR-017/FR-018) — not left to lazy reconciliation for this
  one-time bulk change.
- The legacy sqlite `memory` table is left in place but unused after migration (simplest,
  reversible choice — dropping it is not required by any FR and would remove a rollback option);
  no code path reads from it after this feature ships.
- After this release, `src/storage/fs/` is deleted from the codebase (research.md §10) — Part B
  is the last code that ever constructs a `FileSystemStorage`.
