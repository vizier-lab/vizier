use axum::{
    Extension, Router,
    extract::{Path, State},
    routing::get,
    Json,
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
    storage::agent::AgentStorage,
    utils::build_path,
};

use super::user_can_view_agent;

pub fn documents() -> Router<HTTPState> {
    Router::new()
        .route("/agent", get(get_agent_doc).put(update_agent_doc))
        .route("/identity", get(get_identity_doc).put(update_identity_doc))
        .route("/heartbeat", get(get_heartbeat_doc).put(update_heartbeat_doc))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateDocumentRequest {
    content: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct DocumentContentResponse {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct DocumentUpdateResponse {
    pub message: String,
}

fn get_document_path(workspace: &str, agent_id: &str, doc_name: &str) -> String {
    build_path(workspace, &["agents", agent_id, doc_name]).to_string_lossy().to_string()
}

fn read_document(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}

fn write_document(path: &str, content: &str) -> Result<(), std::io::Error> {
    std::fs::write(path, content)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/documents/agent",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Get SOUL.md content", body = APIResponse<DocumentContentResponse>),
        (status = 404, description = "Agent or document not found", body = APIResponse<String>)
    )
)]
pub async fn get_agent_doc(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<DocumentContentResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let path = get_document_path(&state.config.workspace, &agent_id, "SOUL.md");
    match read_document(&path) {
        Ok(content) => api_response(StatusCode::OK, DocumentContentResponse { content }),
        Err(e) => err_response(StatusCode::NOT_FOUND, format!("failed to read SOUL.md: {}", e)),
    }
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/documents/agent",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = UpdateDocumentRequest,
    responses(
        (status = 200, description = "SOUL.md updated successfully", body = APIResponse<DocumentUpdateResponse>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn update_agent_doc(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<UpdateDocumentRequest>,
) -> models::response::Response<DocumentUpdateResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let path = get_document_path(&state.config.workspace, &agent_id, "SOUL.md");
    match write_document(&path, &body.content) {
        Ok(_) => api_response(StatusCode::OK, DocumentUpdateResponse { message: "SOUL.md updated successfully".to_string() }),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("failed to update SOUL.md: {}", e)),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/documents/identity",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Get IDENTITY.md content", body = APIResponse<DocumentContentResponse>),
        (status = 404, description = "Agent or document not found", body = APIResponse<String>)
    )
)]
pub async fn get_identity_doc(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<DocumentContentResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let path = get_document_path(&state.config.workspace, &agent_id, "IDENTITY.md");
    match read_document(&path) {
        Ok(content) => api_response(StatusCode::OK, DocumentContentResponse { content }),
        Err(e) => err_response(StatusCode::NOT_FOUND, format!("failed to read IDENTITY.md: {}", e)),
    }
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/documents/identity",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = UpdateDocumentRequest,
    responses(
        (status = 200, description = "IDENTITY.md updated successfully", body = APIResponse<DocumentUpdateResponse>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn update_identity_doc(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<UpdateDocumentRequest>,
) -> models::response::Response<DocumentUpdateResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let path = get_document_path(&state.config.workspace, &agent_id, "IDENTITY.md");
    match write_document(&path, &body.content) {
        Ok(_) => api_response(StatusCode::OK, DocumentUpdateResponse { message: "IDENTITY.md updated successfully".to_string() }),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("failed to update IDENTITY.md: {}", e)),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/documents/heartbeat",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Get HEARTBEAT.md content", body = APIResponse<DocumentContentResponse>),
        (status = 404, description = "Agent or document not found", body = APIResponse<String>)
    )
)]
pub async fn get_heartbeat_doc(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<DocumentContentResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let path = get_document_path(&state.config.workspace, &agent_id, "HEARTBEAT.md");
    match read_document(&path) {
        Ok(content) => api_response(StatusCode::OK, DocumentContentResponse { content }),
        Err(e) => err_response(StatusCode::NOT_FOUND, format!("failed to read HEARTBEAT.md: {}", e)),
    }
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/documents/heartbeat",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = UpdateDocumentRequest,
    responses(
        (status = 200, description = "HEARTBEAT.md updated successfully", body = APIResponse<DocumentUpdateResponse>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn update_heartbeat_doc(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<UpdateDocumentRequest>,
) -> models::response::Response<DocumentUpdateResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let path = get_document_path(&state.config.workspace, &agent_id, "HEARTBEAT.md");
    match write_document(&path, &body.content) {
        Ok(_) => api_response(StatusCode::OK, DocumentUpdateResponse { message: "HEARTBEAT.md updated successfully".to_string() }),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("failed to update HEARTBEAT.md: {}", e)),
    }
}