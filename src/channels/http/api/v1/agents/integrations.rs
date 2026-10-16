//! The agent-scoped integrations routes (`specs/016-integrations-section/contracts/http-api.md`
//! § Agent-scoped): what Integrations › Channels / Addons / MCP show for one agent, and
//! creating and attaching integrations on it. Every write publishes `IntegrationChanged` after
//! it commits; the response never waits for agents or readers to catch up.

use std::collections::HashMap;
use std::time::Duration;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{get, put},
};
use chrono::Utc;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::{
        connections::status_of,
        http::{
            api::v1::integrations::{
                FamilyQuery, IntegrationView, agents_by_id, fail, ok, plain, publish,
                storage_error, view,
            },
            auth::AuthenticatedUser,
            state::HTTPState,
        },
    },
    integrations::access::{Caller, can_manage_on_agent, integration_access, scope_for_creator},
    schema::{
        AgentCommand, AgentConfig, AgentId, Integration, IntegrationFamily, IntegrationStatus,
    },
    storage::{agent::AgentStorage, integration::IntegrationStorage},
};

use super::user_can_view_agent;

/// How long a page waits for the agent's own view of its integrations.
const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

pub fn integrations() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{integration_id}/attachment", put(set_attachment))
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AgentRef {
    pub agent_id: AgentId,
    pub name: String,
}

/// One integration as the agent's page shows it.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationItem {
    #[serde(flatten)]
    pub integration: IntegrationView,
    pub attached: bool,
    /// The other agents it is attached to that the caller can see.
    pub also_used_by: Vec<AgentRef>,
    /// How many more agents use it that the caller can't see.
    #[serde(skip_serializing_if = "is_zero")]
    pub others: usize,
    pub status: IntegrationStatus,
    pub can_edit: bool,
    pub can_attach: bool,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationList {
    pub family: Option<IntegrationFamily>,
    /// May add integrations to this agent, or attach and detach them.
    pub can_manage: bool,
    pub items: Vec<IntegrationItem>,
}

