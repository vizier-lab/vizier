use anyhow::Result;
use chrono::{DateTime, Utc};

/// One capability's last successful OpenRouter media listing plus its last failure
/// (contract `media-catalogue-storage.md` § Trait).
#[derive(Debug, Clone, PartialEq)]
pub struct MediaCatalogueSnapshotRow {
    /// `speech`, `transcription` or `image`.
    pub capability: String,
    /// Payload version; a mismatch ignores the row.
    pub schema: i64,
    pub fetched_at: DateTime<Utc>,
    pub checked_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub failed_at: Option<DateTime<Utc>>,
    /// gzip of `serde_json::to_vec(&Vec<MediaModel>)`.
    pub data: Vec<u8>,
}

/// The three OpenRouter media model listings, cached so a refresh survives restarts. Keyed by
/// capability and kept separate from the chat/embedding `catalogue_snapshot`.
#[async_trait::async_trait]
pub trait MediaCatalogueStorage {
    async fn get_media_snapshot(
        &self,
        capability: &str,
    ) -> Result<Option<MediaCatalogueSnapshotRow>>;

    /// Insert or replace one capability, clearing the failure fields in the same transaction.
    async fn put_media_snapshot(&self, row: &MediaCatalogueSnapshotRow) -> Result<()>;

    /// A failed check: record the reason/time, leave data and `checked_at` untouched.
    async fn record_media_failure(
        &self,
        capability: &str,
        error: &str,
        at: DateTime<Utc>,
    ) -> Result<()>;
}
