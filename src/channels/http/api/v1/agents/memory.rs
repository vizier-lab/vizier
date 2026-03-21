use axum::{
    Router,
    extract::{Path, State},
    routing::{delete, get},
};
use reqwest::StatusCode;

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response},
        },
        state::HTTPState,
    },
    schema::Memory,
    storage::memory::MemoryStorage,
};

pub fn memory() -> Router<HTTPState> {
    Router::new()
        .route("/", get(get_all_memories))
        .route("/{slug}", get(get_memory_detail))
        .route("/{slug}", delete(delete_memory))
}

pub async fn get_all_memories(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<serde_json::Value>> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    match state.storage.get_all_agent_memory(agent_id).await {
        Ok(memory) => {
            let response = memory
                .iter()
                .map(|memory| {
                    serde_json::json!({
                        "agent_id": memory.agent_id,
                        "slug": memory.slug,
                        "title": memory.title,
                        "timestamp": memory.timestamp
                    })
                })
                .collect();

            api_response(StatusCode::OK, response)
        }
        _ => err_response(StatusCode::NOT_FOUND, "Not Found".into()),
    }
}

pub async fn get_memory_detail(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
) -> models::response::Response<serde_json::Value> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    match state.storage.get_memory_detail(agent_id, slug).await {
        Ok(Some(memory)) => api_response(
            StatusCode::OK,
            serde_json::json!({
                "agent_id": memory.agent_id,
                "slug": memory.slug,
                "title": memory.title,
                "timestamp": memory.timestamp
            }),
        ),
        _ => err_response(StatusCode::NOT_FOUND, "Not Found".into()),
    }
}

pub async fn delete_memory(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
) -> models::response::Response<String> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    match state.storage.delete_memory(agent_id, slug.clone()).await {
        Ok(_) => api_response(StatusCode::OK, format!("{slug} deleted")),
        _ => err_response(StatusCode::NOT_FOUND, "Not Found".into()),
    }
}
