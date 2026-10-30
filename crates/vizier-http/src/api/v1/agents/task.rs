use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json,
};
use chrono::Utc;
use crate::models::page::{self, ListQuery, Listing, SortKey, SortOrder};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{models::{self, response::{api_response, err_response, APIResponse}}, state::HTTPState};
use vizier_core::{
    schema::{SessionHistory, TaskRunState},
    service::{ServiceError, Services, tasks},
};

pub use vizier_core::service::tasks::{
    CreateTaskRequest, ScheduleRequest, TaskResponse, TaskRunResponse, TaskRunsResponse,
};

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

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct GetTasksQuery {
    is_active: Option<bool>,
}

fn services(state: &HTTPState) -> Services {
    Services::new(state.deps.clone())
}

fn fail<T: serde::Serialize + Clone>(e: ServiceError) -> models::response::Response<T> {
    err_response(
        StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        e.message().to_string(),
    )
}

/// Filters `active` and `last_run_failed`; sorts `next_run` (soonest first, none last),
/// `name` and `last_run` (newest first).
struct TaskListing;

impl Listing<TaskResponse> for TaskListing {
    fn matches(&self, item: &TaskResponse, q: &str) -> bool {
        item.title.to_lowercase().contains(q) || item.slug.to_lowercase().contains(q)
    }

    fn filter_keys(&self, _: &[TaskResponse]) -> Vec<String> {
        vec!["active".into(), "last_run_failed".into()]
    }

    fn in_filter(&self, filter: &str, item: &TaskResponse) -> bool {
        match filter {
            "active" => item.is_active,
            "last_run_failed" => item
                .last_run
                .as_ref()
                .is_some_and(|r| matches!(r.state, TaskRunState::NoResponse | TaskRunState::Interrupted)),
            _ => false,
        }
    }

    fn sort_key(&self, item: &TaskResponse, sort: &str) -> SortKey {
        match sort {
            "name" => SortKey::text(&item.title),
            "last_run" => SortKey::Num(item.last_run.as_ref().map_or(0, |r| r.ran_at.timestamp_millis())),
            // No next run sorts after every real one.
            _ => SortKey::Num(item.next_run.map_or(i64::MAX, |t| t.timestamp_millis())),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["next_run", "name", "last_run"]
    }

    fn default_order(&self, sort: &str) -> SortOrder {
        if sort == "last_run" { SortOrder::Desc } else { SortOrder::Asc }
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/tasks",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ListQuery
    ),
    request_body = GetTasksQuery,
    responses(
        (status = 200, description = "List of tasks; with `limit`, a page `{items, total, counts}` (filters `active`, `last_run_failed`; sorts `next_run`, `name`, `last_run`)", body = APIResponse<Vec<TaskResponse>>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn get_tasks(
    Path(agent_id): Path<String>,
    Query(params): Query<GetTasksQuery>,
    Query(list): Query<ListQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<serde_json::Value> {
    match tasks::list(&services(&state), &user, &agent_id, params.is_active).await {
        Ok(response) => page::respond(response, &TaskListing, &list),
        Err(e) => fail(e),
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
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<TaskResponse> {
    match tasks::get(&services(&state), &user, &agent_id, &slug).await {
        Ok(task) => api_response(StatusCode::OK, task),
        Err(e) => fail(e),
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
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<CreateTaskRequest>,
) -> models::response::Response<TaskResponse> {
    match tasks::create(&services(&state), &user, &agent_id, body).await {
        Ok(task) => api_response(StatusCode::CREATED, task),
        Err(e) => fail(e),
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
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<CreateTaskRequest>,
) -> models::response::Response<TaskResponse> {
    match tasks::update(&services(&state), &user, &agent_id, &slug, body).await {
        Ok(task) => api_response(StatusCode::OK, task),
        Err(e) => fail(e),
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
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<String> {
    match tasks::delete(&services(&state), &user, &agent_id, &slug).await {
        Ok(message) => api_response(StatusCode::OK, message),
        Err(e) => fail(e),
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
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<TaskRunsResponse> {
    match tasks::list_runs(
        &services(&state),
        &user,
        &agent_id,
        &slug,
        params.before,
        params.before_id,
        params.limit,
    )
    .await
    {
        Ok(runs) => api_response(StatusCode::OK, runs),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/tasks/{slug}/runs/{run_id}/history",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Task slug"),
        ("run_id" = String, Path, description = "The run's address, as the run list gives it (RFC3339)")
    ),
    request_body = crate::api::v1::agents::channel::HistoryQuery,
    responses(
        (status = 200, description = "That run's full exchange", body = APIResponse<Vec<SessionHistory>>),
        (status = 403, description = "Not permitted to view the agent", body = APIResponse<String>),
        (status = 404, description = "Agent, task or run not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn get_run_history(
    Path((agent_id, slug, run_id)): Path<(String, String, chrono::DateTime<Utc>)>,
    Query(params): Query<crate::api::v1::agents::channel::HistoryQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<Vec<SessionHistory>> {
    match tasks::run_history(
        &services(&state),
        &user,
        &agent_id,
        &slug,
        run_id,
        params.before,
        params.before_seq,
        params.limit,
    )
    .await
    {
        Ok(history) => api_response(StatusCode::OK, history),
        Err(e) => fail(e),
    }
}

#[cfg(test)]
mod listing_tests {
    use super::*;
    use crate::models::page::paginate;
    use vizier_core::schema::{Requester, Task, TaskSchedule};
    use vizier_core::service::tasks::next_run_of;

    fn task(slug: &str, schedule: TaskSchedule, active: bool) -> Task {
        Task {
            slug: slug.into(),
            requester: Requester::User("me".into()),
            agent_id: "a".into(),
            title: slug.into(),
            instruction: "do".into(),
            is_active: active,
            schedule,
            last_executed_at: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn next_run_follows_the_schedule_and_activity() {
        let now = Utc::now();
        let ahead = now + chrono::Duration::hours(3);
        assert_eq!(next_run_of(&task("a", TaskSchedule::OneTimeTask(ahead), true), now), Some(ahead));
        assert_eq!(next_run_of(&task("a", TaskSchedule::OneTimeTask(now - chrono::Duration::hours(1)), true), now), None);
        assert_eq!(next_run_of(&task("a", TaskSchedule::OneTimeTask(ahead), false), now), None);
        let cron = next_run_of(&task("a", TaskSchedule::CronTask("0 * * * *".into()), true), now).unwrap();
        assert!(cron > now && cron <= now + chrono::Duration::hours(1));
    }

    #[test]
    fn lists_by_next_run_with_the_unscheduled_last() {
        let now = Utc::now();
        let items: Vec<TaskResponse> = vec![
            task("later", TaskSchedule::OneTimeTask(now + chrono::Duration::hours(9)), true),
            task("off", TaskSchedule::OneTimeTask(now + chrono::Duration::hours(1)), false),
            task("soon", TaskSchedule::OneTimeTask(now + chrono::Duration::hours(2)), true),
        ]
        .into_iter()
        .map(TaskResponse::from)
        .collect();
        let q = ListQuery { limit: Some(10), ..Default::default() };
        let page = paginate(items.clone(), &TaskListing, &q).ok().unwrap();
        assert_eq!(page.items.iter().map(|t| t.slug.as_str()).collect::<Vec<_>>(), ["soon", "later", "off"]);
        assert_eq!(page.counts["active"], 2);

        let q = ListQuery { filter: Some("active".into()), q: Some("LAT".into()), limit: Some(10), ..Default::default() };
        let page = paginate(items, &TaskListing, &q).ok().unwrap();
        assert_eq!(page.total, 1);
    }
}
