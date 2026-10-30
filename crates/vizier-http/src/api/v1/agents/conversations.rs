//! The conversation picker's list (spec 024, FR-018e). The rules live in
//! `vizier_core::service::conversations`.

use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    routing::get,
};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    auth::Principal,
    models::{
        self,
        response::{APIResponse, api_response, err_response},
    },
    state::HTTPState,
};
use vizier_core::service::{Services, conversations};

pub use vizier_core::service::conversations::{
    ConversationCounts, ConversationItem, ConversationKind, ConversationPage,
};

pub fn conversations() -> Router<HTTPState> {
    Router::new().route("/", get(list))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ConversationQuery {
    /// Matches titles, and message text within the agent's newest 5,000 messages.
    pub q: Option<String>,
    pub channel: Option<ConversationKind>,
    /// The `next` of the previous page.
    pub cursor: Option<String>,
    /// 1–100; default 25.
    pub limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/conversations",
    params(("agent_id" = String, Path, description = "Agent ID"), ConversationQuery),
    description = "The agent's conversations on the WebUI (the caller's own), Discord and Telegram, newest first. `q` searches titles and the text of the agent's newest 5,000 messages only, so an older match may not be found.",
    responses(
        (status = 200, description = "One page of conversations", body = APIResponse<ConversationPage>),
        (status = 400, description = "A cursor that isn't one", body = APIResponse<String>),
        (status = 403, description = "The caller can't see this agent"),
        (status = 404, description = "No such agent")
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Query(query): Query<ConversationQuery>,
) -> models::response::Response<ConversationPage> {
    let services = Services::new(state.deps.clone());
    match conversations::list(
        &services,
        &user,
        &agent_id,
        query.q.as_deref(),
        query.channel,
        query.cursor.as_deref(),
        query.limit,
    )
    .await
    {
        Ok(page) => api_response(StatusCode::OK, page),
        Err(e) => err_response(
            StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            e.message().to_string(),
        ),
    }
}
