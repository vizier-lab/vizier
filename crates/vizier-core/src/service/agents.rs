//! Agents: list, detail, create/update/delete, restart, health, ping, sharing, usage, chat and
//! side chat (spec 038, US2). Moved from `vizier-http`'s `agents/mod.rs`; messages and statuses
//! are the handlers' own.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Services, error::ServiceError};
use crate::{
    access::{can_edit_agent, can_view_agent},
    auth::Principal,
    channels::side_chat::SideChatOutcome,
    config::shell::ShellConfig,
    schema::{
        AgentCommand, AgentCommandResult, AgentConfig, AgentHealthStatus, AgentSummary,
        AgentToolsConfig, EmbeddingConfig, ImageGenToolSettings, IndexerConfig,
        ReadImageToolSettings, ToolConfig, TtsToolSettings, VizierAttachment, VizierChannelId,
        VizierRequest, VizierRequestContent, VizierResponseStats, VizierSession,
        agent::{EmbeddingProvider, IndexerKind, SttToolSettings},
    },
    storage::{
        agent::AgentStorage, history::HistoryStorage, session::SessionStorage, user::UserStorage,
    },
};

/// One row of `GET /agents`: the summary, plus when the agent was last active.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentListItem {
    #[serde(flatten)]
    pub summary: AgentSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip)]
    #[cfg_attr(feature = "utoipa", schema(ignore))]
    pub mine: bool,
    #[serde(skip)]
    #[cfg_attr(feature = "utoipa", schema(ignore))]
    pub live: bool,
    #[serde(skip)]
    #[cfg_attr(feature = "utoipa", schema(ignore))]
    pub failed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentDetail {
    pub agent_id: String,
    pub name: String,
    pub description: Option<String>,
    pub provider: crate::config::provider::ProviderId,
    pub model: String,
    pub system_prompt: Option<String>,
    pub thinking_depth: usize,
    pub checkpoint_threshold: f64,
    pub max_tokens: Option<u64>,
    pub context_window: Option<u64>,
    pub shell: Option<ShellConfig>,
    pub fetch: bool,
    pub http_client: bool,
    pub prompt_timeout: String,
    pub dream_enabled: bool,
    pub dream_schedule: Option<String>,
    pub dream_provider: Option<crate::config::provider::ProviderId>,
    pub dream_model: Option<String>,
    pub tools_timeout: String,
    pub avatar_url: Option<String>,
    pub silent_read_initiative_chance: f32,
    pub tts: bool,
    pub tts_settings: Option<TtsToolSettings>,
    pub stt: bool,
    pub stt_settings: Option<SttToolSettings>,
    pub read_image: bool,
    pub read_image_settings: Option<ReadImageToolSettings>,
    pub image_gen: bool,
    pub image_gen_settings: Option<ImageGenToolSettings>,
    pub embedding: Option<EmbeddingConfig>,
    pub indexer: Option<IndexerConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateAgentRequest {
    pub agent_id: String,
    pub name: String,
    pub description: Option<String>,
    pub provider: crate::config::provider::ProviderId,
    pub model: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub thinking_depth: Option<usize>,
    #[serde(default)]
    pub checkpoint_threshold: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub tools: Option<CreateAgentTools>,
    #[serde(default)]
    pub prompt_timeout: Option<String>,
    #[serde(default)]
    pub dream_enabled: Option<bool>,
    #[serde(default)]
    pub dream_schedule: Option<String>,
    #[serde(default)]
    pub dream_provider: Option<crate::config::provider::ProviderId>,
    #[serde(default)]
    pub dream_model: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub silent_read_initiative_chance: Option<f32>,
    #[serde(default)]
    pub embedding: Option<EmbeddingConfig>,
    #[serde(default)]
    pub indexer: Option<IndexerConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateAgentTools {
    #[serde(default)]
    pub shell: Option<ShellConfig>,
    #[serde(default)]
    pub fetch: Option<bool>,
    #[serde(default)]
    pub http_client: Option<bool>,
    #[serde(default)]
    pub timeout: Option<String>,
    #[serde(default)]
    pub tts: Option<bool>,
    #[serde(default)]
    pub tts_settings: Option<TtsToolSettings>,
    #[serde(default)]
    pub stt: Option<bool>,
    #[serde(default)]
    pub stt_settings: Option<SttToolSettings>,
    #[serde(default)]
    pub read_image: Option<bool>,
    #[serde(default)]
    pub read_image_settings: Option<ReadImageToolSettings>,
    #[serde(default)]
    pub image_gen: Option<bool>,
    #[serde(default)]
    pub image_gen_settings: Option<ImageGenToolSettings>,
}

impl CreateAgentRequest {
    pub fn into_config(self) -> AgentConfig {
        let tools = self.tools.unwrap_or(CreateAgentTools {
            shell: None,
            fetch: None,
            http_client: None,
            timeout: None,
            tts: None,
            tts_settings: None,
            stt: None,
            stt_settings: None,
            read_image: None,
            read_image_settings: None,
            image_gen: None,
            image_gen_settings: None,
        });

        AgentConfig {
            name: self.name,
            owner_id: None,
            shared_to: Vec::new(),
            system_prompt: self.system_prompt,
            description: self.description,
            provider: self.provider,
            model: self.model,
            thinking_depth: self.thinking_depth.unwrap_or(100),
            checkpoint_threshold: self.checkpoint_threshold.unwrap_or(0.8).clamp(0.0, 1.0),
            max_tokens: self.max_tokens,
            context_window: self.context_window,
            tools: AgentToolsConfig {
                timeout: duration_string::DurationString::from_string(
                    tools.timeout.unwrap_or_else(|| "30m".into()),
                )
                .unwrap_or(duration_string::DurationString::from_string("30m".into()).unwrap()),
                shell: tools.shell,
                fetch: ToolConfig {
                    enabled: tools.fetch.unwrap_or(false),
                    settings: (),
                },
                http_client: ToolConfig {
                    enabled: tools.http_client.unwrap_or(false),
                    settings: (),
                },
                tts: ToolConfig {
                    enabled: tools.tts.unwrap_or(false),
                    settings: tools.tts_settings.unwrap_or_default(),
                },
                stt: ToolConfig {
                    enabled: tools.stt.unwrap_or(false),
                    settings: tools.stt_settings.unwrap_or_default(),
                },
                read_image: ToolConfig {
                    enabled: tools.read_image.unwrap_or(false),
                    settings: tools.read_image_settings.unwrap_or_default(),
                },
                image_gen: ToolConfig {
                    enabled: tools.image_gen.unwrap_or(false),
                    settings: tools.image_gen_settings.unwrap_or_default(),
                },
            },
            silent_read_initiative_chance: self.silent_read_initiative_chance.unwrap_or(0.0),
            include_documents: None,
            prompt_timeout: duration_string::DurationString::from_string(
                self.prompt_timeout.unwrap_or("60m".into()),
            )
            .unwrap(),
            documents: vec![],
            heartbeat_interval: duration_string::DurationString::from_string("30m".into()).unwrap(),
            core: None,
            dream_enabled: self.dream_enabled.unwrap_or(false),
            dream_schedule: self.dream_schedule,
            dream_provider: self.dream_provider,
            dream_model: self.dream_model,
            avatar_url: self.avatar_url,
            embedding: Some(self.embedding.unwrap_or(EmbeddingConfig {
                provider: EmbeddingProvider::Local,
                model: "all_mini_lml6_v2".into(),
            })),
            indexer: Some(self.indexer.unwrap_or(IndexerConfig {
                kind: IndexerKind::Sqlite,
            })),
            chunking: Default::default(),
            auto_context: Default::default(),
            completion_retry: Default::default(),
        }
    }
}

/// A saved agent, with anything about its model worth knowing.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentSaved {
    #[serde(flatten)]
    pub summary: AgentSummary,
    pub warnings: Vec<crate::catalogue::AgentWarning>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SharingResponse {
    pub shared_to: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateSharingRequest {
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PingResponse {
    pub alive: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ChatRequest {
    pub channel_id: String,
    pub topic_id: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Option<Vec<VizierAttachment>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ChatResponse {
    pub content: String,
    pub stats: Option<VizierResponseStats>,
    pub attachments: Vec<VizierAttachment>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BtwRequest {
    pub channel_id: String,
    pub topic_id: String,
    /// Trimmed; 1–4,000 characters
    pub question: String,
    /// Earlier turns of this side chat, oldest first
    #[serde(default)]
    pub turns: Vec<crate::schema::SideChatTurn>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BtwResponse {
    pub answer: String,
    pub stats: Option<VizierResponseStats>,
}

/// The side chat's answer, or how long to wait before asking again.
#[allow(clippy::large_enum_variant)]
pub enum BtwReply {
    Answer(BtwResponse),
    RetryAfter { secs: u64 },
}

/// The longest range a usage read covers.
const MAX_USAGE_RANGE_DAYS: i64 = 366;

fn internal(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Internal(e.to_string())
}

fn with_code(status: u16, message: impl Into<String>, code: &str) -> ServiceError {
    ServiceError::Invalid {
        status,
        message: message.into(),
        code: Some(code.into()),
    }
}

async fn load(s: &Services, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => Ok(config),
        Ok(None) => Err(ServiceError::NotFound("Agent not found".into())),
        Err(e) => Err(internal(e)),
    }
}

/// Every agent `who` may see. `paged` also fetches what the listing's filters and sorts need.
pub async fn list(
    s: &Services,
    who: &Principal,
    paged: bool,
) -> Result<Vec<AgentListItem>, ServiceError> {
    let deps = s.deps();
    let agents = deps.storage.list_agents().await.map_err(internal)?;
    // Only what the listing needs: skipped without a `limit`, which has always been just names.
    let (last_active, live, failed) = if paged {
        let failed: std::collections::HashSet<String> = deps
            .transport
            .agent_health()
            .await
            .map(|h| {
                h.into_iter()
                    .filter(|s| s.state == crate::schema::AgentRunState::Failed)
                    .map(|s| s.agent_id)
                    .collect()
            })
            .unwrap_or_default();
        (
            deps.storage.last_activity_by_agent().await.unwrap_or_default(),
            deps.storage.thinking_agents().await.unwrap_or_default(),
            failed,
        )
    } else {
        Default::default()
    };

    let mut res = Vec::new();
    for (id, config) in agents {
        if !can_view_agent(who, &config) {
            continue;
        }

        let owner_username = if let Some(ref owner_id) = config.owner_id {
            deps.storage
                .get_user_by_id(owner_id)
                .await
                .ok()
                .flatten()
                .map(|u| u.username)
        } else {
            None
        };
        res.push(AgentListItem {
            last_active: last_active.get(&id).copied(),
            mine: config.owner_id.as_deref() == Some(who.user_id.as_str()),
            live: live.contains(&id),
            failed: failed.contains(&id),
            summary: AgentSummary {
                agent_id: id,
                name: config.name,
                description: config.description,
                avatar_url: config.avatar_url,
                owner_username,
                owner_id: config.owner_id.clone(),
                shared_to: config.shared_to,
            },
        });
    }
    Ok(res)
}

pub async fn detail(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<AgentDetail, ServiceError> {
    let config = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound("not found".into())),
        Err(e) => return Err(internal(e)),
    };
    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }

    Ok(AgentDetail {
        agent_id: agent_id.to_string(),
        name: config.name,
        description: config.description,
        provider: config.provider,
        model: config.model,
        system_prompt: config.system_prompt,
        thinking_depth: config.thinking_depth,
        checkpoint_threshold: config.checkpoint_threshold,
        max_tokens: config.max_tokens,
        context_window: config.context_window,
        shell: config.tools.shell,
        fetch: config.tools.fetch.enabled,
        http_client: config.tools.http_client.enabled,
        prompt_timeout: config.prompt_timeout.to_string(),
        dream_enabled: config.dream_enabled,
        dream_schedule: config.dream_schedule,
        dream_provider: config.dream_provider,
        dream_model: config.dream_model,
        tools_timeout: config.tools.timeout.to_string(),
        avatar_url: config.avatar_url,
        silent_read_initiative_chance: config.silent_read_initiative_chance,
        tts: config.tools.tts.enabled,
        tts_settings: if config.tools.tts.settings.provider
            != crate::schema::agent::TtsProvider::default()
            || config.tools.tts.settings.model.is_some()
            || config.tools.tts.settings.voice.is_some()
            || config.tools.tts.settings.speed.is_some()
        {
            Some(config.tools.tts.settings.clone())
        } else {
            None
        },
        stt: config.tools.stt.enabled,
        stt_settings: if config.tools.stt.settings.provider
            != crate::schema::agent::SttProvider::default()
            || config.tools.stt.settings.model.is_some()
            || config.tools.stt.settings.language.is_some()
        {
            Some(config.tools.stt.settings.clone())
        } else {
            None
        },
        read_image: config.tools.read_image.enabled,
        read_image_settings: if config.tools.read_image.settings.provider.is_some()
            || config.tools.read_image.settings.model.is_some()
        {
            Some(config.tools.read_image.settings.clone())
        } else {
            None
        },
        image_gen: config.tools.image_gen.enabled,
        image_gen_settings: if config.tools.image_gen.settings.provider
            != crate::schema::agent::ImageGenProvider::default()
            || config.tools.image_gen.settings.model.is_some()
            || config.tools.image_gen.settings.size.is_some()
        {
            Some(config.tools.image_gen.settings.clone())
        } else {
            None
        },
        embedding: config.embedding,
        indexer: config.indexer,
    })
}

async fn saved_with_warnings(s: &Services, config: &AgentConfig, summary: AgentSummary) -> AgentSaved {
    use crate::storage::provider::CustomProviderStorage;
    let deps = s.deps();
    let custom = deps
        .storage
        .get_custom_provider(&config.provider)
        .await
        .ok()
        .flatten();
    AgentSaved {
        summary,
        warnings: crate::catalogue::agent_warnings(&deps.catalogue, custom.as_ref(), config),
    }
}

fn check_embedding(body: &CreateAgentRequest) -> Result<(), ServiceError> {
    if let Some(emb) = &body.embedding {
        // Bedrock's empty model selects the documented Titan V2 default (FR-020); every other
        // remote provider needs an explicit model.
        if emb.model.trim().is_empty() && emb.provider != EmbeddingProvider::Bedrock {
            return Err(ServiceError::invalid("embedding.model is required"));
        }
    }
    Ok(())
}

/// Sends an agent command and maps its answer: a refusal is a `400`, a dead manager a `500`.
async fn command(
    s: &Services,
    build: impl FnOnce(tokio::sync::oneshot::Sender<AgentCommandResult>) -> AgentCommand,
) -> Result<AgentSummary, ServiceError> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    if let Err(e) = s.deps().transport.send_agent_command(build(resp_tx)).await {
        return Err(ServiceError::Internal(format!("failed to send command: {}", e)));
    }
    match resp_rx.await {
        Ok(AgentCommandResult::Ok(summary)) => Ok(summary),
        Ok(AgentCommandResult::Error(e)) => Err(ServiceError::invalid(e)),
        Err(_) => Err(ServiceError::Internal("agent manager unavailable".into())),
    }
}

pub async fn create(
    s: &Services,
    who: &Principal,
    body: CreateAgentRequest,
) -> Result<AgentSaved, ServiceError> {
    check_embedding(&body)?;

    let agent_id = body.agent_id.clone();
    let mut config = body.into_config();
    config.owner_id = Some(who.user_id.clone());
    let saved = config.clone();

    let summary = command(s, |resp| AgentCommand::Create {
        agent_id,
        config,
        resp,
    })
    .await?;
    Ok(saved_with_warnings(s, &saved, summary).await)
}

pub async fn update(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: CreateAgentRequest,
) -> Result<AgentSaved, ServiceError> {
    check_embedding(&body)?;

    let mut config = body.into_config();

    // Preserve existing owner_id and shared_to when updating
    let existing = load(s, agent_id).await?;
    if !can_edit_agent(who, &existing) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }

    config.owner_id = existing.owner_id;
    config.shared_to = existing.shared_to;
    let saved = config.clone();

    let agent_id = agent_id.to_string();
    let summary = command(s, |resp| AgentCommand::Update {
        agent_id,
        config,
        resp,
    })
    .await?;
    Ok(saved_with_warnings(s, &saved, summary).await)
}

pub async fn delete(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    delete_workspace: bool,
) -> Result<String, ServiceError> {
    let can_delete_all =
        who.role.is_system || who.permissions.contains(&"all_agents:delete".to_string());
    let can_delete_owned = who.permissions.contains(&"owned_agents:delete".to_string());

    if !can_delete_all {
        let agent = load(s, agent_id).await?;
        if let Some(ref owner_id) = agent.owner_id {
            if *owner_id != who.user_id && !can_delete_owned {
                return Err(ServiceError::Forbidden("Access denied".into()));
            }
        } else if !can_delete_owned {
            return Err(ServiceError::Forbidden("Access denied".into()));
        }
    }

    let agent_id = agent_id.to_string();
    command(s, |resp| AgentCommand::Delete {
        agent_id,
        delete_workspace,
        resp,
    })
    .await?;
    Ok("deleted".into())
}

pub async fn restart(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<AgentHealthStatus, ServiceError> {
    let deps = s.deps();
    let config = match deps.storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound("agent not found".into())),
        Err(e) => return Err(internal(e)),
    };
    if !can_edit_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }

    let (resp, rx) = tokio::sync::oneshot::channel();
    if let Err(e) = deps
        .transport
        .send_agent_command(AgentCommand::Restart {
            agent_id: agent_id.to_string(),
            resp,
        })
        .await
    {
        return Err(ServiceError::Internal(format!("failed to send command: {e}")));
    }
    match rx.await {
        Ok(AgentCommandResult::Ok(_)) => {}
        Ok(AgentCommandResult::Error(e)) if e.contains("still being created or updated") => {
            return Err(ServiceError::Conflict {
                message: e,
                code: None,
                data: None,
            });
        }
        Ok(AgentCommandResult::Error(e)) => return Err(ServiceError::invalid(e)),
        Err(_) => return Err(ServiceError::Internal("agent manager unavailable".into())),
    }

    match deps.transport.agent_health().await {
        Ok(statuses) => statuses
            .into_iter()
            .find(|s| s.agent_id == agent_id)
            .ok_or_else(|| ServiceError::NotFound("agent not found".into())),
        Err(e) => Err(ServiceError::Unavailable(e.to_string())),
    }
}

/// The storage query for a usage read, or the `400` refusing it.
fn usage_query(
    agent_id: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    activity: Option<&str>,
) -> Result<crate::storage::usage::UsageQuery, ServiceError> {
    let parse = |s: &str| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc));
    let bad_range = |message: &str| with_code(400, message, "invalid_range");

    let end = match end_date {
        Some(s) => parse(s).map_err(|_| bad_range("end_date is not RFC 3339"))?,
        None => Utc::now(),
    };
    let start = match start_date {
        Some(s) => parse(s).map_err(|_| bad_range("start_date is not RFC 3339"))?,
        None => end - chrono::Duration::days(30),
    };
    if start > end {
        return Err(bad_range("start_date is after end_date"));
    }
    if end - start > chrono::Duration::days(MAX_USAGE_RANGE_DAYS) {
        return Err(bad_range("the range is longer than 366 days"));
    }
    let activity = match activity.filter(|a| !a.is_empty()) {
        Some(a) => Some(
            a.parse::<crate::schema::usage::UsageActivity>()
                .map_err(|_| with_code(400, format!("unknown activity '{a}'"), "unknown_activity"))?,
        ),
        None => None,
    };
    Ok(crate::storage::usage::UsageQuery {
        agent_id: agent_id.to_string(),
        start,
        end,
        activity,
        conversation: None,
    })
}

