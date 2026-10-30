use anyhow::Result;
use chrono::{DateTime, Utc};

/// The single `catalogue_snapshot` row (contract `catalogue-snapshot.md` § Cache).
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogueSnapshotRow {
    pub schema: i64,
    pub source_url: String,
    pub etag: Option<String>,
    /// When this data was downloaded (a 200).
    pub fetched_at: DateTime<Utc>,
    /// The last successful check (200 or 304).
    pub checked_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub failed_at: Option<DateTime<Utc>>,
    /// gzip of the trimmed `api.json`.
    pub data: Vec<u8>,
}

/// A logo the refresher looked up: `None` when models.dev has none for this id.
pub type CachedLogo = Option<Vec<u8>>;

/// The refreshed catalogue and lazily fetched logos, cached so they survive restarts.
#[async_trait::async_trait]
pub trait CatalogueCacheStorage {
    async fn get_snapshot(&self) -> Result<Option<CatalogueSnapshotRow>>;
    /// Replace the snapshot, in one transaction.
    async fn put_snapshot(&self, row: &CatalogueSnapshotRow) -> Result<()>;
    /// A 304: the cached data is still current.
    async fn touch_checked(&self, at: DateTime<Utc>) -> Result<()>;
    /// A failed check; the cached data is left as it is.
    async fn record_failure(&self, error: &str, at: DateTime<Utc>) -> Result<()>;
    /// `None` when the logo was never looked up.
    async fn get_logo(&self, id: &str) -> Result<Option<CachedLogo>>;
    async fn put_logo(&self, id: &str, svg: Option<&[u8]>) -> Result<()>;
}
