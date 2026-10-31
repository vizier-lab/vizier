//! Agent-scoped project routes (`specs/040-projects-mode/contracts/http-api.md`): list, open,
//! detail, rename/shell switch, soft removal and the caller's sessions. Authorization and
//! validation are in `vizier_core::service::projects`; these handlers only translate.

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{get, post, put},
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
    CreateProjectSessionResponse, CreateWorktreeRequest, CreateWorktreeResponse, FileSuggestion,
    OpenProjectRequest, ProjectDetail, ProjectRemoved, ProjectSession, ProjectSkillRow,
    ProjectSummary, SetModeRequest, SetModeResponse, SwitchBranchRequest, UpdateProjectRequest,
    WorktreeDeleted, WorktreeDetail, WorktreeSummary,
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
        .route(
            "/{project_id}/sessions/{topic}/mode",
            put(set_mode),
        )
        .route("/{project_id}/files", get(files))
        .route("/{project_id}/skills", get(skills))
        .route("/{project_id}/git", get(git_summary))
        .route("/{project_id}/git/changes", get(git_changes))
        .route("/{project_id}/git/diff", get(git_diff))
        .route("/{project_id}/git/branches", get(git_branches))
        .route("/{project_id}/git/switch", post(git_switch))
        .route(
            "/{project_id}/worktrees",
            get(list_worktrees).post(create_worktree),
        )
        .route(
            "/{project_id}/worktrees/{id}",
            get(worktree_detail).delete(delete_worktree),
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


#[utoipa::path(
    put,
    path = "/agents/{agent_id}/projects/{project_id}/sessions/{topic}/mode",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID"),
        ("topic" = String, Path, description = "Chat topic")
    ),
    request_body = SetModeRequest,
    responses(
        (status = 200, description = "The chat now uses this mode from the next turn", body = APIResponse<SetModeResponse>),
        (status = 403, description = "The caller may not use this project"),
        (status = 404, description = "Agent or project not found, or removed")
    )
)]
pub async fn set_mode(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id, topic)): Path<(String, String, String)>,
    Json(body): Json<SetModeRequest>,
) -> Response {
    respond(
        service::set_mode(&svc(&state), &user, &agent_id, &project_id, &topic, body).await,
        StatusCode::OK,
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct FilesQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}/files",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID"),
        ("q" = Option<String>, Query, description = "What the person typed after @"),
        ("topic" = Option<String>, Query, description = "The chat, for a worktree folder"),
        ("limit" = Option<usize>, Query, description = "1–50, default 20")
    ),
    responses(
        (status = 200, description = "Matching files and folders", body = APIResponse<Vec<FileSuggestion>>),
        (status = 403, description = "The caller may not use this project"),
        (status = 404, description = "Agent or project not found, or removed"),
        (status = 409, description = "`project_unavailable`")
    )
)]
pub async fn files(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
    Query(query): Query<FilesQuery>,
) -> Response {
    respond(
        service::file_suggestions(
            &svc(&state),
            &user,
            &agent_id,
            &project_id,
            query.q.as_deref().unwrap_or(""),
            query.topic.as_deref(),
            query.limit.unwrap_or(20),
        )
        .await,
        StatusCode::OK,
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct SkillsQuery {
    #[serde(default)]
    pub topic: Option<String>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}/skills",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID"),
        ("topic" = Option<String>, Query, description = "The chat, so a worktree offers its own branch's skills")
    ),
    responses(
        (status = 200, description = "The skills this chat's folder carries, one per slug", body = APIResponse<Vec<ProjectSkillRow>>),
        (status = 403, description = "The caller may not use this project"),
        (status = 404, description = "Agent or project not found, or removed"),
        (status = 409, description = "`project_unavailable`")
    )
)]
pub async fn skills(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
    Query(query): Query<SkillsQuery>,
) -> Response {
    respond(
        service::skills(&svc(&state), &user, &agent_id, &project_id, query.topic.as_deref()).await,
        StatusCode::OK,
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct GitQuery {
    #[serde(default)]
    pub topic: Option<String>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}/git",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID"),
        ("topic" = Option<String>, Query, description = "The chat, for a worktree folder")
    ),
    responses(
        (status = 200, description = "The branch and change count; `available: false` without git", body = APIResponse<vizier_core::projects::git::GitSummary>),
        (status = 403, description = "The caller may not use this project"),
        (status = 404, description = "Agent or project not found, or removed")
    )
)]
pub async fn git_summary(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
    Query(query): Query<GitQuery>,
) -> Response {
    respond(
        service::git_summary(&svc(&state), &user, &agent_id, &project_id, query.topic.as_deref())
            .await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}/git/changes",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID"),
        ("topic" = Option<String>, Query, description = "The chat, for a worktree folder")
    ),
    responses(
        (status = 200, description = "Every uncommitted change with line counts", body = APIResponse<Vec<vizier_core::projects::git::GitChange>>),
        (status = 409, description = "`git_unavailable`")
    )
)]
pub async fn git_changes(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
    Query(query): Query<GitQuery>,
) -> Response {
    respond(
        service::git_changes(&svc(&state), &user, &agent_id, &project_id, query.topic.as_deref())
            .await,
        StatusCode::OK,
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct GitDiffQuery {
    #[serde(default)]
    pub topic: Option<String>,
    pub path: String,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}/git/diff",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID"),
        ("topic" = Option<String>, Query, description = "The chat, for a worktree folder"),
        ("path" = String, Query, description = "The changed file, relative to the folder")
    ),
    responses(
        (status = 200, description = "The file's diff against the last commit", body = APIResponse<vizier_core::projects::git::GitFileDiff>),
        (status = 409, description = "`git_unavailable`"),
        (status = 422, description = "`invalid_path`")
    )
)]
pub async fn git_diff(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
    Query(query): Query<GitDiffQuery>,
) -> Response {
    respond(
        service::git_file_diff(
            &svc(&state),
            &user,
            &agent_id,
            &project_id,
            query.topic.as_deref(),
            &query.path,
        )
        .await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}/git/branches",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID"),
        ("topic" = Option<String>, Query, description = "The chat, for a worktree folder")
    ),
    responses(
        (status = 200, description = "Local branches, with the worktree holding each", body = APIResponse<Vec<vizier_core::projects::git::GitBranch>>),
        (status = 409, description = "`git_unavailable`")
    )
)]
pub async fn git_branches(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
    Query(query): Query<GitQuery>,
) -> Response {
    respond(
        service::git_branches(&svc(&state), &user, &agent_id, &project_id, query.topic.as_deref())
            .await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/projects/{project_id}/git/switch",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID")
    ),
    request_body = SwitchBranchRequest,
    responses(
        (status = 200, description = "The folder's new branch", body = APIResponse<vizier_core::projects::git::GitSummary>),
        (status = 403, description = "Switching the main folder needs managing the project"),
        (status = 409, description = "`turn_active`, `branch_checked_out`, `local_changes` or `branch_exists`"),
        (status = 422, description = "`invalid_branch`")
    )
)]
pub async fn git_switch(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
    Json(body): Json<SwitchBranchRequest>,
) -> Response {
    respond(
        service::git_switch(&svc(&state), &user, &agent_id, &project_id, body).await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}/worktrees",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "The project's worktrees", body = APIResponse<Vec<WorktreeSummary>>),
        (status = 403, description = "The caller may not use this project")
    )
)]
pub async fn list_worktrees(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
) -> Response {
    respond(
        service::list_worktrees(&svc(&state), &user, &agent_id, &project_id).await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/projects/{project_id}/worktrees",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID")
    ),
    request_body = CreateWorktreeRequest,
    responses(
        (status = 201, description = "The worktree and the one chat opened in it", body = APIResponse<CreateWorktreeResponse>),
        (status = 409, description = "`git_unavailable`, `name_in_use`, `branch_exists`, `path_exists`"),
        (status = 422, description = "`invalid_name`, `invalid_branch` or `unknown_base`")
    )
)]
pub async fn create_worktree(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id)): Path<(String, String)>,
    Json(body): Json<CreateWorktreeRequest>,
) -> Response {
    respond(
        service::create_worktree(&svc(&state), &user, &agent_id, &project_id, body).await,
        StatusCode::CREATED,
    )
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/projects/{project_id}/worktrees/{id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID"),
        ("id" = String, Path, description = "Worktree ID")
    ),
    responses(
        (status = 200, description = "Availability, change counts and whether the branch is merged", body = APIResponse<WorktreeDetail>),
        (status = 404, description = "Worktree not found")
    )
)]
pub async fn worktree_detail(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id, id)): Path<(String, String, String)>,
) -> Response {
    respond(
        service::worktree_detail(&svc(&state), &user, &agent_id, &project_id, &id).await,
        StatusCode::OK,
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct DeleteWorktreeQuery {
    #[serde(default)]
    pub force: Option<bool>,
    #[serde(default)]
    pub delete_branch: Option<bool>,
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/projects/{project_id}/worktrees/{id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("project_id" = String, Path, description = "Project ID"),
        ("id" = String, Path, description = "Worktree ID"),
        ("force" = Option<bool>, Query, description = "Discard uncommitted changes"),
        ("delete_branch" = Option<bool>, Query, description = "Also delete the branch")
    ),
    responses(
        (status = 200, description = "The chat and folder were removed", body = APIResponse<WorktreeDeleted>),
        (status = 409, description = "`turn_active` or `worktree_dirty {changed, untracked}`")
    )
)]
pub async fn delete_worktree(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, project_id, id)): Path<(String, String, String)>,
    Query(query): Query<DeleteWorktreeQuery>,
) -> Response {
    respond(
        service::delete_worktree(
            &svc(&state),
            &user,
            &agent_id,
            &project_id,
            &id,
            query.force.unwrap_or(false),
            query.delete_branch.unwrap_or(false),
        )
        .await,
        StatusCode::OK,
    )
}
