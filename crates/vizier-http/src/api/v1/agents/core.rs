use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use reqwest::StatusCode;

use crate::{
    error::ApiError,
    models::response::{APIResponse, api_response},
    state::HTTPState,
};
use vizier_core::{
    schema::RollbackResponse,
    service::{self, Services},
};

// Deprecated: identity is the memory document `core/soul` (specs/043-unify-core-and-memory,
// FR-034). Every operation here is a projection over that one document, so it cannot disagree
// with the memory endpoints (FR-035). Removal is tracked in issue #82; it is deferred, not
// cancelled.

pub use vizier_core::service::core::{
    CoreConflict, CoreContentResponse, CoreHistoryQuery, CoreHistoryResponse, CoreUpdateResponse,
    RollbackRequest, UpdateCoreRequest,
};

type Reply<T> = Result<(StatusCode, Json<APIResponse<T>>), ApiError>;

pub fn core() -> Router<HTTPState> {
    Router::new()
        .route("/", get(get_core).put(update_core))
        .route("/history", get(get_core_history).post(rollback_core))
}

fn services(state: &HTTPState) -> Services {
    Services::new(state.deps.clone())
}

/// **Deprecated.** Read the identity document at `GET /agents/{agent_id}/memory/doc/core/soul`
/// instead; this is a projection over that same document (specs/043-unify-core-and-memory).
#[utoipa::path(
    get,
    path = "/agents/{agent_id}/core",
    description = "DEPRECATED — identity is the memory document `core/soul`. Use `GET /agents/{agent_id}/memory/doc/core/soul` instead. Kept working with unchanged response shapes; it reads the same document, so the two can never disagree.",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Get CORE content", body = APIResponse<CoreContentResponse>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn get_core(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Reply<CoreContentResponse> {
    let content = service::core::get(&services(&state), &user, &agent_id).await?;
    Ok(api_response(StatusCode::OK, content))
}

/// **Deprecated.** Write the identity document at `PUT /agents/{agent_id}/memory/doc/core/soul`
/// instead; this is a projection over that same document.
#[utoipa::path(
    put,
    path = "/agents/{agent_id}/core",
    description = "DEPRECATED — identity is the memory document `core/soul`. Use `PUT /agents/{agent_id}/memory/doc/core/soul` instead. `base_seq` still refuses a stale save with 409.",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = UpdateCoreRequest,
    responses(
        (status = 200, description = "CORE updated successfully", body = APIResponse<CoreUpdateResponse>),
        (status = 409, description = "`base_seq` is not the current revision (`stale_revision`); `data` holds the current one", body = APIResponse<CoreConflict>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn update_core(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<UpdateCoreRequest>,
) -> Reply<serde_json::Value> {
    let updated = service::core::update(&services(&state), &user, &agent_id, body).await?;
    Ok(api_response(StatusCode::OK, updated))
}

/// **Deprecated.** Use the memory revision endpoints for `core/soul` instead; this projects the
/// same `memory_revision` rows.
#[utoipa::path(
    get,
    path = "/agents/{agent_id}/core/history",
    description = "DEPRECATED — this projects the memory revision history of `core/soul`. Use the memory revision endpoints for `core/soul` instead.",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        CoreHistoryQuery
    ),
    responses(
        (status = 200, description = "Paginated list (no `seq`/`to`), one version (`seq`), or a line diff (`to`, optional `from`; `from` defaults to `to - 1`)", body = APIResponse<CoreHistoryResponse>),
        (status = 404, description = "Agent, CORE, or version not found", body = APIResponse<String>)
    )
)]
pub async fn get_core_history(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Query(q): Query<CoreHistoryQuery>,
) -> Reply<CoreHistoryResponse> {
    let history = service::core::history(&services(&state), &user, &agent_id, q).await?;
    Ok(api_response(StatusCode::OK, history))
}

/// **Deprecated.** Use the memory rollback endpoint for `core/soul` instead; this restores
/// through the same write path.
#[utoipa::path(
    post,
    path = "/agents/{agent_id}/core/history",
    description = "DEPRECATED — this restores a memory revision of `core/soul`. Use the memory rollback endpoint for `core/soul` instead.",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = RollbackRequest,
    responses(
        (status = 200, description = "Version restored as a new revision (or `no_change` when already identical)", body = APIResponse<RollbackResponse>),
        (status = 404, description = "Agent or version not found", body = APIResponse<String>)
    )
)]
pub async fn rollback_core(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<RollbackRequest>,
) -> Reply<RollbackResponse> {
    let res = service::core::rollback(&services(&state), &user, &agent_id, body).await?;
    Ok(api_response(StatusCode::OK, res))
}
