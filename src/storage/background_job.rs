use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::{
        AgentId, BackgroundJob, JobState, PieceState, SessionHistoryContent, TopicId,
        VizierChannelId, VizierResponseContent, VizierSession,
    },
    storage::{VizierStorage, history::HistoryStorage},
};

#[async_trait::async_trait]
pub trait BackgroundJobStorage {
    /// Job and pieces in one transaction, before any piece is dispatched.
    async fn open_background_job(&self, job: BackgroundJob) -> Result<()>;

    /// Close one piece. Only a `running` piece is closed, so a late close cannot overwrite
    /// a state already recorded (a cancel, or the sweep).
    async fn close_background_piece(
        &self,
        job_id: &str,
        ordinal: u32,
        state: PieceState,
        reason: Option<String>,
        finished_at: DateTime<Utc>,
    ) -> Result<()>;

    /// Guarded transition out of `running` (to `reporting` or `cancelled`), or out of
    /// `reporting` (to `reported`/`undelivered`). Returns false if the guard did not match,
    /// i.e. someone else already moved the job. Cancelling also closes the job's still-running
    /// pieces.
    async fn transition_background_job(
        &self,
        job_id: &str,
        from: JobState,
        to: JobState,
        cancelled_by: Option<String>,
        reason: Option<String>,
        at: DateTime<Utc>,
    ) -> Result<bool>;

    async fn get_background_job(&self, job_id: &str) -> Result<Option<BackgroundJob>>;

    /// The tray read and the cancel cascade: in-flight jobs launched from `origin`, oldest first.
    async fn list_running_background_jobs(
        &self,
        origin: VizierSession,
    ) -> Result<Vec<BackgroundJob>>;

    /// `list_background_jobs`: in-flight jobs launched by `agent_id` from any of its sessions.
    async fn list_agent_running_background_jobs(
        &self,
        agent_id: AgentId,
    ) -> Result<Vec<BackgroundJob>>;

    /// The topic-list badge: in-flight job counts for one agent and channel, by topic.
    async fn count_running_background_jobs(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
    ) -> Result<HashMap<Option<TopicId>, usize>>;

    /// Startup sweep: every `running`/`reporting` job and `running` piece becomes
    /// `interrupted`. Nothing can be in flight before the agents start, so a row left open is
    /// a job that died with the previous process.
    async fn interrupt_open_background_jobs(&self) -> Result<usize>;
}

#[async_trait::async_trait]
impl BackgroundJobStorage for VizierStorage {
    async fn open_background_job(&self, job: BackgroundJob) -> Result<()> {
        self.0.open_background_job(job).await
    }

    async fn close_background_piece(
        &self,
        job_id: &str,
        ordinal: u32,
        state: PieceState,
        reason: Option<String>,
        finished_at: DateTime<Utc>,
    ) -> Result<()> {
        self.0
            .close_background_piece(job_id, ordinal, state, reason, finished_at)
            .await
    }

    async fn transition_background_job(
        &self,
        job_id: &str,
        from: JobState,
        to: JobState,
        cancelled_by: Option<String>,
        reason: Option<String>,
        at: DateTime<Utc>,
    ) -> Result<bool> {
        self.0
            .transition_background_job(job_id, from, to, cancelled_by, reason, at)
            .await
    }

    async fn get_background_job(&self, job_id: &str) -> Result<Option<BackgroundJob>> {
        self.0.get_background_job(job_id).await
    }

    async fn list_running_background_jobs(
        &self,
        origin: VizierSession,
    ) -> Result<Vec<BackgroundJob>> {
        self.0.list_running_background_jobs(origin).await
    }

    async fn list_agent_running_background_jobs(
        &self,
        agent_id: AgentId,
    ) -> Result<Vec<BackgroundJob>> {
        self.0.list_agent_running_background_jobs(agent_id).await
    }

    async fn count_running_background_jobs(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
    ) -> Result<HashMap<Option<TopicId>, usize>> {
        self.0.count_running_background_jobs(agent_id, channel).await
    }

    async fn interrupt_open_background_jobs(&self) -> Result<usize> {
        self.0.interrupt_open_background_jobs().await
    }
}

/// What a piece answered: the last `Response` carrying message content in its own session.
///
/// Not a stored field, for the same reason as `task_run_response`: the answer lives in the
/// piece's conversation, and a second copy would drift from it.
pub async fn piece_answer(storage: &VizierStorage, session: &VizierSession) -> Option<String> {
    let history = match storage
        .list_session_history(session.clone(), None, None, None)
        .await
    {
        Ok(history) => history,
        Err(e) => {
            tracing::warn!(
                "failed to read the conversation of background piece {}: {}",
                session.to_slug(),
                e
            );
            return None;
        }
    };

    history.iter().rev().find_map(|entry| match &entry.content {
        SessionHistoryContent::Response(response) => match &response.content {
            VizierResponseContent::Message { content, .. } => Some(content.clone()),
            VizierResponseContent::AudioReply(_, Some(text), _) => Some(text.clone()),
            _ => None,
        },
        _ => None,
    })
}
