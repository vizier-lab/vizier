# Contract: Shared SQL Layer

Module `crates/vizier-core/src/storage/sql/`.

```rust
pub enum SqlValue { Null, Int(i64), Real(f64), Text(String), Blob(Vec<u8>) }

pub trait ToSqlValue { fn to_sql_value(&self) -> SqlValue; }      // i64, i32, u32, usize, bool, f64, &str, String, Option<T>, Vec<u8>, &[u8], DateTime<Utc> (ms)
pub trait FromSqlValue: Sized { fn from_sql_value(v: SqlValue) -> anyhow::Result<Self>; }

#[macro_export] macro_rules! sql_params { ($($v:expr),* $(,)?) => { &[$(&$v as &dyn ToSqlValue),*] } }

pub trait SqlRow { fn get<T: FromSqlValue>(&self, idx: usize) -> anyhow::Result<T>; }   // by column index, like rusqlite

pub trait SqlConn {
    fn dialect(&self) -> &'static dyn Dialect;
    fn execute(&mut self, sql: &str, params: &[&dyn ToSqlValue]) -> anyhow::Result<usize>;
    fn execute_batch(&mut self, sql: &str) -> anyhow::Result<()>;
    fn query_rows(&mut self, sql: &str, params: &[&dyn ToSqlValue]) -> anyhow::Result<Vec<Row>>;
    fn query_opt(&mut self, sql: &str, params: &[&dyn ToSqlValue]) -> anyhow::Result<Option<Row>>;  // first row
    fn transaction<T>(&mut self, f: impl FnOnce(&mut dyn SqlConn) -> anyhow::Result<T>) -> anyhow::Result<T>;
}

pub trait SqlPool: Send + Sync {
    fn with_conn<T>(&self, f: impl FnOnce(&mut dyn SqlConn) -> anyhow::Result<T>) -> anyhow::Result<T>;
    fn backend(&self) -> Backend;                 // Sqlite | Postgres (diagnostics, health)
    fn as_sqlite(&self) -> Option<&SqlitePool>;   // legacy SQLite-only migrations only (R11)
}
```

`Row` is a concrete `Vec<SqlValue>` wrapper implementing `SqlRow`, so row-mapping closures are backend-neutral. (`transaction`/`with_conn` are generic, so they live on a concrete `SqlStore` wrapper over `Arc<dyn SqlPoolDyn>`; the dyn-safe inner trait takes `&mut dyn FnMut`.)

## Rules

- SQL text uses `?1…?N`. `PgConn` rewrites to `$1…$N` outside quoted literals and caches per statement text.
- Integers bind as `BIGINT` (`i64`). Reading a PostgreSQL `INT4`/`BOOL` column into `i64`/`bool` is supported for robustness, but schemas use `BIGINT`.
- `transaction` commits on `Ok`, rolls back on `Err` or panic. Nested `transaction` on a transaction conn is a savepoint.
- `execute` returns affected rows; callers implementing a guarded transition MUST treat `0` as "lost".
- `with_conn` on PostgreSQL wraps the call in `tokio::task::block_in_place` when inside a multi-thread runtime; outside a runtime it calls directly.
- Errors: `anyhow::Error` inside storage (as today); unique-violation is detectable through `is_unique_violation(&err)` for both backends.

## Dialect

```rust
pub trait Dialect: Sync {
    fn json_text(&self, column: &str, path: &[&str]) -> String;   // SQL expression yielding TEXT or NULL
    fn json_array_len(&self, column: &str) -> String;
    fn day_of_millis(&self, column: &str) -> String;                // 'YYYY-MM-DD'
    fn lock_for_update(&self) -> &'static str;                      // "" | " FOR UPDATE"
    fn memory_text_match(&self) -> MemoryTextMatch;                 // FTS5 vs tsvector query shapes
}
```

New dialect methods are added only when a statement cannot be written portably.

## Storage type

`SqliteStorage` is renamed `SqlStorage { sql: SqlStore, documents: Arc<dyn DocumentStore>, cipher, secret_paths, session_files }`. Constructors:

- `SqlStorage::sqlite(conn: rusqlite::Connection, …)` — today's behavior (runs `init_schema`).
- `SqlStorage::postgres(pool, …)` — runs PostgreSQL migrations.
- `SqlStorage::sqlite_in_memory()` — tests.

`BundleMemoryStore` takes `SqlStore` instead of `Arc<Mutex<Connection>>`.
