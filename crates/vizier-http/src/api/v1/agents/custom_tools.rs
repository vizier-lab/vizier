//! An agent's custom tools over HTTP (`specs/061-agent-custom-tools/contracts/http-api.md`).
//! People inspect, edit and govern; agents create, so there is no create or run route. The rules
//! live in `vizier_core::service::custom_tools`; refusals use the standard envelope, with the
//! agent tools' `problems` list in `data` for a `422 invalid`.

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    api::v1::integrations::{ok, svc},
    auth::Principal,
    error::ApiError,
    models::response::APIResponse,
    state::HTTPState,
};
use vizier_core::service::{ServiceError, custom_tools as service};

pub use service::{
    EditCustomToolRequest, RollbackCustomToolRequest, SetCustomToolEnabledRequest,
    UpdateCustomToolCategoryRequest,
};

/// `/agents/{agent_id}/custom-tools`.
pub fn custom_tools() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list))
        .route("/events", get(events))
        .route("/{tool}", get(detail).put(edit).delete(remove))
        .route("/{tool}/enabled", put(set_enabled))
        .route("/{tool}/versions", get(versions))
        .route("/{tool}/diff", get(diff))
        .route("/{tool}/rollback", post(rollback))
}

/// `/agents/{agent_id}/custom-tool-categories`.
pub fn custom_tool_categories() -> Router<HTTPState> {
    Router::new().route("/{name}", put(update_category))
}

