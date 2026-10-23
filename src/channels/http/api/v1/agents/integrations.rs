//! The agent-scoped integrations routes (`specs/016-integrations-section/contracts/http-api.md`
//! § Agent-scoped, as changed by `specs/017-agent-self-integrations/contracts/http-api.md`): what
//! Integrations › Channels / Addons / MCP show for one agent, creating and deleting its
//! integrations, its self-management grants and its change record. Every write goes through
//! `integrations::service`, which publishes `IntegrationChanged` after it commits; the response
//! never waits for agents or readers to catch up.

use std::collections::{BTreeMap, HashMap};

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{delete, get, put},
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::{
        connections::status_of,
        http::{
            api::v1::integrations::{
                FamilyQuery, IntegrationView, agents_by_id, fail, ok, plain, requester_name,
                service_error, storage_error, usernames, view,
            },
            auth::AuthenticatedUser,
            models::page::{Listing, ListQuery, SortKey, SortOrder, paginate},
            state::HTTPState,
        },
    },
    integrations::{
        access::{Caller, can_manage_on_agent, integration_access},
        service::{self, Actor, NewIntegration},
    },
    schema::{
        AgentConfig, AgentId, Integration, IntegrationEvent, IntegrationFamily,
        IntegrationStatus, Requester,
    },
    storage::{agent::AgentStorage, integration::IntegrationStorage},
};

use super::user_can_view_agent;

pub fn integrations() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/grants", get(get_grants).put(set_grants))
        .route("/events", get(list_events))
        .route("/{integration_id}", delete(remove))
        .route("/{integration_id}/attachment", put(set_attachment))
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AgentRef {
    pub agent_id: AgentId,
    pub name: String,
}

/// The other agents a legacy integration (FR-023) is still linked to.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct LegacyLinked {
    /// Those the caller can see.
    pub agents: Vec<AgentRef>,
    /// How many more the caller can't.
    pub others: usize,
}

/// One of the agent's integrations as its page shows it.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationItem {
    #[serde(flatten)]
    pub integration: IntegrationView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_linked: Option<LegacyLinked>,
    pub status: IntegrationStatus,
    pub can_edit: bool,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationList {
    pub family: Option<IntegrationFamily>,
    /// May add integrations to this agent.
    pub can_manage: bool,
    pub items: Vec<IntegrationItem>,
    /// With `limit`: matches after `q` and `filter`, before `offset`/`limit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    /// With `limit`: matches per filter key (`all`, `down`) after `q`, ignoring `filter`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<BTreeMap<String, usize>>,
}

/// MCP servers on the agent's page. Filters `down` (a connection that is not up) and `disabled`;
/// sorts `name` and `tools` (most first).
pub struct McpListing;

impl McpListing {
    fn tool_count(item: &IntegrationItem) -> i64 {
        match item.status {
            IntegrationStatus::Running { tools } => tools.unwrap_or(0) as i64,
            _ => 0,
        }
    }
}

impl Listing<IntegrationItem> for McpListing {
    fn matches(&self, item: &IntegrationItem, q: &str) -> bool {
        item.integration.name.to_lowercase().contains(q)
    }

    fn filter_keys(&self, _: &[IntegrationItem]) -> Vec<String> {
        vec!["down".into(), "disabled".into()]
    }

    fn in_filter(&self, filter: &str, item: &IntegrationItem) -> bool {
        match filter {
            "down" => matches!(
                item.status,
                IntegrationStatus::Reconnecting { .. }
                    | IntegrationStatus::Failed { .. }
                    | IntegrationStatus::Unreachable { .. }
            ),
            "disabled" => !item.integration.enabled,
            _ => false,
        }
    }