fn not_found_lowercase() -> ServiceError {
    ServiceError::NotFound("agent not found".into())
}

async fn exists(s: &Services, agent_id: &str) -> bool {
    s.deps()
        .storage
        .get_agent(agent_id)
        .await
        .ok()
        .flatten()
        .is_some()
}

pub async fn usage(
    s: &Services,
    _who: &Principal,
    agent_id: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    activity: Option<&str>,
) -> Result<crate::schema::usage::UsageReport, ServiceError> {
    if !exists(s, agent_id).await {
        return Err(not_found_lowercase());
    }
    let q = usage_query(agent_id, start_date, end_date, activity)?;
    let deps = s.deps();
    crate::usage::build_report(&deps.storage, &deps.catalogue, &deps.integration_kinds, q)
        .await
        .map_err(internal)
}

#[allow(clippy::too_many_arguments)]
pub async fn usage_conversation(
    s: &Services,
    _who: &Principal,
    agent_id: &str,
    channel: Option<String>,
    topic: Option<String>,
    start_date: Option<&str>,
    end_date: Option<&str>,
    activity: Option<&str>,
    limit: Option<usize>,
) -> Result<crate::schema::usage::ConversationUsage, ServiceError> {
    if !exists(s, agent_id).await {
        return Err(not_found_lowercase());
    }
    let Some(channel) = channel.filter(|c| !c.is_empty()) else {
        return Err(with_code(400, "channel is required", "missing_channel"));
    };
    let limit = limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err(with_code(400, "limit must be between 1 and 100", "invalid_limit"));
    }
    let mut q = usage_query(agent_id, start_date, end_date, activity)?;
    q.conversation = Some((channel, topic.filter(|t| !t.is_empty())));
    let deps = s.deps();
    crate::usage::conversation_usage(&deps.storage, &deps.catalogue, &deps.integration_kinds, q, limit)
        .await
        .map_err(internal)
}

