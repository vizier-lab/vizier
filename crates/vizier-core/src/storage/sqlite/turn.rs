use crate::storage::sql::{self, Connection, params};
use anyhow::Result;
use crate::storage::fence::Scope;
use crate::storage::sqlite::fence::check_fence;


use crate::{
    schema::{VizierRequest, VizierSession},
    storage::{
        sqlite::SqlStorage,
        turn::{PendingTurn, TurnState, TurnStorage},
    },
};

fn key(session: &VizierSession) -> (&str, String, &str) {
    (
        &session.0,
        session.1.to_slug(),
        session.2.as_deref().unwrap_or(""),
    )
}

/// Delete a session's rows, on the connection already held (`delete_session`).
pub fn delete_session(
    conn: &Connection,
    agent_id: &str,
    channel: &str,
    topic: &str,
) -> sql::Result<usize> {
    conn.execute(
        "DELETE FROM session_turn WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
        params![agent_id, channel, topic],
    )
}

fn pending(conn: &Connection, agent_id: &str, below: Option<u64>) -> Result<Vec<PendingTurn>> {
    // `below` keeps only the rows whose owner is older than it (or null): the expired ones.
    let mut stmt = conn.prepare(
        "SELECT id, session, request, state, request_uid FROM session_turn \
         WHERE agent_id = ?1 AND (CAST(?2 AS BIGINT) IS NULL OR owner_token IS NULL OR owner_token < CAST(?2 AS BIGINT)) ORDER BY id",
    )?;
    let rows = stmt.query_map(params![agent_id, below.map(|t| t as i64)], |row| {
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
            state: if state == "running" {
                TurnState::Running
            } else {
                TurnState::Queued
            },
            request_uid,
        });
    }
    Ok(out)
}

impl SqlStorage {
    /// Refuse a write of turn `id` when a newer owner holds its agent's scope. The agent is read
    /// from the row, in the writer's transaction; a missing row has nothing to protect.
    fn check_turn_fence(&self, tx: &Connection, id: i64) -> Result<()> {
        let agent = tx
            .query_all("SELECT agent_id FROM session_turn WHERE id = ?1", params![id])?
            .first()
            .map(|row| row.get::<_, String>(0))
            .transpose()?;
        match agent {
            Some(agent) => check_fence(tx, &self.fences, &Scope::Agent(agent)),
            None => Ok(()),
        }
    }
}

