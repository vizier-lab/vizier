//! Detached scripts of a WebUI topic (spec 058 contracts/http-and-frames.md). Read-only: only
//! the agent cancels in this feature. The rules live in `vizier_core::service::detached`.

use axum::{
    Extension, Router,
    extract::{Path, State},
    routing::get,
};
use reqwest::StatusCode;

use crate::{
    auth::Principal,
    models::{
        self,
        response::{APIResponse, api_response, err_response},
    },
    state::HTTPState,
};
use vizier_core::{
    schema::{DetachedScriptDetail, DetachedScriptSnapshot, TopicId},
    service::{ServiceError, Services, detached},
};

pub fn detached_scripts() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topic/{topic_id}/detached-scripts", get(list_detached_scripts))
        .route(
            "/{channel_id}/topic/{topic_id}/detached-scripts/{script_id}",
            get(get_detached_script),
        )
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

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/detached-scripts",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    responses(
        (status = 200, description = "The topic's detached scripts: the running ones, then the last 50 that ended, oldest first", body = APIResponse<Vec<DetachedScriptSnapshot>>),
        (status = 404, description = "Conversation not found", body = APIResponse<String>)
    )
)]
pub async fn list_detached_scripts(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<Vec<DetachedScriptSnapshot>> {
    match detached::list(&services(&state), &user, &agent_id, &channel_id, &topic_id).await {
        Ok(scripts) => api_response(StatusCode::OK, scripts),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/detached-scripts/{script_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        ("script_id" = String, Path, description = "Script ID, e.g. d_k3x9a1b2")
    ),
    responses(
        (status = 200, description = "One script with its code, the calls it made and, once final, its report", body = APIResponse<DetachedScriptDetail>),
        (status = 404, description = "Script not found", body = APIResponse<String>)
    )
)]
pub async fn get_detached_script(
    Path((agent_id, channel_id, topic_id, script_id)): Path<(String, String, TopicId, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<DetachedScriptDetail> {
    match detached::get(&services(&state), &user, &agent_id, &channel_id, &topic_id, &script_id).await {
        Ok(script) => api_response(StatusCode::OK, script),
        Err(e) => fail(e),
    }
}
