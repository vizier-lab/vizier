//! Background jobs launched from a WebUI topic (`specs/012-background-subagent-results/
//! contracts/http-api.md`). The rules live in `vizier_core::service::jobs`.

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
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
use vizier_core::{
    schema::{BackgroundJobSnapshot, SessionHistory, TopicId},
    service::{ServiceError, Services, jobs},
};

use super::channel::HistoryQuery;

pub fn jobs() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topic/{topic_id}/jobs", get(list_jobs))
        .route("/{channel_id}/topic/{topic_id}/jobs/{job_id}", get(get_job))
        .route(
            "/{channel_id}/topic/{topic_id}/jobs/{job_id}/cancel",
            post(cancel_job),
        )
        .route(
            "/{channel_id}/topic/{topic_id}/jobs/{job_id}/pieces/{ordinal}/history",
            get(get_piece_history),
        )
}

#[derive(Debug, Default, Deserialize, utoipa::ToSchema)]
pub struct CancelJobBody {
    pub reason: Option<String>,
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
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/jobs",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    responses(
        (status = 200, description = "In-flight background jobs launched from this topic, oldest first", body = APIResponse<Vec<BackgroundJobSnapshot>>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn list_jobs(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<Vec<BackgroundJobSnapshot>> {
    match jobs::list(&services(&state), &user, &agent_id, &channel_id, &topic_id).await {
        Ok(jobs) => api_response(StatusCode::OK, jobs),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/jobs/{job_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        ("job_id" = String, Path, description = "Background job ID")
    ),
    responses(
        (status = 200, description = "The job, in any state", body = APIResponse<BackgroundJobSnapshot>),
        (status = 404, description = "No job with that id was launched from this topic", body = APIResponse<String>)
    )
)]
pub async fn get_job(
    Path((agent_id, channel_id, topic_id, job_id)): Path<(String, String, TopicId, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<BackgroundJobSnapshot> {
    match jobs::get(&services(&state), &user, &agent_id, &channel_id, &topic_id, &job_id).await {
        Ok(job) => api_response(StatusCode::OK, job),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/jobs/{job_id}/cancel",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        ("job_id" = String, Path, description = "Background job ID")
    ),
    request_body = CancelJobBody,
    responses(
        (status = 200, description = "The job, now cancelled", body = APIResponse<BackgroundJobSnapshot>),
        (status = 409, description = "The job already finished; its final state", body = APIResponse<BackgroundJobSnapshot>),
        (status = 404, description = "No job with that id was launched from this topic", body = APIResponse<String>)
    )
)]
pub async fn cancel_job(
    Path((agent_id, channel_id, topic_id, job_id)): Path<(String, String, TopicId, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    body: Option<Json<CancelJobBody>>,
) -> models::response::Response<BackgroundJobSnapshot> {
    let reason = body.and_then(|Json(body)| body.reason);
    match jobs::cancel(&services(&state), &user, &agent_id, &channel_id, &topic_id, &job_id, reason).await {
        Ok(jobs::CancelResult::Cancelled(job)) => api_response(StatusCode::OK, job),
        Ok(jobs::CancelResult::AlreadyFinished(job)) => api_response(StatusCode::CONFLICT, job),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/jobs/{job_id}/pieces/{ordinal}/history",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        ("job_id" = String, Path, description = "Background job ID"),
        ("ordinal" = u32, Path, description = "0-based piece position")
    ),
    request_body = HistoryQuery,
    responses(
        (status = 200, description = "The piece's own conversation", body = APIResponse<Vec<SessionHistory>>),
        (status = 404, description = "Unknown job or piece", body = APIResponse<String>)
    )
)]
pub async fn get_piece_history(
    Path((agent_id, channel_id, topic_id, job_id, ordinal)): Path<(
        String,
        String,
        TopicId,
        String,
        u32,
    )>,
    Query(params): Query<HistoryQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<Vec<SessionHistory>> {
    match jobs::piece_history(
        &services(&state),
        &user,
        &agent_id,
        &channel_id,
        &topic_id,
        &job_id,
        ordinal,
        params.before,
        params.before_seq,
        params.limit,
    )
    .await
    {
        Ok(history) => api_response(StatusCode::OK, history),
        Err(e) => fail(e),
    }
}
