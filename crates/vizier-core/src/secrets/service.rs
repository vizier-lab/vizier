//! The one place validation, actors and storage meet for secrets (`specs/027-agent-secrets`
//! data-model.md §6). Both the agent tools and the HTTP handlers call here, so an invalid name
//! or value is refused identically on both surfaces and nothing is written.

use anyhow::Error;

use crate::{
    schema::{AgentSecretEvent, Requester},
    secrets::{validate_name, validate_value},
    storage::{
        VizierStorage,
        secret::{SecretRecord, SecretStorage, SecretWrite},
    },
};

/// How a secret operation failed. Validation is separated from storage so a caller can map a
/// bad name to a `422` (HTTP) or a `refused` result (tools) and a real failure to a `500` or a
/// raised script error.
#[derive(Debug)]
pub enum SecretError {
    InvalidName(String),
    InvalidValue(String),
    NotFound,
    Storage(Error),
}

impl SecretError {
    /// A message safe to show to a person or return to a script.
    pub fn message(&self) -> String {
        match self {
            Self::InvalidName(message) | Self::InvalidValue(message) => message.clone(),
            Self::NotFound => "secret not found".to_string(),
            Self::Storage(err) => err.to_string(),
        }
    }
}

impl std::fmt::Display for SecretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for SecretError {}

impl From<SecretError> for crate::error::VizierError {
    fn from(err: SecretError) -> Self {
        crate::error::VizierError(err.message())
    }
}

/// Create or replace one value, recording the change.
pub async fn set(
    storage: &VizierStorage,
    agent_id: &str,
    name: &str,
    value: &str,
    actor: &Requester,
) -> Result<SecretWrite, SecretError> {
    validate_name(name).map_err(SecretError::InvalidName)?;
    validate_value(value).map_err(SecretError::InvalidValue)?;
    storage
        .set_agent_secret(agent_id, name, value, actor)
        .await
        .map_err(SecretError::Storage)
}

/// The stored value. An invalid name is refused before storage, so it is never reported as a
/// misleading not-found.
pub async fn get(storage: &VizierStorage, agent_id: &str, name: &str) -> Result<String, SecretError> {
    validate_name(name).map_err(SecretError::InvalidName)?;
    storage
        .get_agent_secret(agent_id, name)
        .await
        .map_err(SecretError::Storage)?
        .ok_or(SecretError::NotFound)
}

/// Delete one value and record the change. A missing name is `NotFound`.
pub async fn delete(
    storage: &VizierStorage,
    agent_id: &str,
    name: &str,
    actor: &Requester,
) -> Result<(), SecretError> {
    validate_name(name).map_err(SecretError::InvalidName)?;
    let deleted = storage
        .delete_agent_secret(agent_id, name, actor)
        .await
        .map_err(SecretError::Storage)?;
    if deleted {
        Ok(())
    } else {
        Err(SecretError::NotFound)
    }
}

/// Metadata for every secret of one agent, sorted by name. Never a value.
pub async fn list(storage: &VizierStorage, agent_id: &str) -> Result<Vec<SecretRecord>, SecretError> {
    storage
        .list_agent_secrets(agent_id)
        .await
        .map_err(SecretError::Storage)
}

