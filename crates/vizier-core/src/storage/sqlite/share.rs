//! The shared-snapshot tables (`specs/055-shared-chat/data-model.md`). Both are new, created
//! with `CREATE TABLE IF NOT EXISTS`; nothing existing is altered (SC-009). The single
//! implementation of [`ShareStorage`] is written against the backend-neutral `Connection`.
//! `snapshot_json` is `TEXT`, so the stored bytes are the bytes that were previewed, and the
//! list queries never select it.

use anyhow::Result;

use crate::storage::{
    share::{Audience, InsertOutcome, ShareContent, ShareRecord, ShareStorage, ShareSummary},
    sql::{Connection, OptionalExtension, ToSql, params, params_from_iter},
    sqlite::SqlStorage,
};

pub fn init_share_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS shared_snapshot (
            share_id       TEXT PRIMARY KEY,
            creator_id     TEXT NOT NULL,
            agent_id       TEXT NOT NULL,
            topic_id       TEXT NOT NULL,
            title          TEXT NOT NULL,
            audience       TEXT NOT NULL CHECK (audience IN ('specific', 'signed_in', 'public')),
            include_trail  INTEGER NOT NULL,
            format_version INTEGER NOT NULL,
            size_bytes     INTEGER NOT NULL,
            created_at     INTEGER NOT NULL,
            revoked_at     INTEGER,
            revoked_by     TEXT,
            removals_json  TEXT NOT NULL,
            snapshot_json  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_snapshot_creator
            ON shared_snapshot(creator_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_shared_snapshot_created
            ON shared_snapshot(created_at DESC);

        CREATE TABLE IF NOT EXISTS shared_snapshot_audience (
            share_id TEXT NOT NULL REFERENCES shared_snapshot(share_id) ON DELETE CASCADE,
            user_id  TEXT NOT NULL,
            PRIMARY KEY (share_id, user_id)
        );
        CREATE INDEX IF NOT EXISTS idx_shared_snapshot_audience_user
            ON shared_snapshot_audience(user_id);
        ",
    )?;
    Ok(())
}

/// Every column of a listing; never `snapshot_json`.
const META: &str = "s.share_id, s.creator_id, s.agent_id, s.topic_id, s.title, s.audience, \
                    s.include_trail, s.format_version, s.size_bytes, s.created_at, s.revoked_at, \
                    s.revoked_by, s.removals_json";

fn record_from(row: &crate::storage::sql::Row) -> crate::storage::sql::Result<ShareRecord> {
    let audience: String = row.get(5)?;
    Ok(ShareRecord {
        share_id: row.get(0)?,
        creator_id: row.get(1)?,
        agent_id: row.get(2)?,
        topic_id: row.get(3)?,
        title: row.get(4)?,
        // A value outside the CHECK cannot be stored; the fallback is the narrowest audience.
        audience: Audience::from_name(&audience).unwrap_or(Audience::Specific),
        include_trail: row.get::<_, i64>(6)? != 0,
        format_version: row.get(7)?,
        size_bytes: row.get(8)?,
        created_at: row.get(9)?,
        revoked_at: row.get(10)?,
        revoked_by: row.get(11)?,
        removals_json: row.get(12)?,
    })
}

fn summaries(
    conn: &Connection,
    sql: &str,
    params: Vec<Box<dyn ToSql>>,
) -> Result<Vec<ShareSummary>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter()), |row| {
            Ok(ShareSummary {
                record: record_from(row)?,
                people_count: row.get::<_, i64>(13)?.max(0) as usize,
            })
        })?
        .collect::<crate::storage::sql::Result<Vec<_>>>()?;
    Ok(rows)
}

