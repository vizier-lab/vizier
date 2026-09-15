use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
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
        CoreRevision, PaginatedCoreRevisions, RevisionDiff, RevisionOrigin, RollbackResponse,
    },
    storage::agent::AgentStorage,
};

use super::user_can_view_agent;

pub fn core() -> Router<HTTPState> {
    Router::new()
        .route("/", get(get_core).put(update_core))
        .route("/history", get(get_core_history).post(rollback_core))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateCoreRequest {
    content: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CoreContentResponse {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CoreUpdateResponse {
    pub message: String,
}

/// One `GET /history` handler dispatches on which query params are present:
/// `seq` ⇒ one version; `to` (± `from`) ⇒ diff; otherwise a paginated list.
#[derive(Debug, Deserialize, Default, utoipa::IntoParams)]
pub struct CoreHistoryQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub seq: Option<i64>,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RollbackRequest {
    pub seq: i64,
}

/// The three shapes `GET /history` can answer with, depending on the query.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum CoreHistoryResponse {
    List(PaginatedCoreRevisions),
    Revision(CoreRevision),
    Diff(RevisionDiff),
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/core",
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
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<CoreContentResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state.storage.get_agent_core(&agent_id).await {
        Ok(Some(content)) => api_response(StatusCode::OK, CoreContentResponse { content }),
        Ok(None) => api_response(StatusCode::OK, CoreContentResponse { content: String::new() }),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("failed to read CORE: {}", e)),
    }
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/core",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = UpdateCoreRequest,
    responses(
        (status = 200, description = "CORE updated successfully", body = APIResponse<CoreUpdateResponse>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn update_core(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<UpdateCoreRequest>,
) -> models::response::Response<CoreUpdateResponse> {
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
        .set_agent_core(&agent_id, &body.content, &RevisionOrigin::from_user(&user))
        .await
    {
        Ok(_) => api_response(StatusCode::OK, CoreUpdateResponse { message: "CORE updated successfully".to_string() }),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("failed to update CORE: {}", e)),
    }
}

async fn require_viewable_agent(
    state: &HTTPState,
    agent_id: &str,
    user: &crate::channels::http::auth::AuthenticatedUser,
) -> Result<(), (StatusCode, String)> {
    let config = match state.storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err((StatusCode::NOT_FOUND, format!("agent {agent_id} not found"))),
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    if !user_can_view_agent(user, &config) {
        return Err((StatusCode::FORBIDDEN, "Access denied".into()));
    }
    Ok(())
}

fn history_error_status(message: &str) -> StatusCode {
    if message.contains("unknown version") {
        StatusCode::NOT_FOUND
    } else if message.contains("deletion entry") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/core/history",
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
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Query(q): Query<CoreHistoryQuery>,
) -> models::response::Response<CoreHistoryResponse> {
    if let Err((status, message)) = require_viewable_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }

    if let Some(seq) = q.seq {
        return match state.storage.get_core_revision(&agent_id, seq).await {
            Ok(Some(rev)) => api_response(StatusCode::OK, CoreHistoryResponse::Revision(rev)),
            Ok(None) => err_response(StatusCode::NOT_FOUND, format!("unknown version {seq}")),
            Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
    }

    if let Some(to) = q.to {
        return match state.storage.diff_core_revisions(&agent_id, q.from, to).await {
            Ok(diff) => api_response(StatusCode::OK, CoreHistoryResponse::Diff(diff)),
            Err(e) => {
                let msg = e.to_string();
                err_response(history_error_status(&msg), msg)
            }
        };
    }

    match state
        .storage
        .list_core_revisions(&agent_id, q.offset.unwrap_or(0), q.limit.unwrap_or(50))
        .await
    {
        Ok(page) if page.total == 0 => {
            err_response(StatusCode::NOT_FOUND, "agent has no CORE".into())
        }
        Ok(page) => api_response(StatusCode::OK, CoreHistoryResponse::List(page)),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/core/history",
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
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<RollbackRequest>,
) -> models::response::Response<RollbackResponse> {
    if let Err((status, message)) = require_viewable_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }

    match state
        .storage
        .rollback_core(&agent_id, body.seq, &RevisionOrigin::from_user(&user))
        .await
    {
        Ok(res) => api_response(StatusCode::OK, res),
        Err(e) => {
            let msg = e.to_string();
            err_response(history_error_status(&msg), msg)
        }
    }
}
