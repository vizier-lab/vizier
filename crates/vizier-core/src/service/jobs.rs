//! Background jobs launched from a WebUI topic (`specs/012-background-subagent-results/
//! contracts/http-api.md`). Authorization follows the topic routes: the session is always
//! built from the caller's own username, and a job is visible only when it was launched
//! from that very session — any other id is a 404, so a caller cannot tell it exists.

use chrono::{DateTime, Utc};

use super::{ServiceError, Services};
use crate::{
    access::can_view_agent,
    agents::background::{CancelOutcome, CancelScope},
    auth::Principal,
    schema::{
        BackgroundJob, BackgroundJobSnapshot, Canceller, SessionHistory, SessionHistoryContent, TopicId,
        VizierChannelId, VizierSession,
    },
    storage::{agent::AgentStorage, background_job::BackgroundJobStorage, history::HistoryStorage},
};

/// What a cancel did. `AlreadyFinished` carries the job's final state.
pub enum CancelResult {
    Cancelled(BackgroundJobSnapshot),
    AlreadyFinished(BackgroundJobSnapshot),
}

/// The caller's own topic session, after the same agent check as the topic routes.
async fn topic_session(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
) -> Result<VizierSession, ServiceError> {
    let config = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound(format!("agent {agent_id} not found"))),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };

    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    super::topics::authorize_project_channel(s, who, &config, agent_id, channel_id).await?;

    Ok(VizierSession(
        agent_id.to_string(),
        VizierChannelId::HTTP(who.username.clone(), channel_id.to_string()),
        Some(topic_id.clone()),
    ))
}

/// A job launched from `session`, or `None` for any other id.
async fn owned_job(
    s: &Services,
    session: &VizierSession,
    job_id: &str,
) -> anyhow::Result<Option<BackgroundJob>> {
    Ok(s.deps()
        .storage
        .get_background_job(job_id)
        .await?
        .filter(|job| &job.origin == session))
}

/// In-flight jobs launched from this topic, oldest first.
pub async fn list(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
) -> Result<Vec<BackgroundJobSnapshot>, ServiceError> {
    let session = topic_session(s, who, agent_id, channel_id, topic_id).await?;
    match s.deps().storage.list_running_background_jobs(session).await {
        Ok(jobs) => Ok(jobs.iter().map(BackgroundJobSnapshot::from).collect()),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

/// The job, in any state.
pub async fn get(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    job_id: &str,
) -> Result<BackgroundJobSnapshot, ServiceError> {
    let session = topic_session(s, who, agent_id, channel_id, topic_id).await?;
    match owned_job(s, &session, job_id).await {
        Ok(Some(job)) => Ok(BackgroundJobSnapshot::from(&job)),
        Ok(None) => Err(ServiceError::NotFound("Job not found".into())),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

/// Cancels a job launched from this topic. A blank `reason` counts as none.
pub async fn cancel(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    job_id: &str,
    reason: Option<String>,
) -> Result<CancelResult, ServiceError> {
    let session = topic_session(s, who, agent_id, channel_id, topic_id).await?;
    let reason = reason.filter(|reason| !reason.trim().is_empty());

    let outcome = s
        .deps()
        .background_jobs
        .cancel(
            job_id,
            Canceller::Person(who.username.clone()),
            reason,
            CancelScope::Origin(session.clone()),
        )
        .await;

    match outcome {
        Ok(CancelOutcome::Cancelled { job, .. }) => {
            // Recorded for the person, not the agent: `Command` entries never reach the
            // model, so the cancel does not wake it.
            if let Err(e) = s
                .deps()
                .storage
                .save_session_history(
                    session,
                    SessionHistoryContent::Command(format!("cancelled background job {}", job.id)),
                )
                .await
            {
                tracing::warn!("failed to record the cancel of {}: {}", job.id, e);
            }
            Ok(CancelResult::Cancelled(BackgroundJobSnapshot::from(&job)))
        }
        Ok(CancelOutcome::AlreadyFinished(job)) => {
            Ok(CancelResult::AlreadyFinished(BackgroundJobSnapshot::from(&job)))
        }
        Ok(CancelOutcome::NotFound) => Err(ServiceError::NotFound("Job not found".into())),
        Err(e) => Err(ServiceError::Internal(e.0)),
    }
}

/// One piece's own conversation.
#[allow(clippy::too_many_arguments)]
pub async fn piece_history(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    job_id: &str,
    ordinal: u32,
    before: Option<DateTime<Utc>>,
    before_seq: Option<i64>,
    limit: Option<usize>,
) -> Result<Vec<SessionHistory>, ServiceError> {
    let session = topic_session(s, who, agent_id, channel_id, topic_id).await?;

    let job = match owned_job(s, &session, job_id).await {
        Ok(Some(job)) => job,
        Ok(None) => return Err(ServiceError::NotFound("Job not found".into())),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    // The piece's session comes from the stored row, never from the caller.
    let Some(piece) = job.pieces.into_iter().find(|piece| piece.ordinal == ordinal) else {
        return Err(ServiceError::NotFound("Piece not found".into()));
    };

    s.deps()
        .storage
        .list_session_history(piece.session, before, before_seq, limit)
        .await
        .map_err(|_| ServiceError::NotFound("Not found".into()))
}
