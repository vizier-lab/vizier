use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use bytes::Bytes;

use crate::storage::blob::{BlobKey, BlobStore, LocalBlobStore};

/// Where memory document *bytes* live.
///
/// Keyed by a path-like string (e.g. `andy/notes.md`, `default/friends/bred.md`).
/// `BundleMemoryStore` is the only caller of this trait — the rest of the codebase never
/// depends on it directly.
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

/// The `DocumentStore`: memory documents as blobs. Callers (`BundleMemoryStore`) address
/// `<agent>/memory/...`; with the `agents` prefix that is the blob key
/// `agents/<agent>/memory/...`, which `LocalBlobStore` maps to `<workspace>/agents/...`.
pub struct BlobDocumentStore {
    blobs: Arc<dyn BlobStore>,
    prefix: Option<BlobKey>,
}

impl BlobDocumentStore {
    pub fn new(blobs: Arc<dyn BlobStore>, prefix: BlobKey) -> Self {
        Self {
            blobs,
            prefix: Some(prefix),
        }
    }

    /// Documents directly under a local directory (tests and tools that own a scratch root).
    pub fn local(root: PathBuf) -> Self {
        Self {
            blobs: Arc::new(LocalBlobStore::new(root)),
            prefix: None,
        }
    }

    /// The blob key for `path`. Empty segments are skipped (as a path join would); `.`/`..`
    /// and other invalid segments are refused, so a `..` smuggled in through a markdown link,
    /// an agent-supplied path or a bundle name cannot escape the agent's directory.
    fn key(&self, path: &str) -> Result<BlobKey> {
        let relative = path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        match (&self.prefix, relative.is_empty()) {
            (Some(prefix), false) => prefix.join(&relative),
            (None, false) => BlobKey::new(&relative),
            (Some(prefix), true) => Ok(prefix.clone()),
            (None, true) => Err(anyhow!("empty document path")),
        }
    }
}

#[async_trait::async_trait]
impl DocumentStore for BlobDocumentStore {
    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let key = self.key(path)?;
        Ok(self.blobs.get(&key).await?.map(|b| b.to_vec()))
    }

    async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<()> {
        let key = self.key(path)?;
        self.blobs.put(&key, Bytes::from(bytes)).await?;
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let key = self.key(path)?;
        self.blobs.delete(&key).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let base = self.key(prefix)?;
        Ok(self
            .blobs
            .list(&base)
            .await?
            .into_iter()
            .filter_map(|meta| meta.key.strip_prefix(&base).map(str::to_string))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_on_missing_path_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobDocumentStore::local(dir.path().to_path_buf());
        assert!(store.get("nope.md").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn rejects_parent_directory_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobDocumentStore::local(dir.path().to_path_buf());

        // A secret file that lives *outside* the intended root.
        let secret = dir
            .path()
            .parent()
            .unwrap()
            .join("vizier-traversal-secret.txt");
        std::fs::write(&secret, b"top secret").unwrap();

        let attempt = format!(
            "a1/memory/andy/../../../{}",
            secret.file_name().unwrap().to_string_lossy()
        );
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
        let store = BlobDocumentStore::local(dir.path().to_path_buf());
        store.put("a/b/c.md", b"hello".to_vec()).await.unwrap();
        let bytes = store.get("a/b/c.md").await.unwrap().unwrap();
        assert_eq!(bytes, b"hello");
    }

    #[tokio::test]
    async fn delete_missing_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobDocumentStore::local(dir.path().to_path_buf());
        store.delete("nope.md").await.unwrap();
    }

    #[tokio::test]
    async fn list_returns_relative_paths_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobDocumentStore::local(dir.path().to_path_buf());
        store.put("bundle/a.md", b"1".to_vec()).await.unwrap();
        store
            .put("bundle/nested/b.md", b"2".to_vec())
            .await
            .unwrap();

        let mut listed = store.list("bundle").await.unwrap();
        listed.sort();
        assert_eq!(listed, vec!["a.md".to_string(), "nested/b.md".to_string()]);
    }

    /// The workspace layout is unchanged: with the `agents` prefix on a workspace-rooted
    /// store, `<agent>/memory/...` lands at `<workspace>/agents/<agent>/memory/...`.
    #[tokio::test]
    async fn the_agents_prefix_keeps_the_workspace_layout() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobDocumentStore::new(
            Arc::new(LocalBlobStore::new(dir.path())),
            BlobKey::new("agents").unwrap(),
        );
        store
            .put("a1/memory/default/x.md", b"x".to_vec())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("agents/a1/memory/default/x.md")).unwrap(),
            b"x"
        );
        assert_eq!(store.list("a1/memory").await.unwrap(), vec!["default/x.md"]);
    }

    /// Regression test for a real bug: a workspace root containing a `.` component (e.g. the
    /// `./.vizier` produced by resolving a config path given as a bare relative filename, as
    /// `--config dev.vizier.yaml` does) made `list` leak the workspace directory's own name
    /// (e.g. `.vizier`) as a bogus top-level "bundle".
    #[tokio::test]
    async fn list_strips_prefix_even_when_root_has_a_curdir_component() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("nested")).unwrap();
        // Same shape as "./.vizier/agents": a literal `.` path component embedded in root.
        let root_with_curdir = PathBuf::from(format!("{}/./nested", dir.path().display()));
        let store = BlobDocumentStore::local(root_with_curdir);

        store
            .put("viz/memory/default/a.md", b"1".to_vec())
            .await
            .unwrap();
        store
            .put("viz/memory/gaming/b.md", b"2".to_vec())
            .await
            .unwrap();

        let files = store.list("viz/memory").await.unwrap();
        let top_level: std::collections::HashSet<String> = files
            .iter()
            .filter_map(|f| f.split('/').next().map(|s| s.to_string()))
            .collect();
        assert_eq!(
            top_level,
            std::collections::HashSet::from(["default".to_string(), "gaming".to_string()])
        );
    }
}
