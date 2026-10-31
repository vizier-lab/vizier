use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{models::{self, response::{APIResponse, api_response, err_response}}, state::HTTPState};
use vizier_core::{
    service,
    config::shell::ShellConfig,
    schema::{
        AgentCommand, AgentCommandResult, AgentConfig, AgentHealthStatus, AgentSummary,
        AgentToolsConfig, EmbeddingConfig, ImageGenToolSettings, IndexerConfig,
        ReadImageToolSettings, ToolConfig, TtsToolSettings, VizierAttachment, VizierChannelId,
        VizierRequest, VizierRequestContent, VizierResponseContent, VizierResponseStats,
        VizierSession,
        agent::{EmbeddingProvider, IndexerKind, SttToolSettings},
    },
    storage::{VizierStorage, agent::AgentStorage, history::HistoryStorage, user::UserStorage},
};

pub mod artifacts;
pub mod channel;
pub mod conversations;
pub mod core;
pub mod dream;
pub mod integrations;
pub mod jobs;
pub mod memory;
pub mod projects;
pub mod secrets;
pub mod skills;
pub mod task;

use crate::models::page::{self, ListQuery, Listing, SortKey, SortOrder};
use vizier_core::storage::session::SessionStorage;
use channel::channel;
use core::core;
use dream::dream;
use jobs::jobs;
use memory::memory;
use secrets::secrets;
use skills::agent_skills;
use task::task;

pub fn user_can_view_agent(user: &crate::auth::Principal, config: &AgentConfig) -> bool {
    vizier_core::access::can_view_agent(user, config)
}

/// Whether `user` may read this agent's Discord (`platform` = `"discord"`) or Telegram
/// (`"telegram"`) conversations: `all_agents:show_<platform>_chats` for any agent,
/// `owned_agents:show_<platform>_chats` for agents they own.
pub fn user_can_show_platform_chats(user: &crate::auth::Principal, config: &AgentConfig, platform: &str) -> bool {
    vizier_core::access::can_show_platform_chats(user, config, platform)
}

/// The platform a conversation slug belongs to, if it is a Discord or Telegram one.
pub fn platform_of_slug(channel: &str) -> Option<&'static str> {
    vizier_core::access::platform_of_slug(channel)
}

#[allow(dead_code)]
fn user_can_edit_agent(user: &crate::auth::Principal, config: &AgentConfig) -> bool {
    vizier_core::access::can_edit_agent(user, config)
}

pub(crate) fn svc(state: &HTTPState) -> vizier_core::service::Services {
    vizier_core::service::Services::new(state.deps.clone())
}

/// A service result as the API's envelope: `ok` on success, the error's own status and message
/// otherwise. A coded error carries `{"error": code}` in `data`, as the handlers always did.
pub(crate) fn respond<T: Serialize + Clone>(
    result: Result<T, vizier_core::service::ServiceError>,
    ok: StatusCode,
) -> axum::response::Response {
    use vizier_core::service::ServiceError;
    match result {
        Ok(value) => api_response(ok, value).into_response(),
        Err(e) => {
            let status = StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let message = e.message().to_string();
            match &e {
                ServiceError::Invalid { code: Some(code), .. }
                | ServiceError::Conflict { code: Some(code), .. } => crate::api::v1::integrations::fail(
                    status,
                    message,
                    serde_json::json!({"error": code}),
                ),
                _ => err_response::<String>(status, message).into_response(),
            }
        }
    }
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
        .route(
            "/{agent_id}/usage/conversation",
            get(agent_usage_conversation),
        )
        .route("/{agent_id}/ping", get(ping_agent))
        .route("/{agent_id}/restart", post(restart_agent))
        .route("/{agent_id}/chat", post(agent_chat))
        .route("/{agent_id}/btw", post(agent_btw))
        .route(
            "/{agent_id}/sharing",
            get(get_sharing).patch(update_sharing),
        )
        .nest("/{agent_id}/channel", channel().merge(jobs()))
        .nest("/{agent_id}/artifacts", artifacts::artifacts())
        .nest("/{agent_id}/core", core())
        .nest("/{agent_id}/memory", memory())
        .nest("/{agent_id}/secrets", secrets())
        .nest("/{agent_id}/projects", projects::projects())
        .nest("/{agent_id}/skills", agent_skills())
        .nest("/{agent_id}/tasks", task())
        .nest("/{agent_id}/dream", dream())
        .nest("/{agent_id}/integrations", integrations::integrations())
        .nest("/{agent_id}/conversations", conversations::conversations())
}

pub use vizier_core::service::agents::AgentListItem;

/// Filters `mine`/`live`/`failed`; sorts `last_active` (newest first) and `name`.
struct AgentListing;

impl Listing<AgentListItem> for AgentListing {
    fn matches(&self, item: &AgentListItem, q: &str) -> bool {
        item.summary.name.to_lowercase().contains(q)
            || item.summary.agent_id.to_lowercase().contains(q)
            || item
                .summary
                .description
                .as_deref()
                .is_some_and(|d| d.to_lowercase().contains(q))
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
        if sort == "last_active" {
            SortOrder::Desc
        } else {
            SortOrder::Asc
        }
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    match service::agents::list(&svc(&state), &user, query.is_paged()).await {
        Ok(res) => page::respond(res, &AgentListing, &query).into_response(),
        Err(e) => respond::<()>(Err(e), StatusCode::OK),
    }
}

pub use vizier_core::service::agents::AgentDetail;

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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(service::agents::detail(&svc(&state), &user, &agent_id).await, StatusCode::OK)
}