    fn sort_key(&self, item: &IntegrationItem, sort: &str) -> SortKey {
        match sort {
            "tools" => SortKey::Num(Self::tool_count(item)),
            _ => SortKey::text(&item.integration.name),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["name", "tools"]
    }

    fn default_order(&self, sort: &str) -> SortOrder {
        if sort == "tools" { SortOrder::Desc } else { SortOrder::Asc }
    }
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

/// What the agent sees of one integration: its reader for a channel connection, otherwise
/// the agent's own answer.
fn status(
    state: &HTTPState,
    integration: &Integration,
    statuses: &HashMap<String, IntegrationStatus>,
) -> IntegrationStatus {
    if !integration.enabled {
        return IntegrationStatus::Disabled;
    }
    status_of(&state.deps.channel_board, &integration.id)
        .or_else(|| statuses.get(&integration.id).cloned())
        .unwrap_or(IntegrationStatus::Unknown)
}

struct ItemContext<'a> {
    state: &'a HTTPState,
    user: &'a AuthenticatedUser,
    agent_id: &'a str,
    agents: &'a HashMap<AgentId, AgentConfig>,
    users: &'a HashMap<String, String>,
    statuses: &'a HashMap<String, IntegrationStatus>,
}

impl ItemContext<'_> {
    fn item(&self, integration: &Integration) -> IntegrationItem {
        let caller = Caller::from(self.user);
        let access =
            integration_access(caller, integration, self.agents, |a| user_can_view_agent(self.user, a));
        let legacy_linked = (integration.agents.len() > 1).then(|| {
            let mut linked = LegacyLinked { agents: vec![], others: 0 };
            for agent_id in integration.agents.iter().filter(|a| *a != self.agent_id) {
                match self.agents.get(agent_id) {
                    Some(config) if caller.manages_all() || user_can_view_agent(self.user, config) => {
                        linked.agents.push(AgentRef { agent_id: agent_id.clone(), name: config.name.clone() })
                    }
                    _ => linked.others += 1,
                }
            }
            linked
        });
        let kind = self.state.deps.integration_kinds.get(&integration.kind);
        IntegrationItem {
            integration: view(kind.as_deref(), integration, self.agents, self.users),
            legacy_linked,
            status: status(self.state, integration, self.statuses),
            can_edit: access.edit,
        }
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/integrations",
    params(("agent_id" = String, Path, description = "Agent ID"), FamilyQuery, ListQuery),
    responses(
        (status = 200, description = "The agent's integrations. With `family=mcp` and `limit`, one page of them, with `total` and `counts`", body = crate::channels::http::models::response::APIResponse<IntegrationList>),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(agent_id): Path<String>,
    Query(query): Query<FamilyQuery>,
    Query(paging): Query<ListQuery>,
) -> Response {
    let agent = match viewable_agent(&state, &user, &agent_id).await {
        Ok(agent) => agent,
        Err(r) => return r,
    };
    let integrations = match state.storage.list_agent_integrations(&agent_id).await {
        Ok(integrations) => integrations,
        Err(e) => return storage_error(e),
    };
    let agents = agents_by_id(&state).await;
    let statuses = service::agent_statuses(&state.deps, &agent_id).await;
    let users = usernames(&state).await;
    let ctx = ItemContext {
        state: &state,
        user: &user,
        agent_id: &agent_id,
        agents: &agents,
        users: &users,
        statuses: &statuses,
    };
    let items: Vec<IntegrationItem> = integrations
        .iter()
        .filter(|i| query.family.is_none_or(|f| i.family == f))
        .map(|i| ctx.item(i))
        .collect();
    let can_manage = can_manage_on_agent(Caller::from(&user), &agent);
    // Paging is the MCP page's: the other families hold a handful of rows.
    if paging.is_paged() && query.family == Some(IntegrationFamily::Mcp) {
        return match paginate(items, &McpListing, &paging) {
            Ok(page) => ok(
                StatusCode::OK,
                IntegrationList {
                    family: query.family,
                    can_manage,
                    items: page.items,
                    total: Some(page.total),
                    counts: Some(page.counts),
                },
            ),
            Err(e) => plain(StatusCode::BAD_REQUEST, e.code),
        };
    }
    ok(
        StatusCode::OK,
        IntegrationList { family: query.family, can_manage, items, total: None, counts: None },
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
        (status = 201, description = "Created on the agent", body = crate::channels::http::models::response::APIResponse<IntegrationItem>),
        (status = 403, description = "The caller may not add integrations to this agent"),
        (status = 409, description = "name_in_use_on_agent, platform_in_use_on_agent, addon_installed or token_in_use"),
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
    let actor = Actor::Person(Caller::from(&user));
    let new = NewIntegration { kind: body.kind, name: body.name, settings: body.settings, enabled: body.enabled };
    let integration = match service::create(&state.deps, &actor, &agent_id, &agent, new).await {
        Ok(integration) => integration,
        Err(e) => return service_error(e),
    };

    let agents = agents_by_id(&state).await;
    let statuses = HashMap::new();
    let users = usernames(&state).await;
    let ctx = ItemContext {
        state: &state,
        user: &user,
        agent_id: &agent_id,
        agents: &agents,
        users: &users,
        statuses: &statuses,
    };
    let mut item = ctx.item(&integration);
    // Saved this instant: the agent picks it up from its next turn.
    if item.status == IntegrationStatus::Unknown {
        item.status = IntegrationStatus::Connecting;
    }
    ok(StatusCode::CREATED, item)
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationRemoved {
    /// `deleted`, or `unlinked` for a legacy integration still linked to other agents.
    pub outcome: &'static str,
    pub affected_agents: Vec<AgentId>,
}

/// Delete it, or unlink a legacy integration from this agent only (FR-023).
async fn remove_from_agent(state: &HTTPState, user: &AuthenticatedUser, agent_id: &str, integration_id: &str) -> Response {
    if let Err(r) = viewable_agent(state, user, agent_id).await {
        return r;
    }
    let linked = match state.storage.get_integration(integration_id).await {
        Ok(Some(integration)) => integration.agents.len(),
        Ok(None) => return plain(StatusCode::NOT_FOUND, "integration not found"),
        Err(e) => return storage_error(e),
    };
    let actor = Actor::Person(Caller::from(user));
    let can_view = |a: &AgentConfig| user_can_view_agent(user, a);
    match service::delete(&state.deps, &actor, integration_id, Some(agent_id), &can_view).await {
        Ok(affected) => ok(
            StatusCode::OK,
            IntegrationRemoved { outcome: if linked > 1 { "unlinked" } else { "deleted" }, affected_agents: affected },
        ),
        Err(e) => service_error(e),
    }
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/integrations/{integration_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("integration_id" = String, Path, description = "Integration id")
    ),
    responses(
        (status = 200, description = "Deleted, or unlinked from this agent if it is a legacy integration linked to several", body = crate::channels::http::models::response::APIResponse<IntegrationRemoved>),
        (status = 403, description = "The caller may not change it"),
        (status = 404, description = "Not one of this agent's integrations")
    )
)]
pub async fn remove(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path((agent_id, integration_id)): Path<(String, String)>,
) -> Response {
    remove_from_agent(&state, &user, &agent_id, &integration_id).await
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AttachmentBody {
    pub attached: bool,
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
        (status = 200, description = "A legacy integration unlinked from this agent", body = crate::channels::http::models::response::APIResponse<IntegrationRemoved>),
        (status = 409, description = "use_delete: it belongs to this agent alone"),
        (status = 410, description = "attach_removed: integrations belong to one agent")
    )
)]
pub async fn set_attachment(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path((agent_id, integration_id)): Path<(String, String)>,
    Json(body): Json<AttachmentBody>,
) -> Response {
    if body.attached {
        return fail(
            StatusCode::GONE,
            "integrations belong to one agent; create one on this agent instead",
            serde_json::json!({ "error": "attach_removed" }),
        );
    }
    match state.storage.get_integration(&integration_id).await {
        Ok(Some(integration)) if integration.agents.len() <= 1 => {
            return fail(
                StatusCode::CONFLICT,
                "it belongs to this agent alone; delete it instead",
                serde_json::json!({ "error": "use_delete" }),
            );
        }
        Ok(_) => {}
        Err(e) => return storage_error(e),
    }
    remove_from_agent(&state, &user, &agent_id, &integration_id).await
}

// ---------------------------------------------------------------------------------------------
// Self-management grants (contracts/http-api.md § Grants)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct GrantState {
    pub on: bool,
    /// The username of whoever switched it on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granted_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct GrantStates {
    pub mcp: GrantState,
    pub addon: GrantState,
    pub channel: GrantState,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationGrants {
    /// May switch these on or off.
    pub can_manage: bool,
    pub grants: GrantStates,
}

async fn grants_response(state: &HTTPState, user: &AuthenticatedUser, agent_id: &str, agent: &AgentConfig) -> Response {
    let grants = match state.storage.list_grants(agent_id).await {
        Ok(grants) => grants,
        Err(e) => return storage_error(e),
    };
    let users = usernames(state).await;
    let of = |family: IntegrationFamily| {
        grants
            .iter()
            .find(|g| g.family == family)
            .map(|g| GrantState {
                on: true,
                granted_by: Some(users.get(&g.granted_by).cloned().unwrap_or_else(|| g.granted_by.clone())),
                granted_at: Some(g.granted_at),
            })
            .unwrap_or_default()
    };
    ok(
        StatusCode::OK,
        IntegrationGrants {
            can_manage: can_manage_on_agent(Caller::from(user), agent),
            grants: GrantStates {
                mcp: of(IntegrationFamily::Mcp),
                addon: of(IntegrationFamily::Addon),
                channel: of(IntegrationFamily::Channel),
            },
        },
    )
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/integrations/grants",
    params(("agent_id" = String, Path, description = "Agent ID")),
    responses(
        (status = 200, description = "Which integration families the agent may manage itself", body = crate::channels::http::models::response::APIResponse<IntegrationGrants>),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn get_grants(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(agent_id): Path<String>,
) -> Response {
    match viewable_agent(&state, &user, &agent_id).await {
        Ok(agent) => grants_response(&state, &user, &agent_id, &agent).await,
        Err(r) => r,
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SetGrants {
    pub mcp: Option<bool>,
    pub addon: Option<bool>,
    pub channel: Option<bool>,
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/integrations/grants",
    params(("agent_id" = String, Path, description = "Agent ID")),
    request_body = SetGrants,
    responses(
        (status = 200, description = "Saved; applies from the agent's next turn, without a restart", body = crate::channels::http::models::response::APIResponse<IntegrationGrants>),
        (status = 403, description = "The caller may not manage this agent's integrations"),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn set_grants(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(agent_id): Path<String>,
    Json(body): Json<SetGrants>,
) -> Response {
    let agent = match viewable_agent(&state, &user, &agent_id).await {
        Ok(agent) => agent,
        Err(r) => return r,
    };
    let changes: Vec<(IntegrationFamily, bool)> = [
        (IntegrationFamily::Mcp, body.mcp),
        (IntegrationFamily::Addon, body.addon),
        (IntegrationFamily::Channel, body.channel),
    ]
    .into_iter()
    .filter_map(|(family, on)| Some((family, on?)))
    .collect();
    if let Err(e) = service::set_grants(&state.deps, Caller::from(&user), &agent_id, &agent, &changes).await {
        return service_error(e);
    }
    grants_response(&state, &user, &agent_id, &agent).await
}

// ---------------------------------------------------------------------------------------------
// Change record (contracts/http-api.md § Events)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct EventsQuery {
    pub integration_id: Option<String>,
    /// `at|id` from the previous page's `next`.
    pub before: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct EventItem {
    #[serde(flatten)]
    pub event: IntegrationEvent,
    /// The agent's name, or the username.
    pub actor_name: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct EventPage {
    pub items: Vec<EventItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/integrations/events",
    params(("agent_id" = String, Path, description = "Agent ID"), EventsQuery),
    responses(
        (status = 200, description = "Changes to the agent's integrations and grants, newest first", body = crate::channels::http::models::response::APIResponse<EventPage>),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn list_events(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(agent_id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> Response {
    if let Err(r) = viewable_agent(&state, &user, &agent_id).await {
        return r;
    }
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let before = match query.before.as_deref().map(parse_cursor) {
        Some(Some(cursor)) => Some(cursor),
        Some(None) => return plain(StatusCode::UNPROCESSABLE_ENTITY, "malformed cursor"),
        None => None,
    };
    let events = match state
        .storage
        .list_events(&agent_id, query.integration_id.as_deref(), before, limit)
        .await
    {
        Ok(events) => events,
        Err(e) => return storage_error(e),
    };
    let agents = agents_by_id(&state).await;
    let users = usernames(&state).await;
    let next = (events.len() == limit)
        .then(|| events.last().map(|e| format!("{}|{}", e.at.to_rfc3339(), e.id)))
        .flatten();
    let items = events
        .into_iter()
        .map(|event| {
            let actor_name = requester_name(&event.actor, &agents, &users);
            EventItem { event, actor_name }
        })
        .collect();
    ok(StatusCode::OK, EventPage { items, next })
}

fn parse_cursor(cursor: &str) -> Option<(DateTime<Utc>, String)> {
    let (at, id) = cursor.split_once('|')?;
    let at = DateTime::parse_from_rfc3339(at).ok()?.with_timezone(&Utc);
    Some((at, id.to_string()))
}
