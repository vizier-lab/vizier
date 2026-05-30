use std::str::FromStr;

use axum::{
    Router,
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
    schema::{Task, TaskSchedule},
    storage::task::TaskStorage,
};

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
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct GetTasksQuery {
    is_active: Option<bool>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateTaskRequest {
    slug: String,
    user: String,
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

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct TaskResponse {
    slug: String,
    user: String,
    title: String,
    instruction: String,
    is_active: bool,
    schedule: TaskSchedule,
    last_executed_at: Option<chrono::DateTime<Utc>>,
    timestamp: chrono::DateTime<Utc>,
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
        Self {
            slug: task.slug,
            user: task.user,
            title: task.title,
            instruction: task.instruction,
            is_active: task.is_active,
            schedule: task.schedule,
            last_executed_at: task.last_executed_at,
            timestamp: task.timestamp,
        }
    }
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
) -> models::response::Response<Vec<TaskResponse>> {
    if !state.is_agent_exists(&agent_id).await {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    match state
        .storage
        .get_task_list(Some(agent_id), params.is_active)
        .await
    {
        Ok(tasks) => {
            let response: Vec<TaskResponse> = tasks.into_iter().map(TaskResponse::from).collect();
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
) -> models::response::Response<TaskResponse> {
    if !state.is_agent_exists(&agent_id).await {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    match state.storage.get_task_list(Some(agent_id), None).await {
        Ok(tasks) => {
            if let Some(task) = tasks.into_iter().find(|t| t.slug == slug) {
                api_response(StatusCode::OK, TaskResponse::from(task))
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
    Json(body): Json<CreateTaskRequest>,
) -> models::response::Response<TaskResponse> {
    if !state.is_agent_exists(&agent_id).await {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    // Validate schedule
    if let Err(err) = validate_schedule(&body.schedule) {
        return err_response(StatusCode::BAD_REQUEST, err);
    }

    let task = Task {
        slug: body.slug,
        user: body.user,
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
    Json(body): Json<CreateTaskRequest>,
) -> models::response::Response<TaskResponse> {
    if !state.is_agent_exists(&agent_id).await {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
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
        user: body.user,
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
) -> models::response::Response<String> {
    if !state.is_agent_exists(&agent_id).await {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    match state.storage.delete_task(agent_id, slug.clone()).await {
        Ok(_) => api_response(StatusCode::OK, format!("task {slug} deleted")),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}
