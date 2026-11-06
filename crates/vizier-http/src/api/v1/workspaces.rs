//! Workspace routes (`specs/053-workspace-channel/contracts/http-api.md`). Authorization and
//! validation are in `vizier_core::service::workspaces`; these handlers only translate. A person
//! with no standing in a Workspace gets `404` from every route, as for one that does not exist.

use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::Response,
    routing::{delete, get, post},
};
use reqwest::StatusCode;
use serde::Serialize;

use vizier_core::{
    auth::Principal,
    service::{ServiceError, workspaces as service},
};

use crate::{
    api::v1::integrations::{fail, ok, svc},
    models::response::APIResponse,
    state::HTTPState,
};

pub use vizier_core::service::workspaces::{
    AddMemberRequest, CreateTopicRequest, CreateWorkspaceRequest, MemberView, TopicSummary,
    WorkspaceDetail, WorkspaceSummary,
};

pub fn workspaces() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail))
        .route("/{id}/topics", post(create_topic))
        .route("/{id}/members", post(add_member))
        .route("/{id}/members/{kind}/{handle}", delete(remove_member))
}

/// A service refusal in the existing envelope, with its stable `code` in `data.error`.
fn refusal(e: ServiceError) -> Response {
    let status = StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let code = match e.code() {
        Some(code) => code.to_string(),
        None => match status {
            StatusCode::NOT_FOUND => "not_found".into(),
            StatusCode::FORBIDDEN => "forbidden".into(),
            _ => "error".into(),
        },
    };
    fail(status, e.message().to_string(), serde_json::json!({ "error": code }))
}

fn respond<T: Serialize>(result: Result<T, ServiceError>, status: StatusCode) -> Response {
    match result {
        Ok(value) => ok(status, value),
        Err(e) => refusal(e),
    }
}

#[utoipa::path(
    get,
    path = "/workspaces",
    responses((status = 200, description = "The caller's Workspaces, with their topics", body = APIResponse<Vec<WorkspaceSummary>>))
)]
pub async fn list(State(state): State<HTTPState>, Extension(user): Extension<Principal>) -> Response {
    respond(service::list(&svc(&state), &user).await, StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/workspaces",
    request_body = CreateWorkspaceRequest,
    responses(
        (status = 201, description = "Created, with a first topic and the caller as owner", body = APIResponse<WorkspaceDetail>),
        (status = 403, description = "Missing the `workspaces:create` permission"),
        (status = 409, description = "`name_in_use`"),
        (status = 422, description = "`invalid_name`")
    )
)]
pub async fn create(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Json(body): Json<CreateWorkspaceRequest>,
) -> Response {
    respond(service::create(&svc(&state), &user, body).await, StatusCode::CREATED)
}

#[utoipa::path(
    get,
    path = "/workspaces/{id}",
    params(("id" = String, Path, description = "Workspace ID")),
    responses(
        (status = 200, description = "The Workspace with its members", body = APIResponse<WorkspaceDetail>),
        (status = 404, description = "No such Workspace, or the caller has no standing in it")
    )
)]
pub async fn detail(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
) -> Response {
    respond(service::detail(&svc(&state), &user, &id).await, StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/workspaces/{id}/topics",
    params(("id" = String, Path, description = "Workspace ID")),
    request_body = CreateTopicRequest,
    responses(
        (status = 201, description = "Created", body = APIResponse<TopicSummary>),
        (status = 404, description = "No such Workspace, or the caller has no standing in it"),
        (status = 409, description = "`name_in_use`"),
        (status = 422, description = "`invalid_name`")
    )
)]
pub async fn create_topic(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
    Json(body): Json<CreateTopicRequest>,
) -> Response {
    respond(service::create_topic(&svc(&state), &user, &id, body).await, StatusCode::CREATED)
}

#[utoipa::path(
    post,
    path = "/workspaces/{id}/members",
    params(("id" = String, Path, description = "Workspace ID")),
    request_body = AddMemberRequest,
    responses(
        (status = 201, description = "Added", body = APIResponse<MemberView>),
        (status = 403, description = "Not an owner, or `cannot_edit_agent`"),
        (status = 404, description = "No such Workspace, person or agent"),
        (status = 409, description = "`handle_in_use`")
    )
)]
pub async fn add_member(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
    Json(body): Json<AddMemberRequest>,
) -> Response {
    respond(service::add_member(&svc(&state), &user, &id, body).await, StatusCode::CREATED)
}

#[utoipa::path(
    delete,
    path = "/workspaces/{id}/members/{kind}/{handle}",
    params(
        ("id" = String, Path, description = "Workspace ID"),
        ("kind" = String, Path, description = "`person` or `agent`"),
        ("handle" = String, Path, description = "Username or agent ID")
    ),
    responses(
        (status = 200, description = "Removed"),
        (status = 403, description = "Not an owner"),
        (status = 404, description = "No such Workspace or member"),
        (status = 409, description = "`last_owner`")
    )
)]
pub async fn remove_member(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((id, kind, handle)): Path<(String, String, String)>,
) -> Response {
    respond(
        service::remove_member(&svc(&state), &user, &id, &kind, &handle).await,
        StatusCode::OK,
    )
}
