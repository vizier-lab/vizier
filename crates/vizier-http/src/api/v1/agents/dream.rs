use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{models::{self, response::{api_response, err_response, APIResponse}}, state::HTTPState};
use vizier_core::{
    schema::dream_journal::{DreamJournalEntry, DreamNights},
    service::{ServiceError, Services, dream},
};

pub use vizier_core::service::dream::{DreamCycles, DreamStatusResponse, parse_offset};

pub fn dream() -> Router<HTTPState> {
    Router::new()
        .route("/trigger", post(trigger_dream))
        .route("/status", get(dream_status))
        .route("/nights", get(dream_nights))
        .route("/cycles", get(dream_cycles))
        .route("/journal", get(get_dream_journal))
        .route("/journal/{entry_id}", get(get_dream_entry))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct DreamJournalQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
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
    post,
    path = "/agents/{agent_id}/dream/trigger",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Dream cycle started", body = APIResponse<String>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn trigger_dream(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<String> {
    match dream::trigger(&services(&state), &user, &agent_id).await {
        Ok(message) => api_response(StatusCode::OK, message),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/status",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Dream status", body = APIResponse<DreamStatusResponse>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn dream_status(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<DreamStatusResponse> {
    match dream::status(&services(&state), &user, &agent_id).await {
        Ok(response) => api_response(StatusCode::OK, response),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/journal",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("limit" = Option<usize>, Query, description = "Max entries"),
        ("offset" = Option<usize>, Query, description = "Offset")
    ),
    responses(
        (status = 200, description = "Dream journal entries", body = APIResponse<Vec<DreamJournalEntry>>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn get_dream_journal(
    Path(agent_id): Path<String>,
    Query(query): Query<DreamJournalQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<Vec<DreamJournalEntry>> {
    match dream::journal(&services(&state), &user, &agent_id, query.limit, query.offset).await {
        Ok(entries) => api_response(StatusCode::OK, entries),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/journal/{entry_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("entry_id" = String, Path, description = "Entry ID")
    ),
    responses(
        (status = 200, description = "Dream journal entry", body = APIResponse<DreamJournalEntry>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Entry not found", body = APIResponse<String>)
    )
)]
pub async fn get_dream_entry(
    Path((agent_id, entry_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<DreamJournalEntry> {
    match dream::entry(&services(&state), &user, &agent_id, &entry_id).await {
        Ok(entry) => api_response(StatusCode::OK, entry),
        Err(e) => fail(e),
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct NightsQuery {
    /// 1–90; default 30.
    pub days: Option<u32>,
    /// The person's UTC offset, e.g. `+07:00`; default UTC.
    pub tz: Option<String>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/nights",
    params(("agent_id" = String, Path, description = "Agent ID"), NightsQuery),
    responses(
        (status = 200, description = "One entry per night, oldest first, with what each dream changed", body = APIResponse<DreamNights>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn dream_nights(
    Path(agent_id): Path<String>,
    Query(query): Query<NightsQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<DreamNights> {
    match dream::nights(&services(&state), &user, &agent_id, query.days, query.tz.as_deref()).await {
        Ok(nights) => api_response(StatusCode::OK, nights),
        Err(e) => fail(e),
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct CyclesQuery {
    /// The `next` of the previous page: an RFC 3339 start time; cycles older than it follow.
    pub before: Option<DateTime<Utc>>,
    /// 1–50; default 20.
    pub limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/cycles",
    params(("agent_id" = String, Path, description = "Agent ID"), CyclesQuery),
    responses(
        (status = 200, description = "Dream cycles, newest first, with their changes and cost", body = APIResponse<DreamCycles>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn dream_cycles(
    Path(agent_id): Path<String>,
    Query(query): Query<CyclesQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<DreamCycles> {
    match dream::cycles(&services(&state), &user, &agent_id, query.before, query.limit).await {
        Ok(cycles) => api_response(StatusCode::OK, cycles),
        Err(e) => fail(e),
    }
}
