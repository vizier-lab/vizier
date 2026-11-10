//! Agents: list, detail, create/update/delete, restart, health, ping, sharing, usage, chat and
//! side chat (spec 038, US2). Moved from `vizier-http`'s `agents/mod.rs`; messages and statuses
//! are the handlers' own.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::{Services, error::ServiceError};
use crate::{
    access::{can_edit_agent, can_view_agent},
    auth::Principal,
    channels::side_chat::SideChatOutcome,
    config::shell::ShellConfig,
    schema::{
        AgentConfig, AgentHealthStatus, AgentSummary,
        AgentToolsConfig, EmbeddingConfig, ImageGenToolSettings, IndexerConfig,
        ReadImageToolSettings, ToolConfig, TtsToolSettings, VizierAttachment, VizierChannelId,
        ModelOverride, VizierRequest, VizierRequestContent, VizierResponseStats, VizierSession,
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
    pub last_active: Option<chrono::DateTime<chrono::Local>>,
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
    /// Whether this agent can hold a live voice conversation (FR-004): the switch is on **and**
    /// it has a usable recogniser and voice. Additive: an older client ignores it.
    pub live_voice: bool,
    /// How long a silence ends the utterance, from the agent's live voice settings (G1).
    ///
    /// It travels to the client because the client is where the gate runs: the value used to be
    /// hardcoded in the browser, so the setting was never read by anything.
    pub live_voice_end_of_utterance_ms: u32,
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

/// The live voice block a person may send when saving an agent (T041, FR-003).
///
/// Without this the session could never be turned on: `into_config` used to hard-code
/// `enabled: false`, so every API call was ignored and `agent_detail` reported `live_voice: false`
/// no matter what anyone asked for.
///
/// `settings` is the same [`LiveVoiceSettings`] the config stores, so a body can carry the whole
/// block or just `{"enabled": true}` — the latter means "use this agent's own stt/tts settings",
/// which is what the field's `Default` already says.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct LiveVoiceInput {
    pub enabled: bool,
    #[serde(default)]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub settings: Option<crate::schema::agent::LiveVoiceSettings>,
}

impl LiveVoiceInput {
    pub fn into_tool(self) -> crate::schema::agent::ToolConfig<crate::schema::agent::LiveVoiceSettings> {
        crate::schema::agent::ToolConfig {
            enabled: self.enabled,
            settings: self.settings.unwrap_or_default(),
        }
    }
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
    /// Its own switch, and never turned on by the speech tools (FR-003). `None` on an update means
    /// "leave it as it is", which is what keeps saving an agent from silently ending the session.
    #[serde(default)]
    pub live_voice: Option<LiveVoiceInput>,
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
            live_voice: None,
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
                // Its own switch, and never turned on by the speech tools (FR-003). A body that
                // does not mention it leaves the session off, which is the behaviour every agent
                // had before T041.
                live_voice: tools
                    .live_voice
                    .map(LiveVoiceInput::into_tool)
                    .unwrap_or(ToolConfig {
                        enabled: false,
                        settings: crate::schema::agent::LiveVoiceSettings::default(),
                    }),
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
            limits: Default::default(),
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

/// An explicit per-chat model choice. Provider and complete model id remain separate.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ModelOverrideRequest {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ChatRequest {
    pub channel_id: String,
    pub topic_id: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Option<Vec<VizierAttachment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_override: Option<ModelOverrideRequest>,
}

#[cfg(test)]
mod model_override_request_tests {
    use super::{ChatRequest, ModelOverrideRequest};

    #[test]
    fn legacy_rest_chat_requests_omit_model_override_and_slash_ids_round_trip() {
        let legacy: ChatRequest = serde_json::from_value(serde_json::json!({
            "channel_id": "vizier-webui",
            "topic_id": "topic",
            "content": "hello"
        }))
        .unwrap();
        assert!(legacy.model_override.is_none());
        assert!(serde_json::to_value(&legacy).unwrap().get("model_override").is_none());

        let selected: ChatRequest = serde_json::from_value(serde_json::json!({
            "channel_id": "vizier-webui",
            "topic_id": "topic",
            "content": "hello",
            "model_override": { "provider": "opencode", "model": "deepseek/deepseek-flash" }
        }))
        .unwrap();
        assert_eq!(selected.model_override.unwrap().model, "deepseek/deepseek-flash");
    }
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
        // The agents' health comes from the published views, not from the manager's memory
        // (`specs/048-agent-service-seam` contracts/shared-state-views.md).
        let failed: std::collections::HashSet<String> = failed_agents(deps).await;
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

/// Whether this agent has a recogniser the live voice session could use (FR-004).
///
/// Its own block wins; otherwise the agent's speech tool setting counts as configured only when it
/// names something, because `whisper` and `kokoro` are the defaults and an untouched default means
/// "nobody chose this".
pub fn has_recogniser(config: &AgentConfig) -> bool {
    config.tools.live_voice.settings.stt.is_some()
        || config.tools.stt.settings.provider != crate::schema::agent::SttProvider::default()
        || config.tools.stt.settings.model.is_some()
        || config.tools.stt.settings.language.is_some()
}

/// The same for the voice half of the session (FR-004).
pub fn has_voice(config: &AgentConfig) -> bool {
    config.tools.live_voice.settings.tts.is_some()
        || config.tools.tts.settings.provider != crate::schema::agent::TtsProvider::default()
        || config.tools.tts.settings.model.is_some()
        || config.tools.tts.settings.voice.is_some()
        || config.tools.tts.settings.speed.is_some()
}

/// Apply the live voice block of an update: `Some` sets it, `None` leaves what is stored.
///
/// Extracted so the rule can be tested without a `Services` handle. The "leave it" half is the one
/// that matters: the agent form does not send the block, and a save that quietly ended a call would
/// look like the feature breaking a second time.
fn merge_live_voice(
    config: &mut AgentConfig,
    input: Option<LiveVoiceInput>,
    existing: &AgentConfig,
) {
    config.tools.live_voice = match input {
        Some(input) => input.into_tool(),
        None => existing.tools.live_voice.clone(),
    };
}

/// Refuse a config whose live voice switch is on but which could never show the control (T041).
///
/// Saving it would be worse than refusing: the agent would look configured, the pill would never
/// appear, and the next person to look would have to read this code to find out why.
fn check_live_voice(config: &AgentConfig) -> Result<(), ServiceError> {
    if !config.tools.live_voice.enabled {
        return Ok(());
    }
    let missing: Vec<&str> = [
        (!has_recogniser(config)).then_some("a recogniser"),
        (!has_voice(config)).then_some("a voice"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(ServiceError::invalid(format!(
        "live_voice needs {}: set tools.live_voice.settings.{} to a provider, or give the agent its own stt/tts settings a non-default provider",
        missing.join(" and "),
        if missing.len() == 2 { "stt and .tts" } else if missing[0] == "a recogniser" { "stt" } else { "tts" }
    )))
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

    // FR-004: the session is offered only when the agent can actually hear and speak. Each half is
    // its own setting when the live voice block names one, and otherwise the agent's speech tool
    // setting — "configured" in the same sense the detail already reports for `stt` / `tts`.
    let has_recogniser = has_recogniser(&config);
    let has_voice = has_voice(&config);

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
        // FR-004: on only when the switch is on and both halves exist.
        live_voice: config.tools.live_voice.enabled && has_recogniser && has_voice,
        live_voice_end_of_utterance_ms: config.tools.live_voice.settings.end_of_utterance_ms,
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

/// A seam failure, as the command path reports it: a refusal is the `400` it has always
/// been, and an agent that could not be reached is the `500` a command that was not
/// delivered returns.
fn command_error(e: crate::error::VizierError) -> ServiceError {
    match crate::agents_client::refusal(&e.0) {
        Some(message) => ServiceError::invalid(message),
        None => ServiceError::Internal(format!("failed to send command: {}", e.0)),
    }
}

/// A seam failure, as the read path reports it: an agent that could not be reached is
/// the `503` the health reads have always returned.
fn unavailable(e: crate::error::VizierError) -> ServiceError {
    ServiceError::Unavailable(e.0)
}

pub async fn create(
    s: &Services,
    who: &Principal,
    body: CreateAgentRequest,
) -> Result<AgentSaved, ServiceError> {
    check_embedding(&body)?;

    let agent_id = body.agent_id.clone();
    let mut config = body.into_config();
    check_live_voice(&config)?;
    config.owner_id = Some(who.user_id.clone());
    let saved = config.clone();

    let summary = s
        .deps()
        .agents
        .create(&agent_id, config)
        .await
        .map_err(command_error)?;
    Ok(saved_with_warnings(s, &saved, summary).await)
}

pub async fn update(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: CreateAgentRequest,
) -> Result<AgentSaved, ServiceError> {
    check_embedding(&body)?;

    // Whether the body said anything about live voice, read before `into_config` consumes it.
    // `None` must mean "leave it alone": the agent form does not send the block today, and a save
    // that silently ended a running session would be a bug the person could not see (T041).
    let live_voice = body
        .tools
        .as_ref()
        .and_then(|tools| tools.live_voice.clone());

    let mut config = body.into_config();

    // Preserve existing owner_id and shared_to when updating
    let existing = load(s, agent_id).await?;
    if !can_edit_agent(who, &existing) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    merge_live_voice(&mut config, live_voice, &existing);
    check_live_voice(&config)?;

    config.owner_id = existing.owner_id;
    config.shared_to = existing.shared_to;
    let saved = config.clone();

    let agent_id = agent_id.to_string();
    let summary = s
        .deps()
        .agents
        .update(&agent_id, config)
        .await
        .map_err(command_error)?;
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
    s.deps()
        .agents
        .delete(&agent_id, delete_workspace)
        .await
        .map_err(command_error)?;
    // The agent's notifications went with it, and it can no longer be checked for who may
    // view it, so every client reloads.
    s.deps().transport.publish_inbox_event(crate::transport::InboxEvent::Stale {
        user: None,
        agent_id: None,
    });
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

    // The restart goes through the seam, like every other operation on an agent.
    match deps.agents.restart(agent_id).await {
        Ok(_) => {}
        Err(e) => {
            let message = match crate::agents_client::refusal(&e.0) {
                Some(message) => message,
                None => return Err(command_error(e)),
            };
            if message.contains("still being created or updated") {
                return Err(ServiceError::Conflict {
                    message,
                    code: None,
                    data: None,
                });
            }
            return Err(ServiceError::invalid(message));
        }
    }

    match deps.agents.health(agent_id).await {
        Ok(status) => Ok(status),
        Err(e) => Err(unavailable(e)),
    }
}

/// The storage query for a usage read, or the `400` refusing it.
fn usage_query(
    agent_id: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    activity: Option<&str>,
) -> Result<crate::storage::usage::UsageQuery, ServiceError> {
    let parse = |s: &str| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Local));
    let bad_range = |message: &str| with_code(400, message, "invalid_range");

    let end = match end_date {
        Some(s) => parse(s).map_err(|_| bad_range("end_date is not RFC 3339"))?,
        None => crate::time::now(),
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

/// The ids of the agents whose published health says they failed.
///
/// A missing or expired view reads as *unknown*, never as a stale value, so an agent the
/// fleet says nothing about is simply not failed.
async fn failed_agents(deps: &crate::dependencies::VizierDependencies) -> std::collections::HashSet<String> {
    let Ok(agents) = deps.storage.list_agents().await else {
        return Default::default();
    };
    let mut failed = std::collections::HashSet::new();
    for (id, _) in agents {
        let Some(status) = crate::agents::views::read_health(deps, &id).await else {
            continue;
        };
        if status.state == crate::schema::AgentRunState::Failed {
            failed.insert(id);
        }
    }
    failed
}

pub async fn health(s: &Services, _who: &Principal) -> Result<Vec<AgentHealthStatus>, ServiceError> {
    let deps = s.deps();
    // The health of every agent, read from the views the host published. An agent with no
    // live view is not in the list: its state is unknown, which is not the same as failed.
    let agents = deps.storage.list_agents().await.map_err(internal)?;
    let live = deps.storage.thinking_agents().await.unwrap_or_default();
    let mut statuses = Vec::new();
    for (id, _) in agents {
        let Some(mut status) = crate::agents::views::read_health(deps, &id).await else {
            continue;
        };
        status.live = status.alive && live.contains(&id);
        statuses.push(status);
    }
    Ok(statuses)
}

pub async fn ping(s: &Services, _who: &Principal, agent_id: &str) -> Result<PingResponse, ServiceError> {
    if !exists(s, agent_id).await {
        return Err(not_found_lowercase());
    }
    match s.deps().agents.health(agent_id).await {
        Ok(status) => Ok(PingResponse { alive: status.alive }),
        Err(e) => Err(unavailable(e)),
    }
}

pub async fn chat(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: ChatRequest,
) -> Result<ChatResponse, ServiceError> {
    let deps = s.deps();
    let who = if body.model_override.is_some() {
        s.resolve(&who.user_id, who.method.clone()).await?
    } else {
        who.clone()
    };
    if let Some(choice) = &body.model_override {
        if body.channel_id != "vizier-webui"
            && !crate::projects::channel::is_project_channel(&body.channel_id)
        {
            return Err(ServiceError::Invalid {
                status: 422,
                message: "model override is supported only for WebUI chat channels".into(),
                code: Some("model_override_unsupported".into()),
            });
        }
        crate::access::require(&who, "model_override")?;
        let choice = ModelOverride { provider: choice.provider.clone(), model: choice.model.clone() };
        crate::service::catalogue::validate_chat_model_override(deps, &choice).await?;
    }
    let config = match deps.storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(not_found_lowercase()),
        Err(e) => return Err(ServiceError::Internal(format!("failed to get agent: {}", e))),
    };
    if !can_view_agent(&who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    // A REST chat naming a project channel goes through the same matrix as its socket.
    super::topics::authorize_project_channel(s, &who, &config, agent_id, &body.channel_id).await?;

    let transport = deps.transport.clone();
    let session = VizierSession(
        agent_id.to_string(),
        VizierChannelId::from_web(&who.username, &body.channel_id),
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
        timestamp: crate::time::now(),
        user: who.username.clone(),
        content: VizierRequestContent::Chat(body.content),
        model_override: body.model_override.map(|choice| ModelOverride {
            provider: choice.provider,
            model: choice.model,
        }),
        platform_message_id: None,
        metadata: serde_json::json!({}),
        attachments,
        expect_audio_reply: None,
        scheduled_task: None,
        background_depth: 0,
        ..Default::default()
    };

    let (response_tx, response_rx) = transport.reply_channel().await.map_err(|e| ServiceError::Internal(format!("failed to open a reply stream: {e}")))?;
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
        VizierChannelId::from_web(&who.username, &body.channel_id),
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

#[cfg(test)]
mod live_voice_input_tests {
    //! T041 — the switch that made the whole feature unreachable. `into_config` used to hard-code
    //! `enabled: false`, so an API caller could ask for the session and be silently refused.

    use super::*;
    use crate::schema::agent::{SttProvider, SttToolSettings, TtsProvider, TtsToolSettings};

    fn body(tools: serde_json::Value) -> CreateAgentRequest {
        serde_json::from_value(serde_json::json!({
            "agent_id": "helper",
            "name": "helper",
            "provider": "openai",
            "model": "gpt",
            "tools": tools,
        }))
        .expect("a request with tools")
    }

    #[test]
    fn a_body_without_the_block_leaves_the_session_off() {
        // Regression: every agent that existed before T041 keeps exactly what it had.
        let config = body(serde_json::json!({ "fetch": true })).into_config();
        assert!(!config.tools.live_voice.enabled);
        assert!(config.tools.live_voice.settings.stt.is_none());
        assert!(check_live_voice(&config).is_ok(), "off is never refused");
    }

    #[test]
    fn a_body_that_turns_it_on_is_honoured() {
        let config = body(serde_json::json!({
            "live_voice": {
                "enabled": true,
                "settings": {
                    "stt": { "provider": "groq", "language": "id" },
                    "tts": { "provider": "elevenlabs", "voice": "rachel" }
                }
            }
        }))
        .into_config();
        assert!(config.tools.live_voice.enabled);
        let stt = config.tools.live_voice.settings.stt.as_ref().expect("a recogniser");
        assert_eq!(stt.provider, SttProvider::Groq);
        assert_eq!(stt.language.as_deref(), Some("id"));
        let tts = config.tools.live_voice.settings.tts.as_ref().expect("a voice");
        assert_eq!(tts.provider, TtsProvider::Elevenlabs);
        // The numbers the block did not name keep the spec's defaults.
        assert_eq!(config.tools.live_voice.settings.idle_stop_seconds, 180);
        assert!(check_live_voice(&config).is_ok());
        // And the detail's gate agrees: on, because both halves exist.
        assert!(config.tools.live_voice.enabled && has_recogniser(&config) && has_voice(&config));
    }

    #[test]
    fn turning_it_on_without_a_recogniser_or_a_voice_is_refused() {
        let config = body(serde_json::json!({ "live_voice": { "enabled": true } })).into_config();
        let refused = check_live_voice(&config).expect_err("nothing to hear with, nothing to speak with");
        let message = refused.message();
        assert_eq!(refused.status(), 400);
        assert!(message.contains("a recogniser"), "{message}");
        assert!(message.contains("a voice"), "{message}");
        assert!(message.contains("tools.live_voice.settings"), "says where to fix it: {message}");
    }

    #[test]
    fn the_agents_own_speech_settings_are_enough() {
        // A person who already configured stt/tts on the agent does not have to repeat them.
        let mut config = body(serde_json::json!({ "live_voice": { "enabled": true } })).into_config();
        config.tools.stt.settings = SttToolSettings {
            provider: SttProvider::Openai,
            ..Default::default()
        };
        config.tools.tts.settings = TtsToolSettings {
            provider: TtsProvider::Openai,
            ..Default::default()
        };
        assert!(check_live_voice(&config).is_ok());
        assert!(has_recogniser(&config) && has_voice(&config));
    }

    #[test]
    fn saving_an_agent_without_the_block_keeps_a_live_session_on() {
        // The half that matters most: the agent form does not send the block, and a save must not
        // end a session the person is in the middle of.
        let mut stored = body(serde_json::json!({
            "live_voice": { "enabled": true, "settings": { "stt": { "provider": "groq" }, "tts": { "provider": "openai" } } }
        }))
        .into_config();
        stored.name = "stored".into();

        let mut incoming = body(serde_json::json!({ "fetch": true })).into_config();
        merge_live_voice(&mut incoming, None, &stored);
        assert!(incoming.tools.live_voice.enabled, "still on");
        assert_eq!(incoming.tools.live_voice.settings.stt.as_ref().unwrap().provider, SttProvider::Groq);

        // And a body that does send the block replaces it.
        let asked = Some(LiveVoiceInput {
            enabled: false,
            settings: None,
        });
        merge_live_voice(&mut incoming, asked, &stored);
        assert!(!incoming.tools.live_voice.enabled, "an explicit off is honoured");
    }
}