pub use vizier_core::service::agents::{CreateAgentRequest, CreateAgentTools};

/// Settings that moved to Integrations (spec 016, FR-021): a body that still carries any is
/// refused rather than silently ignored.
const MOVED_TO_INTEGRATIONS: &[&str] = &["discord_token", "telegram_token"];
const TOOLS_MOVED_TO_INTEGRATIONS: &[&str] =
    &["discord", "telegram", "brave_search", "mcp_servers"];

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
fn parse_agent_body(
    body: serde_json::Value,
) -> Result<CreateAgentRequest, axum::response::Response> {
    use axum::response::IntoResponse;
    if body.get("self_management").is_some() {
        return Err(crate::api::v1::integrations::fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            "self-management is set through /agents/{id}/integrations/grants, by someone who manages the agent's integrations",
            serde_json::json!({"error": "use_grants_endpoint"}),
        ));
    }
    let moved = moved_fields(&body);
    if !moved.is_empty() {
        return Err(crate::api::v1::integrations::fail(
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

pub use vizier_core::service::agents::AgentSaved;

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
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    match parse_agent_body(body) {
        Ok(body) => respond(
            service::agents::create(&svc(&state), &user, body).await,
            StatusCode::CREATED,
        ),
        Err(refused) => refused,
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
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    match parse_agent_body(body) {
        Ok(body) => respond(
            service::agents::update(&svc(&state), &user, &agent_id, body).await,
            StatusCode::OK,
        ),
        Err(refused) => refused,
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(
        service::agents::delete(&svc(&state), &user, &agent_id, query.delete_workspace).await,
        StatusCode::OK,
    )
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(
        service::agents::restart(&svc(&state), &user, &agent_id).await,
        StatusCode::ACCEPTED,
    )
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

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/usage",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        UsageParams,
    ),
    responses(
        (status = 200, description = "Agent usage report", body = APIResponse<vizier_core::schema::usage::UsageReport>),
        (status = 400, description = "invalid_range or unknown_activity", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
async fn agent_usage(
    Path(agent_id): Path<String>,
    Query(params): Query<UsageParams>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(
        service::agents::usage(
            &svc(&state),
            &user,
            &agent_id,
            params.start_date.as_deref(),
            params.end_date.as_deref(),
            params.activity.as_deref(),
        )
        .await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/usage/conversation",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ConversationUsageParams,
    ),
    responses(
        (status = 200, description = "One conversation's usage, calls most expensive first", body = APIResponse<vizier_core::schema::usage::ConversationUsage>),
        (status = 400, description = "missing_channel, invalid_limit, invalid_range or unknown_activity", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
async fn agent_usage_conversation(
    Path(agent_id): Path<String>,
    Query(params): Query<ConversationUsageParams>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(
        service::agents::usage_conversation(
            &svc(&state),
            &user,
            &agent_id,
            params.channel,
            params.topic,
            params.start_date.as_deref(),
            params.end_date.as_deref(),
            params.activity.as_deref(),
            params.limit,
        )
        .await,
        StatusCode::OK,
    )
}

pub use vizier_core::service::agents::SharingResponse;

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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(service::agents::get_sharing(&svc(&state), &user, &agent_id).await, StatusCode::OK)
}

pub use vizier_core::service::agents::UpdateSharingRequest;

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
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<UpdateSharingRequest>,
) -> axum::response::Response {
    respond(
        service::agents::update_sharing(&svc(&state), &user, &agent_id, body).await,
        StatusCode::OK,
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(service::agents::health(&svc(&state), &user).await, StatusCode::OK)
}

pub use vizier_core::service::agents::PingResponse;

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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(service::agents::ping(&svc(&state), &user, &agent_id).await, StatusCode::OK)
}

pub use vizier_core::service::agents::{ChatRequest, ChatResponse};

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
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<ChatRequest>,
) -> axum::response::Response {
    respond(
        service::agents::chat(&svc(&state), &user, &agent_id, body).await,
        StatusCode::OK,
    )
}

pub use vizier_core::service::agents::{BtwRequest, BtwResponse};

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/btw",
    params(("agent_id" = String, Path, description = "Agent ID")),
    request_body = BtwRequest,
    responses(
        (status = 200, description = "The side chat answer", body = APIResponse<BtwResponse>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 408, description = "Agent response timeout", body = APIResponse<String>),
        (status = 422, description = "The reason, e.g. the side chat is full", body = APIResponse<String>),
        (status = 429, description = "Over 5 side-chat asks per 60 s; reply header Retry-After", body = APIResponse<String>),
        (status = 500, description = "The agent answered with an error, or its response channel closed", body = APIResponse<String>),
        (status = 503, description = "The agent is unavailable", body = APIResponse<String>)
    )
)]
async fn agent_btw(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<BtwRequest>,
) -> axum::response::Response {
    use service::agents::BtwReply;
    match service::agents::btw(&svc(&state), &user, &agent_id, body).await {
        Ok(BtwReply::Answer(answer)) => api_response(StatusCode::OK, answer).into_response(),
        Ok(BtwReply::RetryAfter { secs }) => {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(axum::http::header::RETRY_AFTER, secs.to_string().parse().unwrap());
            let (status, Json(body)) = err_response::<String>(
                StatusCode::TOO_MANY_REQUESTS,
                format!("slow down — try again in {secs}s"),
            );
            (status, headers, Json(body)).into_response()
        }
        Err(e) => respond::<()>(Err(e), StatusCode::OK),
    }
}
