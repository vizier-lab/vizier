//! Scheduled tasks and their runs (`specs/011-task-completion-reports/`).
//!
//! Every operation first checks that the agent exists and the caller may view it. The list is
//! returned whole; paging and filtering it is the adapter's business.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use croner::Cron;
use serde::{Deserialize, Serialize};

use super::{ServiceError, Services};
use crate::{
    access::can_view_agent,
    auth::Principal,
    schema::{
        Requester, SessionHistory, Task, TaskRun, TaskRunState, TaskSchedule, VizierChannelId, VizierSession,
    },
    storage::{
        agent::AgentStorage,
        history::HistoryStorage,
        task::TaskStorage,
        task_run::{TaskRunStorage, task_run_response},
    },
};

/// The run-list page size, and its cap. A minutely task accrues ~525k runs a year, so the
/// listing is bounded whatever a caller asks for.
const DEFAULT_RUN_LIMIT: usize = 20;
const MAX_RUN_LIMIT: usize = 100;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateTaskRequest {
    pub slug: String,
    pub title: String,
    pub instruction: String,
    pub schedule: ScheduleRequest,
    /// Whether the task is switched on. Omitted means on, which is what both routes always did.
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type")]
pub enum ScheduleRequest {
    Cron { expression: String },
    OneTime { datetime: DateTime<Utc> },
}

impl From<ScheduleRequest> for TaskSchedule {
    fn from(req: ScheduleRequest) -> Self {
        match req {
            ScheduleRequest::Cron { expression } => TaskSchedule::CronTask(expression),
            ScheduleRequest::OneTime { datetime } => TaskSchedule::OneTimeTask(datetime),
        }
    }
}

/// One run as the task screen reads it.
///
/// `response` is `Some` only on the detail route, where the response is what a person came
/// to read. The listing and the task list leave it `None`, so neither can spend a page of
/// JSON on text nobody asked for.
#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TaskRunResponse {
    /// The run's address — its `ran_at` — which the detail routes take.
    pub run_id: DateTime<Utc>,
    pub id: i64,
    pub ran_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub state: TaskRunState,
    /// Absent on the listing routes; `null` for a run that answered nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Option<String>>,
}

impl TaskRunResponse {
    /// Without the response text — the listing shape (FR-014, FR-020).
    pub fn summary(run: &TaskRun) -> Self {
        Self {
            run_id: run.ran_at,
            id: run.id,
            ran_at: run.ran_at,
            finished_at: run.finished_at,
            state: run.state,
            response: None,
        }
    }

    /// With it. `Some(None)` is a run that reached a terminal state having produced nothing
    /// a person can read, which is distinct from a task that has never run at all — that
    /// one has no `last_run` at all.
    pub fn detailed(run: &TaskRun, response: Option<String>) -> Self {
        Self {
            run_id: run.ran_at,
            id: run.id,
            ran_at: run.ran_at,
            finished_at: run.finished_at,
            state: run.state,
            response: Some(response),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TaskRunsResponse {
    pub runs: Vec<TaskRunResponse>,
    /// FR-009: a caller must not be offered a page that does not exist.
    pub has_more: bool,
}

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TaskResponse {
    pub slug: String,
    pub requester: Requester,
    pub title: String,
    pub instruction: String,
    pub is_active: bool,
    pub schedule: TaskSchedule,
    pub last_executed_at: Option<DateTime<Utc>>,
    pub timestamp: DateTime<Utc>,
    /// `null` for a task that has never run — distinct from a run that answered nothing,
    /// which is a `last_run` whose `response` is `null`.
    pub last_run: Option<TaskRunResponse>,
    /// When it runs next: a cron task's next occurrence, a one-time task's moment while it is
    /// still ahead; `null` when inactive or finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<DateTime<Utc>>,
}

pub fn next_run_of(task: &Task, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    if !task.is_active {
        return None;
    }
    match &task.schedule {
        TaskSchedule::CronTask(expr) => Cron::from_str(expr)
            .ok()
            .and_then(|c| c.find_next_occurrence(&now, false).ok()),
        TaskSchedule::OneTimeTask(at) => (*at > now).then_some(*at),
    }
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
        let next_run = next_run_of(&task, Utc::now());
        Self {
            slug: task.slug,
            requester: task.requester,
            title: task.title,
            instruction: task.instruction,
            is_active: task.is_active,
            schedule: task.schedule,
            last_executed_at: task.last_executed_at,
            timestamp: task.timestamp,
            last_run: None,
            next_run,
        }
    }
}

fn validate_schedule(schedule: &ScheduleRequest) -> Result<(), String> {
    match schedule {
        ScheduleRequest::Cron { expression } => {
            if expression.trim().is_empty() {
                return Err("Cron expression cannot be empty".to_string());
            }
            match Cron::from_str(expression) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Invalid cron expression: {}", e)),
            }
        }
        ScheduleRequest::OneTime { datetime } => {
            let now = Utc::now();
            if *datetime < now {
                return Err("One-time task datetime must be in the future".to_string());
            }
            Ok(())
        }
    }
}

fn internal(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Internal(e.to_string())
}

/// The agent must exist and the caller must be permitted to view it.
async fn view_agent(s: &Services, who: &Principal, agent_id: &str) -> Result<(), ServiceError> {
    let config = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound(format!("agent {agent_id} not found"))),
        Err(e) => return Err(internal(e)),
    };

    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    Ok(())
}

