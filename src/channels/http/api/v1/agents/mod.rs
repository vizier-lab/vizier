use axum::{
    Router,
    Json,
    extract::{Path, Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response, APIResponse},
        },
        state::HTTPState,
    },
    schema::{
        AgentCommand, AgentCommandResult, AgentConfig, AgentSummary, AgentToolsConfig,
        AgentUsageStats, BraveSearchToolSettings, MemoryConfig, ToolConfig,
    },
    storage::{agent::AgentStorage, history::HistoryStorage, VizierStorage},
};

pub mod channel;
pub mod documents;
pub mod memory;
pub mod task;

use channel::channel;
use documents::documents;
use memory::memory;
use task::task;

pub fn agents() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_agents).post(create_agent))
        .route(
            "/{agent_id}",
            get(agent_detail).put(update_agent).delete(delete_agent),
        )
        .route("/{agent_id}/usage", get(agent_usage))
        .nest("/{agent_id}/channel", channel())
        .nest("/{agent_id}/documents", documents())
        .nest("/{agent_id}/memory", memory())
        .nest("/{agent_id}/tasks", task())
}

#[utoipa::path(
    get,
    path = "/agents",
    responses(
        (status = 200, description = "List of agents", body = APIResponse<Vec<AgentSummary>>)
    )
)]
async fn list_agents(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<AgentSummary>> {
    match state.storage.list_agents().await {
        Ok(agents) => {
            let res: Vec<AgentSummary> = agents
                .into_iter()
                .map(|(id, config)| AgentSummary {
                    agent_id: id,
                    name: config.name,
                    description: config.description,
                })
                .collect();
            api_response(StatusCode::OK, res)
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AgentDetail {
    pub agent_id: String,
    pub name: String,
    pub description: Option<String>,
    pub provider: crate::config::provider::ProviderVariant,
    pub model: String,
    pub system_prompt: Option<String>,
    pub thinking_depth: usize,
    pub session_memory_capacity: usize,
    pub max_tokens: Option<u64>,
    pub shell_access: bool,
    pub brave_search: bool,
    pub brave_search_settings: Option<BraveSearchToolSettings>,
    pub vector_memory: bool,
    pub discord: bool,
    pub telegram: bool,
    pub fetch: bool,
    pub http_client: bool,
    pub prompt_timeout: String,
    pub heartbeat_interval: String,
    pub dream_interval: String,
    pub discord_token: Option<String>,
    pub telegram_token: Option<String>,
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
) -> models::response::Response<AgentDetail> {
    match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => api_response(
            StatusCode::OK,
            AgentDetail {
                agent_id,
                name: config.name,
                description: config.description,
                provider: config.provider,
                model: config.model,
                system_prompt: config.system_prompt,
                thinking_depth: config.thinking_depth,
                session_memory_capacity: config.session_memory.max_capacity,
                max_tokens: config.max_tokens,
                shell_access: config.tools.shell_access,
                brave_search: config.tools.brave_search.enabled,
                brave_search_settings: if config.tools.brave_search.settings.api_key.is_some()
                    || config.tools.brave_search.settings.safesearch.is_some()
                {
                    Some(config.tools.brave_search.settings.clone())
                } else {
                    None
                },
                vector_memory: config.tools.vector_memory.enabled,
                discord: config.tools.discord.enabled,
                telegram: config.tools.telegram.enabled,
                fetch: config.tools.fetch.enabled,
                http_client: config.tools.http_client.enabled,
                prompt_timeout: config.prompt_timeout.to_string(),
                heartbeat_interval: config.heartbeat_interval.to_string(),
                dream_interval: config.dream_interval.to_string(),
                discord_token: config.discord_token,
                telegram_token: config.telegram_token,
            },
        ),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct CreateAgentRequest {
    pub agent_id: String,
    pub name: String,
    pub description: Option<String>,
    pub provider: crate::config::provider::ProviderVariant,
    pub model: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub thinking_depth: Option<usize>,
    #[serde(default)]
    pub session_memory_capacity: Option<usize>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub tools: Option<CreateAgentTools>,
    #[serde(default)]
    pub prompt_timeout: Option<String>,
    #[serde(default)]
    pub heartbeat_interval: Option<String>,
    #[serde(default)]
    pub dream_interval: Option<String>,
    #[serde(default)]
    pub discord_token: Option<String>,
    #[serde(default)]
    pub telegram_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct CreateAgentTools {
    #[serde(default)]
    pub shell_access: Option<bool>,
    #[serde(default)]
    pub brave_search: Option<bool>,
    #[serde(default)]
    pub brave_search_settings: Option<BraveSearchToolSettings>,
    #[serde(default)]
    pub vector_memory: Option<bool>,
    #[serde(default)]
    pub discord: Option<bool>,
    #[serde(default)]
    pub telegram: Option<bool>,
    #[serde(default)]
    pub fetch: Option<bool>,
    #[serde(default)]
    pub http_client: Option<bool>,
}

impl CreateAgentRequest {
    fn into_config(self) -> AgentConfig {
        let tools = self.tools.unwrap_or(CreateAgentTools {
            shell_access: None,
            brave_search: None,
            brave_search_settings: None,
            vector_memory: None,
            discord: None,
            telegram: None,
            fetch: None,
            http_client: None,
        });

        AgentConfig {
            name: self.name,
            system_prompt: self.system_prompt,
            description: self.description,
            provider: self.provider,
            model: self.model,
            session_memory: MemoryConfig {
                max_capacity: self.session_memory_capacity.unwrap_or(10),
            },
            thinking_depth: self.thinking_depth.unwrap_or(10),
            max_tokens: self.max_tokens,
            tools: AgentToolsConfig {
                timeout: duration_string::DurationString::from_string("1m".into()).unwrap(),
                programmatic_sandbox: false,
                shell_access: tools.shell_access.unwrap_or(false),
                brave_search: ToolConfig {
                    enabled: tools.brave_search.unwrap_or(false),
                    settings: tools.brave_search_settings.unwrap_or_default(),
                },
                vector_memory: ToolConfig {
                    enabled: tools.vector_memory.unwrap_or(true),
                    settings: (),
                },
                discord: ToolConfig {
                    enabled: tools.discord.unwrap_or(false),
                    settings: (),
                },
                telegram: ToolConfig {
                    enabled: tools.telegram.unwrap_or(false),
                    settings: (),
                },
                notify_primary_user: ToolConfig { enabled: true, settings: () },
                fetch: ToolConfig {
                    enabled: tools.fetch.unwrap_or(false),
                    settings: (),
                },
                http_client: ToolConfig {
                    enabled: tools.http_client.unwrap_or(false),
                    settings: (),
                },
                mcp_servers: vec![],
            },
            silent_read_initiative_chance: 0.0,
            show_thinking: Some(false),
            show_tool_calls: None,
            include_documents: None,
            prompt_timeout: duration_string::DurationString::from_string(
                self.prompt_timeout.unwrap_or("5m".into()),
            )
            .unwrap(),
            documents: vec![],
            heartbeat_interval: duration_string::DurationString::from_string(
                self.heartbeat_interval.unwrap_or("30m".into()),
            )
            .unwrap(),
            dream_interval: duration_string::DurationString::from_string(
                self.dream_interval.unwrap_or("24h".into()),
            )
            .unwrap(),
            discord_token: self.discord_token,
            telegram_token: self.telegram_token,
        }
    }
}

#[utoipa::path(
    post,
    path = "/agents",
    request_body = CreateAgentRequest,
    responses(
        (status = 201, description = "Agent created", body = APIResponse<AgentSummary>),
        (status = 400, description = "Bad request", body = APIResponse<String>)
    )
)]
async fn create_agent(
    State(state): State<HTTPState>,
    Json(body): Json<CreateAgentRequest>,
) -> models::response::Response<AgentSummary> {
    let agent_id = body.agent_id.clone();
    let config = body.into_config();

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
        Ok(AgentCommandResult::Ok(summary)) => api_response(StatusCode::CREATED, summary),
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
        (status = 200, description = "Agent updated", body = APIResponse<AgentSummary>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
async fn update_agent(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Json(body): Json<CreateAgentRequest>,
) -> models::response::Response<AgentSummary> {
    let config = body.into_config();

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
        Ok(AgentCommandResult::Ok(summary)) => api_response(StatusCode::OK, summary),
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
) -> models::response::Response<String> {
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

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct UsageQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/usage",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = UsageQuery,
    responses(
        (status = 200, description = "Agent usage statistics", body = APIResponse<AgentUsageStats>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
async fn agent_usage(
    Path(agent_id): Path<String>,
    Query(query): Query<UsageQuery>,
    State(state): State<HTTPState>,
) -> models::response::Response<AgentUsageStats> {
    if !state.is_agent_exists(&agent_id).await {
        return err_response(StatusCode::NOT_FOUND, "agent not found".into());
    }

    let start_date = query.start_date.as_ref().and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    let end_date = query.end_date.as_ref().and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    let storage: &VizierStorage = &state.storage;
    let usage_result = storage
        .aggregate_usage(&agent_id, start_date, end_date)
        .await;

    match usage_result {
        Ok(stats) => api_response(StatusCode::OK, stats),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}