//! Background jobs launched from a WebUI topic (`specs/012-background-subagent-results/
//! contracts/http-api.md`). Authorization follows the topic routes: the session is always
//! built from the caller's own username, and a job is visible only when it was launched
//! from that very session — any other id is a 404, so a caller cannot tell it exists.

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    agents::background::{CancelOutcome, CancelScope},
    channels::http::{
        auth::AuthenticatedUser,
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    schema::{
        BackgroundJob, BackgroundJobSnapshot, Canceller, SessionHistory, SessionHistoryContent,
        TopicId, VizierChannelId, VizierSession,
    },
    storage::{
        agent::AgentStorage, background_job::BackgroundJobStorage, history::HistoryStorage,
    },
};

use super::{channel::HistoryQuery, user_can_view_agent};

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

/// The caller's own topic session, after the same agent check as the topic routes.
async fn topic_session<T: serde::Serialize + Clone>(
    state: &HTTPState,
    user: AuthenticatedUser,
    agent_id: String,
    channel_id: String,
    topic_id: TopicId,
) -> Result<VizierSession, models::response::Response<T>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return Err(err_response(
                StatusCode::NOT_FOUND,
                format!("agent {agent_id} not found"),
            ));
        }
        Err(e) => return Err(err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    if !user_can_view_agent(&user, &config) {
        return Err(err_response(StatusCode::FORBIDDEN, "Access denied".into()));
    }

    Ok(VizierSession(
        agent_id,
        VizierChannelId::HTTP(user.username, channel_id),
        Some(topic_id),
    ))
}

/// A job launched from `session`, or `None` for any other id.
async fn owned_job(
    state: &HTTPState,
    session: &VizierSession,
    job_id: &str,
) -> anyhow::Result<Option<BackgroundJob>> {
    Ok(state
        .storage
        .get_background_job(job_id)
        .await?
        .filter(|job| &job.origin == session))
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
    Extension(user): Extension<AuthenticatedUser>,
) -> models::response::Response<Vec<BackgroundJobSnapshot>> {
    let session = match topic_session(&state, user, agent_id, channel_id, topic_id).await {
        Ok(session) => session,
        Err(response) => return response,
    };

    match state.storage.list_running_background_jobs(session).await {
        Ok(jobs) => api_response(
            StatusCode::OK,
            jobs.iter().map(BackgroundJobSnapshot::from).collect(),
        ),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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
    Extension(user): Extension<AuthenticatedUser>,
) -> models::response::Response<BackgroundJobSnapshot> {
    let session = match topic_session(&state, user, agent_id, channel_id, topic_id).await {
        Ok(session) => session,
        Err(response) => return response,
    };

    match owned_job(&state, &session, &job_id).await {
        Ok(Some(job)) => api_response(StatusCode::OK, BackgroundJobSnapshot::from(&job)),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "Job not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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
    Extension(user): Extension<AuthenticatedUser>,
    body: Option<Json<CancelJobBody>>,
) -> models::response::Response<BackgroundJobSnapshot> {
    let username = user.username.clone();
    let session = match topic_session(&state, user, agent_id, channel_id, topic_id).await {
        Ok(session) => session,
        Err(response) => return response,
    };
    let reason = body
        .and_then(|Json(body)| body.reason)
        .filter(|reason| !reason.trim().is_empty());

    let outcome = state
        .background_jobs
        .cancel(
            &job_id,
            Canceller::Person(username),
            reason,
            CancelScope::Origin(session.clone()),
        )
        .await;

    match outcome {
        Ok(CancelOutcome::Cancelled { job, .. }) => {
            // Recorded for the person, not the agent: `Command` entries never reach the
            // model, so the cancel does not wake it.
            if let Err(e) = state
                .storage
                .save_session_history(
                    session,
                    SessionHistoryContent::Command(format!("cancelled background job {}", job.id)),
                )
                .await
            {
                tracing::warn!("failed to record the cancel of {}: {}", job.id, e);
            }
            api_response(StatusCode::OK, BackgroundJobSnapshot::from(&job))
        }
        Ok(CancelOutcome::AlreadyFinished(job)) => {
            api_response(StatusCode::CONFLICT, BackgroundJobSnapshot::from(&job))
        }
        Ok(CancelOutcome::NotFound) => err_response(StatusCode::NOT_FOUND, "Job not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.0),
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
    Extension(user): Extension<AuthenticatedUser>,
) -> models::response::Response<Vec<SessionHistory>> {
    let session = match topic_session(&state, user, agent_id, channel_id, topic_id).await {
        Ok(session) => session,
        Err(response) => return response,
    };

    let job = match owned_job(&state, &session, &job_id).await {
        Ok(Some(job)) => job,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Job not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    // The piece's session comes from the stored row, never from the caller.
    let Some(piece) = job.pieces.into_iter().find(|piece| piece.ordinal == ordinal) else {
        return err_response(StatusCode::NOT_FOUND, "Piece not found".into());
    };

    match state
        .storage
        .list_session_history(piece.session, params.before, params.before_seq, params.limit)
        .await
    {
        Ok(history) => api_response(StatusCode::OK, history),
        Err(_) => err_response(StatusCode::NOT_FOUND, "Not found".into()),
    }
}