#[async_trait::async_trait]
impl TurnStorage for SqlStorage {
    async fn accept_turn(&self, session: &VizierSession, request: &VizierRequest) -> Result<i64> {
        let (agent_id, channel, topic) = key(session);
        let conn = self.conn.lock()?;
        let id = conn.query_row(
            "INSERT INTO session_turn (agent_id, channel, topic, session, request, state, accepted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6) RETURNING id",
            params![
                agent_id,
                channel,
                topic,
                serde_json::to_string(session)?,
                serde_json::to_string(request)?,
                crate::time::stored_now()
            ],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    async fn start_turn(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        self.check_turn_fence(&tx, id)?;
        tx.execute(
            "UPDATE session_turn SET state = 'running', started_at = ?2 WHERE id = ?1",
            params![id, crate::time::stored_now()],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn set_turn_request_uid(&self, id: i64, uid: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        self.check_turn_fence(&tx, id)?;
        tx.execute(
            "UPDATE session_turn SET request_uid = ?2 WHERE id = ?1",
            params![id, uid],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn note_resume(&self, id: i64, steps: usize) -> Result<u32> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        self.check_turn_fence(&tx, id)?;
        let (count, at): (i64, i64) = tx.query_row(
            "SELECT resume_count, steps_at_resume FROM session_turn WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let next = if count > 0 && at == steps as i64 {
            count + 1
        } else {
            1
        };
        tx.execute(
            "UPDATE session_turn SET resume_count = ?2, steps_at_resume = ?3 WHERE id = ?1",
            params![id, next, steps as i64],
        )?;
        tx.commit()?;
        Ok(next as u32)
    }

    async fn finish_turn(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        self.check_turn_fence(&tx, id)?;
        tx.execute("DELETE FROM session_turn WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    async fn cancel_session_turns(&self, session: &VizierSession) -> Result<usize> {
        let (agent_id, channel, topic) = key(session);
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Agent(agent_id.to_owned()))?;
        let removed = delete_session(&tx, agent_id, &channel, topic)?;
        tx.commit()?;
        Ok(removed)
    }

    async fn pending_turns(&self, agent_id: &str) -> Result<Vec<PendingTurn>> {
        pending(&self.conn.lock()?, agent_id, None)
    }

    async fn expired_turns(&self, agent_id: &str, below: u64) -> Result<Vec<PendingTurn>> {
        pending(&self.conn.lock()?, agent_id, Some(below))
    }

    async fn sessions_with_pending_turns_of(&self, agent_id: &str) -> Result<Vec<VizierSession>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT DISTINCT session FROM session_turn WHERE agent_id = ?1")?;
        let rows = stmt.query_map([agent_id], |row| row.get::<_, String>(0))?;
        let mut out = vec![];
        for row in rows {
            if let Ok(session) = serde_json::from_str(&row?) {
                out.push(session);
            }
        }
        Ok(out)
    }

    async fn claim_turn_on_receipt(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        let agent = tx
            .query_all("SELECT agent_id FROM session_turn WHERE id = ?1", params![id])?
            .first()
            .map(|row| row.get::<_, String>(0))
            .transpose()?;
        let Some(agent) = agent else {
            // Finished or cancelled in the meantime: nothing to take, nothing to run.
            return Ok(false);
        };
        let scope = Scope::Agent(agent);
        check_fence(&tx, &self.fences, &scope)?;
        let (node, token) = self.fences.owner_of(&scope);
        let Some(token) = token else {
            // No fence held (a single process before any claim, or a test): every delivery runs.
            return Ok(true);
        };
        let taken = tx.execute(
            "UPDATE session_turn SET owner_node = ?2, owner_token = ?3 \
             WHERE id = ?1 AND (owner_token IS NULL OR owner_token < ?3)",
            params![id, node, token],
        )?;
        tx.commit()?;
        Ok(taken > 0)
    }

    async fn sessions_with_pending_turns(&self) -> Result<Vec<VizierSession>> {
        let conn = self.conn.lock()?;
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

    use crate::storage::sql::SqlStore;
    use rusqlite::Connection;

    use super::*;
    use crate::{schema::VizierChannelId, storage::document::BlobDocumentStore};

    fn setup() -> (SqlStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::tests_support::test_store(|| {
            let conn = Connection::open_in_memory().unwrap();
            crate::storage::sqlite::init_turn_schema(&conn).unwrap();
            SqlStore::sqlite(conn)
        });
        let storage = SqlStorage::new(
            store,
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
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
        let id = storage
            .accept_turn(&s, &VizierRequest::default())
            .await
            .unwrap();
        assert_eq!(
            storage.pending_turns("a").await.unwrap()[0].state,
            TurnState::Queued
        );
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
        let first = storage
            .accept_turn(&session("a", "t1"), &VizierRequest::default())
            .await
            .unwrap();
        let second = storage
            .accept_turn(&session("a", "t2"), &VizierRequest::default())
            .await
            .unwrap();
        storage
            .accept_turn(&session("b", "t"), &VizierRequest::default())
            .await
            .unwrap();
        let ids: Vec<_> = storage
            .pending_turns("a")
            .await
            .unwrap()
            .iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(ids, vec![first, second]);
        assert_eq!(
            storage.sessions_with_pending_turns().await.unwrap().len(),
            3
        );
    }

    #[tokio::test]
    async fn note_resume_counts_only_without_progress() {
        let (storage, _dir) = setup();
        let id = storage
            .accept_turn(&session("a", "t"), &VizierRequest::default())
            .await
            .unwrap();
        assert_eq!(storage.note_resume(id, 2).await.unwrap(), 1);
        assert_eq!(storage.note_resume(id, 2).await.unwrap(), 2);
        assert_eq!(storage.note_resume(id, 2).await.unwrap(), 3);
        assert_eq!(
            storage.note_resume(id, 5).await.unwrap(),
            1,
            "progress resets the count"
        );
    }

    #[tokio::test]
    async fn cancel_and_session_delete_remove_rows() {
        let (storage, _dir) = setup();
        let s = session("a", "t");
        storage
            .accept_turn(&s, &VizierRequest::default())
            .await
            .unwrap();
        storage
            .accept_turn(&session("a", "other"), &VizierRequest::default())
            .await
            .unwrap();
        assert_eq!(storage.cancel_session_turns(&s).await.unwrap(), 1);
        assert_eq!(storage.pending_turns("a").await.unwrap().len(), 1);
    }

    crate::postgres_tests!(
        a_turn_goes_queued_running_deleted,
        pending_turns_come_in_acceptance_order,
        note_resume_counts_only_without_progress,
        cancel_and_session_delete_remove_rows,
    );
}
