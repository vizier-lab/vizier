use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use base64::Engine;
use bytes::Bytes;
use chrono::Utc;
use parking_lot::RwLock;

use crate::{
    schema::{AttachmentResolver, FileRecord, VizierAttachment, VizierAttachmentContent},
    storage::blob::{BlobKey, BlobMeta, BlobRef, BlobStore},
    transport::VizierTransport,
};

/// Where local copies handed to shell tools live (`get_session_file_path`), under the workspace.
const MATERIALIZED_DIR: &str = "tmp/session-files";

fn upload_dir(file_id: &str) -> Result<BlobKey> {
    BlobKey::new("uploads")?.join(file_id)
}

/// The one object inside `uploads/<file_id>/`.
async fn find_upload(blobs: &dyn BlobStore, file_id: &str) -> Result<BlobMeta> {
    let dir = upload_dir(file_id)?;
    let mut found = blobs.list(&dir).await?;
    // One object per upload; the oldest wins should a stray second one appear.
    found.sort_by(|a, b| a.modified.cmp(&b.modified).then_with(|| a.key.cmp(&b.key)));
    found
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no file found for upload {file_id}"))
}

/// Read the one file inside `uploads/<file_id>/` and return its name and bytes. Free so both
/// the `FileManager` and the artifact service (which cannot own each other) can use it
/// (`specs/035-add-artifacts` R3).
pub async fn read_upload(blobs: &dyn BlobStore, file_id: &str) -> Result<(String, Vec<u8>)> {
    let meta = find_upload(blobs, file_id).await?;
    let bytes = blobs
        .get(&meta.key)
        .await?
        .ok_or_else(|| anyhow!("upload {file_id} vanished while being read"))?;
    Ok((meta.key.file_name().to_string(), bytes.to_vec()))
}

#[derive(Clone)]
pub struct FileManager {
    /// Local directory for materialized copies (shell tools need a real path).
    workspace: PathBuf,
    blobs: Arc<dyn BlobStore>,
    /// Installed by `VizierDependencies` after construction, because `Artifacts` and
    /// `FileManager` cannot own each other. Every clone shares the handle.
    artifact_resolver: Arc<RwLock<Option<Arc<dyn AttachmentResolver>>>>,
}

impl FileManager {
    pub fn new(workspace: impl Into<PathBuf>, blobs: Arc<dyn BlobStore>) -> Self {
        Self {
            workspace: workspace.into(),
            blobs,
            artifact_resolver: Arc::new(RwLock::new(None)),
        }
    }

    pub fn blobs(&self) -> &Arc<dyn BlobStore> {
        &self.blobs
    }

    /// Install the artifact resolver used by `resolve`.
    pub fn install_artifact_resolver(&self, resolver: Arc<dyn AttachmentResolver>) {
        *self.artifact_resolver.write() = Some(resolver);
    }

