//! The workers a WebUI topic hired, for people (spec 059 US7). Read-only: only the agent stops a
//! worker in this feature. Authorization follows the detached-script routes: the hiring session
//! is built from the caller's own username, and a worker is visible only when it was hired from
//! that very session; any other id is a 404, so a caller cannot tell it exists.

use super::{ServiceError, Services, detached::topic_session};
use crate::{
    agents::subagents::LIST_FINAL_LIMIT,
    schema::{SessionHistory, Subagent, SubagentSnapshot, TopicId},
    storage::{history::HistoryStorage, subagent::SubagentStorage},
};

/// How many entries of a worker's own conversation one read returns at most.
pub const HISTORY_LIMIT: usize = 500;

fn internal(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Internal(e.to_string())
}

/// The topic's workers: the running and asking ones, then the last 50 final ones, oldest first.
pub async fn list(
    s: &Services,
    who: &crate::auth::Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
) -> Result<Vec<SubagentSnapshot>, ServiceError> {
    let session = topic_session(s, who, agent_id, channel_id, topic_id).await?;
    let rows = s
        .deps()
        .storage
        .list_subagents(&session, LIST_FINAL_LIMIT)
        .await
        .map_err(internal)?;
    Ok(rows.iter().map(Subagent::snapshot).collect())
}

/// The worker, when it was hired from the caller's topic.
async fn hired_here(
    s: &Services,
    who: &crate::auth::Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    subagent_id: &str,
) -> Result<Subagent, ServiceError> {
    let session = topic_session(s, who, agent_id, channel_id, topic_id).await?;
    match s.deps().storage.get_subagent(subagent_id).await {
        Ok(Some(row)) if row.hiring_session == session => Ok(row),
        Ok(_) => Err(ServiceError::NotFound("subagent not found".into())),
        Err(e) => Err(internal(e)),
    }
}

/// One worker with its whole task and result.
pub async fn get(
    s: &Services,
    who: &crate::auth::Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    subagent_id: &str,
) -> Result<SubagentSnapshot, ServiceError> {
    Ok(hired_here(s, who, agent_id, channel_id, topic_id, subagent_id)
        .await?
        .detail())
}

/// The worker's own conversation, in stored order: the steps its activity trail is drawn from.
pub async fn history(
    s: &Services,
    who: &crate::auth::Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    subagent_id: &str,
) -> Result<Vec<SessionHistory>, ServiceError> {
    let row = hired_here(s, who, agent_id, channel_id, topic_id, subagent_id).await?;
    s.deps()
        .storage
        .list_session_history(row.worker_session(), None, None, Some(HISTORY_LIMIT))
        .await
        .map_err(internal)
}