fn reply(result: Result<serde_json::Value, ServiceError>) -> Response {
    match result {
        Ok(value) => ok(StatusCode::OK, value),
        Err(e) => ApiError(e).into_response(),
    }
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct VersionQuery {
    /// A past version; the current one when left out.
    pub version: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct PageQuery {
    /// Exclusive bound: a version number for versions, epoch milliseconds for events.
    pub before: Option<i64>,
    /// Default 50, max 200.
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct DiffQuery {
    pub from: i64,
    /// The current version when left out.
    pub to: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/custom-tools",
    params(("agent_id" = String, Path, description = "Agent ID")),
    responses(
        (status = 200, description = "Every custom tool, grouped by category (by name, `custom` last)", body = APIResponse<serde_json::Value>),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
) -> Response {
    reply(service::list(&svc(&state), &user, &agent_id).await)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/custom-tools/events",
    params(("agent_id" = String, Path, description = "Agent ID"), PageQuery),
    responses(
        (status = 200, description = "Switches, deletions and category edits, newest first", body = APIResponse<serde_json::Value>),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn events(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Query(page): Query<PageQuery>,
) -> Response {
    reply(service::events(&svc(&state), &user, &agent_id, page.before, page.limit).await)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/custom-tools/{tool}",
    params(("agent_id" = String, Path, description = "Agent ID"), ("tool" = String, Path, description = "Tool id or name"), VersionQuery),
    responses(
        (status = 200, description = "One version's definition, what it calls, who calls it and whether it needs attention", body = APIResponse<serde_json::Value>),
        (status = 404, description = "Agent or tool not found")
    )
)]
pub async fn detail(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, tool)): Path<(String, String)>,
    Query(query): Query<VersionQuery>,
) -> Response {
    reply(service::detail(&svc(&state), &user, &agent_id, &tool, query.version).await)
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/custom-tools/{tool}",
    params(("agent_id" = String, Path, description = "Agent ID"), ("tool" = String, Path, description = "Tool id or name")),
    request_body = EditCustomToolRequest,
    responses(
        (status = 200, description = "Saved as a new version authored by the caller", body = APIResponse<serde_json::Value>),
        (status = 403, description = "The caller may view but not edit this agent"),
        (status = 404, description = "Agent or tool not found"),
        (status = 409, description = "`out_of_date`: the tool changed since `expected_version`"),
        (status = 422, description = "`invalid`: every problem in `data.problems`")
    )
)]
pub async fn edit(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, tool)): Path<(String, String)>,
    Json(body): Json<EditCustomToolRequest>,
) -> Response {
    reply(service::edit(&svc(&state), &user, &agent_id, &tool, body).await)
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/custom-tools/{tool}",
    params(("agent_id" = String, Path, description = "Agent ID"), ("tool" = String, Path, description = "Tool id or name")),
    responses(
        (status = 204, description = "Deleted; the deletion stays in the events"),
        (status = 403, description = "The caller may view but not edit this agent"),
        (status = 404, description = "Agent or tool not found")
    )
)]
pub async fn remove(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, tool)): Path<(String, String)>,
) -> Response {
    match service::delete(&svc(&state), &user, &agent_id, &tool).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => ApiError(e).into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/custom-tools/{tool}/enabled",
    params(("agent_id" = String, Path, description = "Agent ID"), ("tool" = String, Path, description = "Tool id or name")),
    request_body = SetCustomToolEnabledRequest,
    responses(
        (status = 200, description = "Switched; a tool a person switched off stays off until a person switches it on", body = APIResponse<serde_json::Value>),
        (status = 403, description = "The caller may view but not edit this agent"),
        (status = 404, description = "Agent or tool not found")
    )
)]
pub async fn set_enabled(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, tool)): Path<(String, String)>,
    Json(body): Json<SetCustomToolEnabledRequest>,
) -> Response {
    reply(service::set_enabled(&svc(&state), &user, &agent_id, &tool, body.enabled).await)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/custom-tools/{tool}/versions",
    params(("agent_id" = String, Path, description = "Agent ID"), ("tool" = String, Path, description = "Tool id or name"), PageQuery),
    responses(
        (status = 200, description = "Versions, newest first, with `next` for the following page", body = APIResponse<serde_json::Value>),
        (status = 404, description = "Agent or tool not found")
    )
)]
pub async fn versions(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, tool)): Path<(String, String)>,
    Query(page): Query<PageQuery>,
) -> Response {
    reply(service::versions(&svc(&state), &user, &agent_id, &tool, page.before, page.limit).await)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/custom-tools/{tool}/diff",
    params(("agent_id" = String, Path, description = "Agent ID"), ("tool" = String, Path, description = "Tool id or name"), DiffQuery),
    responses(
        (status = 200, description = "Line diffs of description, input schema and script", body = APIResponse<serde_json::Value>),
        (status = 404, description = "Agent, tool or version not found")
    )
)]
pub async fn diff(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, tool)): Path<(String, String)>,
    Query(query): Query<DiffQuery>,
) -> Response {
    reply(service::diff(&svc(&state), &user, &agent_id, &tool, query.from, query.to).await)
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/custom-tools/{tool}/rollback",
    params(("agent_id" = String, Path, description = "Agent ID"), ("tool" = String, Path, description = "Tool id or name")),
    request_body = RollbackCustomToolRequest,
    responses(
        (status = 200, description = "The old version saved as a new one", body = APIResponse<serde_json::Value>),
        (status = 403, description = "The caller may view but not edit this agent"),
        (status = 404, description = "Agent, tool or version not found"),
        (status = 409, description = "`out_of_date`"),
        (status = 422, description = "`invalid`: the old version no longer passes the checks")
    )
)]
pub async fn rollback(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, tool)): Path<(String, String)>,
    Json(body): Json<RollbackCustomToolRequest>,
) -> Response {
    reply(service::rollback(&svc(&state), &user, &agent_id, &tool, body).await)
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/custom-tool-categories/{name}",
    params(("agent_id" = String, Path, description = "Agent ID"), ("name" = String, Path, description = "Category name")),
    request_body = UpdateCustomToolCategoryRequest,
    responses(
        (status = 200, description = "Renamed or redescribed; the agent sees it from its next turn", body = APIResponse<serde_json::Value>),
        (status = 403, description = "The caller may view but not edit this agent"),
        (status = 404, description = "Agent or category not found"),
        (status = 409, description = "`name_taken`")
    )
)]
pub async fn update_category(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, name)): Path<(String, String)>,
    Json(body): Json<UpdateCustomToolCategoryRequest>,
) -> Response {
    reply(service::update_category(&svc(&state), &user, &agent_id, &name, body).await)
}
