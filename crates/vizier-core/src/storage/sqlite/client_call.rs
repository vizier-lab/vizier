use anyhow::Result;
use crate::storage::fence::Scope;
use crate::storage::sqlite::fence::check_fence;


use crate::storage::sql::{self, params};

use crate::{
    schema::{ClientCallOutcome, VizierSession},
    storage::{client_call::ClientCallStorage, sqlite::SqlStorage},
};

impl SqlStorage {
    /// Refuse a write of client call `uid` when a newer owner holds its agent's scope. The agent
    /// is read from the row, in the writer's transaction; a missing row has nothing to protect.
    fn check_call_fence(&self, tx: &sql::Connection, uid: &str) -> Result<()> {
        let agent = tx
            .query_all(
                "SELECT agent_id FROM client_tool_call WHERE history_uid = ?1",
                params![uid],
            )?
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
impl ClientCallStorage for SqlStorage {
    async fn open_client_call(&self, uid: &str, session: &VizierSession) -> Result<()> {
        let scope = Scope::Agent(session.0.clone());
        let (owner_node, owner_token) = self.fences.owner_of(&scope);
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        tx.execute(
            "INSERT INTO client_tool_call
                (history_uid, agent_id, channel, topic, status, created_at, owner_node, owner_token)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?6, ?7)
             ON CONFLICT DO NOTHING",
            params![
                uid,
                session.0,
                session.1.to_slug(),
                session.2,
                crate::time::stored_now(),
                owner_node,
                owner_token,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn finish_client_call(&self, uid: &str, outcome: &ClientCallOutcome) -> Result<bool> {
        let detail = outcome
            .detail()
            .map(|detail| serde_json::to_string(&detail))
            .transpose()?;
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        self.check_call_fence(&tx, uid)?;
        // Only an open call changes, which is what makes an outcome final: a late write
        // changes nothing.
        let changed = tx.execute(
            "UPDATE client_tool_call SET status = ?1, detail = ?2, finished_at = ?3
             WHERE history_uid = ?4 AND status IN ('pending', 'deferred')",
            params![outcome.status(), detail, crate::time::stored_now(), uid],
        )?;
        tx.commit()?;
        Ok(changed > 0)
    }

    async fn save_client_call_state(&self, uid: &str, state: &serde_json::Value) -> Result<()> {
        let state = serde_json::to_string(state)?;
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        self.check_call_fence(&tx, uid)?;
        tx.execute(
            "UPDATE client_tool_call SET state = ?1, state_updated_at = ?2 WHERE history_uid = ?3",
            params![state, crate::time::stored_now(), uid],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn interrupt_pending_client_calls(&self) -> Result<usize> {
        let conn = self.conn.lock()?;
        Ok(conn.execute(
            "UPDATE client_tool_call SET status = 'interrupted', finished_at = ?1
             WHERE status = 'pending'",
            params![crate::time::stored_now()],
        )?)
    }

    async fn interrupt_expired_client_calls(&self, agent_id: &str, below: u64) -> Result<usize> {
        let scope = Scope::Agent(agent_id.to_owned());
        let (node, token) = self.fences.owner_of(&scope);
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        let changed = tx.execute(
            "UPDATE client_tool_call SET status = 'interrupted', finished_at = ?1, \
             owner_node = ?2, owner_token = ?3 \
             WHERE status = 'pending' AND agent_id = ?4 \
             AND (owner_token IS NULL OR owner_token < ?5)",
            params![crate::time::stored_now(), node, token, agent_id, below as i64],
        )?;
        tx.commit()?;
        Ok(changed)
    }

    async fn defer_client_call(&self, uid: &str) -> Result<bool> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        self.check_call_fence(&tx, uid)?;
        let changed = tx.execute(
            "UPDATE client_tool_call SET status = 'deferred' WHERE history_uid = ?1 AND status = 'pending'",
            [uid],
        )?;
        tx.commit()?;
        Ok(changed > 0)
    }

    async fn claim_client_call(
        &self,
        uid: &str,
        claimant: &str,
        timeout: chrono::Duration,
    ) -> Result<bool> {
        let now = crate::time::now();
        let conn = self.conn.lock()?;
        // One conditional update: exactly one gateway's statement changes the row, so
        // exactly one of them delivers the call. A claim older than the timeout has
        // lapsed - the gateway that held it died - and the call is free again.
        let changed = conn.execute(
            "UPDATE client_tool_call SET claimed_by = ?1, claimed_at = ?2
             WHERE history_uid = ?3 AND status = 'deferred'
               AND (claimed_by IS NULL OR claimed_at < ?4)",
            params![
                claimant,
                crate::time::stored(now),
                uid,
                crate::time::stored(now - timeout),
            ],
        )?;
        Ok(changed > 0)
    }

    async fn release_client_call_claim(&self, uid: &str, claimant: &str) -> Result<bool> {
        let conn = self.conn.lock()?;
        // Only the holder gives the claim back, and only while the call is still waiting.
        let changed = conn.execute(
            "UPDATE client_tool_call SET claimed_by = NULL, claimed_at = NULL
             WHERE history_uid = ?1 AND status = 'deferred' AND claimed_by = ?2",
            params![uid, claimant],
        )?;
        Ok(changed > 0)
    }

    async fn deferred_client_calls(&self, session: &VizierSession) -> Result<Vec<String>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT history_uid FROM client_tool_call
             WHERE agent_id = ?1 AND channel = ?2 AND topic IS NOT DISTINCT FROM ?3 AND status = 'deferred'
             ORDER BY created_at, rowid",
        )?;
        let uids = stmt
            .query_map(params![session.0, session.1.to_slug(), session.2], |row| {
                row.get(0)
            })?
            .collect::<sql::Result<Vec<String>>>()?;
        Ok(uids)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::storage::sql::SqlStore;
    use rusqlite::Connection;
    use serde_json::json;

    use super::*;
    use crate::{schema::VizierChannelId, storage::document::BlobDocumentStore};

    fn setup() -> (SqlStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::tests_support::test_store(|| {
            let conn = Connection::open_in_memory().unwrap();
            crate::storage::sqlite::init_client_call_schema(&conn).unwrap();
            SqlStore::sqlite(conn)
        });
        let storage = SqlStorage::new(
            store,
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
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

    fn row(storage: &SqlStorage, uid: &str) -> (String, Option<String>, Option<String>) {
        storage
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT status, detail, state FROM client_tool_call WHERE history_uid = ?1",
                [uid],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    }

    /// The call's claim: who holds it, and since when.
    fn claim(storage: &SqlStorage, uid: &str) -> (Option<String>, Option<String>) {
        storage
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT claimed_by, claimed_at FROM client_tool_call WHERE history_uid = ?1",
                [uid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    }

    #[tokio::test]
    async fn an_outcome_is_recorded_once() {
        let (storage, _dir) = setup();
        storage.open_client_call("c1", &session()).await.unwrap();
        assert_eq!(row(&storage, "c1").0, "pending");

        let ok = ClientCallOutcome::Ok {
            result: json!({ "hex": "#3366ff" }),
        };
        assert!(storage.finish_client_call("c1", &ok).await.unwrap());
        assert_eq!(
            row(&storage, "c1"),
            ("ok".into(), Some(r##"{"hex":"#3366ff"}"##.into()), None)
        );

        // A late timeout, say, changes nothing.
        assert!(
            !storage
                .finish_client_call("c1", &ClientCallOutcome::Timeout)
                .await
                .unwrap()
        );
        assert_eq!(row(&storage, "c1").0, "ok");
    }

    #[tokio::test]
    async fn state_saves_in_any_status() {
        let (storage, _dir) = setup();
        storage.open_client_call("c1", &session()).await.unwrap();
        storage
            .save_client_call_state("c1", &json!({ "n": 1 }))
            .await
            .unwrap();
        assert_eq!(row(&storage, "c1").2.as_deref(), Some(r#"{"n":1}"#));

        storage
            .finish_client_call("c1", &ClientCallOutcome::Cancelled)
            .await
            .unwrap();
        storage
            .save_client_call_state("c1", &json!({ "n": 2 }))
            .await
            .unwrap();
        assert_eq!(
            row(&storage, "c1"),
            ("cancelled".into(), None, Some(r#"{"n":2}"#.into()))
        );
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
        assert_eq!(
            storage.deferred_client_calls(&session()).await.unwrap(),
            ["a", "b"]
        );

        assert_eq!(storage.interrupt_pending_client_calls().await.unwrap(), 0);
        assert_eq!(row(&storage, "a").0, "deferred");

        let ok = ClientCallOutcome::Ok { result: json!(1) };
        assert!(storage.finish_client_call("a", &ok).await.unwrap());
        assert!(
            !storage
                .finish_client_call("a", &ClientCallOutcome::Timeout)
                .await
                .unwrap()
        );
        assert_eq!(
            storage.deferred_client_calls(&session()).await.unwrap(),
            ["b"]
        );
        let other = VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("bob".into(), "webui".into()),
            Some("t".into()),
        );
        assert!(
            storage
                .deferred_client_calls(&other)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn the_sweep_only_interrupts_pending_calls() {
        let (storage, _dir) = setup();
        storage.open_client_call("a", &session()).await.unwrap();
        storage.open_client_call("b", &session()).await.unwrap();
        storage
            .finish_client_call(
                "b",
                &ClientCallOutcome::Error {
                    message: "no".into(),
                },
            )
            .await
            .unwrap();

        assert_eq!(storage.interrupt_pending_client_calls().await.unwrap(), 1);
        assert_eq!(row(&storage, "a").0, "interrupted");
        assert_eq!(
            row(&storage, "b"),
            ("error".into(), Some(r#""no""#.into()), None)
        );
    }

    #[tokio::test]
    async fn two_claims_on_one_deferred_call_change_one_row() {
        let (storage, _dir) = setup();
        storage.open_client_call("c1", &session()).await.unwrap();
        assert!(storage.defer_client_call("c1").await.unwrap());

        // Two gateways claim the same call: exactly one of them changes the row.
        let first = storage
            .claim_client_call("c1", "gateway-1", chrono::Duration::minutes(1))
            .await
            .unwrap();
        let second = storage
            .claim_client_call("c1", "gateway-2", chrono::Duration::minutes(1))
            .await
            .unwrap();
        assert_ne!(first, second, "exactly one of the two claims wins");
        let (claimed_by, _) = claim(&storage, "c1");
        assert_eq!(
            claimed_by.as_deref(),
            Some(if first { "gateway-1" } else { "gateway-2" })
        );
        // The claim is a lease on the row, not a status: the call is still deferred.
        assert_eq!(row(&storage, "c1").0, "deferred");
    }

    #[tokio::test]
    async fn a_lapsed_claim_is_free_again() {
        let (storage, _dir) = setup();
        storage.open_client_call("c1", &session()).await.unwrap();
        assert!(storage.defer_client_call("c1").await.unwrap());

        assert!(
            storage
                .claim_client_call("c1", "gateway-1", chrono::Duration::minutes(1))
                .await
                .unwrap()
        );
        // The claim is live: a second gateway does not get the call.
        assert!(
            !storage
                .claim_client_call("c1", "gateway-2", chrono::Duration::minutes(1))
                .await
                .unwrap()
        );

        // The claim has lapsed: the gateway that took it is gone.
        storage
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE client_tool_call SET claimed_at = ?1 WHERE history_uid = ?2",
                params!["2000-01-01T00:00:00.000000000+00:00", "c1"],
            )
            .unwrap();
        assert!(
            storage
                .claim_client_call("c1", "gateway-2", chrono::Duration::minutes(1))
                .await
                .unwrap()
        );
        let (claimed_by, _) = claim(&storage, "c1");
        assert_eq!(claimed_by.as_deref(), Some("gateway-2"));
        // A lapsed claim leaves the call deferred, for the next gateway.
        assert_eq!(row(&storage, "c1").0, "deferred");
    }

    crate::postgres_tests!(
        an_outcome_is_recorded_once,
        state_saves_in_any_status,
        a_deferred_call_survives_the_sweep_and_finishes_once,
        the_sweep_only_interrupts_pending_calls,
        two_claims_on_one_deferred_call_change_one_row,
        a_lapsed_claim_is_free_again,
    );
}
