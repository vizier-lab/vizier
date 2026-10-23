use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    config::shell::ShellConfig,
    schema::{
        AgentCommand, AgentCommandResult, AgentConfig, AgentHealthStatus, AgentSummary,
        AgentToolsConfig, EmbeddingConfig,
        ImageGenToolSettings, IndexerConfig, ReadImageToolSettings, ToolConfig,
        TtsToolSettings,
        agent::{EmbeddingProvider, IndexerKind, SttToolSettings},
        VizierAttachment, VizierChannelId, VizierRequest, VizierRequestContent,
        VizierResponseContent, VizierResponseStats, VizierSession,
    },
    storage::{VizierStorage, agent::AgentStorage, history::HistoryStorage, user::UserStorage},
};

pub mod conversations;
pub mod channel;
pub mod core;
pub mod dream;
pub mod integrations;
pub mod jobs;
pub mod memory;
pub mod skills;
pub mod task;

use channel::channel;
use crate::channels::http::models::page::{self, ListQuery, Listing, SortKey, SortOrder};
use crate::storage::session::SessionStorage;
use core::core;
use dream::dream;
use jobs::jobs;
use memory::memory;
use skills::agent_skills;
use task::task;

pub fn user_can_view_agent(
    user: &crate::channels::http::auth::AuthenticatedUser,
    config: &AgentConfig,
) -> bool {
    if user.role.is_system || user.permissions.contains(&"all_agents:view".to_string()) {
        return true;
    }
    if let Some(ref owner_id) = config.owner_id {
        if *owner_id == user.user_id {
            return true;
        }
    }
    if config.shared_to.contains(&user.user_id) {
        return true;
    }
    false
}

fn user_can_edit_agent(
    user: &crate::channels::http::auth::AuthenticatedUser,
    config: &AgentConfig,
) -> bool {
    if user.role.is_system || user.permissions.contains(&"all_agents:edit".to_string()) {
        return true;
    }
    if let Some(ref owner_id) = config.owner_id {
        if *owner_id == user.user_id {
            return true;
        }
    }
    false
}

pub fn agents() -> Router<HTTPState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/", get(list_agents).post(create_agent))
        .route(
            "/{agent_id}",
            get(agent_detail).put(update_agent).delete(delete_agent),
        )
        .route("/{agent_id}/usage", get(agent_usage))
        .route("/{agent_id}/usage/conversation", get(agent_usage_conversation))
        .route("/{agent_id}/ping", get(ping_agent))
        .route("/{agent_id}/restart", post(restart_agent))
        .route("/{agent_id}/chat", post(agent_chat))
        .route(
            "/{agent_id}/sharing",
            get(get_sharing).patch(update_sharing),
        )
        .nest("/{agent_id}/channel", channel().merge(jobs()))
        .nest("/{agent_id}/core", core())
        .nest("/{agent_id}/memory", memory())
        .nest("/{agent_id}/skills", agent_skills())
        .nest("/{agent_id}/tasks", task())
        .nest("/{agent_id}/dream", dream())
        .nest("/{agent_id}/integrations", integrations::integrations())
        .nest("/{agent_id}/conversations", conversations::conversations())
}

/// One row of `GET /agents`: the summary, plus when the agent was last active.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AgentListItem {
    #[serde(flatten)]
    pub summary: AgentSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip)]
    #[schema(ignore)]
    mine: bool,
    #[serde(skip)]
    #[schema(ignore)]
    live: bool,
    #[serde(skip)]
    #[schema(ignore)]
    failed: bool,
}

/// Filters `mine`/`live`/`failed`; sorts `last_active` (newest first) and `name`.
struct AgentListing;

impl Listing<AgentListItem> for AgentListing {
    fn matches(&self, item: &AgentListItem, q: &str) -> bool {
        item.summary.name.to_lowercase().contains(q)
            || item.summary.agent_id.to_lowercase().contains(q)
            || item.summary.description.as_deref().is_some_and(|d| d.to_lowercase().contains(q))
    }

    fn filter_keys(&self, _: &[AgentListItem]) -> Vec<String> {
        vec!["mine".into(), "live".into(), "failed".into()]
    }

    fn in_filter(&self, filter: &str, item: &AgentListItem) -> bool {
        match filter {
            "mine" => item.mine,
            "live" => item.live,
            "failed" => item.failed,
            _ => false,
        }
    }

