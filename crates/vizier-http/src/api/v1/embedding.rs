use axum::{Json, Router, extract::State, routing::get};
use reqwest::StatusCode;

use crate::{models::{self, response::api_response}, state::HTTPState};

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct LocalEmbeddingModel {
    pub variant: String,
    pub name: String,
    pub tier: String,
}

pub fn embedding() -> Router<HTTPState> {
    Router::new().route("/local", get(list_local_embedding_models))
}

#[utoipa::path(
    get,
    path = "/embedding-models/local",
    responses(
        (status = 200, description = "List of local embedding models", body = models::response::APIResponse<Vec<LocalEmbeddingModel>>)
    )
)]
pub async fn list_local_embedding_models(
    State(_state): State<HTTPState>,
) -> models::response::Response<Vec<LocalEmbeddingModel>> {
    let models: Vec<LocalEmbeddingModel> = vizier_core::service::embedding::local_models()
        .into_iter()
        .map(|(name, tier)| LocalEmbeddingModel { variant: name.clone(), name, tier })
        .collect();
    api_response(StatusCode::OK, models)
}
