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

    match state
        .db
        .conn
        .query("SELECT slug, title, timestamp FROM type::table(memory) WHERE agent_id = $agent_id")
        .bind(("agent_id", agent_id))
        .await
    {
        Ok(mut data) => {
            let response: Vec<serde_json::Value> = data.take(0).unwrap();

            api_response(StatusCode::OK, response)
        }
        _ => err_response(StatusCode::NOT_FOUND, "Not Found".into()),
    }
}

pub async fn get_memory_detail(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
) -> models::response::Response<Option<serde_json::Value>> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    match state
        .db
        .conn
        .query("SELECT slug, title, content, timestamp FROM type::table(memory) WHERE slug = $slug AND agent_id = $agent_id")
        .bind(("slug", slug))
        .bind(("agent_id", agent_id))
        .await
    {
        Ok(mut data) => {
            let response: Option<serde_json::Value> = data.take(0).unwrap();

            api_response(StatusCode::OK, response)
        }
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

    match state
        .db
        .conn
        .delete::<Option<Memory>>(("memory", slug.clone()))
        .await
    {
        Ok(_) => api_response(StatusCode::OK, format!("{slug} deleted")),
        _ => err_response(StatusCode::NOT_FOUND, "Not Found".into()),
    }
}
