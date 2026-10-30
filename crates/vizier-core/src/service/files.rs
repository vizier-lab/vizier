//! Session file uploads and downloads (spec 038 US2). Multipart/base64 parsing and Range
//! handling are the adapter's; the service validates, stores and locates.

use std::path::PathBuf;

use base64::Engine;
use serde::Serialize;

use super::{ServiceError, Services};
use crate::auth::Principal;

const UPLOADS_DIR: &str = "uploads";

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UploadResponse {
    pub file_id: String,
    pub filename: String,
    pub url: String,
}

/// Stores `file` (base64) as a session file.
pub async fn upload(
    s: &Services,
    _who: &Principal,
    file: &str,
    filename: String,
) -> Result<UploadResponse, ServiceError> {
    if file.is_empty() {
        return Err(ServiceError::invalid("No file provided"));
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(file)
        .map_err(|e| ServiceError::invalid(format!("Invalid base64: {}", e)))?;
    if data.is_empty() {
        return Err(ServiceError::invalid("Empty file content"));
    }
    match s.deps().transport.send_file_upload(filename, data).await {
        Ok(record) => Ok(UploadResponse {
            file_id: record.id,
            filename: record.filename,
            url: record.url,
        }),
        Err(e) => {
            tracing::error!("Failed to upload file: {}", e);
            Err(ServiceError::Internal(format!("Failed to upload file: {}", e)))
        }
    }
}

/// Where an uploaded file is on disk. Downloads are not authenticated: the file id is the
/// capability, as it always was.
pub async fn locate(s: &Services, file_id: &str) -> Result<PathBuf, ServiceError> {
    let dir = PathBuf::from(&s.deps().config.workspace)
        .join(UPLOADS_DIR)
        .join(file_id);
    let not_found = || ServiceError::NotFound("File not found".into());
    let mut entries = tokio::fs::read_dir(&dir).await.map_err(|_| not_found())?;
    match entries.next_entry().await {
        Ok(Some(entry)) => Ok(entry.path()),
        _ => Err(not_found()),
    }
}
