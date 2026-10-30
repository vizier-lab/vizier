//! Integrations, agent-scoped and global (`specs/016-integrations-section`,
//! `specs/017-agent-self-integrations`; spec 038 US2). Every write goes through
//! [`crate::integrations::service`], which validates, saves with its event row and publishes
//! `IntegrationChanged` after commit; this module adds who may see and do what, and the views.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{ServiceError, Services};
use crate::{
    access::can_view_agent,
    auth::Principal,
    channels::connections::status_of,
    integrations::{
        access::{Caller, can_manage_on_agent, integration_access},
        kind::{IntegrationKind, KindField},
        service::{self as core_service, Actor, IntegrationPatch, NewIntegration},
    },
    schema::{
        AgentConfig, AgentId, Integration, IntegrationEvent, IntegrationFamily, IntegrationStatus, Requester,
    },
    storage::{
        agent::AgentStorage,
        integration::{IntegrationConflict, IntegrationStorage},
        user::UserStorage,
    },
};

pub use crate::integrations::access::{MANAGE_ALL, MANAGE_OWNED};

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

fn conflict(conflict: &IntegrationConflict) -> ServiceError {
    let (id, name) = conflict.conflicting();
    let code = conflict.code();
    ServiceError::Conflict {
        message: conflict.to_string(),
        code: Some(code.to_string()),
        data: Some(serde_json::json!({ "error": code, "integration": { "id": id, "name": name } })),
    }
}

/// A storage error: a rule conflict is `409` with its code and the integration already holding
/// what was asked for, anything else `500`.
pub fn storage(e: anyhow::Error) -> ServiceError {
    match e.downcast_ref::<IntegrationConflict>() {
        Some(c) => conflict(c),
        None => ServiceError::Internal(e.to_string()),
    }
}

/// The core integration service's refusal, as the service layer's.
pub fn refusal(e: core_service::ServiceError) -> ServiceError {
    match e {
        core_service::ServiceError::NotFound => ServiceError::NotFound("integration not found".into()),
        core_service::ServiceError::Forbidden(message) => ServiceError::Forbidden(message),
        core_service::ServiceError::Invalid(message) => ServiceError::Invalid {
            status: 422,
            message,
            code: Some("invalid".into()),
        },
        core_service::ServiceError::Conflict(c) => conflict(&c),
        core_service::ServiceError::Storage(e) => storage(e),
    }
}

// ---------------------------------------------------------------------------------------------
// Shared helpers (the secrets service reads names the same way)
// ---------------------------------------------------------------------------------------------

/// Who a `Requester` is, as a person reads it: the agent's name, or the username.
pub fn requester_name(
    requester: &Requester,
    agents: &HashMap<AgentId, AgentConfig>,
    users: &HashMap<String, String>,
) -> String {
    match requester {
        Requester::User(id) => users.get(id).cloned().unwrap_or_else(|| id.clone()),
        Requester::Agent(id) => agents.get(id).map(|a| a.name.clone()).unwrap_or_else(|| id.clone()),
    }
}

/// Usernames by user id.
pub async fn usernames(s: &Services) -> HashMap<String, String> {
    s.deps()
        .storage
        .list_users()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|user| (user.user_id, user.username))
        .collect()
}

/// Every stored agent, by id.
pub async fn agents_by_id(s: &Services) -> HashMap<AgentId, AgentConfig> {
    s.deps().storage.list_agents().await.unwrap_or_default().into_iter().collect()
}

/// An integration as every response shows it: settings redacted by its kind.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IntegrationView {
    pub id: String,
    pub kind: String,
    pub family: IntegrationFamily,
    pub name: String,
    pub enabled: bool,
    /// Who made it: `{"user": …}` or `{"agent": …}`. Informational only.
    pub creator: Option<Requester>,
    /// The agent's name, or the username.
    pub creator_name: Option<String>,
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub settings: serde_json::Value,
    /// Required settings that are missing; the integration contributes nothing until set.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
}

pub fn view(
    kind: Option<&dyn IntegrationKind>,
    integration: &Integration,
    agents: &HashMap<AgentId, AgentConfig>,
    users: &HashMap<String, String>,
) -> IntegrationView {
    IntegrationView {
        id: integration.id.clone(),
        kind: integration.kind.clone(),
        family: integration.family,
        name: integration.name.clone(),
        enabled: integration.enabled,
        creator: integration.creator.clone(),
        creator_name: integration.creator.as_ref().map(|creator| requester_name(creator, agents, users)),
        // An unknown kind's settings could hold anything; show none of them.
        settings: kind
            .map(|k| k.redact(&integration.settings))
            .unwrap_or(serde_json::json!({})),
        missing: kind.map(|k| k.missing(&integration.settings)).unwrap_or_default(),
    }
}