/// A task's newest run, without its response text — what the task *list* shows.
async fn last_run_summary(s: &Services, task: &Task) -> Option<TaskRunResponse> {
    let runs = s
        .deps()
        .storage
        .list_task_runs(task.agent_id.clone(), task.slug.clone(), None, None, 1)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to read runs of task '{}': {}", task.slug, e);
            Vec::new()
        });

    runs.first().map(TaskRunResponse::summary)
}

/// A task's newest run *with* its response — what the task screen shows. The response is
/// not truncated: this is where it is read, and FR-015 renders it rather than hiding it.
async fn last_run_detail(s: &Services, task: &Task) -> Option<TaskRunResponse> {
    let runs = s
        .deps()
        .storage
        .list_task_runs(task.agent_id.clone(), task.slug.clone(), None, None, 1)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to read runs of task '{}': {}", task.slug, e);
            Vec::new()
        });

    let run = runs.first()?;
    // A run still in flight, and one that answered nothing, both read as a `last_run` with
    // a `null` response — the state is what tells them apart.
    let response = task_run_response(&s.deps().storage, run).await;

    Some(TaskRunResponse::detailed(run, response))
}

/// Every task of the agent, each with its newest run (no response text).
pub async fn list(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    is_active: Option<bool>,
) -> Result<Vec<TaskResponse>, ServiceError> {
    view_agent(s, who, agent_id).await?;

    let tasks = s
        .deps()
        .storage
        .get_task_list(Some(agent_id.to_string()), is_active)
        .await
        .map_err(internal)?;
    let mut response: Vec<TaskResponse> = Vec::with_capacity(tasks.len());
    for task in tasks {
        // The list shows when each task last ran and the state that run reached,
        // and nothing more — no response text anywhere in it.
        let last_run = last_run_summary(s, &task).await;
        let mut entry = TaskResponse::from(task);
        entry.last_run = last_run;
        response.push(entry);
    }
    Ok(response)
}

pub async fn get(s: &Services, who: &Principal, agent_id: &str, slug: &str) -> Result<TaskResponse, ServiceError> {
    view_agent(s, who, agent_id).await?;

    let tasks = s
        .deps()
        .storage
        .get_task_list(Some(agent_id.to_string()), None)
        .await
        .map_err(internal)?;
    if let Some(task) = tasks.into_iter().find(|t| t.slug == slug) {
        let last_run = last_run_detail(s, &task).await;
        let mut response = TaskResponse::from(task);
        response.last_run = last_run;
        Ok(response)
    } else {
        Err(ServiceError::NotFound(format!("task {slug} not found")))
    }
}

fn build_task(who: &Principal, agent_id: &str, body: CreateTaskRequest) -> Task {
    Task {
        slug: body.slug,
        // The requester is the authenticated caller, not anything the body claims. A body
        // still carrying `user` is accepted and the field ignored rather than rejected: it
        // was never validated and most often held the literal string "user", so refusing
        // it would break callers to no benefit.
        requester: Requester::User(who.username.clone()),
        agent_id: agent_id.to_string(),
        title: body.title,
        instruction: body.instruction,
        is_active: body.is_active.unwrap_or(true),
        schedule: body.schedule.into(),
        last_executed_at: None,
        timestamp: Utc::now(),
    }
}

