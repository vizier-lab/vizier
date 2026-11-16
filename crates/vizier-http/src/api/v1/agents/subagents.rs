//! The workers a WebUI topic hired (spec 059 US7). Read-only: only the agent stops a worker in
//! this feature. The rules live in `vizier_core::service::subagents`.

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
    schema::{SessionHistory, SubagentSnapshot, TopicId},
    service::{ServiceError, Services, subagents},
};

pub fn subagent_routes() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topic/{topic_id}/subagents", get(list_subagents))
        .route("/{channel_id}/topic/{topic_id}/subagents/{subagent_id}", get(get_subagent))
        .route(
            "/{channel_id}/topic/{topic_id}/subagents/{subagent_id}/history",
            get(subagent_history),
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
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/subagents",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    responses(
        (status = 200, description = "The topic's workers: the running and asking ones, then the last 50 that ended, oldest first", body = APIResponse<Vec<SubagentSnapshot>>),
        (status = 404, description = "Conversation not found", body = APIResponse<String>)
    )
)]
pub async fn list_subagents(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<Vec<SubagentSnapshot>> {
    match subagents::list(&services(&state), &user, &agent_id, &channel_id, &topic_id).await {
        Ok(rows) => api_response(StatusCode::OK, rows),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/subagents/{subagent_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        ("subagent_id" = String, Path, description = "Worker ID, e.g. s_k3x9a1b2")
    ),
    responses(
        (status = 200, description = "One worker with its whole task and result", body = APIResponse<SubagentSnapshot>),
        (status = 404, description = "Worker not found", body = APIResponse<String>)
    )
)]
pub async fn get_subagent(
    Path((agent_id, channel_id, topic_id, subagent_id)): Path<(String, String, TopicId, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<SubagentSnapshot> {
    match subagents::get(&services(&state), &user, &agent_id, &channel_id, &topic_id, &subagent_id).await {
        Ok(row) => api_response(StatusCode::OK, row),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/subagents/{subagent_id}/history",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        ("subagent_id" = String, Path, description = "Worker ID, e.g. s_k3x9a1b2")
    ),
    responses(
        (status = 200, description = "The worker's own conversation, in stored order", body = APIResponse<Vec<SessionHistory>>),
        (status = 404, description = "Worker not found", body = APIResponse<String>)
    )
)]
pub async fn subagent_history(
    Path((agent_id, channel_id, topic_id, subagent_id)): Path<(String, String, TopicId, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<Vec<SessionHistory>> {
    match subagents::history(&services(&state), &user, &agent_id, &channel_id, &topic_id, &subagent_id)
        .await
    {
        Ok(rows) => api_response(StatusCode::OK, rows),
        Err(e) => fail(e),
    }
}
