use std::path::PathBuf;

use anyhow::{Result, anyhow};

/// A pluggable abstraction over where memory document *bytes* physically live.
///
/// Keyed by a path-like string (e.g. `andy/notes.md`, `default/friends/bred.md`).
/// `BundleMemoryStore` is the only caller of this trait — the rest of the codebase
/// never depends on it directly, so a future non-local implementation (S3, a remote
/// filesystem) is a second `impl DocumentStore` with no change anywhere else.
#[async_trait::async_trait]
pub trait DocumentStore: Send + Sync {
    /// Read raw bytes at `path`. `Ok(None)` if it doesn't exist.
    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>>;

    /// Write `bytes` at `path`, creating any missing parent directories/prefixes.
    async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<()>;

    /// Remove whatever is at `path`. Not an error if it doesn't exist.
    async fn delete(&self, path: &str) -> Result<()>;

    /// List every path under `prefix` (recursive). Returned paths are relative to
    /// `prefix`, matching `get`/`put`/`delete`'s addressing, and use `/` separators
    /// regardless of host OS.
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;
}

/// Default `DocumentStore`: local filesystem, rooted at `{root}/...` where callers
/// (`BundleMemoryStore`) compose `agent_id/memory/...` themselves on top of `root`.
pub struct LocalDocumentStore {
    root: PathBuf,
}

impl LocalDocumentStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Resolves `path` under `root`, rejecting `..`/`.` segments outright rather than letting
    /// the OS collapse them at the syscall level — without this, a `..` segment smuggled in
    /// through a same-bundle markdown link, an agent-supplied `path`, or a bundle name would
    /// let a read/write escape the intended agent/bundle directory (path traversal).
    fn resolve(&self, path: &str) -> Result<PathBuf> {
        let mut full = self.root.clone();
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            if segment == ".." || segment == "." {
                return Err(anyhow!("invalid path segment '{segment}' in '{path}'"));
            }
            full.push(segment);
        }
        Ok(full)
    }
}

#[async_trait::async_trait]
impl DocumentStore for LocalDocumentStore {
    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let full = self.resolve(path)?;
        match tokio::fs::read(&full).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<()> {
        let full = self.resolve(path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&full, bytes).await?;
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let full = self.resolve(path)?;
        match tokio::fs::remove_file(&full).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let base = self.resolve(prefix)?;
        if !base.exists() {
            return Ok(vec![]);
        }

        let pattern = crate::utils::build_glob_path(
            &base.to_string_lossy(),
            &["**", "*"],
        );

        let mut results = Vec::new();
        for entry in glob::glob(&pattern)? {
            let entry = entry?;
            if !entry.is_file() {
                continue;
            }
            let relative = entry.strip_prefix(&base).unwrap_or(&entry);
            let relative = relative.to_string_lossy().replace('\\', "/");
            results.push(relative);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_on_missing_path_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalDocumentStore::new(dir.path().to_path_buf());
        assert!(store.get("nope.md").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn rejects_parent_directory_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalDocumentStore::new(dir.path().to_path_buf());

        // A secret file that lives *outside* the intended root.
        let secret = dir.path().parent().unwrap().join("vizier-traversal-secret.txt");
        std::fs::write(&secret, b"top secret").unwrap();

        let attempt = format!("a1/memory/andy/../../../{}", secret.file_name().unwrap().to_string_lossy());
        assert!(store.get(&attempt).await.is_err());
        assert!(store.put(&attempt, b"pwned".to_vec()).await.is_err());
        assert!(store.delete(&attempt).await.is_err());

        // The file outside the root must be untouched.
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "top secret");
        let _ = std::fs::remove_file(&secret);
    }

    #[tokio::test]
    async fn put_creates_missing_parents_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalDocumentStore::new(dir.path().to_path_buf());
        store
            .put("a/b/c.md", b"hello".to_vec())
            .await
            .unwrap();
        let bytes = store.get("a/b/c.md").await.unwrap().unwrap();
        assert_eq!(bytes, b"hello");
    }

    #[tokio::test]
    async fn delete_missing_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalDocumentStore::new(dir.path().to_path_buf());
        store.delete("nope.md").await.unwrap();
    }

    #[tokio::test]
    async fn list_returns_relative_paths_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalDocumentStore::new(dir.path().to_path_buf());
        store.put("bundle/a.md", b"1".to_vec()).await.unwrap();
        store.put("bundle/nested/b.md", b"2".to_vec()).await.unwrap();

        let mut listed = store.list("bundle").await.unwrap();
        listed.sort();
        assert_eq!(listed, vec!["a.md".to_string(), "nested/b.md".to_string()]);
    }
}