pub async fn create(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: CreateTaskRequest,
) -> Result<TaskResponse, ServiceError> {
    view_agent(s, who, agent_id).await?;

    if let Err(err) = validate_schedule(&body.schedule) {
        return Err(ServiceError::invalid(err));
    }

    let task = build_task(who, agent_id, body);
    s.deps().storage.save_task(task.clone()).await.map_err(internal)?;
    Ok(TaskResponse::from(task))
}

pub async fn update(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
    body: CreateTaskRequest,
) -> Result<TaskResponse, ServiceError> {
    view_agent(s, who, agent_id).await?;

    if let Err(err) = validate_schedule(&body.schedule) {
        return Err(ServiceError::invalid(err));
    }

    let tasks = s
        .deps()
        .storage
        .get_task_list(Some(agent_id.to_string()), None)
        .await
        .map_err(internal)?;
    if !tasks.iter().any(|t| t.slug == slug) {
        return Err(ServiceError::NotFound(format!("task {slug} not found")));
    }

    let task = build_task(who, agent_id, body);
    s.deps().storage.save_task(task.clone()).await.map_err(internal)?;
    Ok(TaskResponse::from(task))
}

pub async fn delete(s: &Services, who: &Principal, agent_id: &str, slug: &str) -> Result<String, ServiceError> {
    view_agent(s, who, agent_id).await?;

    s.deps()
        .storage
        .delete_task(agent_id.to_string(), slug.to_string())
        .await
        .map_err(internal)?;
    Ok(format!("task {slug} deleted"))
}

/// Both run routes need the same two things first: the agent must exist and the caller must
/// be permitted to view it, and the task must exist. Sharing it keeps the permission check
/// from being something either route could be written without.
async fn authorize_task(s: &Services, who: &Principal, agent_id: &str, slug: &str) -> Result<(), ServiceError> {
    view_agent(s, who, agent_id).await?;

    match s.deps().storage.get_task(agent_id.to_string(), slug.to_string()).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(ServiceError::NotFound(format!("task {slug} not found"))),
        Err(e) => Err(internal(e)),
    }
}

/// Past runs, newest first, without response text.
pub async fn list_runs(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
    before: Option<DateTime<Utc>>,
    before_id: Option<i64>,
    limit: Option<usize>,
) -> Result<TaskRunsResponse, ServiceError> {
    authorize_task(s, who, agent_id, slug).await?;

    let limit = limit.unwrap_or(DEFAULT_RUN_LIMIT).clamp(1, MAX_RUN_LIMIT);

    // One past the page, so `has_more` is answered by the same query rather than a count
    // that could disagree with it.
    let mut runs = s
        .deps()
        .storage
        .list_task_runs(
            agent_id.to_string(),
            slug.to_string(),
            before,
            before.and(before_id),
            limit + 1,
        )
        .await
        .map_err(internal)?;

    let has_more = runs.len() > limit;
    runs.truncate(limit);

    Ok(TaskRunsResponse {
        runs: runs.iter().map(TaskRunResponse::summary).collect(),
        has_more,
    })
}

/// One run's full exchange.
#[allow(clippy::too_many_arguments)]
pub async fn run_history(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
    run_id: DateTime<Utc>,
    before: Option<DateTime<Utc>>,
    before_seq: Option<i64>,
    limit: Option<usize>,
) -> Result<Vec<SessionHistory>, ServiceError> {
    authorize_task(s, who, agent_id, slug).await?;

    // A run must exist at that address. Without this, a bad `run_id` would return an empty
    // exchange rather than saying there is no such run.
    match s
        .deps()
        .storage
        .get_task_run(agent_id.to_string(), slug.to_string(), run_id)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err(ServiceError::NotFound(format!("no run of {slug} at {run_id}")));
        }
        Err(e) => return Err(internal(e)),
    }

    // `list_session_history` already accepts any session, so addressing a task's
    // conversation needs no new storage call — only this route to name it.
    let session = VizierSession(agent_id.to_string(), VizierChannelId::Task(slug.to_string(), run_id), None);

    s.deps()
        .storage
        .list_session_history(session, before, before_seq, limit)
        .await
        .map_err(internal)
}
