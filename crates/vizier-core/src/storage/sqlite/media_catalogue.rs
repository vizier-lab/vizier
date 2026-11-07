use crate::time::Timestamp;
use anyhow::Result;


use crate::storage::sql::{OptionalExtension, params};

use crate::storage::{
    media_catalogue::{MediaCatalogueSnapshotRow, MediaCatalogueStorage},
    sqlite::SqlStorage,
};

fn ms(t: Timestamp) -> i64 {
    t.timestamp_millis()
}

fn at(ms: i64) -> Timestamp {
    crate::time::from_millis_opt(ms).unwrap_or(crate::time::epoch())
}

#[async_trait::async_trait]
impl MediaCatalogueStorage for SqlStorage {
    async fn get_media_snapshot(
        &self,
        capability: &str,
    ) -> Result<Option<MediaCatalogueSnapshotRow>> {
        let conn = self.conn.lock()?;
        Ok(conn
            .query_row(
                "SELECT capability, schema, fetched_at, checked_at, last_error, failed_at, data \
                 FROM media_catalogue_snapshot WHERE capability = ?1",
                params![capability],
                |r| {
                    Ok(MediaCatalogueSnapshotRow {
                        capability: r.get(0)?,
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

    async fn put_media_snapshot(&self, row: &MediaCatalogueSnapshotRow) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO media_catalogue_snapshot \
             (capability, schema, fetched_at, checked_at, last_error, failed_at, data) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(capability) DO UPDATE SET \
                 schema = excluded.schema, \
                 fetched_at = excluded.fetched_at, \
                 checked_at = excluded.checked_at, \
                 last_error = excluded.last_error, \
                 failed_at = excluded.failed_at, \
                 data = excluded.data",
            params![
                row.capability,
                row.schema,
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

    async fn record_media_failure(
        &self,
        capability: &str,
        error: &str,
        at: Timestamp,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute(
            "UPDATE media_catalogue_snapshot SET last_error = ?2, failed_at = ?3 \
             WHERE capability = ?1",
            params![capability, error, ms(at)],
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
            crate::storage::sqlite::init_media_catalogue_schema(&conn).unwrap();
            crate::storage::sql::SqlStore::sqlite(conn)
        });
        let storage = SqlStorage::new(
            store,
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn row(capability: &str, data: &[u8]) -> MediaCatalogueSnapshotRow {
        MediaCatalogueSnapshotRow {
            capability: capability.into(),
            schema: 1,
            fetched_at: crate::time::from_millis_opt(1_700_000_000_000).unwrap(),
            checked_at: crate::time::from_millis_opt(1_700_000_060_000).unwrap(),
            last_error: None,
            failed_at: None,
            data: data.to_vec(),
        }
    }

    #[tokio::test]
    async fn a_row_round_trips_and_an_unknown_capability_is_none() {
        let (storage, _dir) = setup();
        assert!(
            storage
                .get_media_snapshot("speech")
                .await
                .unwrap()
                .is_none()
        );

        storage
            .put_media_snapshot(&row("speech", b"gzipped"))
            .await
            .unwrap();
        let got = storage.get_media_snapshot("speech").await.unwrap().unwrap();
        assert_eq!(got, row("speech", b"gzipped"));

        assert!(storage.get_media_snapshot("video").await.unwrap().is_none());
        assert!(
            storage
                .get_media_snapshot("transcription")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn a_success_clears_the_failure_and_a_failure_keeps_the_data() {
        let (storage, _dir) = setup();
        storage
            .put_media_snapshot(&row("image", b"first"))
            .await
            .unwrap();

        let at = crate::time::from_millis_opt(1_700_000_120_000).unwrap();
        storage
            .record_media_failure("image", "network down", at)
            .await
            .unwrap();
        let failed = storage.get_media_snapshot("image").await.unwrap().unwrap();
        assert_eq!(failed.last_error.as_deref(), Some("network down"));
        assert_eq!(failed.failed_at, Some(at));
        assert_eq!(failed.data, b"first", "a failure never replaces the list");
        assert_eq!(failed.checked_at, row("image", b"first").checked_at);

        // A missing row is a no-op, not an error.
        assert!(
            storage
                .record_media_failure("speech", "x", at)
                .await
                .is_ok()
        );

        storage
            .put_media_snapshot(&row("image", b"second"))
            .await
            .unwrap();
        let fresh = storage.get_media_snapshot("image").await.unwrap().unwrap();
        assert_eq!(fresh.last_error, None);
        assert_eq!(fresh.failed_at, None);
        assert_eq!(fresh.data, b"second");
    }

    crate::postgres_tests!(
        a_row_round_trips_and_an_unknown_capability_is_none,
        a_success_clears_the_failure_and_a_failure_keeps_the_data,
    );
}
