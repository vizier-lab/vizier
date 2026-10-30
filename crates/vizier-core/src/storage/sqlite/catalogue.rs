use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::storage::{
    catalogue::{CachedLogo, CatalogueCacheStorage, CatalogueSnapshotRow},
    sqlite::SqliteStorage,
};

fn ms(t: DateTime<Utc>) -> i64 {
    t.timestamp_millis()
}

fn at(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
}

#[async_trait::async_trait]
impl CatalogueCacheStorage for SqliteStorage {
    async fn get_snapshot(&self) -> Result<Option<CatalogueSnapshotRow>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT schema, source_url, etag, fetched_at, checked_at, last_error, failed_at, data \
                 FROM catalogue_snapshot WHERE id = 1",
                [],
                |r| {
                    Ok(CatalogueSnapshotRow {
                        schema: r.get(0)?,
                        source_url: r.get(1)?,
                        etag: r.get(2)?,
                        fetched_at: at(r.get(3)?),
                        checked_at: at(r.get(4)?),
                        last_error: r.get(5)?,
                        failed_at: r.get::<_, Option<i64>>(6)?.map(at),
                        data: r.get(7)?,
                    })
                },
            )
            .optional()?)
    }

    async fn put_snapshot(&self, row: &CatalogueSnapshotRow) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO catalogue_snapshot \
             (id, schema, source_url, etag, fetched_at, checked_at, last_error, failed_at, data) \
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                row.schema,
                row.source_url,
                row.etag,
                ms(row.fetched_at),
                ms(row.checked_at),
                row.last_error,
                row.failed_at.map(ms),
                row.data,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn touch_checked(&self, when: DateTime<Utc>) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE catalogue_snapshot SET checked_at = ?1, last_error = NULL, failed_at = NULL WHERE id = 1",
            rusqlite::params![ms(when)],
        )?;
        Ok(())
    }

    async fn record_failure(&self, error: &str, when: DateTime<Utc>) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE catalogue_snapshot SET last_error = ?1, failed_at = ?2 WHERE id = 1",
            rusqlite::params![error, ms(when)],
        )?;
        Ok(())
    }

    async fn get_logo(&self, id: &str) -> Result<Option<CachedLogo>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT svg FROM catalogue_logo WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()?)
    }

    async fn put_logo(&self, id: &str, svg: Option<&[u8]>) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO catalogue_logo (id, svg, fetched_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, svg, ms(Utc::now())],
        )?;
        Ok(())
    }
}
