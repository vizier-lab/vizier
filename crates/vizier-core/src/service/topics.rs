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
        project::ProjectStorage, session::SessionStorage, session_file::SessionFileStorage,
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
        VizierChannelId::from_web(username, channel_id),
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

/// A project channel resolves its project by `(agent_id, project_id)`, requires it to exist,
/// belong to this agent and be non-removed, and applies the project permission matrix. A
/// non-project channel passes through. The channel username is always the caller's.
pub async fn authorize_project_channel(
    s: &Services,
    who: &Principal,
    config: &crate::schema::AgentConfig,
    agent_id: &str,
    channel_id: &str,
) -> Result<(), ServiceError> {
    let Some(project_id) = crate::projects::channel::project_id(channel_id) else {
        return Ok(());
    };
    let project = s
        .deps()
        .storage
        .get_project(agent_id, project_id)
        .await
        .map_err(internal)?
        .filter(|project| project.removed_at.is_none())
        .ok_or_else(|| ServiceError::NotFound("project not found".into()))?;
    if !crate::projects::access::can_use(Caller::from(who), config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    let _ = project;
    Ok(())
}

/// [`authorize_view`], and for a Discord or Telegram conversation the permission to read it, and
/// for a project channel the project's own authorization.
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
    authorize_project_channel(s, who, &config, agent_id, channel_id).await?;
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
    let channel = VizierChannelId::from_web(&who.username, channel_id);

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

/// Gives one of the caller's own WebUI conversations a name of their choosing. A titled
/// conversation is never re-titled by the generator (`refresh_session_detail`), so it sticks.
pub async fn rename(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: TopicId,
    title: &str,
) -> Result<TopicEntry, ServiceError> {
    authorize_topic(s, who, agent_id, channel_id).await?;
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(ServiceError::invalid("The title must be 1–120 characters"));
    }
    let storage = &s.deps().storage;
    let mut detail = match storage
        .get_session_detail_by_topic(
            agent_id.to_string(),
            VizierChannelId::from_web(&who.username, channel_id),
            Some(topic_id),
        )
        .await
    {
        Ok(Some(detail)) => detail,
        Ok(None) => return Err(ServiceError::NotFound("Topic not found".into())),
        Err(e) => return Err(internal(e)),
    };
    detail.title = title.to_string();
    storage
        .update_session_detail(detail.clone())
        .await
        .map_err(internal)?;
    Ok(TopicEntry::from(detail))
}

pub async fn delete(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: TopicId,
) -> Result<String, ServiceError> {
    authorize_topic(s, who, agent_id, channel_id).await?;
    // A worktree chat's delete is the worktree's: same checks, and the folder goes with it
    // (`specs/041-project-coding-workspace` FR-030/R13).
    if let Some(project_id) = crate::projects::channel::project_id(channel_id)
        && let Some(worktree) = s
            .deps()
            .storage
            .worktree_by_topic(project_id, &topic_id)
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?
    {
        super::projects::delete_worktree(s, who, agent_id, project_id, &worktree.id, false, false)
            .await?;
        return Ok("Topic deleted".into());
    }
    let session = VizierSession(
        agent_id.to_string(),
        VizierChannelId::from_web(&who.username, channel_id),
        Some(topic_id),
    );
    let files = &s.deps().session_files;
    if files.has_active_turn(&session).await {
        return Err(conflict(TURN_ACTIVE));
    }
    match files.delete_conversation(&session).await {
        Ok(_) => {
            // The stale-read ledger is per conversation and dies with it.
            s.deps().project_ledger.forget_session(&session.to_slug());
            Ok("Topic deleted".into())
        }
        Err(ClearError::TurnActive) => Err(conflict(TURN_ACTIVE)),
        Err(ClearError::AgentNotFound) => Err(ServiceError::NotFound("Topic not found".into())),
        Err(ClearError::Storage(e)) => Err(ServiceError::Internal(e)),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::{
        auth::AuthMethod,
        config::VizierConfig,
        projects::access,
        schema::Project,
        storage::{project::ProjectStorage, user::UserStorage},
    };

    fn project(agent: &str, owner: &str) -> Project {
        Project {
            id: "p3xY9aQ2mZ".into(),
            agent_id: agent.into(),
            name: "alpha".into(),
            folder: std::path::PathBuf::from("/tmp/does-not-matter"),
            owner_id: owner.into(),
            shell_enabled: true,
            created_at: 0,
            removed_at: None,
        }
    }

    async fn setup(permissions: Vec<String>) -> (Services, Principal, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let deps = crate::dependencies::VizierDependencies::new(VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let role = deps.storage.create_role("r", permissions, false).await.unwrap();
        let user = deps.storage.create_user("alice", "hash", &role.role_id).await.unwrap();
        deps.storage
            .create_agent("dp-a", &crate::tests_support::agent_config("dp-a"))
            .await
            .unwrap();
        deps.storage.open_project(project("dp-a", &user.user_id)).await.unwrap();
        let who = Principal {
            user_id: user.user_id,
            username: "alice".into(),
            role: role.clone(),
            permissions: role.permissions.clone(),
            method: AuthMethod::Jwt,
        };
        (Services::new(deps), who, dir)
    }

    #[tokio::test]
    async fn an_allowed_project_channel_passes() {
        let (services, who, _dir) = setup(vec!["all_agents:view".into(), access::USE_ALL.into()]).await;
        assert!(
            authorize_topic(&services, &who, "dp-a", "project__p3xY9aQ2mZ")
                .await
                .is_ok()
        );
        // An ordinary channel is unchanged.
        assert!(authorize_topic(&services, &who, "dp-a", "vizier-webui").await.is_ok());
    }

    #[tokio::test]
    async fn a_removed_project_and_a_wrong_agent_are_not_found() {
        let (services, who, _dir) = setup(vec!["all_agents:view".into(), access::USE_ALL.into()]).await;
        assert_eq!(
            authorize_topic(&services, &who, "dp-b", "project__p3xY9aQ2mZ")
                .await
                .unwrap_err()
                .status(),
            404
        );
        services
            .deps()
            .storage
            .remove_project("p3xY9aQ2mZ", Utc::now().timestamp_millis())
            .await
            .unwrap();
        assert_eq!(
            authorize_topic(&services, &who, "dp-a", "project__p3xY9aQ2mZ")
                .await
                .unwrap_err()
                .status(),
            404
        );
    }

    #[tokio::test]
    async fn a_permissionless_caller_is_refused() {
        let (services, who, _dir) = setup(vec!["all_agents:view".into()]).await;
        assert_eq!(
            authorize_topic(&services, &who, "dp-a", "project__p3xY9aQ2mZ")
                .await
                .unwrap_err()
                .status(),
            403
        );
    }
}
