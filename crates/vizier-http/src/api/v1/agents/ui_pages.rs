//! Interactive pages of a WebUI topic (`specs/063-html-generative-ui` contracts/http.md).
//! Read-only: pages are made by the agent. The rules live in `vizier_core::service::ui_pages`.

use axum::{
    Extension, Router,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
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
    schema::TopicId,
    service::{
        ServiceError, Services,
        ui_pages::{self, UiPageView},
    },
};

pub fn ui_page_routes() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topic/{topic_id}/pages/{page_id}", get(get_ui_page))
        .route(
            "/{channel_id}/topic/{topic_id}/pages/{page_id}/versions/{version}",
            get(get_ui_page_version),
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
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/pages/{page_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        ("page_id" = String, Path, description = "Page ID, e.g. pg_3f9K…")
    ),
    responses(
        (status = 200, description = "The page, its versions oldest first and its saved state", body = APIResponse<UiPageView>),
        (status = 404, description = "Page not found", body = APIResponse<String>)
    )
)]
pub async fn get_ui_page(
    Path((agent_id, channel_id, topic_id, page_id)): Path<(String, String, TopicId, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<UiPageView> {
    match ui_pages::get_page(&services(&state), &user, &agent_id, &channel_id, &topic_id, &page_id).await {
        Ok(page) => api_response(StatusCode::OK, page),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/pages/{page_id}/versions/{version}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        ("page_id" = String, Path, description = "Page ID"),
        ("version" = u32, Path, description = "Version number, from 1")
    ),
    responses(
        (status = 200, description = "The version's HTML (text/html); a version never changes, so it may be cached for good"),
        (status = 404, description = "Page or version not found", body = APIResponse<String>)
    )
)]
pub async fn get_ui_page_version(
    Path((agent_id, channel_id, topic_id, page_id, version)): Path<(String, String, TopicId, String, u32)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> Response {
    match ui_pages::get_version(&services(&state), &user, &agent_id, &channel_id, &topic_id, &page_id, version)
        .await
    {
        Ok(html) => (
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "private, max-age=31536000, immutable"),
            ],
            html,
        )
            .into_response(),
        Err(e) => fail::<String>(e).into_response(),
    }
}