/// The agent, if the person can view it; otherwise the `404` to answer with.
async fn viewable_agent(s: &Services, who: &Principal, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) if can_view_agent(who, &config) => Ok(config),
        Ok(_) => Err(ServiceError::NotFound("agent not found".into())),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

async fn load(s: &Services, id: &str) -> Result<Integration, ServiceError> {
    match s.deps().storage.get_integration(id).await {
        Ok(Some(integration)) => Ok(integration),
        Ok(None) => Err(ServiceError::NotFound("integration not found".into())),
        Err(e) => Err(storage(e)),
    }
}

// ---------------------------------------------------------------------------------------------
// An agent's integrations
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentRef {
    pub agent_id: AgentId,
    pub name: String,
}

/// The other agents a legacy integration (FR-023) is still linked to.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct LegacyLinked {
    /// Those the caller can see.
    pub agents: Vec<AgentRef>,
    /// How many more the caller can't.
    pub others: usize,
}

/// One of the agent's integrations as its page shows it.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IntegrationItem {
    #[serde(flatten)]
    pub integration: IntegrationView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_linked: Option<LegacyLinked>,
    pub status: IntegrationStatus,
    pub can_edit: bool,
}

/// What the agent sees of one integration: its reader for a channel connection, otherwise
/// the agent's own answer.
fn status(
    s: &Services,
    integration: &Integration,
    statuses: &HashMap<String, IntegrationStatus>,
) -> IntegrationStatus {
    if !integration.enabled {
        return IntegrationStatus::Disabled;
    }
    status_of(&s.deps().channel_board, &integration.id)
        .or_else(|| statuses.get(&integration.id).cloned())
        .unwrap_or(IntegrationStatus::Unknown)
}

struct ItemContext<'a> {
    s: &'a Services,
    who: &'a Principal,
    agent_id: &'a str,
    agents: &'a HashMap<AgentId, AgentConfig>,
    users: &'a HashMap<String, String>,
    statuses: &'a HashMap<String, IntegrationStatus>,
}

impl ItemContext<'_> {
    fn item(&self, integration: &Integration) -> IntegrationItem {
        let caller = Caller::from(self.who);
        let access = integration_access(caller, integration, self.agents, |a| can_view_agent(self.who, a));
        let legacy_linked = (integration.agents.len() > 1).then(|| {
            let mut linked = LegacyLinked { agents: vec![], others: 0 };
            for agent_id in integration.agents.iter().filter(|a| *a != self.agent_id) {
                match self.agents.get(agent_id) {
                    Some(config) if caller.manages_all() || can_view_agent(self.who, config) => {
                        linked.agents.push(AgentRef { agent_id: agent_id.clone(), name: config.name.clone() })
                    }
                    _ => linked.others += 1,
                }
            }
            linked
        });
        let kind = self.s.deps().integration_kinds.get(&integration.kind);
        IntegrationItem {
            integration: view(kind.as_deref(), integration, self.agents, self.users),
            legacy_linked,
            status: status(self.s, integration, self.statuses),
            can_edit: access.edit,
        }
    }
}

/// The agent's integrations (of one family, if given) before any paging.
#[derive(Debug, Clone)]
pub struct AgentIntegrations {
    pub can_manage: bool,
    pub items: Vec<IntegrationItem>,
}

pub async fn list(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    family: Option<IntegrationFamily>,
) -> Result<AgentIntegrations, ServiceError> {
    let agent = viewable_agent(s, who, agent_id).await?;
    let integrations = s
        .deps()
        .storage
        .list_agent_integrations(agent_id)
        .await
        .map_err(storage)?;
    let agents = agents_by_id(s).await;
    let statuses = core_service::agent_statuses(s.deps(), agent_id).await;
    let users = usernames(s).await;
    let ctx = ItemContext { s, who, agent_id, agents: &agents, users: &users, statuses: &statuses };
    let items = integrations
        .iter()
        .filter(|i| family.is_none_or(|f| i.family == f))
        .map(|i| ctx.item(i))
        .collect();
    Ok(AgentIntegrations { can_manage: can_manage_on_agent(Caller::from(who), &agent), items })
}

/// What a person sends to add an integration to an agent.
#[derive(Debug, Clone)]
pub struct CreateIntegration {
    pub kind: String,
    pub name: String,
    pub settings: serde_json::Value,
    pub enabled: bool,
}

