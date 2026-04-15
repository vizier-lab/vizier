use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::{AgentUsageStats, SessionHistory, SessionHistoryContent, VizierSession},
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

    async fn aggregate_usage(
        &self,
        agent_id: &str,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> Result<AgentUsageStats>;

    async fn list_session_by_time_window(
        &self,
        session: VizierSession,
        start_datetime: Option<DateTime<Utc>>,
        end_datetime: Option<DateTime<Utc>>,
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

    async fn aggregate_usage(
        &self,
        agent_id: &str,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> Result<AgentUsageStats> {
        self.0.aggregate_usage(agent_id, start_date, end_date).await
    }

    async fn list_session_by_time_window(
        &self,
        session: VizierSession,
        start_datetime: Option<DateTime<Utc>>,
        end_datetime: Option<DateTime<Utc>>,
    ) -> Result<Vec<SessionHistory>> {
        self.0
            .list_session_by_time_window(session, start_datetime, end_datetime)
            .await
    }
}
