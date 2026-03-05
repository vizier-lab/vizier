use axum::extract::{Path, State};
use reqwest::StatusCode;

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response},
        },
        state::HTTPState,
    },
    database::schema::Memory,
};

pub async fn get_all_memories(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<serde_json::Value>> {
    match state
        .db
        .conn
        .query("SELECT slug, title, timestamp FROM type::table(memory)")
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
    Path(slug): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<Option<serde_json::Value>> {
    match state
        .db
        .conn
        .query("SELECT slug, title, content, timestamp FROM type::table(memory) WHERE slug = $slug")
        .bind(("slug", slug))
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
    Path(slug): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<String> {
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
