use std::path::PathBuf;

use anyhow::Result;
use base64::Engine;
use chrono::Utc;

use crate::{
    schema::{FileRecord, VizierAttachment, VizierAttachmentContent},
    transport::VizierTransport,
    error::VizierError,
};

#[derive(Clone)]
pub struct FileManager {
    workspace: String,
}

impl FileManager {
    pub fn new(workspace: String) -> Self {
        Self { workspace }
    }

    pub async fn upload(&self, filename: &str, content: Vec<u8>) -> Result<FileRecord> {
        let file_id = uuid::Uuid::new_v4().to_string();
        let uploads_dir = PathBuf::from(&self.workspace).join("uploads");
        tokio::fs::create_dir_all(&uploads_dir).await?;

        let file_dir = uploads_dir.join(&file_id);
        tokio::fs::create_dir_all(&file_dir).await?;

        let file_path = file_dir.join(filename);
        tokio::fs::write(&file_path, &content).await?;

        let mime_type = mime_guess::from_path(filename)
            .first_or_octet_stream()
            .to_string();

        let size = content.len() as u64;
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
        let uploads_dir = PathBuf::from(&self.workspace)
            .join("uploads")
            .join(file_id);

        let mut entries = tokio::fs::read_dir(&uploads_dir).await.map_err(|e| {
            anyhow::anyhow!("Failed to read uploads dir {}: {}", uploads_dir.display(), e)
        })?;

        let entry = entries
            .next_entry()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read dir entry: {}", e))?
            .ok_or_else(|| anyhow::anyhow!("No file found in {}", uploads_dir.display()))?;

        let filename = entry.file_name().to_string_lossy().to_string();
        let content = tokio::fs::read(entry.path()).await.map_err(|e| {
            anyhow::anyhow!("Failed to read file {}: {}", entry.path().display(), e)
        })?;

        Ok((filename, content))
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
                    .map_err(|e| anyhow::anyhow!("Invalid base64: {}", e))?;
                Ok((attachment.filename.clone(), content))
            }
            VizierAttachmentContent::Url(url) => {
                let response = reqwest::get(url).await?;
                let content = response.bytes().await?.to_vec();
                Ok((attachment.filename.clone(), content))
            }
        }
    }

    pub async fn run(&self, transport: VizierTransport) {
        loop {
            match transport.recv_file_command().await {
                Ok(cmd) => {
                    match cmd {
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
                    }
                }
                Err(e) => {
                    tracing::error!("FileManager channel error: {}", e);
                    break;
                }
            }
        }
    }
}
