use anyhow::Result;

use crate::{
    schema::{SessionHistory, SessionHistoryContent, VizierSession},
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait HistoryStorage {
    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<()>;

    // TODO: cursor based pagination
    async fn list_session_history(&self, session: VizierSession) -> Result<Vec<SessionHistory>>;
}

#[async_trait::async_trait]
impl HistoryStorage for VizierStorage {
    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<()> {
        self.save_session_history(session, content).await
    }

    // TODO: cursor based pagination
    async fn list_session_history(&self, session: VizierSession) -> Result<Vec<SessionHistory>> {
        self.list_session_history(session).await
    }
}
