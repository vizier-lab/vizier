use anyhow::Result;
use chrono::Utc;

use crate::{
    schema::{ClientCallOutcome, VizierSession},
    storage::{client_call::ClientCallStorage, sqlite::SqliteStorage},
};

#[async_trait::async_trait]
impl ClientCallStorage for SqliteStorage {
    async fn open_client_call(&self, uid: &str, session: &VizierSession) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO client_tool_call
                (history_uid, agent_id, channel, topic, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
            rusqlite::params![
                uid,
                session.0,
                session.1.to_slug(),
                session.2,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    async fn finish_client_call(&self, uid: &str, outcome: &ClientCallOutcome) -> Result<bool> {
        let detail = outcome
            .detail()
            .map(|detail| serde_json::to_string(&detail))
            .transpose()?;
        let conn = self.conn.lock();
        // Only an open call changes, which is what makes an outcome final: a late write
        // changes nothing.
        let changed = conn.execute(
            "UPDATE client_tool_call SET status = ?1, detail = ?2, finished_at = ?3
             WHERE history_uid = ?4 AND status IN ('pending', 'deferred')",
            rusqlite::params![outcome.status(), detail, Utc::now().to_rfc3339(), uid],
        )?;
        Ok(changed > 0)
    }

    async fn save_client_call_state(&self, uid: &str, state: &serde_json::Value) -> Result<()> {
        let state = serde_json::to_string(state)?;
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE client_tool_call SET state = ?1, state_updated_at = ?2 WHERE history_uid = ?3",
            rusqlite::params![state, Utc::now().to_rfc3339(), uid],
        )?;
        Ok(())
    }

    async fn interrupt_pending_client_calls(&self) -> Result<usize> {
        let conn = self.conn.lock();
        Ok(conn.execute(
            "UPDATE client_tool_call SET status = 'interrupted', finished_at = ?1
             WHERE status = 'pending'",
            rusqlite::params![Utc::now().to_rfc3339()],
        )?)
    }

    async fn defer_client_call(&self, uid: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE client_tool_call SET status = 'deferred' WHERE history_uid = ?1 AND status = 'pending'",
            [uid],
        )?;
        Ok(changed > 0)
    }

    async fn deferred_client_calls(&self, session: &VizierSession) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT history_uid FROM client_tool_call
             WHERE agent_id = ?1 AND channel = ?2 AND topic IS ?3 AND status = 'deferred'
             ORDER BY created_at, rowid",
        )?;
        let uids = stmt
            .query_map(
                rusqlite::params![session.0, session.1.to_slug(), session.2],
                |row| row.get(0),
            )?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(uids)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;
    use rusqlite::Connection;
    use serde_json::json;

    use super::*;
    use crate::{schema::VizierChannelId, storage::document::LocalDocumentStore};

    fn setup() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_client_call_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn session() -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("alice".into(), "webui".into()),
            Some("t".into()),
        )
    }

    fn row(storage: &SqliteStorage, uid: &str) -> (String, Option<String>, Option<String>) {
        storage
            .conn
            .lock()
            .query_row(
                "SELECT status, detail, state FROM client_tool_call WHERE history_uid = ?1",
                [uid],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    }

    #[tokio::test]
    async fn an_outcome_is_recorded_once() {
        let (storage, _dir) = setup();
        storage.open_client_call("c1", &session()).await.unwrap();
        assert_eq!(row(&storage, "c1").0, "pending");

        let ok = ClientCallOutcome::Ok { result: json!({ "hex": "#3366ff" }) };
        assert!(storage.finish_client_call("c1", &ok).await.unwrap());
        assert_eq!(row(&storage, "c1"), ("ok".into(), Some(r##"{"hex":"#3366ff"}"##.into()), None));

        // A late timeout, say, changes nothing.
        assert!(!storage.finish_client_call("c1", &ClientCallOutcome::Timeout).await.unwrap());
        assert_eq!(row(&storage, "c1").0, "ok");
    }

    #[tokio::test]
    async fn state_saves_in_any_status() {
        let (storage, _dir) = setup();
        storage.open_client_call("c1", &session()).await.unwrap();
        storage.save_client_call_state("c1", &json!({ "n": 1 })).await.unwrap();
        assert_eq!(row(&storage, "c1").2.as_deref(), Some(r#"{"n":1}"#));

        storage.finish_client_call("c1", &ClientCallOutcome::Cancelled).await.unwrap();
        storage.save_client_call_state("c1", &json!({ "n": 2 })).await.unwrap();
        assert_eq!(row(&storage, "c1"), ("cancelled".into(), None, Some(r#"{"n":2}"#.into())));
    }

    #[tokio::test]
    async fn a_deferred_call_survives_the_sweep_and_finishes_once() {
        let (storage, _dir) = setup();
        storage.open_client_call("a", &session()).await.unwrap();
        storage.open_client_call("b", &session()).await.unwrap();
        assert!(storage.defer_client_call("a").await.unwrap());
        assert!(storage.defer_client_call("b").await.unwrap());
        // Only from pending.
        assert!(!storage.defer_client_call("a").await.unwrap());
        assert_eq!(storage.deferred_client_calls(&session()).await.unwrap(), ["a", "b"]);

        assert_eq!(storage.interrupt_pending_client_calls().await.unwrap(), 0);
        assert_eq!(row(&storage, "a").0, "deferred");

        let ok = ClientCallOutcome::Ok { result: json!(1) };
        assert!(storage.finish_client_call("a", &ok).await.unwrap());
        assert!(!storage.finish_client_call("a", &ClientCallOutcome::Timeout).await.unwrap());
        assert_eq!(storage.deferred_client_calls(&session()).await.unwrap(), ["b"]);
        let other = VizierSession("agent-1".into(), VizierChannelId::HTTP("bob".into(), "webui".into()), Some("t".into()));
        assert!(storage.deferred_client_calls(&other).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_sweep_only_interrupts_pending_calls() {
        let (storage, _dir) = setup();
        storage.open_client_call("a", &session()).await.unwrap();
        storage.open_client_call("b", &session()).await.unwrap();
        storage
            .finish_client_call("b", &ClientCallOutcome::Error { message: "no".into() })
            .await
            .unwrap();

        assert_eq!(storage.interrupt_pending_client_calls().await.unwrap(), 1);
        assert_eq!(row(&storage, "a").0, "interrupted");
        assert_eq!(row(&storage, "b"), ("error".into(), Some(r#""no""#.into()), None));
    }
}
