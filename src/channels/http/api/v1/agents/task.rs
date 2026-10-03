use std::str::FromStr;

use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json,
};
use chrono::Utc;
use croner::Cron;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response, APIResponse},
        },
        state::HTTPState,
    },
    schema::{
        Requester, SessionHistory, Task, TaskRun, TaskRunState, TaskSchedule, VizierChannelId,
        VizierSession,
    },
    storage::{
        agent::AgentStorage,
        history::HistoryStorage,
        task::TaskStorage,
        task_run::{TaskRunStorage, task_run_response},
    },
};

use super::user_can_view_agent;

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

pub fn task() -> Router<HTTPState> {
    Router::new()
        .route("/", get(get_tasks))
        .route("/", post(create_task))
        .route("/{slug}", get(get_task))
        .route("/{slug}", put(update_task))
        .route("/{slug}", delete(delete_task))
        // Run results live under `/tasks/{slug}` rather than under `/channel` because that
        // router derives its session from the caller — `VizierChannelId::HTTP(username, ..)`
        // — so it can only ever name the caller's own web conversations, never a task's.
        // Here the permission check is the one tasks already use.
        .route("/{slug}/runs", get(list_runs))
        .route("/{slug}/runs/{run_id}/history", get(get_run_history))
}

/// The run-list page size, and its cap. A minutely task accrues ~525k runs a year, so the
/// listing is bounded whatever a caller asks for.
const DEFAULT_RUN_LIMIT: usize = 20;
const MAX_RUN_LIMIT: usize = 100;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct GetTasksQuery {
    is_active: Option<bool>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateTaskRequest {
    slug: String,
    title: String,
    instruction: String,
    schedule: ScheduleRequest,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type")]
pub enum ScheduleRequest {
    Cron { expression: String },
    OneTime { datetime: chrono::DateTime<Utc> },
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
#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct TaskRunResponse {
    /// The run's address — its `ran_at` — which the detail routes take.
    run_id: chrono::DateTime<Utc>,
    id: i64,
    ran_at: chrono::DateTime<Utc>,
    finished_at: Option<chrono::DateTime<Utc>>,
    state: TaskRunState,
    /// Absent on the listing routes; `null` for a run that answered nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<Option<String>>,
}

impl TaskRunResponse {
    /// Without the response text — the listing shape (FR-014, FR-020).
    fn summary(run: &TaskRun) -> Self {
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
    fn detailed(run: &TaskRun, response: Option<String>) -> Self {
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

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct TaskRunsResponse {
    runs: Vec<TaskRunResponse>,
    /// FR-009: a caller must not be offered a page that does not exist.
    has_more: bool,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct TaskResponse {
    slug: String,
    requester: Requester,
    title: String,
    instruction: String,
    is_active: bool,
    schedule: TaskSchedule,
    last_executed_at: Option<chrono::DateTime<Utc>>,
    timestamp: chrono::DateTime<Utc>,
    /// `null` for a task that has never run — distinct from a run that answered nothing,
    /// which is a `last_run` whose `response` is `null`.
    last_run: Option<TaskRunResponse>,
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
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
        }
    }
}

/// A task's newest run, without its response text — what the task *list* shows.
async fn last_run_summary(state: &HTTPState, task: &Task) -> Option<TaskRunResponse> {
    let runs = state
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
async fn last_run_detail(state: &HTTPState, task: &Task) -> Option<TaskRunResponse> {
    let runs = state
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
    let response = task_run_response(&state.storage, run).await;

    Some(TaskRunResponse::detailed(run, response))
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/tasks",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = GetTasksQuery,
    responses(
        (status = 200, description = "List of tasks", body = APIResponse<Vec<TaskResponse>>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn get_tasks(
    Path(agent_id): Path<String>,
    Query(params): Query<GetTasksQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<TaskResponse>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state
        .storage
        .get_task_list(Some(agent_id), params.is_active)
        .await
    {
        Ok(tasks) => {
            let mut response: Vec<TaskResponse> = Vec::with_capacity(tasks.len());
            for task in tasks {
                // The list shows when each task last ran and the state that run reached,
                // and nothing more — no response text anywhere in it.
                let last_run = last_run_summary(&state, &task).await;
                let mut entry = TaskResponse::from(task);
                entry.last_run = last_run;
                response.push(entry);
            }
            api_response(StatusCode::OK, response)
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/tasks/{slug}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Task slug")
    ),
    responses(
        (status = 200, description = "Task details", body = APIResponse<TaskResponse>),
        (status = 404, description = "Agent or task not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn get_task(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<TaskResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state.storage.get_task_list(Some(agent_id), None).await {
        Ok(tasks) => {
            if let Some(task) = tasks.into_iter().find(|t| t.slug == slug) {
                let last_run = last_run_detail(&state, &task).await;
                let mut response = TaskResponse::from(task);
                response.last_run = last_run;
                api_response(StatusCode::OK, response)
            } else {
                err_response(StatusCode::NOT_FOUND, format!("task {slug} not found"))
            }
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/tasks",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = CreateTaskRequest,
    responses(
        (status = 201, description = "Task created", body = APIResponse<TaskResponse>),
        (status = 400, description = "Invalid schedule", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn create_task(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<CreateTaskRequest>,
) -> models::response::Response<TaskResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    // Validate schedule
    if let Err(err) = validate_schedule(&body.schedule) {
        return err_response(StatusCode::BAD_REQUEST, err);
    }

    let task = Task {
        slug: body.slug,
        // The requester is the authenticated caller, not anything the body claims. A body
        // still carrying `user` is accepted and the field ignored rather than rejected: it
        // was never validated and most often held the literal string "user", so refusing
        // it would break callers to no benefit.
        requester: Requester::User(user.username.clone()),
        agent_id,
        title: body.title,
        instruction: body.instruction,
        is_active: true,
        schedule: body.schedule.into(),
        last_executed_at: None,
        timestamp: Utc::now(),
    };

    match state.storage.save_task(task.clone()).await {
        Ok(_) => api_response(StatusCode::CREATED, TaskResponse::from(task)),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/tasks/{slug}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Task slug")
    ),
    request_body = CreateTaskRequest,
    responses(
        (status = 200, description = "Task updated", body = APIResponse<TaskResponse>),
        (status = 400, description = "Invalid schedule", body = APIResponse<String>),
        (status = 404, description = "Agent or task not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn update_task(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<CreateTaskRequest>,
) -> models::response::Response<TaskResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    // Validate schedule
    if let Err(err) = validate_schedule(&body.schedule) {
        return err_response(StatusCode::BAD_REQUEST, err);
    }

    // Check if task exists
    match state.storage.get_task_list(Some(agent_id.clone()), None).await {
        Ok(tasks) => {
            if !tasks.iter().any(|t| t.slug == slug) {
                return err_response(StatusCode::NOT_FOUND, format!("task {slug} not found"));
            }
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }

    let task = Task {
        slug: body.slug,
        // As in `create_task`: taken from the caller, never from the body.
        requester: Requester::User(user.username.clone()),
        agent_id,
        title: body.title,
        instruction: body.instruction,
        is_active: true,
        schedule: body.schedule.into(),
        last_executed_at: None,
        timestamp: Utc::now(),
    };

    match state.storage.save_task(task.clone()).await {
        Ok(_) => api_response(StatusCode::OK, TaskResponse::from(task)),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/tasks/{slug}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Task slug")
    ),
    responses(
        (status = 200, description = "Task deleted", body = APIResponse<String>),
        (status = 404, description = "Agent or task not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn delete_task(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<String> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state.storage.delete_task(agent_id, slug.clone()).await {
        Ok(_) => api_response(StatusCode::OK, format!("task {slug} deleted")),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RunsQuery {
    /// `ran_at` of the oldest run on the previous page.
    before: Option<chrono::DateTime<Utc>>,
    /// That run's `id`, completing the cursor so runs sharing a millisecond cannot
    /// straddle a page boundary. Ignored without `before`.
    before_id: Option<i64>,
    limit: Option<usize>,
}

/// Both run routes need the same two things first: the agent must exist and the caller must
/// be permitted to view it, and the task must exist. Sharing it keeps the permission check
/// from being something either route could be written without.
async fn authorize_task(
    state: &HTTPState,
    agent_id: &str,
    slug: &str,
    user: &crate::channels::http::auth::AuthenticatedUser,
) -> Result<(), (StatusCode, String)> {
    let config = match state.storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                format!("agent {agent_id} not found"),
            ));
        }
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    if !user_can_view_agent(user, &config) {
        return Err((StatusCode::FORBIDDEN, "Access denied".into()));
    }

    match state
        .storage
        .get_task(agent_id.to_string(), slug.to_string())
        .await
    {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err((StatusCode::NOT_FOUND, format!("task {slug} not found"))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/tasks/{slug}/runs",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Task slug")
    ),
    request_body = RunsQuery,
    responses(
        (status = 200, description = "Past runs, newest first", body = APIResponse<TaskRunsResponse>),
        (status = 403, description = "Not permitted to view the agent", body = APIResponse<String>),
        (status = 404, description = "Agent or task not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn list_runs(
    Path((agent_id, slug)): Path<(String, String)>,
    Query(params): Query<RunsQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<TaskRunsResponse> {
    if let Err((status, message)) = authorize_task(&state, &agent_id, &slug, &user).await {
        return err_response(status, message);
    }

    let limit = params
        .limit
        .unwrap_or(DEFAULT_RUN_LIMIT)
        .clamp(1, MAX_RUN_LIMIT);

    // One past the page, so `has_more` is answered by the same query rather than a count
    // that could disagree with it.
    let mut runs = match state
        .storage
        .list_task_runs(
            agent_id,
            slug,
            params.before,
            params.before.and(params.before_id),
            limit + 1,
        )
        .await
    {
        Ok(runs) => runs,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let has_more = runs.len() > limit;
    runs.truncate(limit);

    api_response(
        StatusCode::OK,
        TaskRunsResponse {
            runs: runs.iter().map(TaskRunResponse::summary).collect(),
            has_more,
        },
    )
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/tasks/{slug}/runs/{run_id}/history",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Task slug"),
        ("run_id" = String, Path, description = "The run's address, as the run list gives it (RFC3339)")
    ),
    request_body = crate::channels::http::api::v1::agents::channel::HistoryQuery,
    responses(
        (status = 200, description = "That run's full exchange", body = APIResponse<Vec<SessionHistory>>),
        (status = 403, description = "Not permitted to view the agent", body = APIResponse<String>),
        (status = 404, description = "Agent, task or run not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn get_run_history(
    Path((agent_id, slug, run_id)): Path<(String, String, chrono::DateTime<Utc>)>,
    Query(params): Query<crate::channels::http::api::v1::agents::channel::HistoryQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<SessionHistory>> {
    if let Err((status, message)) = authorize_task(&state, &agent_id, &slug, &user).await {
        return err_response(status, message);
    }

    // A run must exist at that address. Without this, a bad `run_id` would return an empty
    // exchange rather than saying there is no such run.
    match state
        .storage
        .get_task_run(agent_id.clone(), slug.clone(), run_id)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            return err_response(StatusCode::NOT_FOUND, format!("no run of {slug} at {run_id}"));
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }

    // `list_session_history` already accepts any session, so addressing a task's
    // conversation needs no new storage call — only this route to name it.
    let session = VizierSession(agent_id, VizierChannelId::Task(slug, run_id), None);

    match state
        .storage
        .list_session_history(session, params.before, params.before_seq, params.limit)
        .await
    {
        Ok(history) => api_response(StatusCode::OK, history),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}