    fn sort_key(&self, item: &AgentListItem, sort: &str) -> SortKey {
        match sort {
            "name" => SortKey::text(&item.summary.name),
            _ => SortKey::Num(item.last_active.map_or(0, |t| t.timestamp_millis())),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["last_active", "name"]
    }

    fn default_order(&self, sort: &str) -> SortOrder {
        if sort == "last_active" { SortOrder::Desc } else { SortOrder::Asc }
    }
}

#[utoipa::path(
    get,
    path = "/agents",
    params(ListQuery),
    responses(
        (status = 200, description = "List of agents; with `limit`, a page `{items, total, counts}` (filters `mine`, `live`, `failed`; sorts `last_active`, `name`)", body = APIResponse<Vec<AgentListItem>>),
        (status = 400, description = "Unknown filter", body = APIResponse<String>)
    )
)]
async fn list_agents(
    State(state): State<HTTPState>,
    Query(query): Query<ListQuery>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<serde_json::Value> {
    let agents = match state.storage.list_agents().await {
        Ok(agents) => agents,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    // Only what the listing needs: skipped without a `limit`, which has always been just names.
    let (last_active, live, failed) = if query.is_paged() {
        let failed: std::collections::HashSet<String> = state
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
            state.storage.last_activity_by_agent().await.unwrap_or_default(),
            state.storage.thinking_agents().await.unwrap_or_default(),
            failed,
        )
    } else {
        Default::default()
    };

    let mut res = Vec::new();
    for (id, config) in agents {
        if !user_can_view_agent(&user, &config) {
            continue;
        }

        let owner_username = if let Some(ref owner_id) = config.owner_id {
            state.storage.get_user_by_id(owner_id).await.ok().flatten().map(|u| u.username)
        } else {
            None
        };
        res.push(AgentListItem {
            last_active: last_active.get(&id).copied(),
            mine: config.owner_id.as_deref() == Some(user.user_id.as_str()),
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
    page::respond(res, &AgentListing, &query)
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
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

#[utoipa::path(
    get,
    path = "/agents/{agent_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Agent details", body = APIResponse<AgentDetail>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
async fn agent_detail(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<AgentDetail> {
    match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => {
            if !user_can_view_agent(&user, &config) {
                return err_response(StatusCode::FORBIDDEN, "Access denied".into());
            }

            api_response(
                StatusCode::OK,
                AgentDetail {
                agent_id,
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
            },
        )
        }
        Ok(None) => err_response(StatusCode::NOT_FOUND, "not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
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

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
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
    fn into_config(self) -> AgentConfig {
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
            indexer: Some(
                self.indexer
                    .unwrap_or(IndexerConfig { kind: IndexerKind::Sqlite }),
            ),
            chunking: Default::default(),
            auto_context: Default::default(),
            completion_retry: Default::default(),
        }
    }
}

/// Settings that moved to Integrations (spec 016, FR-021): a body that still carries any is
/// refused rather than silently ignored.
const MOVED_TO_INTEGRATIONS: &[&str] = &["discord_token", "telegram_token"];
const TOOLS_MOVED_TO_INTEGRATIONS: &[&str] = &["discord", "telegram", "brave_search", "mcp_servers"];

/// The moved fields `body` carries, as `tools.x` for those under `tools`.
fn moved_fields(body: &serde_json::Value) -> Vec<String> {
    let mut fields: Vec<String> = MOVED_TO_INTEGRATIONS
        .iter()
        .filter(|key| body.get(**key).is_some())
        .map(|key| key.to_string())
        .collect();
    if let Some(tools) = body.get("tools") {
        fields.extend(
            TOOLS_MOVED_TO_INTEGRATIONS
                .iter()
                .filter(|key| tools.get(**key).is_some())
                .map(|key| format!("tools.{key}")),
        );
    }
    fields
}

/// The agent body, or the response refusing it: `422 moved_to_integrations` naming the moved
/// fields, or `400` when it doesn't parse.
fn parse_agent_body(body: serde_json::Value) -> Result<CreateAgentRequest, axum::response::Response> {
    use axum::response::IntoResponse;
    if body.get("self_management").is_some() {
        return Err(crate::channels::http::api::v1::integrations::fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            "self-management is set through /agents/{id}/integrations/grants, by someone who manages the agent's integrations",
            serde_json::json!({"error": "use_grants_endpoint"}),
        ));
    }
    let moved = moved_fields(&body);
    if !moved.is_empty() {
        return Err(crate::channels::http::api::v1::integrations::fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "{} moved to Integrations; manage them under /agents/{{id}}/integrations",
                moved.join(", ")
            ),
            serde_json::json!({"error": "moved_to_integrations", "fields": moved}),
        ));
    }
    serde_json::from_value(body)
        .map_err(|e| err_response::<String>(StatusCode::BAD_REQUEST, e.to_string()).into_response())
}

/// A saved agent, with anything about its model worth knowing (contract `http-api.md` §
/// Agents).
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AgentSaved {
    #[serde(flatten)]
    pub summary: AgentSummary,
    pub warnings: Vec<crate::catalogue::AgentWarning>,
}

async fn saved_with_warnings(
    state: &HTTPState,
    config: &crate::schema::AgentConfig,
    summary: AgentSummary,
) -> AgentSaved {
    use crate::storage::provider::CustomProviderStorage;
    let custom = state
        .storage
        .get_custom_provider(&config.provider)
        .await
        .ok()
        .flatten();
    AgentSaved {
        summary,
        warnings: crate::catalogue::agent_warnings(&state.catalogue, custom.as_ref(), config),
    }
}

#[utoipa::path(
    post,
    path = "/agents",
    request_body = CreateAgentRequest,
    responses(
        (status = 201, description = "Agent created", body = APIResponse<AgentSaved>),
        (status = 400, description = "Bad request", body = APIResponse<String>),
        (status = 422, description = "The body carries settings that moved to Integrations", body = APIResponse<String>)
    )
)]
async fn create_agent(
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match parse_agent_body(body) {
        Ok(body) => create_agent_parsed(state, user, body).await.into_response(),
        Err(refused) => refused,
    }
}

async fn create_agent_parsed(
    state: HTTPState,
    user: crate::channels::http::auth::AuthenticatedUser,
    body: CreateAgentRequest,
) -> models::response::Response<AgentSaved> {
    if let Some(emb) = &body.embedding {
        if emb.model.trim().is_empty() {
            return err_response(StatusCode::BAD_REQUEST, "embedding.model is required".into());
        }
    }

    let agent_id = body.agent_id.clone();
    let mut config = body.into_config();
    config.owner_id = Some(user.user_id);
    let saved = config.clone();

    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();

    let cmd = AgentCommand::Create {
        agent_id,
        config,
        resp: resp_tx,
    };

    if let Err(e) = state.transport.send_agent_command(cmd).await {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to send command: {}", e),
        );
    }

    match resp_rx.await {
        Ok(AgentCommandResult::Ok(summary)) => {
            api_response(StatusCode::CREATED, saved_with_warnings(&state, &saved, summary).await)
        }
        Ok(AgentCommandResult::Error(e)) => err_response(StatusCode::BAD_REQUEST, e),
        Err(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "agent manager unavailable".into(),
        ),
    }
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = CreateAgentRequest,
    responses(
        (status = 200, description = "Agent updated", body = APIResponse<AgentSaved>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 422, description = "The body carries settings that moved to Integrations", body = APIResponse<String>)
    )
)]
async fn update_agent(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match parse_agent_body(body) {
        Ok(body) => update_agent_parsed(agent_id, state, user, body).await.into_response(),
        Err(refused) => refused,
    }
}

