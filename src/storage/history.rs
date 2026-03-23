use anyhow::Result;
use chrono::{DateTime, Utc};

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

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>>;
}

#[async_trait::async_trait]
impl HistoryStorage for VizierStorage {
    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<()> {
        self.0.save_session_history(session, content).await
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        self.0.list_session_history(session, before, limit).await
    }
}
