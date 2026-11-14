//! Detached scripts of a WebUI topic, for people (spec 058 contracts/http-and-frames.md).
//! Read-only: only the agent cancels in this feature. Authorization follows the topic routes:
//! the session is built from the caller's own username, and a script is visible only when it
//! was launched from that very session; any other id is a 404, so a caller cannot tell it
//! exists.

use super::{ServiceError, Services};
use crate::{
    access::can_view_agent,
    auth::Principal,
    schema::{DetachedScriptDetail, DetachedScriptSnapshot, TopicId, VizierChannelId, VizierSession},
    storage::{agent::AgentStorage, detached_script::DetachedScriptStorage},
};

/// How many final scripts the list shows beside the running ones.
pub const LISTED_FINAL: usize = 50;

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
        return Err(ServiceError::NotFound(format!("agent {agent_id} not found")));
    }
    super::topics::authorize_project_channel(s, who, &config, agent_id, channel_id).await?;
    Ok(VizierSession(
        agent_id.to_string(),
        VizierChannelId::from_web(&who.username, channel_id),
        Some(topic_id.clone()),
    ))
}

/// The topic's scripts: the running ones and the last 50 final ones, oldest first.
pub async fn list(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
) -> Result<Vec<DetachedScriptSnapshot>, ServiceError> {
    let session = topic_session(s, who, agent_id, channel_id, topic_id).await?;
    match s.deps().storage.list_detached_scripts(&session, LISTED_FINAL).await {
        Ok(scripts) => Ok(scripts.iter().map(|script| script.snapshot()).collect()),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

/// One script with its code, its calls and, once final, its report.
pub async fn get(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    script_id: &str,
) -> Result<DetachedScriptDetail, ServiceError> {
    let session = topic_session(s, who, agent_id, channel_id, topic_id).await?;
    match s.deps().storage.get_detached_script(script_id).await {
        Ok(Some(script)) if script.session == session => Ok(script.detail()),
        Ok(_) => Err(ServiceError::NotFound("script not found".into())),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}