    /// Remove one upload (`uploads/<file_id>/`) and its materialized copy, best effort.
    pub async fn delete_upload(&self, file_id: &str) -> Result<()> {
        self.blobs.delete_prefix(&upload_dir(file_id)?).await?;
        let copy = self.workspace.join(MATERIALIZED_DIR).join(file_id);
        match tokio::fs::remove_dir_all(&copy).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow!(
                "failed to remove the local copy of upload {file_id}: {e}"
            )),
        }
    }

    pub async fn upload(&self, filename: &str, content: Vec<u8>) -> Result<FileRecord> {
        let file_id = uuid::Uuid::new_v4().to_string();
        let key = upload_dir(&file_id)?.join(filename)?;
        let size = content.len() as u64;
        self.blobs.put(&key, Bytes::from(content)).await?;

        let mime_type = mime_guess::from_path(filename)
            .first_or_octet_stream()
            .to_string();
        let url = format!("/api/v1/files/{}", file_id);

        Ok(FileRecord {
            id: file_id,
            filename: filename.to_string(),
            mime_type,
            size,
            url,
            created_at: Utc::now(),
        })
    }

    pub async fn get(&self, file_id: &str) -> Result<(String, Vec<u8>)> {
        read_upload(self.blobs.as_ref(), file_id).await
    }

    /// The upload's object, for streaming (`/files/{id}`). `None` when there is none.
    pub async fn locate(&self, file_id: &str) -> Result<Option<BlobRef>> {
        match find_upload(self.blobs.as_ref(), file_id).await {
            Ok(meta) => Ok(Some(BlobRef {
                store: self.blobs.clone(),
                meta,
            })),
            Err(_) => Ok(None),
        }
    }

    /// Whether upload `file_id` still has its bytes.
    pub async fn exists(&self, file_id: &str) -> bool {
        find_upload(self.blobs.as_ref(), file_id).await.is_ok()
    }

    /// A local filesystem path holding the upload's bytes, for tools that hand a path to a
    /// shell. The bytes are copied out of the blob store into `<workspace>/tmp/session-files/`
    /// (reused while the size matches); the copy is read-only by convention.
    pub async fn path_of(&self, file_id: &str) -> Result<PathBuf> {
        let meta = find_upload(self.blobs.as_ref(), file_id).await?;
        let dir = self.workspace.join(MATERIALIZED_DIR).join(file_id);
        let path = dir.join(meta.key.file_name());
        if let Ok(existing) = tokio::fs::metadata(&path).await
            && existing.is_file()
            && existing.len() == meta.size
        {
            return Ok(tokio::fs::canonicalize(&path).await?);
        }
        let bytes = self
            .blobs
            .get(&meta.key)
            .await?
            .ok_or_else(|| anyhow!("upload {file_id} vanished while being read"))?;
        tokio::fs::create_dir_all(&dir).await?;
        let temp = dir.join(format!(
            ".{}.tmp-{}",
            meta.key.file_name(),
            uuid::Uuid::new_v4()
        ));
        tokio::fs::write(&temp, &bytes).await?;
        if let Err(e) = tokio::fs::rename(&temp, &path).await {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(e.into());
        }
        Ok(tokio::fs::canonicalize(&path).await?)
    }

    pub async fn resolve(&self, attachment: &VizierAttachment) -> Result<(String, Vec<u8>)> {
        match &attachment.content {
            VizierAttachmentContent::Local(path) => {
                let file_id = path.trim_start_matches("/api/v1/files/");
                let (filename, content) = self.get(file_id).await?;
                Ok((filename, content))
            }
            VizierAttachmentContent::Bytes(b) => Ok((attachment.filename.clone(), b.clone())),
            VizierAttachmentContent::Base64(b) => {
                let content = base64::engine::general_purpose::STANDARD
                    .decode(b)
                    .map_err(|e| anyhow!("Invalid base64: {}", e))?;
                Ok((attachment.filename.clone(), content))
            }
            VizierAttachmentContent::Url(url) => {
                let response = reqwest::get(url).await?;
                let content = response.bytes().await?.to_vec();
                Ok((attachment.filename.clone(), content))
            }
            VizierAttachmentContent::Artifact {
                agent_id,
                artifact_id,
                ..
            } => {
                let resolver = self.artifact_resolver.read().clone();
                let unavailable =
                    || anyhow!("artifact '{}' is no longer available", attachment.filename);
                let resolver = resolver.ok_or_else(unavailable)?;
                let content = resolver
                    .artifact_bytes(agent_id, artifact_id)
                    .await
                    .ok()
                    .flatten()
                    .ok_or_else(unavailable)?;
                Ok((attachment.filename.clone(), content))
            }
        }
    }

    pub async fn run(&self, transport: VizierTransport) {
        loop {
            match transport.recv_file_command().await {
                Ok(cmd) => match cmd {
                    crate::schema::FileCommand::Upload {
                        filename,
                        content,
                        response,
                    } => {
                        let result = self.upload(&filename, content).await;
                        let _ = response.send(result);
                    }
                    crate::schema::FileCommand::Resolve {
                        attachment,
                        response,
                    } => {
                        let result = self.resolve(&attachment).await;
                        let _ = response.send(result.map(|(_, content)| content));
                    }
                },
                Err(e) => {
                    tracing::error!("FileManager channel error: {}", e);
                    break;
                }
            }
        }
    }
}
