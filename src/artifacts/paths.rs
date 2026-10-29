//! The artifact content store (`specs/035-add-artifacts`, contracts/artifact-storage.md §2).
//!
//! Layout: `<workspace>/artifacts/<agent_id>/<artifact_id>/content.v<n>`. A new version is
//! written to a temporary name in the same directory and renamed to its version name, which is
//! always a new file — a live file is never replaced, so no platform-specific replace-on-rename
//! behaviour is involved.

use std::path::PathBuf;

use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct ArtifactPaths {
    workspace: PathBuf,
}

impl ArtifactPaths {
    pub fn new(workspace: impl Into<PathBuf>) -> Self {
        Self {
            workspace: workspace.into(),
        }
    }

    pub fn artifact_dir(&self, agent_id: &str, artifact_id: &str) -> PathBuf {
        self.workspace
            .join("artifacts")
            .join(agent_id)
            .join(artifact_id)
    }

    pub fn content_path(&self, agent_id: &str, artifact_id: &str, file: &str) -> PathBuf {
        self.artifact_dir(agent_id, artifact_id).join(file)
    }

    /// Write `bytes` as `content.v<version>` and return that file name.
    pub async fn write_version(
        &self,
        agent_id: &str,
        artifact_id: &str,
        version: u32,
        bytes: &[u8],
    ) -> Result<String> {
        let dir = self.artifact_dir(agent_id, artifact_id);
        tokio::fs::create_dir_all(&dir).await?;
        let file = format!("content.v{version}");
        let target = dir.join(&file);
        // Never overwrite a live version: the same version number is only written for a brand
        // new artifact, and a leftover from a failed attempt is removed by the caller.
        if tokio::fs::try_exists(&target).await? {
            return Err(anyhow!(
                "artifact content {} already exists",
                target.display()
            ));
        }
        let temp = dir.join(format!("{file}.tmp-{}", uuid::Uuid::new_v4()));
        {
            use tokio::io::AsyncWriteExt;
            let mut handle = tokio::fs::File::create(&temp).await?;
            handle.write_all(bytes).await?;
            handle.flush().await?;
        }
        tokio::fs::rename(&temp, &target).await?;
        Ok(file)
    }

    pub async fn read_current(
        &self,
        agent_id: &str,
        artifact_id: &str,
        file: &str,
    ) -> Result<Vec<u8>> {
        let path = self.content_path(agent_id, artifact_id, file);
        tokio::fs::read(&path)
            .await
            .map_err(|e| anyhow!("failed to read artifact content {}: {e}", path.display()))
    }

    pub async fn delete_artifact(&self, agent_id: &str, artifact_id: &str) -> Result<()> {
        let dir = self.artifact_dir(agent_id, artifact_id);
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow!(
                "failed to remove artifact directory {}: {e}",
                dir.display()
            )),
        }
    }

    pub async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        let dir = self.workspace.join("artifacts").join(agent_id);
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow!(
                "failed to remove agent artifacts {}: {e}",
                dir.display()
            )),
        }
    }

    pub fn exists(&self, agent_id: &str, artifact_id: &str, file: &str) -> bool {
        self.content_path(agent_id, artifact_id, file).is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (ArtifactPaths, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (ArtifactPaths::new(dir.path()), dir)
    }

    #[tokio::test]
    async fn content_lives_only_under_the_artifact_directory() {
        let (p, dir) = paths();
        let file = p.write_version("a", "id", 1, b"hello").await.unwrap();
        assert_eq!(file, "content.v1");
        let path = p.content_path("a", "id", &file);
        assert_eq!(
            path,
            dir.path()
                .join("artifacts")
                .join("a")
                .join("id")
                .join("content.v1")
        );
        assert_eq!(p.read_current("a", "id", &file).await.unwrap(), b"hello");
        assert!(!path.to_string_lossy().contains("uploads"));
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
    }

    #[tokio::test]
    async fn a_failed_write_leaves_no_referenced_file() {
        let (p, dir) = paths();
        // A plain file where the agent directory should be makes create_dir_all fail.
        tokio::fs::create_dir_all(dir.path().join("artifacts"))
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("artifacts").join("a"), b"blocked")
            .await
            .unwrap();
        assert!(p.write_version("a", "id", 1, b"x").await.is_err());
        assert!(!p.exists("a", "id", "content.v1"));
    }

    #[tokio::test]
    async fn deletion_is_scoped_and_idempotent() {
        let (p, _d) = paths();
        p.write_version("a", "id1", 1, b"x").await.unwrap();
        p.write_version("a", "id2", 1, b"y").await.unwrap();
        p.write_version("b", "id3", 1, b"z").await.unwrap();
        p.delete_artifact("a", "id1").await.unwrap();
        assert!(!p.exists("a", "id1", "content.v1"));
        assert!(p.exists("a", "id2", "content.v1"));
        assert!(p.exists("b", "id3", "content.v1"));
        // Deleting again is fine.
        p.delete_artifact("a", "id1").await.unwrap();
        p.delete_agent("a").await.unwrap();
        assert!(!p.exists("a", "id2", "content.v1"));
        assert!(p.exists("b", "id3", "content.v1"));
    }
}
