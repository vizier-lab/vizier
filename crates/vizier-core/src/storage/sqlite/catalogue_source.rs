use crate::time::Timestamp;
use anyhow::Result;

use crate::storage::sql::{OptionalExtension, params};

use crate::storage::{
    catalogue_source::{CatalogueSourceRow, CatalogueSourceStorage},
    sqlite::SqlStorage,
};

fn ms(t: Timestamp) -> i64 {
    t.timestamp_millis()
}

fn at(ms: i64) -> Timestamp {
    crate::time::from_millis_opt(ms).unwrap_or(crate::time::epoch())
}

#[async_trait::async_trait]
impl CatalogueSourceStorage for SqlStorage {
    async fn get_source_snapshot(&self, source: &str) -> Result<Option<CatalogueSourceRow>> {
        let conn = self.conn.lock()?;
        Ok(conn
            .query_row(
                "SELECT source, schema, fetched_at, checked_at, last_error, failed_at, data \
                 FROM catalogue_source_snapshot WHERE source = ?1",
                params![source],
                |r| {
                    Ok(CatalogueSourceRow {
                        source: r.get(0)?,
                        schema: r.get(1)?,
                        fetched_at: at(r.get(2)?),
                        checked_at: at(r.get(3)?),
                        last_error: r.get(4)?,
                        failed_at: r.get::<_, Option<i64>>(5)?.map(at),
                        data: r.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    async fn put_source_snapshot(&self, row: &CatalogueSourceRow) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO catalogue_source_snapshot \
             (source, schema, fetched_at, checked_at, last_error, failed_at, data) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(source) DO UPDATE SET \
                 schema = excluded.schema, \
                 fetched_at = excluded.fetched_at, \
                 checked_at = excluded.checked_at, \
                 last_error = excluded.last_error, \
                 failed_at = excluded.failed_at, \
                 data = excluded.data",
            params![
                row.source,
                row.schema,
                ms(row.fetched_at),
                ms(row.checked_at),
                row.last_error,
                row.failed_at.map(ms),
                row.data,
            ],
        )?;
        Ok(())
    }

    async fn record_source_failure(&self, source: &str, error: &str, at: Timestamp) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute(
            "UPDATE catalogue_source_snapshot SET last_error = ?2, failed_at = ?3 WHERE source = ?1",
            params![source, error, ms(at)],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::storage::document::BlobDocumentStore;

    fn setup() -> (SqlStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::tests_support::test_store(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            crate::storage::sqlite::init_catalogue_source_schema(&conn).unwrap();
            crate::storage::sql::SqlStore::sqlite(conn)
        });
        let storage = SqlStorage::new(store, Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())));
        (storage, dir)
    }

    #[tokio::test]
    async fn a_snapshot_round_trips_and_a_failure_keeps_the_data() {
        let (s, _dir) = setup();
        assert!(s.get_source_snapshot("venice").await.unwrap().is_none());
        let row = CatalogueSourceRow {
            source: "venice".into(),
            schema: 1,
            fetched_at: at(1_700_000_000_000),
            checked_at: at(1_700_000_060_000),
            last_error: None,
            failed_at: None,
            data: b"{}".to_vec(),
        };
        s.put_source_snapshot(&row).await.unwrap();
        assert_eq!(s.get_source_snapshot("venice").await.unwrap(), Some(row.clone()));
        s.record_source_failure("venice", "boom", at(1_700_000_120_000)).await.unwrap();
        let failed = s.get_source_snapshot("venice").await.unwrap().unwrap();
        assert_eq!(failed.data, row.data);
        assert_eq!(failed.last_error.as_deref(), Some("boom"));
        s.put_source_snapshot(&row).await.unwrap();
        assert_eq!(s.get_source_snapshot("venice").await.unwrap().unwrap().last_error, None);
        // A failure for a source with no row writes nothing.
        s.record_source_failure("openrouter", "boom", at(1)).await.unwrap();
        assert!(s.get_source_snapshot("openrouter").await.unwrap().is_none());
    }
}
