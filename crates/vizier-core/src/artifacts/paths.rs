//! The artifact content store (`specs/035-add-artifacts`, contracts/artifact-storage.md §2).
//!
//! Keys: `artifacts/<agent_id>/<artifact_id>/content.v<n>` in the [`BlobStore`] (locally,
//! exactly the old `<workspace>/artifacts/...` files). A new version is always a new key and a
//! live version is never overwritten; each `put` is atomic.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use bytes::Bytes;

use crate::storage::blob::{BlobKey, BlobRef, BlobStore};

#[derive(Clone)]
pub struct ArtifactPaths {
    blobs: Arc<dyn BlobStore>,
}

impl ArtifactPaths {
    pub fn new(blobs: Arc<dyn BlobStore>) -> Self {
        Self { blobs }
    }

    fn agent_key(agent_id: &str) -> Result<BlobKey> {
        BlobKey::new("artifacts")?.join(agent_id)
    }

    fn artifact_key(agent_id: &str, artifact_id: &str) -> Result<BlobKey> {
        Self::agent_key(agent_id)?.join(artifact_id)
    }

    pub fn content_key(agent_id: &str, artifact_id: &str, file: &str) -> Result<BlobKey> {
        Self::artifact_key(agent_id, artifact_id)?.join(file)
    }

    /// Write `bytes` as `content.v<version>` and return that file name.
    pub async fn write_version(
        &self,
        agent_id: &str,
        artifact_id: &str,
        version: u32,
        bytes: &[u8],
    ) -> Result<String> {
        let file = format!("content.v{version}");
        let key = Self::content_key(agent_id, artifact_id, &file)?;
        // Never overwrite a live version: the same version number is only written for a brand
        // new artifact, and a leftover from a failed attempt is removed by the caller.
        if self.blobs.head(&key).await?.is_some() {
            return Err(anyhow!("artifact content {key} already exists"));
        }
        self.blobs.put(&key, Bytes::copy_from_slice(bytes)).await?;
        Ok(file)
    }

    pub async fn read_current(
        &self,
        agent_id: &str,
        artifact_id: &str,
        file: &str,
    ) -> Result<Vec<u8>> {
        let key = Self::content_key(agent_id, artifact_id, file)?;
        self.blobs
            .get(&key)
            .await
            .map_err(|e| anyhow!("failed to read artifact content {key}: {e}"))?
            .map(|b| b.to_vec())
            .ok_or_else(|| anyhow!("artifact content {key} is missing"))
    }

    /// The version's object for streaming, `None` when it is missing.
    pub async fn content_ref(
        &self,
        agent_id: &str,
        artifact_id: &str,
        file: &str,
    ) -> Result<Option<BlobRef>> {
        let key = Self::content_key(agent_id, artifact_id, file)?;
        Ok(self.blobs.head(&key).await?.map(|meta| BlobRef {
            store: self.blobs.clone(),
            meta,
        }))
    }

    /// Remove one version. Missing is fine.
    pub async fn delete_version(
        &self,
        agent_id: &str,
        artifact_id: &str,
        file: &str,
    ) -> Result<()> {
        self.blobs
            .delete(&Self::content_key(agent_id, artifact_id, file)?)
            .await
    }

    /// The version file names present for an artifact.
    pub async fn versions(&self, agent_id: &str, artifact_id: &str) -> Result<Vec<String>> {
        let base = Self::artifact_key(agent_id, artifact_id)?;
        Ok(self
            .blobs
            .list(&base)
            .await?
            .into_iter()
            .filter_map(|meta| meta.key.strip_prefix(&base).map(str::to_string))
            .filter(|name| name.starts_with("content.v") && !name.contains('/'))
            .collect())
    }

    pub async fn delete_artifact(&self, agent_id: &str, artifact_id: &str) -> Result<()> {
        let key = Self::artifact_key(agent_id, artifact_id)?;
        self.blobs
            .delete_prefix(&key)
            .await
            .map_err(|e| anyhow!("failed to remove artifact content {key}: {e}"))
    }

    pub async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        let key = Self::agent_key(agent_id)?;
        self.blobs
            .delete_prefix(&key)
            .await
            .map_err(|e| anyhow!("failed to remove agent artifacts {key}: {e}"))
    }

    pub async fn exists(&self, agent_id: &str, artifact_id: &str, file: &str) -> bool {
        match Self::content_key(agent_id, artifact_id, file) {
            Ok(key) => matches!(self.blobs.head(&key).await, Ok(Some(_))),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::blob::LocalBlobStore;

    fn paths() -> (ArtifactPaths, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (
            ArtifactPaths::new(Arc::new(LocalBlobStore::new(dir.path()))),
            dir,
        )
    }

    #[tokio::test]
    async fn content_lives_only_under_the_artifact_directory() {
        let (p, dir) = paths();
        let file = p.write_version("a", "id", 1, b"hello").await.unwrap();
        assert_eq!(file, "content.v1");
        let key = ArtifactPaths::content_key("a", "id", &file).unwrap();
        assert_eq!(key.as_str(), "artifacts/a/id/content.v1");
        assert_eq!(
            std::fs::read(
                dir.path()
                    .join("artifacts")
                    .join("a")
                    .join("id")
                    .join("content.v1")
            )
            .unwrap(),
            b"hello"
        );
        assert_eq!(p.read_current("a", "id", &file).await.unwrap(), b"hello");
    }

    #[tokio::test]
    async fn a_new_version_is_a_new_file_and_a_live_file_is_never_replaced() {
        let (p, _d) = paths();
        p.write_version("a", "id", 1, b"v1").await.unwrap();
        p.write_version("a", "id", 2, b"v2").await.unwrap();
        assert_eq!(
            p.read_current("a", "id", "content.v1").await.unwrap(),
            b"v1"
        );
        assert_eq!(
            p.read_current("a", "id", "content.v2").await.unwrap(),
            b"v2"
        );
        // Writing the same version again is refused instead of clobbering it.
        assert!(p.write_version("a", "id", 2, b"other").await.is_err());
        assert_eq!(
            p.read_current("a", "id", "content.v2").await.unwrap(),
            b"v2"
        );
        let mut versions = p.versions("a", "id").await.unwrap();
        versions.sort();
        assert_eq!(versions, vec!["content.v1", "content.v2"]);
    }

    #[tokio::test]
    async fn a_failed_write_leaves_no_referenced_file() {
        let (p, dir) = paths();
        // A plain file where the agent directory should be makes the write fail.
        tokio::fs::create_dir_all(dir.path().join("artifacts"))
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("artifacts").join("a"), b"blocked")
            .await
            .unwrap();
        assert!(p.write_version("a", "id", 1, b"x").await.is_err());
        assert!(!p.exists("a", "id", "content.v1").await);
    }

    #[tokio::test]
    async fn deletion_is_scoped_and_idempotent() {
        let (p, _d) = paths();
        p.write_version("a", "id1", 1, b"x").await.unwrap();
        p.write_version("a", "id2", 1, b"y").await.unwrap();
        p.write_version("b", "id3", 1, b"z").await.unwrap();
        p.delete_artifact("a", "id1").await.unwrap();
        assert!(!p.exists("a", "id1", "content.v1").await);
        assert!(p.exists("a", "id2", "content.v1").await);
        assert!(p.exists("b", "id3", "content.v1").await);
        // Deleting again is fine.
        p.delete_artifact("a", "id1").await.unwrap();
        p.delete_agent("a").await.unwrap();
        assert!(!p.exists("a", "id2", "content.v1").await);
        assert!(p.exists("b", "id3", "content.v1").await);
    }
}
