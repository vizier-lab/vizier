use anyhow::Result;

use crate::schema::{VizierSession, session_file::SessionFileRecord};

#[async_trait::async_trait]
pub trait SessionFileStorage {
    async fn save_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<SessionFileRecord>;

    async fn list_session_files(&self, session: &VizierSession) -> Result<Vec<SessionFileRecord>>;

    async fn get_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<Option<SessionFileRecord>>;

    async fn delete_session_file(&self, session: &VizierSession, filename: &str) -> Result<()>;

    /// Sets who a saved file came from and the history entry it arrived with. Files a tool
    /// makes are labelled with the agent id when saved; a person's attachment is relabelled
    /// here once the request is saved. A backend that doesn't track this does nothing.
    async fn label_session_file(
        &self,
        _session: &VizierSession,
        _filename: &str,
        _sender: Option<&str>,
        _history_uid: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }

    /// Saves and relabels, as they happen, for whoever shows the session live.
    fn subscribe_session_files(
        &self,
    ) -> Option<tokio::sync::broadcast::Receiver<(VizierSession, SessionFileRecord)>> {
        None
    }
}
