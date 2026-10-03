use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::{
        AgentId, SessionHistoryContent, TaskRun, TaskRunState, VizierChannelId,
        VizierResponseContent, VizierSession,
    },
    storage::{VizierStorage, history::HistoryStorage},
};

#[async_trait::async_trait]
pub trait TaskRunStorage {
    /// Open a run. Called before the agent starts, so a run that dies with the
    /// process is still accounted for.
    async fn open_task_run(
        &self,
        agent_id: AgentId,
        task_slug: String,
        ran_at: DateTime<Utc>,
        session_key: String,
    ) -> Result<i64>;

    /// Close it in a terminal state. Nothing re-opens a closed run.
    async fn close_task_run(
        &self,
        id: i64,
        state: TaskRunState,
        finished_at: DateTime<Utc>,
    ) -> Result<()>;

    /// The overlap lock: at most one `Running` row per `(agent_id, task_slug)`.
    async fn running_task_run(&self, agent_id: AgentId, task_slug: String)
    -> Result<Option<TaskRun>>;

    /// Newest first. `before`/`before_id` continue a previous page; `limit` bounds it.
    async fn list_task_runs(
        &self,
        agent_id: AgentId,
        task_slug: String,
        before: Option<DateTime<Utc>>,
        before_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<TaskRun>>;

    async fn get_task_run(
        &self,
        agent_id: AgentId,
        task_slug: String,
        ran_at: DateTime<Utc>,
    ) -> Result<Option<TaskRun>>;

    /// Startup sweep: every `Running` row becomes `Interrupted`. Nothing can
    /// legitimately be running before the scheduler starts, so a row left open is
    /// a run that died with the previous process.
    async fn interrupt_open_task_runs(&self) -> Result<usize>;

    /// Called from task deletion: a new task reusing a freed slug must find no rows.
    async fn delete_task_runs(&self, agent_id: AgentId, task_slug: String) -> Result<()>;
}

#[async_trait::async_trait]
impl TaskRunStorage for VizierStorage {
    async fn open_task_run(
        &self,
        agent_id: AgentId,
        task_slug: String,
        ran_at: DateTime<Utc>,
        session_key: String,
    ) -> Result<i64> {
        self.0
            .open_task_run(agent_id, task_slug, ran_at, session_key)
            .await
    }

    async fn close_task_run(
        &self,
        id: i64,
        state: TaskRunState,
        finished_at: DateTime<Utc>,
    ) -> Result<()> {
        self.0.close_task_run(id, state, finished_at).await
    }

    async fn running_task_run(
        &self,
        agent_id: AgentId,
        task_slug: String,
    ) -> Result<Option<TaskRun>> {
        self.0.running_task_run(agent_id, task_slug).await
    }

    async fn list_task_runs(
        &self,
        agent_id: AgentId,
        task_slug: String,
        before: Option<DateTime<Utc>>,
        before_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<TaskRun>> {
        self.0
            .list_task_runs(agent_id, task_slug, before, before_id, limit)
            .await
    }

    async fn get_task_run(
        &self,
        agent_id: AgentId,
        task_slug: String,
        ran_at: DateTime<Utc>,
    ) -> Result<Option<TaskRun>> {
        self.0.get_task_run(agent_id, task_slug, ran_at).await
    }

    async fn interrupt_open_task_runs(&self) -> Result<usize> {
        self.0.interrupt_open_task_runs().await
    }

    async fn delete_task_runs(&self, agent_id: AgentId, task_slug: String) -> Result<()> {
        self.0.delete_task_runs(agent_id, task_slug).await
    }
}

/// The session a run wrote into, rebuilt from its address.
///
/// `TaskRun.session_key` is the rendered slug and so cannot be turned back into a
/// `VizierSession`; the pair `(task_slug, ran_at)` can, and is what the run is addressed by
/// everywhere else.
pub fn task_run_session(run: &TaskRun) -> VizierSession {
    VizierSession(
        run.agent_id.clone(),
        VizierChannelId::Task(run.task_slug.clone(), run.ran_at),
        None,
    )
}

/// What a run reported: the last `Response` carrying message content in that run's session.
///
/// Not a stored field, deliberately — a second copy of the response in a place people read
/// as authoritative would drift from the conversation it came from. `TaskRunState` is what
/// records whether there is anything to find here; this is only the reading of it.
///
/// Returns `None` for a run that is not `Answered`, without touching storage: the state was
/// decided when the run closed and is not re-derived at read time.
pub async fn task_run_response(storage: &VizierStorage, run: &TaskRun) -> Option<String> {
    if run.state != TaskRunState::Answered {
        return None;
    }

    let history = match storage
        .list_session_history(task_run_session(run), None, None, None)
        .await
    {
        Ok(history) => history,
        Err(e) => {
            tracing::warn!(
                "failed to read the conversation of run {} of task '{}': {}",
                run.id,
                run.task_slug,
                e
            );
            return None;
        }
    };

    history.iter().rev().find_map(|entry| match &entry.content {
        SessionHistoryContent::Response(response) => match &response.content {
            VizierResponseContent::Message { content, .. } => Some(content.clone()),
            _ => None,
        },
        _ => None,
    })
}
