//! Agent-scoped project routes (`specs/040-projects-mode/contracts/http-api.md`): list, open,
//! detail, rename/shell switch, soft removal and the caller's sessions. Authorization and
//! validation are in `vizier_core::service::projects`; these handlers only translate.

use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use reqwest::StatusCode;
use serde::Serialize;

use vizier_core::{
    auth::Principal,
    service::{ServiceError, projects as service},
};

use crate::{
    api::v1::integrations::{fail, ok, svc},
    models::response::APIResponse,
    state::HTTPState,
};

pub use vizier_core::service::projects::{
    CreateProjectSessionResponse, OpenProjectRequest, ProjectDetail, ProjectRemoved,
    ProjectSession, ProjectSummary, UpdateProjectRequest,
};

pub fn projects() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list).post(open))
        .route(
            "/{project_id}",
            get(detail).patch(update).delete(remove),
        )
        .route(
            "/{project_id}/sessions",
            get(sessions).post(create_session),
        )
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
    path = "/agents/{agent_id}/projects",
    params(("agent_id" = String, Path, description = "Agent ID")),
    responses(
        (status = 200, description = "The agent's open projects, newest first", body = APIResponse<Vec<ProjectSummary>>),
        (status = 403, description = "The caller may not use this agent's projects"),
        (status = 404, description = "Agent not found")
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
) -> Response {
    respond(service::list(&svc(&state), &user, &agent_id).await, StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/projects",
    params(("agent_id" = String, Path, description = "Agent ID")),
    request_body = OpenProjectRequest,
    responses(
        (status = 201, description = "Project created", body = APIResponse<ProjectSummary>),
        (status = 200, description = "Existing project reused or restored", body = APIResponse<ProjectSummary>),
        (status = 403, description = "Missing permission, or not a desktop principal (`desktop_only`)"),
        (status = 404, description = "Agent not found"),
        (status = 422, description = "`invalid_folder` or `workspace_folder`")
    )
)]
pub async fn open(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Json(body): Json<OpenProjectRequest>,
) -> Response {
    match service::open(&svc(&state), &user, user.method, &agent_id, body).await {
        Ok(opened) => {
            let status = if opened.created { StatusCode::CREATED } else { StatusCode::OK };
            ok(status, opened.project)
        }
        Err(e) => refusal(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Project detail; the instruction text is never returned", body = APIResponse<ProjectDetail>),
        (status = 403, description = "The caller may not use this agent's projects"),
        (status = 404, description = "Agent or project not found, or removed")
    )
)]
pub async fn detail(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
) -> Response {
    respond(
        service::get(&svc(&state), &user, &agent_id, &project_id).await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    patch,
    path = "/agents/{agent_id}/projects/{project_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID")
    ),
    request_body = UpdateProjectRequest,
    responses(
        (status = 200, description = "Updated project", body = APIResponse<ProjectSummary>),
        (status = 403, description = "Not the project owner or a whole-workspace project manager"),
        (status = 404, description = "Agent or project not found, or removed"),
        (status = 422, description = "`invalid_name` or an empty update")
    )
)]
pub async fn update(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
    Json(body): Json<UpdateProjectRequest>,
) -> Response {
    respond(
        service::update(&svc(&state), &user, &agent_id, &project_id, body).await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/projects/{project_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Soft-removed; no file or history row was deleted", body = APIResponse<ProjectRemoved>),
        (status = 403, description = "Not the project owner or a whole-workspace project manager"),
        (status = 404, description = "Agent or project not found, or removed"),
        (status = 409, description = "A turn is running or queued in the project (`turn_active`)")
    )
)]
pub async fn remove(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
) -> Response {
    respond(
        service::remove(&svc(&state), &user, &agent_id, &project_id).await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}/sessions",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "The caller's sessions, newest activity first, empty last", body = APIResponse<Vec<ProjectSession>>),
        (status = 403, description = "The caller may not use this agent's projects"),
        (status = 404, description = "Agent or project not found, or removed")
    )
)]
pub async fn sessions(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
) -> Response {
    respond(
        service::sessions(&svc(&state), &user, &agent_id, &project_id).await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/projects/{project_id}/sessions",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID")
    ),
    responses(
        (status = 201, description = "An empty session titled New session", body = APIResponse<CreateProjectSessionResponse>),
        (status = 403, description = "The caller may not use this agent's projects"),
        (status = 404, description = "Agent or project not found, or removed")
    )
)]
pub async fn create_session(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
) -> Response {
    respond(
        service::create_session(&svc(&state), &user, &agent_id, &project_id).await,
        StatusCode::CREATED,
    )
}
