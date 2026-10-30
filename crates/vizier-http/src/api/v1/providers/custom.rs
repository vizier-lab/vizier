//! Operator-defined providers (contract `http-api.md` § Custom providers): an OpenAI-compatible
//! or Anthropic endpoint at any address, with the operator's own model list. Keys and header
//! values are write-only.

use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use reqwest::StatusCode;

use super::{DeleteQuery, InUse, fail, fail_data, usage};
use crate::{
    auth::Principal,
    models::{self, response::{APIResponse, api_response}},
    state::HTTPState,
};
use vizier_core::service::custom_providers as service;

pub use service::{CustomProviderBody, CustomProviderResponse};

pub fn custom() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/test", post(test))
        .route("/{id}", axum::routing::put(update).delete(remove))
        .route("/{id}/fetch-models", post(fetch_models))
}

#[utoipa::path(
    get,
    path = "/providers/custom",
    responses((status = 200, description = "Custom providers, without secrets", body = APIResponse<Vec<CustomProviderResponse>>))
)]
pub(crate) async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<Vec<CustomProviderResponse>> {
    match service::list(&state.deps, &user).await {
        Ok(out) => api_response(StatusCode::OK, out),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    post,
    path = "/providers/custom",
    request_body = CustomProviderBody,
    responses(
        (status = 200, description = "Created", body = APIResponse<CustomProviderResponse>),
        (status = 400, description = "Invalid id or URL", body = APIResponse<String>),
        (status = 409, description = "The id is taken; the message names by what", body = APIResponse<String>)
    )
)]
pub(crate) async fn create(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    axum::Json(body): axum::Json<CustomProviderBody>,
) -> models::response::Response<CustomProviderResponse> {
    match service::create(&state.deps, &user, body).await {
        Ok(p) => api_response(StatusCode::OK, p),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    put,
    path = "/providers/custom/{id}",
    params(("id" = String, Path, description = "Custom provider id")),
    request_body = CustomProviderBody,
    responses(
        (status = 200, description = "Updated", body = APIResponse<CustomProviderResponse>),
        (status = 400, description = "A different id, or an invalid URL", body = APIResponse<String>)
    )
)]
pub(crate) async fn update(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    axum::Json(body): axum::Json<CustomProviderBody>,
) -> models::response::Response<CustomProviderResponse> {
    match service::update(&state.deps, &user, &id, body).await {
        Ok(p) => api_response(StatusCode::OK, p),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    delete,
    path = "/providers/custom/{id}",
    params(("id" = String, Path, description = "Custom provider id"), DeleteQuery),
    responses(
        (status = 200, description = "Deleted", body = APIResponse<String>),
        (status = 409, description = "Agents use this provider; `data` lists them", body = APIResponse<InUse>)
    )
)]
pub(crate) async fn remove(
    Path(id): Path<String>,
    Query(query): Query<DeleteQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<serde_json::Value> {
    match service::remove(&state.deps, &user, &id, query.confirm).await {
        Ok(()) => api_response(StatusCode::OK, "deleted".into()),
        Err(e) => fail_data(e),
    }
}

#[utoipa::path(
    post,
    path = "/providers/custom/{id}/fetch-models",
    params(("id" = String, Path, description = "Custom provider id")),
    responses(
        (status = 200, description = "The provider with the server's models merged in", body = APIResponse<CustomProviderResponse>),
        (status = 502, description = "The server could not be reached; nothing changed", body = APIResponse<String>)
    )
)]
pub(crate) async fn fetch_models(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<CustomProviderResponse> {
    match service::fetch_models(&state.deps, &user, &id).await {
        Ok(p) => api_response(StatusCode::OK, p),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    post,
    path = "/providers/custom/test",
    request_body = CustomProviderBody,
    responses((status = 200, description = "`ok` says whether the server answered", body = APIResponse<usage::TestResult>))
)]
pub(crate) async fn test(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    axum::Json(body): axum::Json<CustomProviderBody>,
) -> models::response::Response<usage::TestResult> {
    match service::test(&state.deps, &user, body).await {
        Ok(result) => api_response(StatusCode::OK, result),
        Err(e) => fail(e),
    }
}
