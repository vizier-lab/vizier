use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::{SessionHistory, SessionHistoryContent, VizierSession},
    storage::VizierStorage,
};

/// Channels no person talks on: the system, subagents, dreams, tasks and agent-to-agent. The one
/// copy every backend and the conversation listing use.
pub(crate) fn is_non_user_channel(channel_slug: &str) -> bool {
    channel_slug == "SYSTEM"
        || channel_slug == "SUBAGENT"
        || channel_slug.starts_with("DREAM__")
        || channel_slug.starts_with("task__")
        || channel_slug.starts_with("inter_agent__")
}

#[async_trait::async_trait]
pub trait HistoryStorage {
    /// The newest history entry of each agent that has any: when it was last active.
    async fn last_activity_by_agent(
        &self,
    ) -> Result<std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>> {
        Ok(Default::default())
    }

    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<String>;

    /// Newest-first-bounded, oldest-first-returned history for one session.
    ///
    /// `before` and `before_seq` together form the paging cursor: taken from the oldest entry
    /// of the previous page they address one exact entry, so a group of entries sharing a
    /// millisecond cannot be split across a page boundary. Passing `before` alone keeps the
    /// pre-`seq` behaviour, and `before_seq` without `before` is ignored.
    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
        before_seq: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>>;

    async fn list_session_by_time_window(
        &self,
        session: VizierSession,
        start_datetime: Option<DateTime<Utc>>,
        end_datetime: Option<DateTime<Utc>>,
    ) -> Result<Vec<SessionHistory>>;

    async fn list_user_sessions_in_window(
        &self,
        agent_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<VizierSession>>;

    async fn list_session_history_until_checkpoint(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
    ) -> Result<(Vec<SessionHistory>, Option<String>)>;

    async fn save_checkpoint(
        &self,
        session: VizierSession,
        handover: Option<String>,
    ) -> Result<SessionHistory>;

    /// How many requests the session has stored, across checkpoints.
    async fn count_session_requests(&self, session: VizierSession) -> Result<usize> {
        Ok(self
            .list_session_history(session, None, None, None)
            .await?
            .iter()
            .filter(|entry| matches!(entry.content, SessionHistoryContent::Request(_)))
            .count())
    }
}

/// Save one entry, retrying twice (after 100 ms, then 400 ms) while SQLite reports the
/// database busy; any other failure, or a third busy one, is returned.
pub async fn save_with_busy_retry(
    storage: &VizierStorage,
    session: &VizierSession,
    content: SessionHistoryContent,
) -> Result<String> {
    const BUSY_RETRIES_MS: [u64; 2] = [100, 400];
    let mut attempt = 0;
    loop {
        match storage.save_session_history(session.clone(), content.clone()).await {
            Ok(uid) => return Ok(uid),
            Err(e) => {
                let busy = {
                    let msg = e.to_string().to_lowercase();
                    msg.contains("database is locked") || msg.contains("busy")
                };
                if busy && let Some(ms) = BUSY_RETRIES_MS.get(attempt) {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
                    continue;
                }
                return Err(e);
            }
        }
    }
}

#[async_trait::async_trait]
impl HistoryStorage for VizierStorage {
    async fn last_activity_by_agent(
        &self,
    ) -> Result<std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>> {
        self.0.last_activity_by_agent().await
    }

    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<String> {
        self.0.save_session_history(session, content).await
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
        before_seq: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        self.0
            .list_session_history(session, before, before_seq, limit)
            .await
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

    async fn list_user_sessions_in_window(
        &self,
        agent_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<VizierSession>> {
        self.0
            .list_user_sessions_in_window(agent_id, start, end)
            .await
    }

    async fn list_session_history_until_checkpoint(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
    ) -> Result<(Vec<SessionHistory>, Option<String>)> {
        self.0
            .list_session_history_until_checkpoint(session, before)
            .await
    }

    async fn save_checkpoint(
        &self,
        session: VizierSession,
        handover: Option<String>,
    ) -> Result<SessionHistory> {
        self.0.save_checkpoint(session, handover).await
    }

    async fn count_session_requests(&self, session: VizierSession) -> Result<usize> {
        self.0.count_session_requests(session).await
    }
}
