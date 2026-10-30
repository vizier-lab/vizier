use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;

use crate::{
    schema::{VizierRequest, VizierSession},
    storage::{
        sqlite::SqliteStorage,
        turn::{PendingTurn, TurnState, TurnStorage},
    },
};

fn key(session: &VizierSession) -> (&str, String, &str) {
    (&session.0, session.1.to_slug(), session.2.as_deref().unwrap_or(""))
}

/// Delete a session's rows, on the connection already held (`delete_session`).
pub fn delete_session(conn: &Connection, agent_id: &str, channel: &str, topic: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM session_turn WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
        rusqlite::params![agent_id, channel, topic],
    )
}

fn pending(conn: &Connection, agent_id: &str) -> Result<Vec<PendingTurn>> {
    let mut stmt = conn.prepare(
        "SELECT id, session, request, state, request_uid FROM session_turn WHERE agent_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map([agent_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut out = vec![];
    for row in rows {
        let (id, session, request, state, request_uid) = row?;
        // A row that no longer parses (a schema change) is skipped, not fatal to startup.
        let (Ok(session), Ok(request)) = (
            serde_json::from_str::<VizierSession>(&session),
            serde_json::from_str::<VizierRequest>(&request),
        ) else {
            tracing::warn!(id, "skipping an unreadable pending turn");
            continue;
        };
        out.push(PendingTurn {
            id,
            session,
            request,
            state: if state == "running" { TurnState::Running } else { TurnState::Queued },
            request_uid,
        });
    }
    Ok(out)
}

#[async_trait::async_trait]
impl TurnStorage for SqliteStorage {
    async fn accept_turn(&self, session: &VizierSession, request: &VizierRequest) -> Result<i64> {
        let (agent_id, channel, topic) = key(session);
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO session_turn (agent_id, channel, topic, session, request, state, accepted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6)",
            rusqlite::params![
                agent_id,
                channel,
                topic,
                serde_json::to_string(session)?,
                serde_json::to_string(request)?,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    async fn start_turn(&self, id: i64) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE session_turn SET state = 'running', started_at = ?2 WHERE id = ?1",
            rusqlite::params![id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    async fn set_turn_request_uid(&self, id: i64, uid: &str) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE session_turn SET request_uid = ?2 WHERE id = ?1",
            rusqlite::params![id, uid],
        )?;
        Ok(())
    }

    async fn note_resume(&self, id: i64, steps: usize) -> Result<u32> {
        let conn = self.conn.lock();
        let (count, at): (i64, i64) = conn.query_row(
            "SELECT resume_count, steps_at_resume FROM session_turn WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let next = if count > 0 && at == steps as i64 { count + 1 } else { 1 };
        conn.execute(
            "UPDATE session_turn SET resume_count = ?2, steps_at_resume = ?3 WHERE id = ?1",
            rusqlite::params![id, next, steps as i64],
        )?;
        Ok(next as u32)
    }

    async fn finish_turn(&self, id: i64) -> Result<()> {
        self.conn.lock().execute("DELETE FROM session_turn WHERE id = ?1", [id])?;
        Ok(())
    }

    async fn cancel_session_turns(&self, session: &VizierSession) -> Result<usize> {
        let (agent_id, channel, topic) = key(session);
        Ok(delete_session(&self.conn.lock(), agent_id, &channel, topic)?)
    }

    async fn pending_turns(&self, agent_id: &str) -> Result<Vec<PendingTurn>> {
        pending(&self.conn.lock(), agent_id)
    }

    async fn sessions_with_pending_turns(&self) -> Result<Vec<VizierSession>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT DISTINCT session FROM session_turn")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = vec![];
        for row in rows {
            if let Ok(session) = serde_json::from_str(&row?) {
                out.push(session);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::{schema::VizierChannelId, storage::document::LocalDocumentStore};

    fn setup() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_turn_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn session(agent: &str, topic: &str) -> VizierSession {
        VizierSession(
            agent.into(),
            VizierChannelId::HTTP("u".into(), "webui".into()),
            Some(topic.into()),
        )
    }

    #[tokio::test]
    async fn a_turn_goes_queued_running_deleted() {
        let (storage, _dir) = setup();
        let s = session("a", "t");
        let id = storage.accept_turn(&s, &VizierRequest::default()).await.unwrap();
        assert_eq!(storage.pending_turns("a").await.unwrap()[0].state, TurnState::Queued);
        storage.start_turn(id).await.unwrap();
        storage.set_turn_request_uid(id, "uid").await.unwrap();
        let rows = storage.pending_turns("a").await.unwrap();
        assert_eq!(rows[0].state, TurnState::Running);
        assert_eq!(rows[0].request_uid.as_deref(), Some("uid"));
        storage.finish_turn(id).await.unwrap();
        assert!(storage.pending_turns("a").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn pending_turns_come_in_acceptance_order() {
        let (storage, _dir) = setup();
        let first = storage.accept_turn(&session("a", "t1"), &VizierRequest::default()).await.unwrap();
        let second = storage.accept_turn(&session("a", "t2"), &VizierRequest::default()).await.unwrap();
        storage.accept_turn(&session("b", "t"), &VizierRequest::default()).await.unwrap();
        let ids: Vec<_> = storage.pending_turns("a").await.unwrap().iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![first, second]);
        assert_eq!(storage.sessions_with_pending_turns().await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn note_resume_counts_only_without_progress() {
        let (storage, _dir) = setup();
        let id = storage.accept_turn(&session("a", "t"), &VizierRequest::default()).await.unwrap();
        assert_eq!(storage.note_resume(id, 2).await.unwrap(), 1);
        assert_eq!(storage.note_resume(id, 2).await.unwrap(), 2);
        assert_eq!(storage.note_resume(id, 2).await.unwrap(), 3);
        assert_eq!(storage.note_resume(id, 5).await.unwrap(), 1, "progress resets the count");
    }

    #[tokio::test]
    async fn cancel_and_session_delete_remove_rows() {
        let (storage, _dir) = setup();
        let s = session("a", "t");
        storage.accept_turn(&s, &VizierRequest::default()).await.unwrap();
        storage.accept_turn(&session("a", "other"), &VizierRequest::default()).await.unwrap();
        assert_eq!(storage.cancel_session_turns(&s).await.unwrap(), 1);
        assert_eq!(storage.pending_turns("a").await.unwrap().len(), 1);
    }
}