fn list(
    conn: &Connection,
    creator: Option<&str>,
    before: Option<i64>,
    limit: usize,
) -> Result<Vec<ShareSummary>> {
    let mut sql = format!(
        "SELECT {META}, \
           (SELECT COUNT(*) FROM shared_snapshot_audience a WHERE a.share_id = s.share_id) \
         FROM shared_snapshot s WHERE 1 = 1"
    );
    let mut args: Vec<Box<dyn ToSql>> = vec![];
    if let Some(creator) = creator {
        sql.push_str(" AND s.creator_id = ?");
        args.push(Box::new(creator.to_string()));
    }
    if let Some(before) = before {
        sql.push_str(" AND s.created_at < ?");
        args.push(Box::new(before));
    }
    sql.push_str(" ORDER BY s.created_at DESC, s.share_id DESC LIMIT ?");
    args.push(Box::new(limit as i64));
    summaries(conn, &sql, args)
}

#[async_trait::async_trait]
impl ShareStorage for SqlStorage {
    async fn insert_share(&self, share: &ShareContent, max_active: usize) -> Result<InsertOutcome> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        // One author's creates are serialised on every backend (a no-op on SQLite, whose
        // `BEGIN IMMEDIATE` already holds the write lock), so the count below stays true.
        conn.lock_key(&format!("share:{}", share.record.creator_id))?;
        let id_taken: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM shared_snapshot WHERE share_id = ?1",
                params![share.record.share_id],
                |row| row.get(0),
            )
            .optional()?;
        if id_taken.is_some() {
            return Ok(InsertOutcome::IdTaken);
        }
        let active: i64 = conn.query_row(
            "SELECT COUNT(*) FROM shared_snapshot WHERE creator_id = ?1 AND revoked_at IS NULL",
            params![share.record.creator_id],
            |row| row.get(0),
        )?;
        if active >= max_active as i64 {
            return Ok(InsertOutcome::CapReached);
        }
        let r = &share.record;
        conn.execute(
            "INSERT INTO shared_snapshot
                (share_id, creator_id, agent_id, topic_id, title, audience, include_trail,
                 format_version, size_bytes, created_at, revoked_at, revoked_by, removals_json,
                 snapshot_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, NULL, ?11, ?12)",
            params![
                r.share_id,
                r.creator_id,
                r.agent_id,
                r.topic_id,
                r.title,
                r.audience.name(),
                i64::from(r.include_trail),
                r.format_version,
                r.size_bytes,
                r.created_at,
                r.removals_json,
                share.snapshot_json,
            ],
        )?;
        for user_id in &share.people {
            conn.execute(
                "INSERT INTO shared_snapshot_audience (share_id, user_id) VALUES (?1, ?2)
                 ON CONFLICT DO NOTHING",
                params![r.share_id, user_id],
            )?;
        }
        tx.commit()?;
        Ok(InsertOutcome::Inserted)
    }

    async fn get_share(&self, share_id: &str) -> Result<Option<ShareContent>> {
        let conn = self.conn.lock()?;
        let found = conn
            .query_row(
                &format!("SELECT {META}, s.snapshot_json FROM shared_snapshot s WHERE s.share_id = ?1"),
                params![share_id],
                |row| Ok((record_from(row)?, row.get::<_, String>(13)?)),
            )
            .optional()?;
        let Some((record, snapshot_json)) = found else {
            return Ok(None);
        };
        let mut stmt = conn.prepare(
            "SELECT user_id FROM shared_snapshot_audience WHERE share_id = ?1 ORDER BY user_id",
        )?;
        let people = stmt
            .query_map(params![share_id], |row| row.get::<_, String>(0))?
            .collect::<crate::storage::sql::Result<Vec<_>>>()?;
        Ok(Some(ShareContent { record, snapshot_json, people }))
    }

    async fn list_shares_by_creator(
        &self,
        creator_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<ShareSummary>> {
        let conn = self.conn.lock()?;
        list(&conn, Some(creator_id), before, limit)
    }

    async fn list_all_shares(&self, before: Option<i64>, limit: usize) -> Result<Vec<ShareSummary>> {
        let conn = self.conn.lock()?;
        list(&conn, None, before, limit)
    }

    async fn revoke_share(&self, share_id: &str, by: &str, at: i64) -> Result<bool> {
        let conn = self.conn.lock()?;
        let changed = conn.execute(
            "UPDATE shared_snapshot SET revoked_at = ?1, revoked_by = ?2
             WHERE share_id = ?3 AND revoked_at IS NULL",
            params![at, by, share_id],
        )?;
        Ok(changed > 0)
    }

    async fn count_active_by_creator(&self, creator_id: &str) -> Result<usize> {
        let conn = self.conn.lock()?;
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM shared_snapshot WHERE creator_id = ?1 AND revoked_at IS NULL",
            params![creator_id],
            |row| row.get(0),
        )?;
        Ok(n.max(0) as usize)
    }

    async fn delete_shares_by_creator(&self, creator_id: &str) -> Result<usize> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        // Explicit, so it holds even where foreign keys are not enforced.
        conn.execute(
            "DELETE FROM shared_snapshot_audience WHERE share_id IN
                (SELECT share_id FROM shared_snapshot WHERE creator_id = ?1)",
            params![creator_id],
        )?;
        let n = conn.execute(
            "DELETE FROM shared_snapshot WHERE creator_id = ?1",
            params![creator_id],
        )?;
        tx.commit()?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::storage::sql::SqlStore;

    /// SQLite in memory, or PostgreSQL (fully migrated) inside `on_postgres`.
    fn storage() -> SqlStorage {
        let store = crate::tests_support::test_store(|| {
            let store = SqlStore::sqlite_in_memory().unwrap();
            store
                .lock()
                .unwrap()
                .with_rusqlite(|c| {
                    c.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
                    SqlStorage::init_schema(c).unwrap();
                })
                .unwrap();
            store
        });
        let dir = tempfile::tempdir().unwrap().keep();
        SqlStorage::new(store, Arc::new(crate::storage::document::BlobDocumentStore::local(dir)))
    }

    fn share(id: &str, creator: &str, audience: Audience, people: &[&str], at: i64) -> ShareContent {
        let snapshot_json = format!("{{\"format_version\":1,\"title\":\"t {id}\",\"x\":\"é \\u0000 \u{2028}  spaced\"}}");
        ShareContent {
            record: ShareRecord {
                share_id: id.into(),
                creator_id: creator.into(),
                agent_id: "agent".into(),
                topic_id: "topic".into(),
                title: format!("t {id}"),
                audience,
                include_trail: false,
                format_version: 1,
                size_bytes: snapshot_json.len() as i64,
                created_at: at,
                revoked_at: None,
                revoked_by: None,
                removals_json: "{\"credentials\":0}".into(),
            },
            snapshot_json,
            people: people.iter().map(|p| p.to_string()).collect(),
        }
    }

    #[tokio::test]
    async fn insert_and_get_keep_the_bytes() {
        let s = storage();
        let c = share("a", "u1", Audience::Specific, &["u2", "u3"], 10);
        assert_eq!(s.insert_share(&c, 200).await.unwrap(), InsertOutcome::Inserted);
        let got = s.get_share("a").await.unwrap().unwrap();
        assert_eq!(got.snapshot_json.as_bytes(), c.snapshot_json.as_bytes());
        assert_eq!(got.record, c.record);
        assert_eq!(got.people, vec!["u2", "u3"]);
        assert!(s.get_share("missing").await.unwrap().is_none());
        assert_eq!(s.insert_share(&c, 200).await.unwrap(), InsertOutcome::IdTaken);
    }

    #[tokio::test]
    async fn lists_carry_no_content_and_page_newest_first() {
        let s = storage();
        for (i, id) in ["a", "b", "c"].iter().enumerate() {
            let who = if i == 2 { "u2" } else { "u1" };
            s.insert_share(&share(id, who, Audience::Public, &[], i as i64), 200).await.unwrap();
        }
        s.insert_share(&share("d", "u1", Audience::Specific, &["x", "y"], 5), 200).await.unwrap();
        let mine = s.list_shares_by_creator("u1", None, 10).await.unwrap();
        let ids: Vec<_> = mine.iter().map(|m| m.record.share_id.as_str()).collect();
        assert_eq!(ids, ["d", "b", "a"]);
        assert_eq!(mine[0].people_count, 2);
        let page = s.list_shares_by_creator("u1", Some(5), 1).await.unwrap();
        assert_eq!(page[0].record.share_id, "b");
        assert_eq!(s.list_all_shares(None, 10).await.unwrap().len(), 4);
        // The listing SQL has no way to read the document.
        assert!(!META.contains("snapshot_json"));
    }

    #[tokio::test]
    async fn revoke_is_conditional_and_idempotent() {
        let s = storage();
        s.insert_share(&share("a", "u1", Audience::Public, &[], 1), 200).await.unwrap();
        assert!(s.revoke_share("a", "u1", 50).await.unwrap());
        assert!(!s.revoke_share("a", "admin", 99).await.unwrap());
        assert!(!s.revoke_share("nope", "u1", 99).await.unwrap());
        let got = s.get_share("a").await.unwrap().unwrap().record;
        assert_eq!((got.revoked_at, got.revoked_by.as_deref()), (Some(50), Some("u1")));
    }

    #[tokio::test]
    async fn the_cap_counts_active_only_and_audience_rows_cascade() {
        let s = storage();
        s.insert_share(&share("a", "u1", Audience::Specific, &["u2"], 1), 2).await.unwrap();
        s.insert_share(&share("b", "u1", Audience::Public, &[], 2), 2).await.unwrap();
        assert_eq!(
            s.insert_share(&share("c", "u1", Audience::Public, &[], 3), 2).await.unwrap(),
            InsertOutcome::CapReached
        );
        assert_eq!(s.count_active_by_creator("u1").await.unwrap(), 2);
        s.revoke_share("b", "u1", 9).await.unwrap();
        assert_eq!(s.count_active_by_creator("u1").await.unwrap(), 1);
        assert_eq!(
            s.insert_share(&share("c", "u1", Audience::Public, &[], 3), 2).await.unwrap(),
            InsertOutcome::Inserted
        );
        assert_eq!(s.delete_shares_by_creator("u1").await.unwrap(), 3);
        let left: i64 = s
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM shared_snapshot_audience", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[tokio::test]
    async fn parallel_creates_at_the_cap_leave_at_most_the_cap() {
        let s = Arc::new(storage());
        let mut tasks = vec![];
        for i in 0..8 {
            let s = s.clone();
            tasks.push(tokio::spawn(async move {
                s.insert_share(&share(&format!("p{i}"), "u1", Audience::Public, &[], i), 3).await.unwrap()
            }));
        }
        let mut inserted = 0;
        for t in tasks {
            if t.await.unwrap() == InsertOutcome::Inserted {
                inserted += 1;
            }
        }
        assert_eq!(inserted, 3);
        assert_eq!(s.count_active_by_creator("u1").await.unwrap(), 3);
    }

    /// SC-009: creating the share tables leaves every other table's definition as it was.
    #[test]
    fn existing_tables_are_unchanged() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        SqlStorage::register_sqlite_vec();
        let snapshot = |c: &rusqlite::Connection| -> Vec<(String, String)> {
            let mut stmt = c
                .prepare(
                    "SELECT name, COALESCE(sql, '') FROM sqlite_master
                     WHERE name NOT LIKE 'shared_snapshot%' AND name NOT LIKE 'idx_shared_snapshot%'
                     ORDER BY name",
                )
                .unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect()
        };
        // Everything else first, then the share tables.
        SqlStorage::init_schema(&conn).unwrap();
        let before = snapshot(&conn);
        conn.execute_batch("DROP TABLE shared_snapshot_audience; DROP TABLE shared_snapshot;").unwrap();
        init_share_schema(&conn).unwrap();
        init_share_schema(&conn).unwrap(); // idempotent
        assert_eq!(before, snapshot(&conn));
        assert!(before.len() > 20);
    }

    #[test]
    fn the_schema_check_still_passes() {
        let expected = SqlStorage::expected_schema().unwrap();
        let n: i64 = expected
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('shared_snapshot','shared_snapshot_audience')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2);
    }

    crate::postgres_tests!(
        insert_and_get_keep_the_bytes,
        lists_carry_no_content_and_page_newest_first,
        revoke_is_conditional_and_idempotent,
        the_cap_counts_active_only_and_audience_rows_cascade,
        parallel_creates_at_the_cap_leave_at_most_the_cap,
    );
}