pub async fn get_sharing(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<SharingResponse, ServiceError> {
    let config = load(s, agent_id).await?;
    if !can_edit_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    Ok(SharingResponse {
        shared_to: config.shared_to,
    })
}

pub async fn update_sharing(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: UpdateSharingRequest,
) -> Result<SharingResponse, ServiceError> {
    let mut config = load(s, agent_id).await?;
    if !can_edit_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }

    for user_id in &body.add {
        if !config.shared_to.contains(user_id) {
            config.shared_to.push(user_id.clone());
        }
    }
    config.shared_to.retain(|id| !body.remove.contains(id));

    s.deps()
        .storage
        .update_agent(agent_id, &config)
        .await
        .map_err(internal)?;
    Ok(SharingResponse {
        shared_to: config.shared_to,
    })
}

pub async fn health(s: &Services, _who: &Principal) -> Result<Vec<AgentHealthStatus>, ServiceError> {
    let deps = s.deps();
    match deps.transport.agent_health().await {
        Ok(mut statuses) => {
            let live = deps.storage.thinking_agents().await.unwrap_or_default();
            for st in &mut statuses {
                st.live = st.alive && live.contains(&st.agent_id.to_string());
            }
            Ok(statuses)
        }
        Err(e) => Err(ServiceError::Unavailable(e.to_string())),
    }
}

