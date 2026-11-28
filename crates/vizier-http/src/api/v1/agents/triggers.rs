//! An agent's triggers for people (`specs/071-triggers/contracts/http-api.md`). Authorization and
//! validation are in `vizier_core::service::triggers`; these handlers translate.

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::{get, post},
};
use serde::Deserialize;
use vizier_core::{
    auth::Principal,
    schema::{TriggerEvent, TriggerRun},
    service::triggers::{self as service, CreatedTrigger, KindResponse, RunDetail, TriggerInput, TriggerResponse, TriggerSummary},
};

use crate::{
    api::v1::integrations::{failure, ok, svc},
    state::HTTPState,
};

pub fn triggers() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{slug}", get(read).put(update).delete(remove))
        .route("/{slug}/approve", post(approve))
        .route("/{slug}/secret", post(replace_secret))
        .route("/{slug}/test", post(test))
        .route("/{slug}/events", get(events))
        .route("/{slug}/runs", get(runs))
        .route("/{slug}/runs/{run_id}", get(run_detail))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct PageQuery {
    /// Continue after the item with this time (RFC 3339) and id.
    pub before: Option<String>,
    pub before_id: Option<i64>,
    /// At most 200; 50 when left out.
    pub limit: Option<usize>,
}

impl PageQuery {
    fn cursor(&self) -> Option<(vizier_core::time::Timestamp, i64)> {
        let at = chrono::DateTime::parse_from_rfc3339(self.before.as_deref()?).ok()?;
        Some((at.with_timezone(&chrono::Local), self.before_id.unwrap_or(i64::MAX)))
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ApproveRequest {
    /// The triage model, when the trigger has triage: `[provider, model]`.
    pub triage_model: Option<(String, String)>,
}

#[utoipa::path(get, path = "/trigger-kinds", responses((status = 200, body = Vec<KindResponse>)))]
pub async fn kinds(State(state): State<HTTPState>) -> Response {
    ok(StatusCode::OK, service::kinds(&state.deps))
}

#[utoipa::path(
    get, path = "/agents/{agent_id}/trigger-summary",
    params(("agent_id" = String, Path)),
    responses((status = 200, body = TriggerSummary), (status = 403), (status = 404))
)]
pub async fn summary(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
) -> Response {
    match service::summary(&svc(&state), &user, &agent_id).await {
        Ok(summary) => ok(StatusCode::OK, summary),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    get, path = "/agents/{agent_id}/triggers",
    params(("agent_id" = String, Path)),
    responses((status = 200, body = Vec<TriggerResponse>), (status = 403), (status = 404))
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
) -> Response {
    match service::list(&svc(&state), &user, &agent_id).await {
        Ok(items) => ok(StatusCode::OK, items),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    post, path = "/agents/{agent_id}/triggers",
    params(("agent_id" = String, Path)),
    request_body = TriggerInput,
    responses(
        (status = 201, description = "Created. A webhook's secret is in this response only", body = CreatedTrigger),
        (status = 400), (status = 403), (status = 409)
    )
)]
pub async fn create(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Json(input): Json<TriggerInput>,
) -> Response {
    match service::create(&svc(&state), &user, &agent_id, input).await {
        Ok(created) => ok(StatusCode::CREATED, created),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    get, path = "/agents/{agent_id}/triggers/{slug}",
    params(("agent_id" = String, Path), ("slug" = String, Path)),
    responses((status = 200, body = TriggerResponse), (status = 403), (status = 404))
)]
pub async fn read(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, slug)): Path<(String, String)>,
) -> Response {
    match service::get(&svc(&state), &user, &agent_id, &slug).await {
        Ok(item) => ok(StatusCode::OK, item),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    put, path = "/agents/{agent_id}/triggers/{slug}",
    params(("agent_id" = String, Path), ("slug" = String, Path)),
    request_body = TriggerInput,
    responses((status = 200, body = TriggerResponse), (status = 400), (status = 403), (status = 404))
)]
pub async fn update(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, slug)): Path<(String, String)>,
    Json(input): Json<TriggerInput>,
) -> Response {
    match service::update(&svc(&state), &user, &agent_id, &slug, input).await {
        Ok(item) => ok(StatusCode::OK, item),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    delete, path = "/agents/{agent_id}/triggers/{slug}",
    params(("agent_id" = String, Path), ("slug" = String, Path)),
    responses((status = 204), (status = 403), (status = 404))
)]
pub async fn remove(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, slug)): Path<(String, String)>,
) -> Response {
    match service::delete(&svc(&state), &user, &agent_id, &slug).await {
        Ok(()) => ok(StatusCode::NO_CONTENT, ()),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    post, path = "/agents/{agent_id}/triggers/{slug}/approve",
    params(("agent_id" = String, Path), ("slug" = String, Path)),
    request_body = ApproveRequest,
    responses((status = 200, description = "Approved. A webhook's new secret is in this response only", body = CreatedTrigger), (status = 400), (status = 403), (status = 404))
)]
pub async fn approve(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, slug)): Path<(String, String)>,
    Json(body): Json<ApproveRequest>,
) -> Response {
    match service::approve(&svc(&state), &user, &agent_id, &slug, body.triage_model).await {
        Ok(item) => ok(StatusCode::OK, item),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    post, path = "/agents/{agent_id}/triggers/{slug}/secret",
    params(("agent_id" = String, Path), ("slug" = String, Path)),
    responses((status = 200, description = "The new secret, in this response only", body = CreatedTrigger), (status = 403), (status = 404))
)]
pub async fn replace_secret(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, slug)): Path<(String, String)>,
) -> Response {
    match service::replace_secret(&svc(&state), &user, &agent_id, &slug).await {
        Ok(created) => ok(StatusCode::OK, created),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    get, path = "/agents/{agent_id}/triggers/{slug}/events",
    params(("agent_id" = String, Path), ("slug" = String, Path), PageQuery),
    responses((status = 200, body = Vec<TriggerEvent>), (status = 403), (status = 404))
)]
pub async fn events(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, slug)): Path<(String, String)>,
    Query(query): Query<PageQuery>,
) -> Response {
    match service::events(&svc(&state), &user, &agent_id, &slug, query.cursor(), query.limit).await {
        Ok(items) => ok(StatusCode::OK, items),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    get, path = "/agents/{agent_id}/triggers/{slug}/runs",
    params(("agent_id" = String, Path), ("slug" = String, Path), PageQuery),
    responses((status = 200, body = Vec<TriggerRun>), (status = 403), (status = 404))
)]
pub async fn runs(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, slug)): Path<(String, String)>,
    Query(query): Query<PageQuery>,
) -> Response {
    match service::runs(&svc(&state), &user, &agent_id, &slug, query.cursor(), query.limit).await {
        Ok(items) => ok(StatusCode::OK, items),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    get, path = "/agents/{agent_id}/triggers/{slug}/runs/{run_id}",
    params(("agent_id" = String, Path), ("slug" = String, Path), ("run_id" = i64, Path)),
    responses((status = 200, body = RunDetail), (status = 403), (status = 404))
)]
pub async fn run_detail(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, slug, run_id)): Path<(String, String, i64)>,
) -> Response {
    match service::run_detail(&svc(&state), &user, &agent_id, &slug, run_id).await {
        Ok(detail) => ok(StatusCode::OK, detail),
        Err(e) => failure(e, None),
    }
}

#[utoipa::path(
    post, path = "/agents/{agent_id}/triggers/{slug}/test",
    params(("agent_id" = String, Path), ("slug" = String, Path)),
    responses(
        (status = 200, description = "What the monitor's script returned. Nothing fires and no state is saved, but the script's tool calls really happen"),
        (status = 400), (status = 403), (status = 404)
    )
)]
pub async fn test(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, slug)): Path<(String, String)>,
) -> Response {
    match service::test(&svc(&state), &user, &agent_id, &slug).await {
        Ok(outcome) => ok(StatusCode::OK, outcome),
        Err(e) => failure(e, None),
    }
}
