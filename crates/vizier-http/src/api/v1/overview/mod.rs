//! Home's data (`specs/024-webui-rewrite/contracts/http-api.md` § Overview): the counts for the
//! greeting and the "Last 24 hours" dot grid. Everything is limited to the agents the signed-in
//! person can see. The old feed panel and its `/feed` endpoint are gone (spec 029); the inbox is
//! `/api/v1/notifications`.

use axum::{
    Extension, Router,
    extract::{Query, State},
    routing::get,
};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    api::v1::agents::dream::parse_offset,
    auth::Principal,
    models::{
        self,
        response::{APIResponse, api_response},
    },
    state::HTTPState,
};
pub use vizier_core::service::overview::{
    Activity, ActivityRow, AgentCounts, Overview, Rank, TodayByKind,
};
use vizier_core::service::overview;

pub fn overview() -> Router<HTTPState> {
    Router::new()
        .route("/", get(overview_counts))
        .route("/activity", get(activity))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TzQuery {
    /// The person's UTC offset, e.g. `+07:00`; default UTC.
    pub tz: Option<String>,
}

#[utoipa::path(
    get,
    path = "/overview",
    params(TzQuery),
    responses((status = 200, description = "Counts and today's spend across the agents the caller can see", body = APIResponse<Overview>))
)]
pub async fn overview_counts(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Query(query): Query<TzQuery>,
) -> models::response::Response<Overview> {
    let offset = parse_offset(query.tz.as_deref());
    match overview::counts(&super::auth::services(&state), &user, offset).await {
        Ok(counts) => api_response(StatusCode::OK, counts),
        Err(e) => super::auth::err(e),
    }
}

// Last 24 hours

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ActivityQuery {
    pub rank: Option<Rank>,
}

#[utoipa::path(
    get,
    path = "/overview/activity",
    params(ActivityQuery, TzQuery),
    responses((status = 200, description = "Turns per hour for the last 24 hours, for the agents the caller can see", body = APIResponse<Activity>))
)]
pub async fn activity(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Query(query): Query<ActivityQuery>,
    Query(tz): Query<TzQuery>,
) -> models::response::Response<Activity> {
    let offset = parse_offset(tz.tz.as_deref());
    match overview::activity(&super::auth::services(&state), &user, query.rank.unwrap_or_default(), offset).await {
        Ok(activity) => api_response(StatusCode::OK, activity),
        Err(e) => super::auth::err(e),
    }
}