/// The change log, newest first. `limit` is clamped to 1–100 so a caller cannot ask for an
/// unbounded read.
pub async fn events(
    storage: &VizierStorage,
    agent_id: &str,
    limit: usize,
) -> Result<Vec<AgentSecretEvent>, SecretError> {
    let limit = limit.clamp(1, 100);
    storage
        .list_agent_secret_events(agent_id, limit)
        .await
        .map_err(SecretError::Storage)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::{
        schema::SecretAction,
        storage::{
            document::LocalDocumentStore,
            sqlite::{SqliteStorage, init_secret_schema},
        },
    };

    fn agent_actor() -> Requester {
        Requester::Agent("dp-a".into())
    }

    fn storage() -> (VizierStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_secret_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (VizierStorage::new(storage), dir)
    }

    #[tokio::test]
    async fn a_set_then_get_reads_through_immediately() {
        let (storage, _dir) = storage();
        let write = set(&storage, "dp-a", "TOKEN", "agent-token-abc123", &agent_actor())
            .await
            .unwrap();
        assert_eq!(write.outcome, crate::storage::secret::SecretOutcome::Created);
        assert_eq!(
            get(&storage, "dp-a", "TOKEN").await.unwrap(),
            "agent-token-abc123"
        );

        // No cache: a second set is visible to the very next get.
        set(&storage, "dp-a", "TOKEN", "rotated-token-xyz789", &agent_actor())
            .await
            .unwrap();
        assert_eq!(
            get(&storage, "dp-a", "TOKEN").await.unwrap(),
            "rotated-token-xyz789"
        );

        // And a delete is visible just as immediately.
        delete(&storage, "dp-a", "TOKEN", &agent_actor()).await.unwrap();
        assert!(matches!(
            get(&storage, "dp-a", "TOKEN").await,
            Err(SecretError::NotFound)
        ));
    }

    #[tokio::test]
    async fn invalid_input_is_refused_before_storage() {
        let (storage, _dir) = storage();
        match set(&storage, "dp-a", "a/b", "agent-token-abc123", &agent_actor()).await {
            Err(SecretError::InvalidName(message)) => assert_eq!(message, crate::secrets::NAME_RULE),
            other => panic!("expected InvalidName, got {other:?}"),
        }
        match set(&storage, "dp-a", "TOKEN", "short", &agent_actor()).await {
            Err(SecretError::InvalidValue(message)) => assert_eq!(message, crate::secrets::VALUE_RULE),
            other => panic!("expected InvalidValue, got {other:?}"),
        }
        assert!(list(&storage, "dp-a").await.unwrap().is_empty());
        assert!(storage.list_agent_secret_events("dp-a", 20).await.unwrap().is_empty());

        // Get and delete validate the name too, rather than reporting a misleading not-found.
        assert!(matches!(
            get(&storage, "dp-a", "a/b").await,
            Err(SecretError::InvalidName(_))
        ));
        assert!(matches!(
            delete(&storage, "dp-a", "a/b", &agent_actor()).await,
            Err(SecretError::InvalidName(_))
        ));
    }

    #[tokio::test]
    async fn a_missing_name_is_not_found() {
        let (storage, _dir) = storage();
        assert!(matches!(
            get(&storage, "dp-a", "MISSING").await,
            Err(SecretError::NotFound)
        ));
        assert!(matches!(
            delete(&storage, "dp-a", "MISSING", &agent_actor()).await,
            Err(SecretError::NotFound)
        ));
    }

    #[tokio::test]
    async fn list_and_events_never_carry_a_value() {
        let (storage, _dir) = storage();
        set(&storage, "dp-a", "TOKEN", "agent-token-abc123", &agent_actor())
            .await
            .unwrap();
        let list = list(&storage, "dp-a").await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].masked, "••••c123");
        let events = events(&storage, "dp-a", 20).await.unwrap();
        assert_eq!(events[0].action, SecretAction::Created);
        let text = format!(
            "{} {}",
            serde_json::to_string(&list.iter().map(|s| s.masked.clone()).collect::<Vec<_>>()).unwrap(),
            serde_json::to_string(&events).unwrap()
        );
        assert!(!text.contains("agent-token-abc123"), "{text}");
    }

    #[tokio::test]
    async fn the_event_limit_is_clamped() {
        let (storage, _dir) = storage();
        for i in 0..3 {
            set(&storage, "dp-a", "T", &format!("agent-token-{i:04}"), &agent_actor())
                .await
                .unwrap();
        }
        // 0 becomes 1, and 1000 becomes 100 (which is more than there are).
        assert_eq!(events(&storage, "dp-a", 0).await.unwrap().len(), 1);
        assert_eq!(events(&storage, "dp-a", 1000).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn a_storage_failure_is_mapped_not_swallowed() {
        // No schema at all: the read fails at the database, not at validation.
        let dir = tempfile::tempdir().unwrap();
        let storage = VizierStorage::new(SqliteStorage::new(
            Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap())),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        ));
        assert!(matches!(
            list(&storage, "dp-a").await,
            Err(SecretError::Storage(_))
        ));
        assert!(matches!(
            get(&storage, "dp-a", "TOKEN").await,
            Err(SecretError::Storage(_))
        ));
    }

    /// The service touches storage only: it never sends an agent command or a transport
    /// message, which is what keeps a replace from restarting an agent or aborting a turn.
    #[tokio::test]
    async fn the_service_never_signals_the_agent() {
        let (storage, _dir) = storage();
        set(&storage, "dp-a", "TOKEN", "agent-token-abc123", &agent_actor())
            .await
            .unwrap();
        let write = set(&storage, "dp-a", "TOKEN", "rotated-token-xyz789", &agent_actor())
            .await
            .unwrap();
        assert_eq!(write.outcome, crate::storage::secret::SecretOutcome::Replaced);
        assert_eq!(
            get(&storage, "dp-a", "TOKEN").await.unwrap(),
            "rotated-token-xyz789"
        );
        // No transport exists in this test at all; the point is that the signature cannot
        // reach one.
    }
}