async fn update_agent_parsed(
    agent_id: String,
    state: HTTPState,
    user: crate::channels::http::auth::AuthenticatedUser,
    body: CreateAgentRequest,
) -> models::response::Response<AgentSaved> {
    if let Some(emb) = &body.embedding {
        if emb.model.trim().is_empty() {
            return err_response(StatusCode::BAD_REQUEST, "embedding.model is required".into());
        }
    }

    let mut config = body.into_config();

    // Preserve existing owner_id and shared_to when updating
    let existing = match state.storage.get_agent(&agent_id).await {
        Ok(Some(existing)) => existing,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_edit_agent(&user, &existing) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    config.owner_id = existing.owner_id;
    config.shared_to = existing.shared_to;
    let saved = config.clone();

    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();

    let cmd = AgentCommand::Update {
        agent_id,
        config,
        resp: resp_tx,
    };

    if let Err(e) = state.transport.send_agent_command(cmd).await {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to send command: {}", e),
        );
    }

    match resp_rx.await {
        Ok(AgentCommandResult::Ok(summary)) => {
            api_response(StatusCode::OK, saved_with_warnings(&state, &saved, summary).await)
        }
        Ok(AgentCommandResult::Error(e)) => err_response(StatusCode::BAD_REQUEST, e),
        Err(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "agent manager unavailable".into(),
        ),
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct DeleteAgentQuery {
    #[serde(default)]
    pub delete_workspace: bool,
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Agent deleted", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
async fn delete_agent(
    Path(agent_id): Path<String>,
    Query(query): Query<DeleteAgentQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<String> {
    let can_delete_all = user.role.is_system || user.permissions.contains(&"all_agents:delete".to_string());
    let can_delete_owned = user.permissions.contains(&"owned_agents:delete".to_string());

    // Check permission
    if !can_delete_all {
        let agent = match state.storage.get_agent(&agent_id).await {
            Ok(Some(agent)) => agent,
            Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
            Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        if let Some(ref owner_id) = agent.owner_id {
            if *owner_id != user.user_id && !can_delete_owned {
                return err_response(StatusCode::FORBIDDEN, "Access denied".into());
            }
        } else if !can_delete_owned {
            return err_response(StatusCode::FORBIDDEN, "Access denied".into());
        }
    }

    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();

    let cmd = AgentCommand::Delete {
        agent_id,
        delete_workspace: query.delete_workspace,
        resp: resp_tx,
    };

    if let Err(e) = state.transport.send_agent_command(cmd).await {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to send command: {}", e),
        );
    }

    match resp_rx.await {
        Ok(AgentCommandResult::Ok(_)) => api_response(StatusCode::OK, "deleted".into()),
        Ok(AgentCommandResult::Error(e)) => err_response(StatusCode::BAD_REQUEST, e),
        Err(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "agent manager unavailable".into(),
        ),
    }
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/restart",
    params(("agent_id" = String, Path, description = "Agent ID")),
    responses(
        (status = 202, description = "Restart started; the agent's health follows", body = APIResponse<AgentHealthStatus>),
        (status = 403, description = "Not allowed to edit this agent", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 409, description = "The agent is still being created or updated", body = APIResponse<String>)
    )
)]
pub(crate) async fn restart_agent(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<AgentHealthStatus> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    if !user_can_edit_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let (resp, rx) = tokio::sync::oneshot::channel();
    if let Err(e) = state
        .transport
        .send_agent_command(AgentCommand::Restart { agent_id: agent_id.clone(), resp })
        .await
    {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("failed to send command: {e}"));
    }
    match rx.await {
        Ok(AgentCommandResult::Ok(_)) => {}
        Ok(AgentCommandResult::Error(e)) if e.contains("still being created or updated") => {
            return err_response(StatusCode::CONFLICT, e);
        }
        Ok(AgentCommandResult::Error(e)) => return err_response(StatusCode::BAD_REQUEST, e),
        Err(_) => {
            return err_response(StatusCode::INTERNAL_SERVER_ERROR, "agent manager unavailable".into());
        }
    }

    match state.transport.agent_health().await {
        Ok(statuses) => match statuses.into_iter().find(|s| s.agent_id == agent_id) {
            Some(status) => api_response(StatusCode::ACCEPTED, status),
            None => err_response(StatusCode::NOT_FOUND, "agent not found".into()),
        },
        Err(e) => err_response(StatusCode::SERVICE_UNAVAILABLE, e.to_string()),
    }
}

#[derive(Debug, Deserialize, Serialize, utoipa::IntoParams)]
pub struct UsageParams {
    /// RFC 3339. Default: `end_date` − 30 days.
    pub start_date: Option<String>,
    /// RFC 3339. Default: now.
    pub end_date: Option<String>,
    /// Narrows every figure to one activity (spec 018 FR-021).
    pub activity: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, utoipa::IntoParams)]
pub struct ConversationUsageParams {
    /// A `channel` from the report's `costliest` (a session slug).
    pub channel: Option<String>,
    pub topic: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub activity: Option<String>,
    /// Calls to return, 1–100. Default 20.
    pub limit: Option<usize>,
}

/// The longest range a usage read covers.
const MAX_USAGE_RANGE_DAYS: i64 = 366;

/// The storage query for a usage read, or the `400` refusing it. The refusal is a whole
/// response, like `parse_agent_body`'s.
#[allow(clippy::result_large_err)]
fn usage_query(
    agent_id: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    activity: Option<&str>,
) -> Result<crate::storage::usage::UsageQuery, axum::response::Response> {
    use crate::channels::http::api::v1::integrations::fail;
    let parse = |s: &str| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc));
    let bad_range = |message: &str| {
        fail(StatusCode::BAD_REQUEST, message, serde_json::json!({"error": "invalid_range"}))
    };

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
        Some(a) => Some(a.parse::<crate::schema::usage::UsageActivity>().map_err(|_| {
            fail(
                StatusCode::BAD_REQUEST,
                format!("unknown activity '{a}'"),
                serde_json::json!({"error": "unknown_activity"}),
            )
        })?),
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

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/usage",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        UsageParams,
    ),
    responses(
        (status = 200, description = "Agent usage report", body = APIResponse<crate::schema::usage::UsageReport>),
        (status = 400, description = "invalid_range or unknown_activity", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
async fn agent_usage(
    Path(agent_id): Path<String>,
    Query(params): Query<UsageParams>,
    State(state): State<HTTPState>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !state.is_agent_exists(&agent_id).await {
        return err_response::<String>(StatusCode::NOT_FOUND, "agent not found".into()).into_response();
    }
    let q = match usage_query(
        &agent_id,
        params.start_date.as_deref(),
        params.end_date.as_deref(),
        params.activity.as_deref(),
    ) {
        Ok(q) => q,
        Err(refused) => return refused,
    };
    match crate::usage::build_report(&state.storage, &state.catalogue, &state.deps.integration_kinds, q).await {
        Ok(report) => api_response(StatusCode::OK, report).into_response(),
        Err(e) => err_response::<String>(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/usage/conversation",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ConversationUsageParams,
    ),
    responses(
        (status = 200, description = "One conversation's usage, calls most expensive first", body = APIResponse<crate::schema::usage::ConversationUsage>),
        (status = 400, description = "missing_channel, invalid_limit, invalid_range or unknown_activity", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
async fn agent_usage_conversation(
    Path(agent_id): Path<String>,
    Query(params): Query<ConversationUsageParams>,
    State(state): State<HTTPState>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    use crate::channels::http::api::v1::integrations::fail;
    if !state.is_agent_exists(&agent_id).await {
        return err_response::<String>(StatusCode::NOT_FOUND, "agent not found".into()).into_response();
    }
    let Some(channel) = params.channel.filter(|c| !c.is_empty()) else {
        return fail(
            StatusCode::BAD_REQUEST,
            "channel is required",
            serde_json::json!({"error": "missing_channel"}),
        );
    };
    let limit = params.limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return fail(
            StatusCode::BAD_REQUEST,
            "limit must be between 1 and 100",
            serde_json::json!({"error": "invalid_limit"}),
        );
    }
    let mut q = match usage_query(
        &agent_id,
        params.start_date.as_deref(),
        params.end_date.as_deref(),
        params.activity.as_deref(),
    ) {
        Ok(q) => q,
        Err(refused) => return refused,
    };
    q.conversation = Some((channel, params.topic.filter(|t| !t.is_empty())));
    match crate::usage::conversation_usage(&state.storage, &state.catalogue, &state.deps.integration_kinds, q, limit).await {
        Ok(usage) => api_response(StatusCode::OK, usage).into_response(),
        Err(e) => err_response::<String>(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SharingResponse {
    pub shared_to: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/sharing",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Agent sharing list", body = APIResponse<SharingResponse>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
async fn get_sharing(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<SharingResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_edit_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    api_response(
        StatusCode::OK,
        SharingResponse {
            shared_to: config.shared_to,
        },
    )
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateSharingRequest {
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub remove: Vec<String>,
}

#[utoipa::path(
    patch,
    path = "/agents/{agent_id}/sharing",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = UpdateSharingRequest,
    responses(
        (status = 200, description = "Agent sharing updated", body = APIResponse<SharingResponse>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
async fn update_sharing(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<UpdateSharingRequest>,
) -> models::response::Response<SharingResponse> {
    let mut config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_edit_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    for user_id in &body.add {
        if !config.shared_to.contains(user_id) {
            config.shared_to.push(user_id.clone());
        }
    }
    config.shared_to.retain(|id| !body.remove.contains(id));

    if let Err(e) = state.storage.update_agent(&agent_id, &config).await {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    api_response(
        StatusCode::OK,
        SharingResponse {
            shared_to: config.shared_to,
        },
    )
}

#[utoipa::path(
    get,
    path = "/agents/health",
    responses(
        (status = 200, description = "Health status of all agents", body = APIResponse<Vec<AgentHealthStatus>>)
    )
)]
async fn health_check(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<AgentHealthStatus>> {
    match state.transport.agent_health().await {
        Ok(mut statuses) => {
            let live = state.storage.thinking_agents().await.unwrap_or_default();
            for s in &mut statuses {
                s.live = s.alive && live.contains(&s.agent_id.to_string());
            }
            api_response(StatusCode::OK, statuses)
        }
        Err(e) => err_response(StatusCode::SERVICE_UNAVAILABLE, e.to_string()),
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct PingResponse {
    pub alive: bool,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/ping",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Agent ping result", body = APIResponse<PingResponse>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
async fn ping_agent(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<PingResponse> {
    if !state.is_agent_exists(&agent_id).await {
        return err_response(StatusCode::NOT_FOUND, "agent not found".into());
    }

    match state.transport.agent_health().await {
        Ok(statuses) => {
            let alive = statuses.iter().any(|s| s.agent_id == agent_id && s.alive);
            api_response(StatusCode::OK, PingResponse { alive })
        }
        Err(e) => err_response(StatusCode::SERVICE_UNAVAILABLE, e.to_string()),
    }
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct ChatRequest {
    pub channel_id: String,
    pub topic_id: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Option<Vec<VizierAttachment>>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ChatResponse {
    pub content: String,
    pub stats: Option<VizierResponseStats>,
    pub attachments: Vec<VizierAttachment>,
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/chat",
    params(("agent_id" = String, Path, description = "Agent ID")),
    request_body = ChatRequest,
    responses(
        (status = 200, description = "Agent chat response", body = APIResponse<ChatResponse>),
        (status = 404, description = "Agent not found"),
        (status = 403, description = "Access denied"),
        (status = 408, description = "Agent response timeout"),
    )
)]
async fn agent_chat(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<ChatRequest>,
) -> models::response::Response<ChatResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "agent not found".into()),
        Err(e) => {
            return err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to get agent: {}", e),
            );
        }
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let transport = state.transport.clone();
    let session = VizierSession(
        agent_id,
        VizierChannelId::HTTP(user.username.clone(), body.channel_id.clone()),
        Some(body.topic_id.clone()),
    );

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
        user: user.username.clone(),
        content: VizierRequestContent::Chat(body.content),
        platform_message_id: None,
        metadata: serde_json::json!({}),
        attachments,
        expect_audio_reply: None,
        scheduled_task: None,
        background_depth: 0,
    };

    let (response_tx, response_rx) = flume::unbounded();
    if let Err(err) = transport
        .send_request(session.clone(), request, Some(response_tx))
        .await
    {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to send request: {}", err),
        );
    }

    let timeout_duration = *config.prompt_timeout;
    match tokio::time::timeout(timeout_duration, async {
        loop {
            match response_rx.recv_async().await {
                Ok(response) => match &response.content {
                    VizierResponseContent::Message { content, stats } => {
                        return Ok(ChatResponse {
                            content: content.clone(),
                            stats: stats.clone(),
                            attachments: response.attachments,
                        });
                    }
                    VizierResponseContent::Abort => {
                        return Err("Agent aborted the request".to_string());
                    }
                    VizierResponseContent::Error { kind, message } => {
                        return Err(format!("{}: {}", kind.label(), message));
                    }
                    _ => continue,
                },
                Err(_) => {
                    return Err("Agent response channel closed".to_string());
                }
            }
        }
    })
    .await
    {
        Ok(Ok(res)) => api_response(StatusCode::OK, res),
        Ok(Err(e)) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e),
        Err(_) => err_response(StatusCode::REQUEST_TIMEOUT, "Agent response timeout".into()),
    }
}

