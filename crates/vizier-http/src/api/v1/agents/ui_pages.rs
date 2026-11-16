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
        ui_pages::{self, UiFrameTicket, UiPageView},
    },
};

pub fn ui_page_routes() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topic/{topic_id}/pages/{page_id}", get(get_ui_page))
        .route(
            "/{channel_id}/topic/{topic_id}/pages/{page_id}/versions/{version}",
            get(get_ui_page_version),
        )
        .route(
            "/{channel_id}/topic/{topic_id}/pages/{page_id}/versions/{version}/frame",
            get(get_ui_frame_ticket),
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
        (status = 200, description = "The version's HTML source as text/plain (it runs only in its frame); a version never changes, so it may be cached for good"),
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
                (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                (header::CACHE_CONTROL, "private, max-age=31536000, immutable"),
                // The agent's HTML as text, never as a document of the WebUI's origin: a page
                // runs only in its frame (`ui_frame`).
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (header::CONTENT_SECURITY_POLICY, crate::ui_frame::FRAME_CSP),
            ],
            html,
        )
            .into_response(),
        Err(e) => fail::<String>(e).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/pages/{page_id}/versions/{version}/frame",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        ("page_id" = String, Path, description = "Page ID"),
        ("version" = u32, Path, description = "Version number, from 1")
    ),
    responses(
        (status = 200, description = "Where to load a frame of this version from: a ticket path that carries no credential and lasts ten minutes", body = APIResponse<UiFrameTicket>),
        (status = 404, description = "Page or version not found", body = APIResponse<String>)
    )
)]
pub async fn get_ui_frame_ticket(
    Path((agent_id, channel_id, topic_id, page_id, version)): Path<(String, String, TopicId, String, u32)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<UiFrameTicket> {
    match ui_pages::frame_ticket(&services(&state), &user, &agent_id, &channel_id, &topic_id, &page_id, version).await {
        Ok(ticket) => api_response(StatusCode::OK, ticket),
        Err(e) => fail(e),
    }
}

/// A page frame (`specs/063-html-generative-ui` research R7a): public, because the ticket is the
/// proof. The document is built here with the kit inlined and served with its own policy.
#[utoipa::path(
    get,
    path = "/ui-frames/{ticket}",
    params(("ticket" = String, Path, description = "A ticket from the frame route")),
    responses(
        (status = 200, description = "The page's frame document (text/html), sandboxed by its Content-Security-Policy header"),
        (status = 404, description = "Ticket forged, expired, or the version is gone", body = APIResponse<String>)
    )
)]
pub async fn ui_frame(Path(ticket): Path<String>, State(state): State<HTTPState>) -> Response {
    let frame = match ui_pages::frame_html(&services(&state), &ticket).await {
        Ok(frame) => frame,
        Err(e) => return fail::<String>(e).into_response(),
    };
    let read = |path: &str| crate::webui::kit_text(path);
    let kit = crate::webui::kit_text("manifest.json")
        .and_then(|text| serde_json::from_str(&text).ok())
        .map(|manifest| crate::ui_frame::Kit { manifest, read: &read });
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
            (header::CONTENT_SECURITY_POLICY, crate::ui_frame::FRAME_CSP),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        crate::ui_frame::document(
            &frame.html,
            kit.as_ref(),
            &serde_json::json!({ "pageId": frame.page_id, "version": frame.version, "state": frame.state }),
        ),
    )
        .into_response()
}