/// The agent, if the caller can view it; otherwise the `404` to answer with.
async fn viewable_agent(
    state: &HTTPState,
    user: &AuthenticatedUser,
    agent_id: &str,
) -> Result<AgentConfig, Response> {
    match state.storage.get_agent(agent_id).await {
        Ok(Some(config)) if user_can_view_agent(user, &config) => Ok(config),
        Ok(_) => Err(plain(StatusCode::NOT_FOUND, "agent not found")),
        Err(e) => Err(plain(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// The agent's own view of its attached integrations, or nothing if it doesn't answer.
async fn agent_statuses(state: &HTTPState, agent_id: &str) -> HashMap<String, IntegrationStatus> {
    let (resp, rx) = tokio::sync::oneshot::channel();
    let sent = state
        .transport
        .send_agent_command(AgentCommand::IntegrationStatus { agent_id: agent_id.to_string(), resp })
        .await;
    if sent.is_err() {
        return HashMap::new();
    }
    tokio::time::timeout(STATUS_TIMEOUT, rx)
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default()
}

/// What `agent_id` sees of one integration: its reader for a channel connection, otherwise
/// the agent's own answer.
fn status(
    state: &HTTPState,
    integration: &Integration,
    attached: bool,
    statuses: &HashMap<String, IntegrationStatus>,
) -> IntegrationStatus {
    if !integration.enabled {
        return IntegrationStatus::Disabled;
    }
    if !attached {
        return IntegrationStatus::Unknown;
    }
    status_of(&state.deps.channel_board, &integration.id)
        .or_else(|| statuses.get(&integration.id).cloned())
        .unwrap_or(IntegrationStatus::Unknown)
}

struct ItemContext<'a> {
    state: &'a HTTPState,
    user: &'a AuthenticatedUser,
    agent_id: &'a str,
    agent: &'a AgentConfig,
    agents: &'a HashMap<AgentId, AgentConfig>,
    statuses: &'a HashMap<String, IntegrationStatus>,
}

impl ItemContext<'_> {
    fn item(&self, integration: &Integration) -> IntegrationItem {
        let caller = Caller::from(self.user);
        let access = integration_access(caller, integration, self.agent_id, self.agent);
        let attached = integration.agents.iter().any(|a| a == self.agent_id);
        let mut also_used_by = vec![];
        let mut others = 0;
        for agent_id in integration.agents.iter().filter(|a| *a != self.agent_id) {
            match self.agents.get(agent_id) {
                Some(config) if caller.manages_all() || user_can_view_agent(self.user, config) => {
                    also_used_by.push(AgentRef { agent_id: agent_id.clone(), name: config.name.clone() })
                }
                _ => others += 1,
            }
        }
        let kind = self.state.deps.integration_kinds.get(&integration.kind);
        IntegrationItem {
            integration: view(kind.as_deref(), integration),
            attached,
            also_used_by,
            others,
            status: status(self.state, integration, attached, self.statuses),
            can_edit: access.edit,
            can_attach: access.attach,
        }
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/integrations",
    params(("agent_id" = String, Path, description = "Agent ID"), FamilyQuery),
    responses(
        (status = 200, description = "The integrations the caller can see, marked attached or not", body = crate::channels::http::models::response::APIResponse<IntegrationList>),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(agent_id): Path<String>,
    Query(query): Query<FamilyQuery>,
) -> Response {
    let agent = match viewable_agent(&state, &user, &agent_id).await {
        Ok(agent) => agent,
        Err(r) => return r,
    };
    let integrations = match state.storage.list_integrations().await {
        Ok(integrations) => integrations,
        Err(e) => return storage_error(e),
    };
    let caller = Caller::from(&user);
    let agents = agents_by_id(&state).await;
    let statuses = agent_statuses(&state, &agent_id).await;
    let ctx = ItemContext {
        state: &state,
        user: &user,
        agent_id: &agent_id,
        agent: &agent,
        agents: &agents,
        statuses: &statuses,
    };
    let items = integrations
        .iter()
        .filter(|i| query.family.is_none_or(|f| i.family == f))
        .filter(|i| integration_access(caller, i, &agent_id, &agent).see)
        .map(|i| ctx.item(i))
        .collect();
    ok(
        StatusCode::OK,
        IntegrationList {
            family: query.family,
            can_manage: can_manage_on_agent(caller, &agent),
            items,
        },
    )
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateIntegration {
    pub kind: String,
    pub name: String,
    #[serde(default = "empty_settings")]
    #[schema(value_type = Object)]
    pub settings: serde_json::Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn empty_settings() -> serde_json::Value {
    serde_json::json!({})
}

fn default_true() -> bool {
    true
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/integrations",
    params(("agent_id" = String, Path, description = "Agent ID")),
    request_body = CreateIntegration,
    responses(
        (status = 201, description = "Created and attached to the agent", body = crate::channels::http::models::response::APIResponse<IntegrationItem>),
        (status = 403, description = "The caller may not add integrations to this agent"),
        (status = 409, description = "name_in_use_on_agent, platform_in_use_on_agent or token_in_use"),
        (status = 422, description = "Settings the kind refuses")
    )
)]
pub async fn create(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(agent_id): Path<String>,
    Json(body): Json<CreateIntegration>,
) -> Response {
    let agent = match viewable_agent(&state, &user, &agent_id).await {
        Ok(agent) => agent,
        Err(r) => return r,
    };
    let caller = Caller::from(&user);
    let Some(scope) = scope_for_creator(caller).filter(|_| can_manage_on_agent(caller, &agent))
    else {
        return plain(StatusCode::FORBIDDEN, "you may not add integrations to this agent");
    };
    let Some(kind) = state.deps.integration_kinds.get(&body.kind) else {
        return plain(StatusCode::UNPROCESSABLE_ENTITY, format!("unknown kind '{}'", body.kind));
    };
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return plain(StatusCode::UNPROCESSABLE_ENTITY, "a name is required");
    }
    if let Err(message) = kind.validate(&name, &body.settings) {
        return plain(StatusCode::UNPROCESSABLE_ENTITY, message);
    }

    let now = Utc::now();
    let mut integration = Integration {
        id: uuid::Uuid::new_v4().to_string(),
        kind: kind.id().to_string(),
        family: kind.family(),
        name,
        settings: body.settings,
        enabled: body.enabled,
        scope,
        created_by: Some(user.user_id.clone()),
        agents: vec![],
        created_at: now,
        updated_at: now,
    };
    let affected = match state.storage.save_integration(&integration, Some(&agent_id)).await {
        Ok(affected) => affected,
        Err(e) => return storage_error(e),
    };
    publish(&state, &integration, affected);
    integration.agents = vec![agent_id.clone()];

    let agents = agents_by_id(&state).await;
    let statuses = HashMap::new();
    let ctx = ItemContext {
        state: &state,
        user: &user,
        agent_id: &agent_id,
        agent: &agent,
        agents: &agents,
        statuses: &statuses,
    };
    let mut item = ctx.item(&integration);
    // Saved this instant: the agent picks it up from its next turn.
    if item.status == IntegrationStatus::Unknown {
        item.status = IntegrationStatus::Connecting;
    }
    ok(StatusCode::CREATED, item)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AttachmentBody {
    pub attached: bool,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AttachmentSaved {
    #[serde(flatten)]
    pub item: IntegrationItem,
    pub affected_agents: Vec<AgentId>,
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/integrations/{integration_id}/attachment",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("integration_id" = String, Path, description = "Integration id")
    ),
    request_body = AttachmentBody,
    responses(
        (status = 200, description = "Attached or detached", body = crate::channels::http::models::response::APIResponse<AttachmentSaved>),
        (status = 403, description = "The caller may not attach it to this agent"),
        (status = 409, description = "incomplete, name_in_use_on_agent or platform_in_use_on_agent")
    )
)]
pub async fn set_attachment(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path((agent_id, integration_id)): Path<(String, String)>,
    Json(body): Json<AttachmentBody>,
) -> Response {
    let agent = match viewable_agent(&state, &user, &agent_id).await {
        Ok(agent) => agent,
        Err(r) => return r,
    };
    let integration = match state.storage.get_integration(&integration_id).await {
        Ok(Some(integration)) => integration,
        Ok(None) => return plain(StatusCode::NOT_FOUND, "integration not found"),
        Err(e) => return storage_error(e),
    };
    let caller = Caller::from(&user);
    let access = integration_access(caller, &integration, &agent_id, &agent);
    if !access.see {
        return plain(StatusCode::NOT_FOUND, "integration not found");
    }
    if !access.attach {
        return plain(StatusCode::FORBIDDEN, "you may not change this agent's integrations");
    }
    if body.attached {
        let missing = state
            .deps
            .integration_kinds
            .get(&integration.kind)
            .map(|k| k.missing(&integration.settings))
            .unwrap_or_default();
        if !missing.is_empty() {
            return fail(
                StatusCode::CONFLICT,
                format!("set {} first", missing.join(", ")),
                serde_json::json!({ "error": "incomplete", "missing": missing }),
            );
        }
    }

    let affected = match state.storage.set_attachment(&integration_id, &agent_id, body.attached).await {
        Ok(affected) => affected,
        Err(e) => return storage_error(e),
    };
    publish(&state, &integration, affected.clone());

    let integration = match state.storage.get_integration(&integration_id).await {
        Ok(Some(integration)) => integration,
        Ok(None) => return plain(StatusCode::NOT_FOUND, "integration not found"),
        Err(e) => return storage_error(e),
    };
    let agents = agents_by_id(&state).await;
    let statuses = HashMap::new();
    let ctx = ItemContext {
        state: &state,
        user: &user,
        agent_id: &agent_id,
        agent: &agent,
        agents: &agents,
        statuses: &statuses,
    };
    ok(StatusCode::OK, AttachmentSaved { item: ctx.item(&integration), affected_agents: affected })
}
