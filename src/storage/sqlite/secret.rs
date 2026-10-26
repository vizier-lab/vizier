//! `SecretStorage` over SQLite (`specs/027-agent-secrets/data-model.md` §1–2, §5): a set or
//! delete is one transaction with its event row, `created_at` survives a replace, and the
//! only read that returns a value is `get_agent_secret`.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use uuid::Uuid;

use crate::{
    schema::{AgentSecretEvent, Requester, SecretAction},
    storage::{
        secret::{SecretOutcome, SecretRecord, SecretStorage, SecretWrite},
        sqlite::SqliteStorage,
    },
    utils::mask_secret,
};

/// The stored form of an action, from its serde name so the column and the API agree.
fn action_text(action: SecretAction) -> String {
    serde_json::to_value(action)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn parse_action(text: &str) -> Result<SecretAction> {
    Ok(serde_json::from_value(serde_json::Value::String(text.to_string()))?)
}

fn parse_time(text: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(text)?.with_timezone(&Utc))
}

fn event_row(
    conn: &Connection,
    agent_id: &str,
    name: &str,
    action: SecretAction,
    actor: &Requester,
) -> Result<()> {
    conn.execute(
        "INSERT INTO agent_secret_event (id, agent_id, name, action, actor, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            Uuid::new_v4().to_string(),
            agent_id,
            name,
            action_text(action),
            serde_json::to_string(actor)?,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Delete an agent's secrets and their change log, on a connection the caller already holds.
pub(crate) fn delete_agent_secrets(conn: &Connection, agent_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM agent_secret WHERE agent_id = ?1",
        rusqlite::params![agent_id],
    )?;
    conn.execute(
        "DELETE FROM agent_secret_event WHERE agent_id = ?1",
        rusqlite::params![agent_id],
    )?;
    Ok(())
}

#[async_trait::async_trait]
impl SecretStorage for SqliteStorage {
    async fn set_agent_secret(
        &self,
        agent_id: &str,
        name: &str,
        value: &str,
        actor: &Requester,
    ) -> Result<SecretWrite> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let now = Utc::now().to_rfc3339();

        // A replace keeps when the name was first created; only a set on an absent name starts
        // it. Read and write share the transaction, so the outcome cannot disagree with the row.
        let created_at: Option<String> = tx
            .query_row(
                "SELECT created_at FROM agent_secret WHERE agent_id = ?1 AND name = ?2",
                rusqlite::params![agent_id, name],
                |row| row.get(0),
            )
            .optional()?;
        let outcome = match created_at {
            Some(_) => SecretOutcome::Replaced,
            None => SecretOutcome::Created,
        };
        let created_at = created_at.unwrap_or_else(|| now.clone());
        let actor_json = serde_json::to_string(actor)?;
        let stored = crate::encryption::fields::protect_agent_secret(
            &self.cipher,
            agent_id,
            name,
            value,
            crate::encryption::ProtectMode::Write,
        )?
        .0;

        tx.execute(
            "INSERT INTO agent_secret (agent_id, name, value, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(agent_id, name) DO UPDATE SET
                 value = excluded.value,
                 updated_at = excluded.updated_at,
                 updated_by = excluded.updated_by",
            rusqlite::params![agent_id, name, stored, created_at, now, actor_json],
        )?;
        event_row(
            &tx,
            agent_id,
            name,
            match outcome {
                SecretOutcome::Created => SecretAction::Created,
                SecretOutcome::Replaced => SecretAction::Replaced,
            },
            actor,
        )?;
        tx.commit()?;

        Ok(SecretWrite {
            outcome,
            record: SecretRecord {
                name: name.to_string(),
                masked: mask_secret(value),
                created_at: parse_time(&created_at)?,
                updated_at: parse_time(&now)?,
                updated_by: actor.clone(),
            },
        })
    }

    async fn get_agent_secret(&self, agent_id: &str, name: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let stored: Option<String> = conn
            .query_row(
                "SELECT value FROM agent_secret WHERE agent_id = ?1 AND name = ?2",
                rusqlite::params![agent_id, name],
                |row| row.get(0),
            )
            .optional()?;
        stored
            .map(|stored| {
                crate::encryption::fields::decrypt_agent_secret(
                    &self.cipher,
                    agent_id,
                    name,
                    &stored,
                )
            })
            .transpose()
    }

    async fn list_agent_secrets(&self, agent_id: &str) -> Result<Vec<SecretRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT name, value, created_at, updated_at, updated_by
             FROM agent_secret WHERE agent_id = ?1 ORDER BY name",
        )?;
        let rows: Vec<(String, String, String, String, String)> = stmt
            .query_map(rusqlite::params![agent_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
            })?
            .collect::<rusqlite::Result<_>>()?;

        rows.into_iter()
            .map(|(name, value, created_at, updated_at, updated_by)| {
                let value = crate::encryption::fields::decrypt_agent_secret(
                    &self.cipher,
                    agent_id,
                    &name,
                    &value,
                )?;
                Ok(SecretRecord {
                    name,
                    masked: mask_secret(&value),
                    created_at: parse_time(&created_at)?,
                    updated_at: parse_time(&updated_at)?,
                    updated_by: serde_json::from_str(&updated_by)?,
                })
            })
            .collect()
    }

    async fn delete_agent_secret(
        &self,
        agent_id: &str,
        name: &str,
        actor: &Requester,
    ) -> Result<bool> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let deleted = tx.execute(
            "DELETE FROM agent_secret WHERE agent_id = ?1 AND name = ?2",
            rusqlite::params![agent_id, name],
        )?;
        if deleted > 0 {
            event_row(&tx, agent_id, name, SecretAction::Deleted, actor)?;
        }
        tx.commit()?;
        Ok(deleted > 0)
    }

    async fn list_agent_secret_events(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<AgentSecretEvent>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, name, action, actor, at
             FROM agent_secret_event WHERE agent_id = ?1
             ORDER BY at DESC, rowid DESC LIMIT ?2",
        )?;
        let rows: Vec<(String, String, String, String, String, String)> = stmt
            .query_map(rusqlite::params![agent_id, limit as i64], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;

        rows.into_iter()
            .map(|(id, agent_id, name, action, actor, at)| {
                Ok(AgentSecretEvent {
                    id,
                    agent_id,
                    name,
                    action: parse_action(&action)?,
                    actor: serde_json::from_str(&actor)?,
                    at: parse_time(&at)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::storage::{document::LocalDocumentStore, sqlite::init_secret_schema};

    fn setup() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        init_secret_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn agent() -> Requester {
        Requester::Agent("dp-a".into())
    }

    fn alice() -> Requester {
        Requester::User("user-1".into())
    }

    async fn names(storage: &SqliteStorage, agent_id: &str) -> Vec<String> {
        storage
            .list_agent_secrets(agent_id)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.name)
            .collect()
    }

    async fn actions(storage: &SqliteStorage, agent_id: &str) -> Vec<SecretAction> {
        storage
            .list_agent_secret_events(agent_id, 100)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.action)
            .collect()
    }

    #[tokio::test]
    async fn a_set_creates_one_row_and_reads_it_back() {
        let (storage, _dir) = setup();
        let write = storage
            .set_agent_secret("dp-a", "TOKEN", "agent-token-abc123", &agent())
            .await
            .unwrap();
        assert_eq!(write.outcome, SecretOutcome::Created);
        assert_eq!(write.record.name, "TOKEN");
        assert_eq!(write.record.masked, "••••c123");
        assert_eq!(write.record.updated_by, agent());

        assert_eq!(
            storage.get_agent_secret("dp-a", "TOKEN").await.unwrap().as_deref(),
            Some("agent-token-abc123")
        );
        assert_eq!(storage.get_agent_secret("dp-a", "OTHER").await.unwrap(), None);
        assert_eq!(names(&storage, "dp-a").await, ["TOKEN"]);
        assert_eq!(actions(&storage, "dp-a").await, [SecretAction::Created]);
    }

    /// A second set replaces: one row, the original `created_at`, the new value, a new actor.
    #[tokio::test]
    async fn a_replace_keeps_created_at_and_leaves_one_row() {
        let (storage, _dir) = setup();
        let first = storage
            .set_agent_secret("dp-a", "TOKEN", "first-value-1", &agent())
            .await
            .unwrap();
        let second = storage
            .set_agent_secret("dp-a", "TOKEN", "second-value-2", &alice())
            .await
            .unwrap();

        assert_eq!(second.outcome, SecretOutcome::Replaced);
        assert_eq!(second.record.created_at, first.record.created_at);
        assert_eq!(second.record.updated_by, alice());
        assert!(second.record.updated_at >= first.record.updated_at);
        assert_eq!(names(&storage, "dp-a").await, ["TOKEN"]);
        assert_eq!(
            storage.get_agent_secret("dp-a", "TOKEN").await.unwrap().as_deref(),
            Some("second-value-2")
        );
        assert_eq!(
            actions(&storage, "dp-a").await,
            [SecretAction::Replaced, SecretAction::Created]
        );
    }

    /// A value is returned byte for byte, whitespace and line breaks included.
    #[tokio::test]
    async fn a_value_round_trips_exactly() {
        let (storage, _dir) = setup();
        let value = "  line one\nline two\r\n  trailing  ";
        storage.set_agent_secret("dp-a", "MULTI", value, &agent()).await.unwrap();
        assert_eq!(
            storage.get_agent_secret("dp-a", "MULTI").await.unwrap().as_deref(),
            Some(value)
        );
    }

    #[tokio::test]
    async fn list_masks_and_sorts_by_name() {
        let (storage, _dir) = setup();
        for (name, value) in [("zeta", "value-zeta-1"), ("Alpha", "value-alpha"), ("alpha", "value-alph2")] {
            storage.set_agent_secret("dp-a", name, value, &agent()).await.unwrap();
        }
        let list = storage.list_agent_secrets("dp-a").await.unwrap();
        let names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
        // Case-sensitive, and SQLite's default binary collation keeps capitals first.
        assert_eq!(names, ["Alpha", "alpha", "zeta"]);
        // tx-zeta-1's last four characters.
        assert_eq!(list[2].masked, "••••ta-1");
        // No record carries the value anywhere.
        let json = serde_json::to_string(&list.iter().map(|s| s.masked.clone()).collect::<Vec<_>>()).unwrap();
        assert!(!json.contains("value-zeta-1"));
    }

    #[tokio::test]
    async fn a_delete_removes_the_row_and_records_it_once() {
        let (storage, _dir) = setup();
        storage.set_agent_secret("dp-a", "TOKEN", "agent-token-abc123", &agent()).await.unwrap();

        assert!(storage.delete_agent_secret("dp-a", "TOKEN", &alice()).await.unwrap());
        assert_eq!(storage.get_agent_secret("dp-a", "TOKEN").await.unwrap(), None);
        assert!(names(&storage, "dp-a").await.is_empty());
        assert_eq!(
            actions(&storage, "dp-a").await,
            [SecretAction::Deleted, SecretAction::Created]
        );

        // A delete of a name that does not exist changes nothing and records nothing.
        assert!(!storage.delete_agent_secret("dp-a", "TOKEN", &alice()).await.unwrap());
        assert_eq!(actions(&storage, "dp-a").await.len(), 2);
    }

    /// Deleting and setting again starts a fresh secret, with a fresh `created_at`.
    #[tokio::test]
    async fn a_name_set_after_a_delete_is_created_again() {
        let (storage, _dir) = setup();
        let first = storage.set_agent_secret("dp-a", "T", "first-value-1", &agent()).await.unwrap();
        storage.delete_agent_secret("dp-a", "T", &agent()).await.unwrap();
        let again = storage.set_agent_secret("dp-a", "T", "second-value-2", &agent()).await.unwrap();
        assert_eq!(again.outcome, SecretOutcome::Created);
        assert!(again.record.created_at >= first.record.created_at);
        assert_eq!(
            storage.get_agent_secret("dp-a", "T").await.unwrap().as_deref(),
            Some("second-value-2")
        );
    }

    /// The same name on two agents is two independent secrets.
    #[tokio::test]
    async fn agents_never_see_each_others_secrets() {
        let (storage, _dir) = setup();
        storage.set_agent_secret("dp-a", "TOKEN", "agent-a-token-1", &agent()).await.unwrap();
        storage.set_agent_secret("dp-b", "TOKEN", "agent-b-token-2", &agent()).await.unwrap();

        assert_eq!(
            storage.get_agent_secret("dp-a", "TOKEN").await.unwrap().as_deref(),
            Some("agent-a-token-1")
        );
        assert_eq!(
            storage.get_agent_secret("dp-b", "TOKEN").await.unwrap().as_deref(),
            Some("agent-b-token-2")
        );
        assert_eq!(
            storage.get_agent_secret("dp-a", "B_ONLY").await.unwrap(),
            None
        );

        assert!(storage.delete_agent_secret("dp-a", "TOKEN", &agent()).await.unwrap());
        assert_eq!(
            storage.get_agent_secret("dp-b", "TOKEN").await.unwrap().as_deref(),
            Some("agent-b-token-2")
        );
    }

    /// Events store the actor and action, never a value; the newest comes first and the limit
    /// is honoured.
    #[tokio::test]
    async fn events_are_newest_first_and_hold_no_value() {
        let (storage, _dir) = setup();
        storage.set_agent_secret("dp-a", "TOKEN", "sensitive-value-1", &agent()).await.unwrap();
        storage.set_agent_secret("dp-a", "TOKEN", "sensitive-value-2", &alice()).await.unwrap();
        storage.delete_agent_secret("dp-a", "TOKEN", &alice()).await.unwrap();

        let events = storage.list_agent_secret_events("dp-a", 20).await.unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].action, SecretAction::Deleted);
        assert_eq!(events[1].action, SecretAction::Replaced);
        assert_eq!(events[2].action, SecretAction::Created);
        assert_eq!(events[2].actor, agent());
        assert_eq!(events[0].actor, alice());
        assert_eq!(events[0].name, "TOKEN");
        for event in &events {
            let json = serde_json::to_string(event).unwrap();
            assert!(!json.contains("sensitive-value"), "{json}");
        }

        assert_eq!(storage.list_agent_secret_events("dp-a", 1).await.unwrap().len(), 1);
        assert!(storage.list_agent_secret_events("dp-b", 20).await.unwrap().is_empty());
    }

    /// Deleting an agent removes both tables' rows for it and nothing for anyone else.
    #[tokio::test]
    async fn deleting_an_agent_removes_its_secrets_and_events() {
        let (storage, _dir) = setup();
        storage.set_agent_secret("dp-a", "TOKEN", "agent-a-token-1", &agent()).await.unwrap();
        storage.set_agent_secret("dp-b", "TOKEN", "agent-b-token-2", &agent()).await.unwrap();

        {
            let conn = storage.conn.lock();
            delete_agent_secrets(&conn, "dp-a").unwrap();
            let secrets: i64 = conn
                .query_row("SELECT count(*) FROM agent_secret WHERE agent_id = 'dp-a'", [], |r| r.get(0))
                .unwrap();
            let events: i64 = conn
                .query_row("SELECT count(*) FROM agent_secret_event WHERE agent_id = 'dp-a'", [], |r| r.get(0))
                .unwrap();
            assert_eq!((secrets, events), (0, 0));
        }
        assert!(!storage.list_agent_secret_events("dp-b", 20).await.unwrap().is_empty());
        assert_eq!(
            storage.get_agent_secret("dp-b", "TOKEN").await.unwrap().as_deref(),
            Some("agent-b-token-2")
        );
    }

    /// With the production cipher the stored column is an envelope, a value round-trips, and a
    /// tampered row is an error rather than a mask or a partial value.
    #[tokio::test]
    async fn values_are_envelopes_at_rest_and_tampering_errors() {
        use crate::encryption::storage_tests::{file_backed, raw_value};

        let h = file_backed();
        let storage = &h.storage;
        storage
            .set_agent_secret("dp-a", "TOKEN", "agent-token-abc123", &agent())
            .await
            .unwrap();

        let raw = raw_value(
            storage,
            "SELECT value FROM agent_secret WHERE agent_id = 'dp-a' AND name = 'TOKEN'",
            &[],
        );
        assert!(raw.starts_with("enc:v1:"), "{raw}");
        assert!(!raw.contains("agent-token-abc123"));
        assert_eq!(
            storage.get_agent_secret("dp-a", "TOKEN").await.unwrap().as_deref(),
            Some("agent-token-abc123")
        );

        storage
            .conn
            .lock()
            .execute("UPDATE agent_secret SET value = value || 'x'", [])
            .unwrap();
        let err = storage.get_agent_secret("dp-a", "TOKEN").await.unwrap_err();
        assert!(format!("{err}").contains("integrity check"), "{err}");
        assert!(storage.list_agent_secrets("dp-a").await.is_err());
    }
}
