//! The agent-scoped integrations routes (`specs/016-integrations-section/contracts/http-api.md`
//! § Agent-scoped, as changed by `specs/017-agent-self-integrations/contracts/http-api.md`): what
//! Integrations › Channels / Addons / MCP show for one agent, creating and deleting its
//! integrations, its self-management grants and its change record. Every write goes through
//! `vizier_core::service::integrations`, which publishes `IntegrationChanged` after it commits;
//! the response never waits for agents or readers to catch up.

use std::collections::BTreeMap;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{delete, get, put},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use vizier_core::{
    schema::{IntegrationFamily, IntegrationStatus},
    service::integrations as service,
};
use crate::{
    api::v1::integrations::{FamilyQuery, failure, integrations_required, ok, plain, svc},
    auth::Principal,
    models::page::{ListQuery, Listing, SortKey, SortOrder, paginate},
    state::HTTPState,
};

pub use service::{
    AgentRef, EventItem, EventPage, GrantState, GrantStates, IntegrationGrants, IntegrationItem,
    IntegrationRemoved, LegacyLinked,
};

pub fn integrations() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/grants", get(get_grants).put(set_grants))
        .route("/events", get(list_events))
        .route("/{integration_id}", delete(remove))
        .route("/{integration_id}/attachment", put(set_attachment))
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

fn fail(e: vizier_core::service::ServiceError) -> Response {
    failure(e, Some(integrations_required()))
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/integrations",
    params(("agent_id" = String, Path, description = "Agent ID"), FamilyQuery, ListQuery),
    responses(
        (status = 200, description = "The agent's integrations. With `family=mcp` and `limit`, one page of them, with `total` and `counts`", body = crate::models::response::APIResponse<IntegrationList>),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Query(query): Query<FamilyQuery>,
    Query(paging): Query<ListQuery>,
) -> Response {
    let listing = match service::list(&svc(&state), &user, &agent_id, query.family).await {
        Ok(listing) => listing,
        Err(e) => return fail(e),
    };
    let (can_manage, items) = (listing.can_manage, listing.items);
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
        (status = 201, description = "Created on the agent", body = crate::models::response::APIResponse<IntegrationItem>),
        (status = 403, description = "The caller may not add integrations to this agent"),
        (status = 409, description = "name_in_use_on_agent, platform_in_use_on_agent, addon_installed or token_in_use"),
        (status = 422, description = "Settings the kind refuses")
    )
)]
pub async fn create(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Json(body): Json<CreateIntegration>,
) -> Response {
    let input = service::CreateIntegration {
        kind: body.kind,
        name: body.name,
        settings: body.settings,
        enabled: body.enabled,
    };
    match service::create(&svc(&state), &user, &agent_id, input).await {
        Ok(item) => ok(StatusCode::CREATED, item),
        Err(e) => fail(e),
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
        (status = 200, description = "Deleted, or unlinked from this agent if it is a legacy integration linked to several", body = crate::models::response::APIResponse<IntegrationRemoved>),
        (status = 403, description = "The caller may not change it"),
        (status = 404, description = "Not one of this agent's integrations")
    )
)]
pub async fn remove(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, integration_id)): Path<(String, String)>,
) -> Response {
    match service::remove(&svc(&state), &user, &agent_id, &integration_id).await {
        Ok(removed) => ok(StatusCode::OK, removed),
        Err(e) => fail(e),
    }
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
        (status = 200, description = "A legacy integration unlinked from this agent", body = crate::models::response::APIResponse<IntegrationRemoved>),
        (status = 409, description = "use_delete: it belongs to this agent alone"),
        (status = 410, description = "attach_removed: integrations belong to one agent")
    )
)]
pub async fn set_attachment(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, integration_id)): Path<(String, String)>,
    Json(body): Json<AttachmentBody>,
) -> Response {
    match service::set_attachment(&svc(&state), &user, &agent_id, &integration_id, body.attached).await {
        Ok(removed) => ok(StatusCode::OK, removed),
        Err(e) => fail(e),
    }
}

// ---------------------------------------------------------------------------------------------
// Self-management grants (contracts/http-api.md § Grants)
// ---------------------------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/integrations/grants",
    params(("agent_id" = String, Path, description = "Agent ID")),
    responses(
        (status = 200, description = "Which integration families the agent may manage itself", body = crate::models::response::APIResponse<IntegrationGrants>),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn get_grants(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
) -> Response {
    match service::grants(&svc(&state), &user, &agent_id).await {
        Ok(grants) => ok(StatusCode::OK, grants),
        Err(e) => fail(e),
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
        (status = 200, description = "Saved; applies from the agent's next turn, without a restart", body = crate::models::response::APIResponse<IntegrationGrants>),
        (status = 403, description = "The caller may not manage this agent's integrations"),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn set_grants(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Json(body): Json<SetGrants>,
) -> Response {
    let changes: Vec<(IntegrationFamily, bool)> = [
        (IntegrationFamily::Mcp, body.mcp),
        (IntegrationFamily::Addon, body.addon),
        (IntegrationFamily::Channel, body.channel),
    ]
    .into_iter()
    .filter_map(|(family, on)| Some((family, on?)))
    .collect();
    match service::set_grants(&svc(&state), &user, &agent_id, &changes).await {
        Ok(grants) => ok(StatusCode::OK, grants),
        Err(e) => fail(e),
    }
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

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/integrations/events",
    params(("agent_id" = String, Path, description = "Agent ID"), EventsQuery),
    responses(
        (status = 200, description = "Changes to the agent's integrations and grants, newest first", body = crate::models::response::APIResponse<EventPage>),
        (status = 404, description = "Agent not found or not visible")
    )
)]
pub async fn list_events(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> Response {
    match service::events(
        &svc(&state),
        &user,
        &agent_id,
        query.integration_id.as_deref(),
        query.before.as_deref(),
        query.limit,
    )
    .await
    {
        Ok(page) => ok(StatusCode::OK, page),
        Err(e) => fail(e),
    }
}
