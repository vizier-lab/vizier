//! Topics: a person's conversations with one agent on a channel — history, files, listing,
//! detail and deletion (spec 038, US2). Moved from `vizier-http`'s `agents/channel.rs`; the
//! live connection (`TopicConnection`) stays in `vizier-http`.

use chrono::Utc;
use serde::Serialize;

use super::{Services, error::ServiceError};
use crate::{
    access::{Caller, can_show_platform_chats, can_view_agent, platform_of_slug},
    artifacts::access as artifacts_access,
    auth::Principal,
    schema::{
        Artifact, Requester, SessionHistory, TopicId, VizierChannelId, VizierSession,
        VizierSessionDetail, session_file::SessionFileRecord,
    },
    session_files::ClearError,
    storage::{
        agent::AgentStorage, background_job::BackgroundJobStorage, history::HistoryStorage,
        session::SessionStorage, session_file::SessionFileStorage,
    },
};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TopicEntry {
    pub topic_id: String,
    pub title: String,
    pub agent_id: String,
    pub channel: String,
    pub is_thinking: bool,
    /// Background jobs launched from this topic that are still in flight.
    pub running_jobs: usize,
}

impl From<VizierSessionDetail> for TopicEntry {
    fn from(detail: VizierSessionDetail) -> Self {
        Self {
            topic_id: detail.topic.unwrap_or_default(),
            title: detail.title,
            agent_id: detail.agent_id,
            channel: format!("{:?}", detail.channel),
            is_thinking: detail.is_thinking,
            running_jobs: 0,
        }
    }
}

/// A page of session files, newest first.
pub struct FilesPage {
    pub items: Vec<SessionFileRecord>,
    pub total: usize,
}

/// The channel a read-only route means by `channel_id`: a Discord or Telegram conversation by its
/// slug (`discord__123`, `telegram__-9`, which the conversation picker lists), anything else a
/// WebUI channel of the caller's own. Routes that write never use this: nobody chats on Discord
/// through the WebUI.
pub fn readable_channel(username: &str, channel_id: &str) -> (VizierChannelId, bool) {
    if let Some(id) = channel_id
        .strip_prefix("discord__")
        .and_then(|id| id.parse::<u64>().ok())
    {
        return (VizierChannelId::DiscordChanel(id), true);
    }
    if let Some(id) = channel_id
        .strip_prefix("telegram__")
        .and_then(|id| id.parse::<i64>().ok())
    {
        return (VizierChannelId::TelegramChannel(id), true);
    }
    // A blocking subagent's conversation is addressable by its topic, even though it has
    // no background job row of its own.
    if channel_id.eq_ignore_ascii_case("SUBAGENT") {
        return (VizierChannelId::Subagent, false);
    }
    (
        VizierChannelId::HTTP(username.to_string(), channel_id.to_string()),
        false,
    )
}

/// The session a read-only route opens. Discord and Telegram conversations have no topic.
pub fn readable_session(
    agent_id: String,
    username: &str,
    channel_id: &str,
    topic_id: TopicId,
) -> VizierSession {
    let (channel, platform) = readable_channel(username, channel_id);
    VizierSession(
        agent_id,
        channel,
        if platform { None } else { Some(topic_id) },
    )
}

fn internal(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Internal(e.to_string())
}

fn conflict(message: &str) -> ServiceError {
    ServiceError::Conflict {
        message: message.into(),
        code: None,
        data: None,
    }
}

const TURN_ACTIVE: &str = "a turn is running or queued in this conversation; wait for it to finish";

/// The agent, if `who` may see it: `NotFound` if there is none, `Forbidden` otherwise.
pub async fn authorize_view(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<crate::schema::AgentConfig, ServiceError> {
    let config = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound(format!("agent {agent_id} not found"))),
        Err(e) => return Err(internal(e)),
    };
    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    Ok(config)
}