pub async fn ping(s: &Services, _who: &Principal, agent_id: &str) -> Result<PingResponse, ServiceError> {
    if !exists(s, agent_id).await {
        return Err(not_found_lowercase());
    }
    match s.deps().transport.agent_health().await {
        Ok(statuses) => Ok(PingResponse {
            alive: statuses.iter().any(|st| st.agent_id == agent_id && st.alive),
        }),
        Err(e) => Err(ServiceError::Unavailable(e.to_string())),
    }
}

pub async fn chat(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: ChatRequest,
) -> Result<ChatResponse, ServiceError> {
    let deps = s.deps();
    let config = match deps.storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(not_found_lowercase()),
        Err(e) => return Err(ServiceError::Internal(format!("failed to get agent: {}", e))),
    };
    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    // A REST chat naming a project channel goes through the same matrix as its socket.
    super::topics::authorize_project_channel(s, who, &config, agent_id, &body.channel_id).await?;

    let transport = deps.transport.clone();
    let session = VizierSession(
        agent_id.to_string(),
        VizierChannelId::HTTP(who.username.clone(), body.channel_id.clone()),
        Some(body.topic_id.clone()),
    );

    // A request attachment is a person's upload. Artifact references are created only by the
    // agent/tool response path, so a client cannot ask agent A to read agent B's artifact by id
    // (`specs/035-add-artifacts` T060).
    if body.attachments.as_ref().is_some_and(|list| {
        list.iter().any(|attachment| {
            matches!(
                attachment.content,
                crate::schema::VizierAttachmentContent::Artifact { .. }
            )
        })
    }) {
        return Err(ServiceError::Invalid {
            status: 422,
            message: "request attachments must be uploads; artifact references are created by the agent"
                .into(),
            code: None,
        });
    }
    let mut attachments = body.attachments.unwrap_or_default();
    for attachment in attachments.iter_mut() {
        match transport.send_file_resolve(attachment.clone()).await {
            Ok(content) => {
                match transport
                    .send_file_upload(attachment.filename.clone(), content)
                    .await
                {
                    Ok(file_record) => {
                        attachment.content =
                            crate::schema::VizierAttachmentContent::Local(file_record.url);
                    }
                    Err(e) => {
                        tracing::error!("failed to upload resolved attachment: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("failed to resolve attachment: {}", e);
            }
        }
    }

    let request = VizierRequest {
        trace: Default::default(),
        client_tools: vec![],
        client_categories: vec![],
        client: None,
        timestamp: Utc::now(),
        user: who.username.clone(),
        content: VizierRequestContent::Chat(body.content),
        platform_message_id: None,
        metadata: serde_json::json!({}),
        attachments,
        expect_audio_reply: None,
        scheduled_task: None,
        background_depth: 0,
        ..Default::default()
    };

    let (response_tx, response_rx) = flume::unbounded();
    if let Err(err) = transport
        .send_request(session.clone(), request, Some(response_tx))
        .await
    {
        return Err(ServiceError::Internal(format!("failed to send request: {}", err)));
    }

    let timeout_duration = *config.prompt_timeout;
    // The same send-and-await loop the side chat's `ask` uses (research R10), mapped to
    // agent_chat's responses.
    match crate::channels::side_chat::await_reply(response_rx, timeout_duration).await {
        Ok(SideChatOutcome::Answered(reply)) => Ok(ChatResponse {
            content: reply.content,
            stats: reply.stats,
            attachments: reply.attachments,
        }),
        Ok(SideChatOutcome::Aborted) => Err(ServiceError::Internal("Agent aborted the request".into())),
        Ok(SideChatOutcome::AgentError { kind, message }) => {
            Err(ServiceError::Internal(format!("{}: {}", kind.label(), message)))
        }
        Ok(SideChatOutcome::NoAgent) => {
            Err(ServiceError::Internal("Agent response channel closed".into()))
        }
        Err(_) => Err(ServiceError::Invalid {
            status: 408,
            message: "Agent response timeout".into(),
            code: None,
        }),
    }
}

pub async fn btw(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: BtwRequest,
) -> Result<BtwReply, ServiceError> {
    let deps = s.deps();
    let config = match deps.storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(not_found_lowercase()),
        Err(e) => return Err(ServiceError::Internal(format!("failed to get agent: {}", e))),
    };
    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    super::topics::authorize_project_channel(s, who, &config, agent_id, &body.channel_id).await?;

    let session = VizierSession(
        agent_id.to_string(),
        VizierChannelId::HTTP(who.username.clone(), body.channel_id.clone()),
        Some(body.topic_id.clone()),
    );
    let side_chat = crate::schema::SideChat {
        question: body.question.trim().to_string(),
        turns: body.turns,
    };

    use crate::channels::side_chat::SideChatError;
    let result = crate::channels::side_chat::ask(
        deps,
        session,
        &format!("http:{}", who.username),
        who.username.clone(),
        side_chat,
        *config.prompt_timeout,
    )
    .await;

    match result {
        Ok(answer) => Ok(BtwReply::Answer(BtwResponse {
            answer: answer.answer,
            stats: answer.stats,
        })),
        Err(SideChatError::Invalid(message)) => Err(ServiceError::Invalid {
            status: 422,
            message,
            code: None,
        }),
        Err(SideChatError::RateLimited { retry_after_secs }) => Ok(BtwReply::RetryAfter {
            secs: retry_after_secs,
        }),
        Err(SideChatError::Timeout) => Err(ServiceError::Invalid {
            status: 408,
            message: "Agent response timeout".into(),
            code: None,
        }),
        Err(SideChatError::Agent { kind, message }) => {
            let text = format!("{kind}: {message}");
            if kind == crate::schema::ErrorKind::AgentUnavailable.label() {
                Err(ServiceError::Unavailable(text))
            } else {
                Err(ServiceError::Internal(text))
            }
        }
    }
}
