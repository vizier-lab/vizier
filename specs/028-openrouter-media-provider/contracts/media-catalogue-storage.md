# Contract: Media Catalogue Storage

**Feature**: `028-openrouter-media-provider` | **Plan**: [../plan.md](../plan.md)

A new storage concern for the three OpenRouter media model snapshots. It mirrors `CatalogueCacheStorage` (`catalogue_snapshot`), but is keyed by capability and is not mixed into the chat/embedding catalogue.

## Trait

`src/storage/media_catalogue.rs`:

```rust
pub struct MediaCatalogueSnapshotRow {
    pub capability: String,          // "speech" | "transcription" | "image"
    pub schema: i64,                 // 1
    pub fetched_at: DateTime<Utc>,
    pub checked_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub failed_at: Option<DateTime<Utc>>,
    pub data: Vec<u8>,               // gzip(JSON Vec<MediaModel>)
}

#[async_trait::async_trait]
pub trait MediaCatalogueStorage {
    async fn get_media_snapshot(
        &self,
        capability: &str,
    ) -> anyhow::Result<Option<MediaCatalogueSnapshotRow>>;

    /// Insert or replace one capability, clearing the failure fields in the same transaction.
    async fn put_media_snapshot(&self, row: &MediaCatalogueSnapshotRow) -> anyhow::Result<()>;

    /// A failed check: record the reason/time, leave data and checked_at untouched.
    async fn record_media_failure(
        &self,
        capability: &str,
        error: &str,
        at: DateTime<Utc>,
    ) -> anyhow::Result<()>;
}
```

`MediaCapability` (a catalogue-domain enum) is not referenced by the storage layer; the catalogue module passes `capability.as_str()`. This keeps storage free of a dependency on higher-level catalogue types.

## Wiring

- Add `MediaCatalogueStorage` to the `VizierStorageProvider` supertrait list in `src/storage/mod.rs`.
- Add `pub mod media_catalogue;` to `src/storage/mod.rs`.
- Forward all three methods from `VizierStorage` to `self.0`, exactly like the other traits.
- Implement the trait for `SqliteStorage` in `src/storage/sqlite/media_catalogue.rs`.
- The removed/legacy filesystem backend is not a `VizierStorageProvider`; no implementation is required there.

## Table

Added in `SqliteStorage::init_schema` (idempotent, no data migration):

```sql
CREATE TABLE IF NOT EXISTS media_catalogue_snapshot (
    capability  TEXT PRIMARY KEY CHECK (capability IN ('speech', 'transcription', 'image')),
    schema      INTEGER NOT NULL,
    fetched_at  INTEGER NOT NULL,
    checked_at  INTEGER NOT NULL,
    last_error  TEXT,
    failed_at   INTEGER,
    data        BLOB NOT NULL
);
```

- Timestamps are Unix milliseconds (`DateTime::timestamp_millis`), matching `catalogue_snapshot`; reads use `DateTime::from_timestamp_millis(...).unwrap_or(UNIX_EPOCH)`.
- `capability` is the primary key; there is at most one row per capability.
- No `source_url`/`etag`: the source URL is fixed per capability and conditional requests are not used (research R7).
- The table is provider-wide, not agent-scoped; deleting an agent does not touch it.

## Method semantics

### `get_media_snapshot`

```rust
conn.query_row(
    "SELECT capability, schema, fetched_at, checked_at, last_error, failed_at, data \
     FROM media_catalogue_snapshot WHERE capability = ?1",
    [capability],
    |r| { ... },
).optional()
```

- Unknown capability → `Ok(None)`.
- A row with an unknown `schema` is still returned; the caller decides to ignore it (so the data can be logged/inspected if needed).
- SQLite errors are propagated; the caller logs and falls back to the seed.

### `put_media_snapshot`

One transaction:

```sql
INSERT INTO media_catalogue_snapshot
    (capability, schema, fetched_at, checked_at, last_error, failed_at, data)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT(capability) DO UPDATE SET
    schema = excluded.schema,
    fetched_at = excluded.fetched_at,
    checked_at = excluded.checked_at,
    last_error = excluded.last_error,
    failed_at = excluded.failed_at,
    data = excluded.data;
```

- A success row sets `last_error`/`failed_at` to `NULL`.
- `data` is `cache::gzip(&serde_json::to_vec(&models)?)`; reuse the existing gzip helpers in `src/catalogue/cache.rs`.
- A write failure is logged by the caller; the in-memory list still swaps (the refresh is useful even if persistence fails), matching the chat catalogue.

### `record_media_failure`

```sql
UPDATE media_catalogue_snapshot
SET last_error = ?2, failed_at = ?3
WHERE capability = ?1;
```

- Does **not** change `fetched_at`, `checked_at` or `data`; the last successful list stays usable.
- A missing row is a no-op; on a fresh install the seed remains the source.

## Startup

`VizierDependencies::new` already loads the chat catalogue cache with `cache::newest`. Immediately after that, call the catalogue's media loader:

```rust
catalogue.load_media_cache(&storage).await;
```

`Catalogue::load_media_cache`:

1. For each capability, `get_media_snapshot(capability)`.
2. Ignore/log a row whose `schema != SCHEMA` or whose data fails to gunzip/parse.
3. Use the row only if `row.fetched_at > seed.fetched_at` for that capability; otherwise keep the seed.
4. On use, set `source = Cached`, `fetched_at = row.fetched_at`, `checked_at = Some(row.checked_at)`, and restore `last_error`/`failed_at` into that capability's refresh state (so the non-blocking notice survives a restart).
5. Never touch the network.

## Refresh write order

On a successful check:

1. Parse/validate the fetched list.
2. Build the row (`schema = 1`, `fetched_at = checked_at = now`, failure fields `None`, gzip data).
3. `put_media_snapshot(row).await`; on error log `tracing::warn!` and continue.
4. Swap the in-memory `MediaList`.
5. Clear the in-memory failure.

On a failed check:

1. `tracing::warn!` with the capability, URL and reason.
2. Set the in-memory failure to `(reason, now)`.
3. `record_media_failure(capability, reason, now).await`; a storage error is logged and does not change the in-memory failure.
4. Keep the current list and `checked_at`; the next lazy check is suppressed for 1 h.

## Corruption and compatibility

- A corrupt row (bad gzip, bad JSON, unknown schema) is logged and ignored; the seed is used and the row is left in place until the next successful refresh overwrites it.
- A row whose `fetched_at` is older than the shipped seed is ignored (a new binary's seed wins).
- No migration of existing data is needed; the table is new.
- The payload schema is independent of the chat catalogue's `SCHEMA`; both are currently `1` but versioned separately.

## Tests

- Round-trip a row with `gzip`/`gunzip` and assert all fields.
- `put` clears a previous failure; `record_failure` leaves the fetched data/check time untouched.
- `get` on an empty database returns `None`; an unknown capability returns `None`.
- A corrupt row is ignored by the loader (unit test with an injected row where the table is available, or a pure `decode` helper test).
- The existing `cargo test` sparse-suite convention applies; most behavior is also exercised by quickstart §1–§5.