pub async fn create(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    input: CreateIntegration,
) -> Result<IntegrationItem, ServiceError> {
    let agent = viewable_agent(s, who, agent_id).await?;
    let actor = Actor::Person(Caller::from(who));
    let new = NewIntegration {
        kind: input.kind,
        name: input.name,
        settings: input.settings,
        enabled: input.enabled,
    };
    let integration = core_service::create(s.deps(), &actor, agent_id, &agent, new)
        .await
        .map_err(refusal)?;

    let agents = agents_by_id(s).await;
    let statuses = HashMap::new();
    let users = usernames(s).await;
    let ctx = ItemContext { s, who, agent_id, agents: &agents, users: &users, statuses: &statuses };
    let mut item = ctx.item(&integration);
    // Saved this instant: the agent picks it up from its next turn.
    if item.status == IntegrationStatus::Unknown {
        item.status = IntegrationStatus::Connecting;
    }
    Ok(item)
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IntegrationRemoved {
    /// `deleted`, or `unlinked` for a legacy integration still linked to other agents.
    pub outcome: &'static str,
    pub affected_agents: Vec<AgentId>,
}

/// Delete it, or unlink a legacy integration from this agent only (FR-023).
pub async fn remove(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    integration_id: &str,
) -> Result<IntegrationRemoved, ServiceError> {
    viewable_agent(s, who, agent_id).await?;
    let linked = load(s, integration_id).await?.agents.len();
    let actor = Actor::Person(Caller::from(who));
    let can_view = |a: &AgentConfig| can_view_agent(who, a);
    let affected = core_service::delete(s.deps(), &actor, integration_id, Some(agent_id), &can_view)
        .await
        .map_err(refusal)?;
    Ok(IntegrationRemoved {
        outcome: if linked > 1 { "unlinked" } else { "deleted" },
        affected_agents: affected,
    })
}

/// `PUT …/attachment`: attaching is gone (`410 attach_removed`); detaching unlinks a legacy
/// integration from this agent and refuses one that belongs to this agent alone.
pub async fn set_attachment(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    integration_id: &str,
    attached: bool,
) -> Result<IntegrationRemoved, ServiceError> {
    if attached {
        return Err(ServiceError::Gone {
            code: "attach_removed".into(),
            message: "integrations belong to one agent; create one on this agent instead".into(),
        });
    }
    match s.deps().storage.get_integration(integration_id).await {
        Ok(Some(integration)) if integration.agents.len() <= 1 => {
            return Err(ServiceError::Conflict {
                message: "it belongs to this agent alone; delete it instead".into(),
                code: Some("use_delete".into()),
                data: Some(serde_json::json!({ "error": "use_delete" })),
            });
        }
        Ok(_) => {}
        Err(e) => return Err(storage(e)),
    }
    remove(s, who, agent_id, integration_id).await
}

// ---------------------------------------------------------------------------------------------
// Self-management grants (contracts/http-api.md § Grants)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GrantState {
    pub on: bool,
    /// The username of whoever switched it on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granted_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GrantStates {
    pub mcp: GrantState,
    pub addon: GrantState,
    pub channel: GrantState,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IntegrationGrants {
    /// May switch these on or off.
    pub can_manage: bool,
    pub grants: GrantStates,
}

async fn grants_of(s: &Services, who: &Principal, agent_id: &str, agent: &AgentConfig) -> Result<IntegrationGrants, ServiceError> {
    let grants = s.deps().storage.list_grants(agent_id).await.map_err(storage)?;
    let users = usernames(s).await;
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
    Ok(IntegrationGrants {
        can_manage: can_manage_on_agent(Caller::from(who), agent),
        grants: GrantStates {
            mcp: of(IntegrationFamily::Mcp),
            addon: of(IntegrationFamily::Addon),
            channel: of(IntegrationFamily::Channel),
        },
    })
}

pub async fn grants(s: &Services, who: &Principal, agent_id: &str) -> Result<IntegrationGrants, ServiceError> {
    let agent = viewable_agent(s, who, agent_id).await?;
    grants_of(s, who, agent_id, &agent).await
}

pub async fn set_grants(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    changes: &[(IntegrationFamily, bool)],
) -> Result<IntegrationGrants, ServiceError> {
    let agent = viewable_agent(s, who, agent_id).await?;
    core_service::set_grants(s.deps(), Caller::from(who), agent_id, &agent, changes)
        .await
        .map_err(refusal)?;
    grants_of(s, who, agent_id, &agent).await
}

// ---------------------------------------------------------------------------------------------
// Change record (contracts/http-api.md § Events)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EventItem {
    #[serde(flatten)]
    pub event: IntegrationEvent,
    /// The agent's name, or the username.
    pub actor_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EventPage {
    pub items: Vec<EventItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

fn parse_cursor(cursor: &str) -> Option<(DateTime<Utc>, String)> {
    let (at, id) = cursor.split_once('|')?;
    let at = DateTime::parse_from_rfc3339(at).ok()?.with_timezone(&Utc);
    Some((at, id.to_string()))
}

pub async fn events(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    integration_id: Option<&str>,
    before: Option<&str>,
    limit: Option<usize>,
) -> Result<EventPage, ServiceError> {
    viewable_agent(s, who, agent_id).await?;
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let before = match before.map(parse_cursor) {
        Some(Some(cursor)) => Some(cursor),
        Some(None) => {
            return Err(ServiceError::Invalid {
                status: 422,
                message: "malformed cursor".into(),
                code: Some("invalid".into()),
            });
        }
        None => None,
    };
    let events = s
        .deps()
        .storage
        .list_events(agent_id, integration_id, before, limit)
        .await
        .map_err(storage)?;
    let agents = agents_by_id(s).await;
    let users = usernames(s).await;
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
    Ok(EventPage { items, next })
}

// ---------------------------------------------------------------------------------------------
// Integration-level routes
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IntegrationDetail {
    #[serde(flatten)]
    pub integration: IntegrationView,
    /// The agents it is attached to that the caller can view.
    pub agents: Vec<AgentId>,
}

pub async fn get(s: &Services, who: &Principal, id: &str) -> Result<IntegrationDetail, ServiceError> {
    let integration = load(s, id).await?;
    let caller = Caller::from(who);
    let agents = agents_by_id(s).await;
    if !integration_access(caller, &integration, &agents, |a| can_view_agent(who, a)).see {
        return Err(ServiceError::NotFound("integration not found".into()));
    }
    let kind = s.deps().integration_kinds.get(&integration.kind);
    let visible = integration
        .agents
        .iter()
        .filter(|a| caller.manages_all() || agents.get(*a).is_some_and(|config| can_view_agent(who, config)))
        .cloned()
        .collect();
    Ok(IntegrationDetail {
        integration: view(kind.as_deref(), &integration, &agents, &usernames(s).await),
        agents: visible,
    })
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IntegrationSaved {
    #[serde(flatten)]
    pub integration: IntegrationView,
    pub affected_agents: Vec<AgentId>,
}

pub async fn update(
    s: &Services,
    who: &Principal,
    id: &str,
    patch: IntegrationPatch,
) -> Result<IntegrationSaved, ServiceError> {
    let actor = Actor::Person(Caller::from(who));
    let can_view = |a: &AgentConfig| can_view_agent(who, a);
    let (integration, affected) = core_service::update(s.deps(), &actor, id, patch, &can_view)
        .await
        .map_err(refusal)?;
    let kind = s.deps().integration_kinds.get(&integration.kind);
    let agents = agents_by_id(s).await;
    Ok(IntegrationSaved {
        integration: view(kind.as_deref(), &integration, &agents, &usernames(s).await),
        affected_agents: affected,
    })
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IntegrationDeleted {
    pub affected_agents: Vec<AgentId>,
}

pub async fn delete(
    s: &Services,
    who: &Principal,
    id: &str,
    confirm: bool,
) -> Result<IntegrationDeleted, ServiceError> {
    let integration = load(s, id).await?;
    let agents = agents_by_id(s).await;
    let access = integration_access(Caller::from(who), &integration, &agents, |a| can_view_agent(who, a));
    if !access.see {
        return Err(ServiceError::NotFound("integration not found".into()));
    }
    if !access.edit {
        return Err(ServiceError::Forbidden("you may not delete this integration".into()));
    }
    if !integration.agents.is_empty() && !confirm {
        return Err(ServiceError::Conflict {
            message: format!(
                "in use by {} agent(s); resend with ?confirm=true to delete anyway",
                integration.agents.len()
            ),
            code: Some("attached".into()),
            data: Some(serde_json::json!({ "error": "attached", "agents": integration.agents })),
        });
    }
    let actor = Actor::Person(Caller::from(who));
    let can_view = |a: &AgentConfig| can_view_agent(who, a);
    let affected = core_service::delete(s.deps(), &actor, id, None, &can_view)
        .await
        .map_err(refusal)?;
    Ok(IntegrationDeleted { affected_agents: affected })
}

// ---------------------------------------------------------------------------------------------
// Catalogue
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct KindView {
    pub id: &'static str,
    pub family: IntegrationFamily,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub fields: Vec<KindField>,
}

/// The built-in kinds, of one family if given.
pub fn kinds(s: &Services, family: Option<IntegrationFamily>) -> Vec<KindView> {
    s.deps()
        .integration_kinds
        .by_family(family)
        .iter()
        .map(|kind| {
            let listing = kind.listing();
            KindView {
                id: kind.id(),
                family: kind.family(),
                name: listing.name,
                description: listing.description,
                icon: listing.icon,
                fields: listing.fields,
            }
        })
        .collect()
}

/// The icon file name of a kind, for the adapter that serves the embedded assets.
pub fn kind_icon(s: &Services, id: &str) -> Option<&'static str> {
    s.deps().integration_kinds.get(id).map(|kind| kind.listing().icon)
}