/// [`authorize_view`], and for a Discord or Telegram conversation the permission to read it.
pub async fn authorize_topic(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
) -> Result<crate::schema::AgentConfig, ServiceError> {
    let config = authorize_view(s, who, agent_id).await?;
    if let Some(platform) = platform_of_slug(channel_id)
        && !can_show_platform_chats(who, &config, platform)
    {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    Ok(config)
}

#[allow(clippy::too_many_arguments)]
pub async fn history(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: TopicId,
    before: Option<chrono::DateTime<Utc>>,
    before_seq: Option<i64>,
    limit: Option<usize>,
) -> Result<Vec<SessionHistory>, ServiceError> {
    authorize_topic(s, who, agent_id, channel_id).await?;
    let session = readable_session(agent_id.to_string(), &who.username, channel_id, topic_id);

    // Every entry kind is returned, `AssistantMessage` included: the intermediate narration
    // an agent writes alongside its tool calls is part of the trail the WebUI renders, and
    // dropping it here is what used to make it invisible.
    s.deps()
        .storage
        .list_session_history(session, before, before_seq, limit)
        .await
        .map_err(|_| ServiceError::NotFound("Not found".into()))
}

pub async fn list_files(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: TopicId,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<FilesPage, ServiceError> {
    authorize_topic(s, who, agent_id, channel_id).await?;
    let session = readable_session(agent_id.to_string(), &who.username, channel_id, topic_id);
    let mut files = s
        .deps()
        .storage
        .list_session_files(&session)
        .await
        .map_err(internal)?;
    files.sort_by_key(|f| std::cmp::Reverse(f.added_at));
    let total = files.len();
    let items = files
        .into_iter()
        .skip(offset.unwrap_or(0))
        .take(limit.unwrap_or(100).clamp(1, 100))
        .collect();
    Ok(FilesPage { items, total })
}

pub async fn clear_files(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: TopicId,
) -> Result<crate::session_files::ClearResult, ServiceError> {
    authorize_topic(s, who, agent_id, channel_id).await?;
    let session = readable_session(agent_id.to_string(), &who.username, channel_id, topic_id);
    match s.deps().session_files.clear(&session).await {
        Ok(result) => Ok(result),
        Err(ClearError::TurnActive) => Err(conflict(TURN_ACTIVE)),
        Err(ClearError::AgentNotFound) => Err(ServiceError::NotFound("agent not found".into())),
        Err(ClearError::Storage(e)) => Err(ServiceError::Internal(e)),
    }
}

pub async fn promote_file(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: TopicId,
    filename: &str,
    name: Option<&str>,
) -> Result<Artifact, ServiceError> {
    let config = authorize_topic(s, who, agent_id, channel_id).await?;
    if !artifacts_access::can_manage(&Caller::from(who), &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }

    let session = readable_session(agent_id.to_string(), &who.username, channel_id, topic_id);
    let source = crate::artifacts::SourceFile {
        session,
        filename: filename.to_string(),
    };
    match s
        .deps()
        .artifacts
        .promote(agent_id, Requester::User(who.user_id.clone()), &source, name)
        .await
    {
        Ok(artifact) => Ok(artifact),
        Err(e) => {
            let message = e.to_string();
            Err(if message.contains("already exists") {
                conflict(&message)
            } else if message.contains("not found") {
                ServiceError::NotFound(message)
            } else {
                ServiceError::Invalid {
                    status: 422,
                    message,
                    code: None,
                }
            })
        }
    }
}

pub async fn list(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
) -> Result<Vec<TopicEntry>, ServiceError> {
    authorize_view(s, who, agent_id).await?;
    let deps = s.deps();
    let channel = VizierChannelId::HTTP(who.username.clone(), channel_id.to_string());

    let response = deps
        .storage
        .get_session_list(agent_id.to_string(), Some(channel.clone()))
        .await;

    // One count query for the whole list; a failure only loses the badge.
    let running_jobs = deps
        .storage
        .count_running_background_jobs(agent_id.to_string(), channel)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to count running background jobs: {}", e);
            Default::default()
        });

    let details = response.map_err(|_| ServiceError::Internal("Failed to fetch topics".into()))?;
    Ok(details
        .into_iter()
        .map(|detail| {
            let running = running_jobs.get(&detail.topic).copied().unwrap_or(0);
            let mut entry = TopicEntry::from(detail);
            entry.running_jobs = running;
            entry
        })
        .collect())
}

pub async fn detail(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: TopicId,
) -> Result<TopicEntry, ServiceError> {
    authorize_topic(s, who, agent_id, channel_id).await?;
    let (channel, platform) = readable_channel(&who.username, channel_id);

    match s
        .deps()
        .storage
        .get_session_detail_by_topic(
            agent_id.to_string(),
            channel,
            if platform { None } else { Some(topic_id) },
        )
        .await
    {
        Ok(Some(detail)) => Ok(TopicEntry::from(detail)),
        Ok(None) => Err(ServiceError::NotFound("Topic not found".into())),
        Err(e) => Err(internal(e)),
    }
}

pub async fn delete(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: TopicId,
) -> Result<String, ServiceError> {
    authorize_view(s, who, agent_id).await?;
    let session = VizierSession(
        agent_id.to_string(),
        VizierChannelId::HTTP(who.username.clone(), channel_id.to_string()),
        Some(topic_id),
    );
    let files = &s.deps().session_files;
    if files.has_active_turn(&session).await {
        return Err(conflict(TURN_ACTIVE));
    }
    match files.delete_conversation(&session).await {
        Ok(_) => Ok("Topic deleted".into()),
        Err(ClearError::TurnActive) => Err(conflict(TURN_ACTIVE)),
        Err(ClearError::AgentNotFound) => Err(ServiceError::NotFound("Topic not found".into())),
        Err(ClearError::Storage(e)) => Err(ServiceError::Internal(e)),
    }
}
