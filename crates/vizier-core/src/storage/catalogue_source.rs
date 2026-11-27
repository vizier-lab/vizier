use crate::time::Timestamp;
use anyhow::Result;

/// One catalogue source's last successful refresh plus its last failure (spec 068, data-model
/// §10). models.dev keeps `catalogue_snapshot`; every other refreshing source has one row here.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogueSourceRow {
    /// The source id (`openrouter`, `venice`).
    pub source: String,
    /// Payload version; a mismatch ignores the row and the seed answers.
    pub schema: i64,
    pub fetched_at: Timestamp,
    pub checked_at: Timestamp,
    pub last_error: Option<String>,
    pub failed_at: Option<Timestamp>,
    /// gzip of the trimmed upstream JSON, in the seed's shape.
    pub data: Vec<u8>,
}

/// Refreshed source snapshots, so a refresh survives restarts.
#[async_trait::async_trait]
pub trait CatalogueSourceStorage {
    async fn get_source_snapshot(&self, source: &str) -> Result<Option<CatalogueSourceRow>>;

    /// Insert or replace one source, clearing its failure fields.
    async fn put_source_snapshot(&self, row: &CatalogueSourceRow) -> Result<()>;

    /// A failed check: record the reason and time, leave the data untouched. A source with no
    /// row yet gets none (the seed answers, and the failure lives in memory until a success).
    async fn record_source_failure(&self, source: &str, error: &str, at: Timestamp) -> Result<()>;
}
