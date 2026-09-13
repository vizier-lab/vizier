# Contract: `DocumentStore` trait

Location: new module `src/storage/document/mod.rs`. This is a new, standalone abstraction — not
a `VizierStorageProvider` sub-trait — because it's addressed by path/bytes, not by the
agent/entity-shaped methods the rest of storage uses. Only `BundleMemoryStore` (the sole
`impl MemoryStorage for SqliteStorage`) calls it in this feature.

```rust
#[async_trait::async_trait]
pub trait DocumentStore: Send + Sync {
    /// Read raw bytes at `path`. `Ok(None)` if it doesn't exist.
    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>>;

    /// Write `bytes` at `path`, creating any missing parent directories/prefixes.
    async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<()>;

    /// Remove whatever is at `path`. Not an error if it doesn't exist.
    async fn delete(&self, path: &str) -> Result<()>;

    /// List every path under `prefix` (recursive), for bundle scans, index rebuilds, and
    /// migration. Returned paths are relative to `prefix`, matching `get`/`put`/`delete`'s
    /// addressing.
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;
}
```

## Default implementation: `LocalDocumentStore`

```rust
pub struct LocalDocumentStore {
    root: PathBuf, // {workspace}/agents  — BundleMemoryStore composes agent_id/memory/... itself
}
```

- `get`/`put`/`delete` map directly to `tokio::fs` calls under `root.join(path)`, using
  `PathBuf::join` throughout (never `format!("{}/{}", ...)` string concatenation — this is the
  Principle IV cleanup called out in `plan.md`'s Complexity Tracking).
- `put` creates missing parent directories via `tokio::fs::create_dir_all` before writing —
  this is what backs FR-007/FR-008's implicit bundle/subdirectory creation.
- `list` uses the already-present `glob` crate with a recursive `**` pattern rooted at
  `root.join(prefix)` (research.md §3) — no new traversal dependency.

## Behavioral contract (backend-independent — any future `impl DocumentStore` must satisfy these)

1. `get` on a path that was never `put` (or was `delete`d) returns `Ok(None)`, never an error.
2. `put` is a full overwrite of whatever was at `path` — `BundleMemoryStore` is responsible for
   collision checking (FR-011) *before* calling `put`; the trait itself has no compare-and-swap
   semantics.
3. `put` must succeed even when intermediate path segments ("directories") don't yet exist
   (FR-007/FR-008) — the trait models a flat key space with directory-shaped keys, not a
   filesystem call that fails on a missing parent.
4. `list(prefix)` reflects the current state at call time — a future implementation is not
   required to be strongly consistent with a `put`/`delete` that raced it, but `LocalDocumentStore`
   (backed by a real filesystem) is.
5. A backend that can be unreachable (network-based) must surface that as an `Err`, not as an
   empty `list` result or a false `Ok(None)` from `get` (Edge Cases: "must fail with a clear
   error rather than silently serving stale cached data or losing the write").

## Why this isn't a `VizierStorageProvider` sub-trait

`VizierStorageProvider`'s traits (`MemoryStorage`, `TaskStorage`, etc.) are all entity-shaped —
methods that speak in agents, slugs, and structured fields, dispatched by whichever backend
(`SqliteStorage`, historically also `FileSystemStorage`) is configured for the whole deployment.
`DocumentStore` is a narrower, byte-oriented abstraction that `BundleMemoryStore` composes
*underneath* its own `impl MemoryStorage for SqliteStorage` — the rest of the codebase never
sees or depends on `DocumentStore` directly, matching the spec's requirement that the medium be
swappable "without changing the bundle model, agent-facing tools, or any other caller" (FR-023).
