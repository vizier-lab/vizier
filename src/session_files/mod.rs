//! The transient session-file tier's one service (`specs/035-add-artifacts` R8).
//!
//! Clear, conversation deletion and agent deletion all share these rules: refuse while a turn is
//! active, delete records before blobs, and remove an upload directory only when no other
//! session-file record references its `file_id`. Artifacts live elsewhere and are never touched.
//! There is deliberately no timer, retention setting or automatic collection.

use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::Result;

use crate::file_manager::FileManager;
use crate::schema::{SessionFileRecord, VizierSession};
use crate::storage::VizierStorage;
use crate::storage::session::SessionStorage;
use crate::storage::session_file::SessionFileStorage;
use crate::storage::turn::TurnStorage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct ClearResult {
    pub removed: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClearError {
    AgentNotFound,
    TurnActive,
    Storage(String),
}

impl std::fmt::Display for ClearError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AgentNotFound => write!(f, "agent not found"),
            Self::TurnActive => write!(
                f,
                "a turn is running or queued in this conversation; wait for it to finish"
            ),
            Self::Storage(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ClearError {}

impl From<anyhow::Error> for ClearError {
    fn from(value: anyhow::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

#[derive(Clone)]
pub struct SessionFiles {
    storage: Arc<VizierStorage>,
    file_manager: FileManager,
}

impl SessionFiles {
    pub fn new(storage: Arc<VizierStorage>, file_manager: FileManager) -> Self {
        Self {
            storage,
            file_manager,
        }
    }

    pub fn storage(&self) -> &Arc<VizierStorage> {
        &self.storage
    }

    /// How many session files the conversation has.
    pub async fn count(&self, session: &VizierSession) -> Result<usize> {
        Ok(self.storage.list_session_files(session).await?.len())
    }

    pub async fn files_for_agent(&self, agent_id: &str) -> Result<Vec<SessionFileRecord>> {
        self.storage.list_agent_files(agent_id).await
    }

    /// Whether the conversation has a running or queued turn. A storage failure is treated as
    /// "not active" for the read-only count path and logged.
    pub async fn has_active_turn(&self, session: &VizierSession) -> bool {
        match self.storage.pending_turns(&session.0).await {
            Ok(turns) => turns.iter().any(|turn| &turn.session == session),
            Err(e) => {
                tracing::warn!("failed to check pending turns: {e}");
                false
            }
        }
    }

    /// Remove one conversation's session-file records and any upload blob no longer referenced
    /// by another record. Returns the count and the sum of the records' sizes.
    pub async fn clear(&self, session: &VizierSession) -> Result<ClearResult, ClearError> {
        if self.has_active_turn(session).await {
            return Err(ClearError::TurnActive);
        }
        let records = self.storage.list_session_files(session).await?;
        let bytes = records.iter().map(|r| r.size).sum();
        let removed = records.len();
        let file_ids: BTreeSet<String> = records.iter().map(|r| r.file_id.clone()).collect();

        self.storage.delete_session_files(session).await?;

        for file_id in file_ids {
            match self.storage.session_file_reference_count(&file_id).await {
                Ok(0) => {
                    if let Err(e) = self.file_manager.delete_upload(&file_id).await {
                        tracing::warn!(file_id, "failed to remove upload after clear: {e}");
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(file_id, "failed to count upload references: {e}");
                }
            }
        }

        Ok(ClearResult { removed, bytes })
    }

    /// Clear a conversation's files and then delete its session metadata. Artifacts promoted
    /// from it remain.
    pub async fn delete_conversation(
        &self,
        session: &VizierSession,
    ) -> Result<ClearResult, ClearError> {
        if self.has_active_turn(session).await {
            return Err(ClearError::TurnActive);
        }
        let result = self.clear(session).await?;
        self.storage
            .delete_session(
                session.0.clone(),
                session.1.clone(),
                session.2.clone().unwrap_or_default(),
            )
            .await?;
        Ok(result)
    }

    /// Remove the upload blobs of records collected before an agent's storage delete. A blob
    /// still referenced by another agent's record is kept. Best effort; returns how many blobs
    /// were removed.
    pub async fn delete_blobs(&self, files: &[SessionFileRecord]) -> usize {
        let mut removed = 0;
        let mut seen = BTreeSet::new();
        for file in files {
            if !seen.insert(file.file_id.clone()) {
                continue;
            }
            match self
                .storage
                .session_file_reference_count(&file.file_id)
                .await
            {
                Ok(0) => match self.file_manager.delete_upload(&file.file_id).await {
                    Ok(()) => removed += 1,
                    Err(e) => tracing::warn!(
                        file_id = %file.file_id,
                        "failed to remove upload after agent delete: {e}"
                    ),
                },
                Ok(_) => {}
                Err(e) => tracing::warn!(
                    file_id = %file.file_id,
                    "failed to count upload references: {e}"
                ),
            }
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::schema::VizierChannelId;
    use crate::storage::document::LocalDocumentStore;
    use crate::storage::sqlite::SqliteStorage;

    struct Harness {
        session_files: SessionFiles,
        storage: Arc<VizierStorage>,
        dir: tempfile::TempDir,
    }

    fn setup() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        SqliteStorage::init_schema(&conn).unwrap();
        let storage = Arc::new(VizierStorage::new(SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        )));
        let fm = FileManager::new(dir.path().to_string_lossy().to_string());
        Harness {
            session_files: SessionFiles::new(storage.clone(), fm),
            storage,
            dir,
        }
    }

    fn session(topic: &str) -> VizierSession {
        VizierSession(
            "a".into(),
            VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
            Some(topic.into()),
        )
    }

    async fn add_file(h: &Harness, session: &VizierSession, name: &str, file_id: &str, size: u64) {
        h.storage
            .save_session_file(session, name, "text/plain", size, file_id)
            .await
            .unwrap();
        let file_dir = h.dir.path().join("uploads").join(file_id);
        tokio::fs::create_dir_all(&file_dir).await.unwrap();
        tokio::fs::write(file_dir.join(name), b"x").await.unwrap();
    }

    #[tokio::test]
    async fn count_and_clear_report_the_files_and_bytes() {
        let h = setup();
        let s = session("t");
        add_file(&h, &s, "a.txt", "f1", 10).await;
        add_file(&h, &s, "b.txt", "f2", 20).await;
        assert_eq!(h.session_files.count(&s).await.unwrap(), 2);
        let result = h.session_files.clear(&s).await.unwrap();
        assert_eq!(
            result,
            ClearResult {
                removed: 2,
                bytes: 30
            }
        );
        assert_eq!(h.session_files.count(&s).await.unwrap(), 0);
        assert!(!h.dir.path().join("uploads").join("f1").exists());
    }

    #[tokio::test]
    async fn an_empty_clear_changes_nothing() {
        let h = setup();
        let s = session("t");
        assert_eq!(
            h.session_files.clear(&s).await.unwrap(),
            ClearResult {
                removed: 0,
                bytes: 0
            }
        );
    }

    #[tokio::test]
    async fn a_shared_blob_survives_until_its_last_reference_is_cleared() {
        let h = setup();
        let first = session("a");
        let second = session("b");
        add_file(&h, &first, "shared.txt", "shared", 5).await;
        // A memory read can re-save the same upload into another conversation.
        h.storage
            .save_session_file(&second, "copy.txt", "text/plain", 5, "shared")
            .await
            .unwrap();
        h.session_files.clear(&first).await.unwrap();
        assert!(h.dir.path().join("uploads").join("shared").exists());
        h.session_files.clear(&second).await.unwrap();
        assert!(!h.dir.path().join("uploads").join("shared").exists());
    }

    #[tokio::test]
    async fn a_running_or_queued_turn_refuses_the_clear() {
        let h = setup();
        let s = session("t");
        add_file(&h, &s, "a.txt", "f1", 1).await;
        let request = crate::schema::VizierRequest {
            content: crate::schema::VizierRequestContent::Chat("hi".into()),
            ..Default::default()
        };
        h.storage.accept_turn(&s, &request).await.unwrap();
        assert!(h.session_files.has_active_turn(&s).await);
        assert_eq!(h.session_files.clear(&s).await, Err(ClearError::TurnActive));
        assert_eq!(h.session_files.count(&s).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn delete_conversation_removes_files_and_metadata_but_not_other_conversations() {
        let h = setup();
        let first = session("a");
        let second = session("b");
        add_file(&h, &first, "a.txt", "f1", 1).await;
        add_file(&h, &second, "b.txt", "f2", 1).await;
        h.session_files.delete_conversation(&first).await.unwrap();
        assert_eq!(h.session_files.count(&first).await.unwrap(), 0);
        assert_eq!(h.session_files.count(&second).await.unwrap(), 1);
        assert!(!h.dir.path().join("uploads").join("f1").exists());
        assert!(h.dir.path().join("uploads").join("f2").exists());
    }
}
