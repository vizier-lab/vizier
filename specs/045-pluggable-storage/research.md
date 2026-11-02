# Research: Pluggable Storage

Codebase facts come from the storage inventory of `upstream/master` at `2fa1eee` (29 storage traits, ≈252 methods, 13.1k lines under `storage/sqlite/` plus 4.8k in `storage/memory_bundle.rs`; one `rusqlite::Connection` behind a `parking_lot::Mutex`, used synchronously inside `#[async_trait]` methods).

## R1. One SQL implementation, two connections

**Decision**: Keep a single implementation of every storage trait. Its method bodies stay synchronous and run against a small project-owned connection trait, `storage::sql::SqlConn`, shaped like the subset of `rusqlite` the code already uses (`execute`, `query_row`, `query_map`/`query_rows`, `prepare`-free helpers, `transaction`). Two implementations: `SqliteConn` (wraps `rusqlite::Connection`, today's behavior) and `PgConn` (wraps a pooled synchronous `postgres::Client`). `SqliteStorage` becomes `SqlStorage` holding `Arc<dyn SqlPool>`.

**Rationale**: Constitution Principle II forbids copy-pasted backend logic; a second 18k-line implementation would drift. Every method today is "lock, run SQL, map rows", so converting it to `SqlConn` is mechanical (parameter macro, placeholder style, a few dialect helpers) and keeps diffs reviewable per file.

**Alternatives considered**: a separate PostgreSQL implementation of each trait (double maintenance, Principle II violation); `sqlx` with an `Any` driver (every method becomes async and query macros need a live database at build time; also loses `rusqlite`'s bundled static SQLite); an ORM (new dependency weight, rewrites every query anyway).

## R2. PostgreSQL driver and threading

**Decision**: `postgres` (sync) + `r2d2` + `r2d2_postgres`, TLS via `tokio-postgres-rustls` on the existing `rustls` (aws-lc-rs) stack. Each storage call checks out a pooled client inside `tokio::task::block_in_place`, which the server's multi-thread runtime supports; the SQLite path is unchanged and never calls `block_in_place`.

**Rationale**: Storage bodies stay synchronous (R1). The sync `postgres` client must not run on a runtime worker without `block_in_place`. All crates are pure Rust, so `cross`/musl targets keep building (Principle IV) and no system library is added.

**Alternatives considered**: `tokio-postgres` async with `block_on` (nested runtime panic); converting every method to async (rewrites 18k lines with no behavioral gain now); `deadpool-postgres` (async-only).

## R3. Placeholders and dialect differences

**Decision**: SQL keeps `?N` placeholders. `PgConn` rewrites `?N` → `$N` once per statement (skipping quoted literals) and caches the result. Remaining differences go through `Dialect` methods used only where SQL differs:

| SQLite today | Portable form |
|---|---|
| `INSERT OR IGNORE` | `INSERT … ON CONFLICT DO NOTHING` (SQLite ≥ 3.24, bundled is 3.5x) |
| `INSERT OR REPLACE` (17) | `INSERT … ON CONFLICT (<key>) DO UPDATE SET …` written per statement |
| `json_extract(col, '$.a.b')` (18) | `dialect.json_text("col", &["a","b"])` → SQLite `json_extract(col,'$.a.b')`, PG `(col::jsonb #>> '{a,b}')` |
| `json_array_length` (1) | `dialect.json_array_len` |
| `last_insert_rowid()` (2) | `INSERT … RETURNING id` (SQLite ≥ 3.35) |
| `rowid` tie-breaks (~15) | explicit integer id column, or the table's own key |
| `date(at/1000,'unixepoch')` | `dialect.day_of_millis("at")` |
| `PRAGMA table_info` | schema-version migrations (R4) |

**Rationale**: Most statements already run unchanged on both; the dialect surface is a short, testable list instead of branches in call sites.

## R4. Schema management

**Decision**: SQLite keeps its existing idempotent `init_*_schema` path untouched (compatibility for every existing workspace). PostgreSQL gets ordered migration files `storage/sql/postgres/migrations/NNNN_name.sql`, applied at startup inside a transaction under `pg_advisory_xact_lock`, recorded in `vizier_schema_migrations(version, name, applied_at)`. Logical column types match across backends (`BIGINT` for every integer, `DOUBLE PRECISION` for reals, `TEXT` for strings and JSON, `BYTEA` for blobs) so row mapping code is shared.

**Rationale**: DDL is inherently dialect-specific; data-moving one-shot migrations from SQLite's history (legacy filesystem, plaintext secrets, provider layout) do not apply to a fresh PostgreSQL database, which is created at the current schema. The advisory lock makes concurrent starts safe (Fleet prerequisite).

## R5. Ordered sequences and one-winner transitions

**Decision**: `session_history.seq` uses `BIGINT GENERATED ALWAYS AS IDENTITY` on PostgreSQL; on SQLite the existing `MAX(seq)+1` stays inside the write transaction (SQLite's database-level write lock serialises it). Revision ordinals (`core_revision`, `memory_revision`) stay read-then-insert inside a transaction that first takes a per-document key lock (`Connection::lock_key` → `dialect.key_lock_sql()`: `pg_advisory_xact_lock(hashtext(key))` on PostgreSQL, nothing on SQLite, whose `BEGIN IMMEDIATE` already excludes writers). Guarded transitions keep their `UPDATE … WHERE state = ?` form and check the affected-row count.

**Rationale**: FR-011/FR-012 need database-level guarantees, not the process mutex.

## R6. Vector search

**Decision**: `PgIndexer` implements `DocumentIndexer` with the `vector` extension: table `document_index_d<N>(context TEXT, path TEXT, embedding vector(N))`, HNSW index with `vector_cosine_ops`, score `1 - (embedding <=> $q)`. Vectors are sent as the text literal `'[x,y,…]'::vector`, so no client crate is needed. Dimension is read from `pg_attribute`/`format_type`; a mismatch is refused with the same message as SQLite.

## R7. Full-text memory search

**Decision**: Nothing to port. FTS5 `memory_fts` indexes only the legacy flat `memory` table and no storage statement queries it (memory search is passage + vector based, R6), so PostgreSQL gets no full-text index. SC-005 compares vector search only.

## R8. Files: one `BlobStore`, hand-rolled S3

**Decision**: New trait `storage::blob::BlobStore` (async): `put(key, bytes)`, `put_stream(key, stream, len)`, `get(key)`, `get_range(key, start, end) -> stream`, `head(key)`, `delete(key)`, `list(prefix)`. Implementations: `LocalBlobStore` (rooted at the workspace, same paths as today, atomic temp + rename) and `S3BlobStore` (path-style or virtual-host requests over the existing `reqwest`, signed with the existing `llm::sigv4` signer using service `s3` and `x-amz-content-sha256`; multipart upload above 64 MiB). `DocumentStore` becomes a thin key-mapping layer over `BlobStore`; uploads (`FileManager`), artifacts (`ArtifactPaths`) and HTTP file serving move onto it. Skills stay on the local filesystem (they are installed code packages, not user data).

**Rationale**: Principle I prefers a few hundred lines over a heavy dependency when the signer already exists; S3 operations needed are five REST calls. One trait covers all four byte trees.

**Alternatives considered**: OpenDAL (large dependency tree and feature surface we do not use; still needs our own trait for the local layout); `aws-sdk-s3` (very heavy, own HTTP and credential stack).

## R9. Streaming and range serving

**Decision**: HTTP handlers for `/files/{id}` and artifact content return `axum::body::Body::from_stream(blob.get_range(..))`; a full download is the range `0..len`. `ArtifactStorage::artifact_content_file` (sync, path-returning) is replaced by an async resolver returning bytes from `BlobStore`.

## R10. Encryption

**Decision**: The cipher and `IntegrationSecretPaths` move with the shared implementation unchanged; AAD stays `(table, record, field)` and table names are identical on both backends, so ciphertext copied by the migration opens on PostgreSQL. The key source rules are unchanged; in PostgreSQL mode a generated `.runtime/encryption.key` is still allowed (single process) and Fleet forbids it later.

## R11. Raw handles outside storage

**Decision**: `VizierDependencies.sqlite_conn` becomes `sql: Arc<dyn SqlPool>`; the SQLite-only one-shot migrations (`provider_migration.rs`, `projects/migrate.rs`, `encryption/migrate.rs`, `migrate_*` in `dependencies.rs`, legacy `storage/fs`) run only when the backend is SQLite, through `SqlPool::as_sqlite()`. PostgreSQL databases are always created at the current layout (by fresh schema or by the migration command, R12), so they never need them.

## R12. Migration command

**Decision**: `vizier storage migrate --workspace <dir> --storage postgres [--objects s3]`: acquires the source workspace lock (stopped source), opens SQLite read-only and verifies that it already has this binary's schema (otherwise instructing the operator to start it once before migration), creates the destination schema, then copies tables in foreign-key order through `SqlConn` (values move as typed `SqlValue`, so encryption envelopes copy byte-for-byte), copies vectors table by table, uploads blobs with size + SHA-256 verification, records progress in `storage_migration` for resume, and prints a counts/checksum report. Refuses a completed source fingerprint unless `--reset-destination`.

## R13. Conformance tests

**Decision**: Backend-agnostic storage tests use `tests_support::test_store()` and `postgres_tests!`; SQLite always runs and PostgreSQL runs when `VIZIER_TEST_DATABASE_URL` is set, with each test in its own schema. Blob tests run against local storage and, when `VIZIER_TEST_S3_ENDPOINT` is set, MinIO. CI adds a managed-storage job with `pgvector/pgvector` and MinIO. Existing in-memory SQLite harnesses keep working through `SqlStore::sqlite`.

## R14. Configuration

**Decision**: `StorageConfig` gains `Postgres { url_env: default "VIZIER_DATABASE_URL", pool_size: default 8, ca_file: Option }` (URL read from the environment only); new `ObjectsConfig { Local (default), S3 { bucket, region, endpoint, path_style } }` with credentials from `VIZIER_S3_ACCESS_KEY_ID`/`VIZIER_S3_SECRET_ACCESS_KEY` (or `AWS_*`). `--storage postgres` and `--objects s3` CLI values; the desktop app always builds `Sqlite` + `Local`.
