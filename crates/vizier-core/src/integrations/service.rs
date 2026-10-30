//! The one write path for integrations and grants (specs/017-agent-self-integrations research
//! R2), used by the HTTP handlers and the agents' self-management tools alike, so a person and
//! an agent are held to the same rules. Each write validates through the kind, saves with its
//! change record in one transaction, and publishes `IntegrationChanged` after the commit.

use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use serde_json::Value;

use crate::{
    dependencies::VizierDependencies,
    integrations::{
        access::{self, Caller},
        kind::IntegrationKind,
    },
    schema::{
        AgentCommand, AgentConfig, AgentId, EventAction, EventDraft, Integration,
        IntegrationChanged, IntegrationFamily, IntegrationStatus, Requester,
    },
    storage::{
        agent::AgentStorage,
        integration::{IntegrationConflict, IntegrationStorage},
    },
    transport::VizierTransport,
};

/// Who is changing an integration.
#[derive(Debug, Clone)]
pub enum Actor<'a> {
    Person(Caller<'a>),
    /// An agent changing its own integrations, from `session`, with the families it was
    /// granted read at the time of the call.
    Agent {
        agent_id: &'a str,
        session: Option<String>,
        granted: Vec<IntegrationFamily>,
    },
}

impl Actor<'_> {
    pub fn requester(&self) -> Requester {
        match self {
            Self::Person(caller) => Requester::User(caller.user_id.to_string()),
            Self::Agent { agent_id, .. } => Requester::Agent(agent_id.to_string()),
        }
    }

    fn draft(&self, action: EventAction) -> EventDraft {
        let mut draft = EventDraft::new(self.requester(), action);
        if let Self::Agent { session, .. } = self {
            draft.session = session.clone();
        }
        draft
    }
}

#[derive(Debug)]
pub enum ServiceError {
    /// Missing, or not visible to the actor.
    NotFound,
    Forbidden(String),
    /// Refused by the kind's validation, or malformed.
    Invalid(String),
    Conflict(IntegrationConflict),
    Storage(anyhow::Error),
}

impl ServiceError {
    pub fn from_storage(e: anyhow::Error) -> Self {
        match e.downcast::<IntegrationConflict>() {
            Ok(conflict) => Self::Conflict(conflict),
            Err(e) => Self::Storage(e),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Forbidden(_) => "forbidden",
            Self::Invalid(_) => "invalid",
            Self::Conflict(conflict) => conflict.code(),
            Self::Storage(_) => "error",
        }
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "integration not found"),
            Self::Forbidden(message) | Self::Invalid(message) => write!(f, "{message}"),
            Self::Conflict(conflict) => write!(f, "{conflict}"),
            Self::Storage(e) => write!(f, "{e}"),
        }
    }
}

pub type ServiceResult<T> = Result<T, ServiceError>;

/// How long to wait for an agent's own view of its integrations.
const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

/// The agent's own view of its integrations, or nothing if it doesn't answer in time. Channel
/// connections are answered by `channels::connections::status_of` instead.
pub async fn agent_statuses(deps: &VizierDependencies, agent_id: &str) -> HashMap<String, IntegrationStatus> {
    let (resp, rx) = tokio::sync::oneshot::channel();
    let sent = deps
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

/// Tell agents and readers, after the write committed.
pub fn publish(transport: &VizierTransport, integration: &Integration, agents: Vec<AgentId>) {
    transport.publish_integration_change(IntegrationChanged {
        integration_id: integration.id.clone(),
        family: integration.family,
        agents,
    });
}

fn not_enabled(family: IntegrationFamily) -> ServiceError {
    ServiceError::Forbidden(format!(
        "managing {} is not enabled for this agent; ask someone who manages its integrations to turn it on",
        family.as_str()
    ))
}

/// The integration, if `actor` may change it. What the actor may not even see is `NotFound`.
async fn editable(
    deps: &VizierDependencies,
    actor: &Actor<'_>,
    id: &str,
    can_view: &(dyn Fn(&AgentConfig) -> bool + Sync),
) -> ServiceResult<Integration> {
    let integration = deps
        .storage
        .get_integration(id)
        .await
        .map_err(ServiceError::Storage)?
        .ok_or(ServiceError::NotFound)?;
    match actor {
        Actor::Person(caller) => {
            let agents = deps.storage.list_agents().await.unwrap_or_default().into_iter().collect();
            let access = access::integration_access(*caller, &integration, &agents, can_view);
            if !access.see {
                return Err(ServiceError::NotFound);
            }
            if !access.edit {
                return Err(ServiceError::Forbidden(if integration.is_legacy() {
                    "it is linked to several agents; only someone who manages every agent's integrations may change it".into()
                } else {
                    "you may not change this agent's integrations".into()
                }));
            }
        }
        Actor::Agent { agent_id, granted, .. } => {
            if integration.owner_agent().is_none_or(|owner| owner != agent_id) {
                return Err(ServiceError::NotFound);
            }
            if !access::agent_may(agent_id, &integration, granted) {
                return Err(not_enabled(integration.family));
            }
        }
    }
    Ok(integration)
}

pub struct NewIntegration {
    pub kind: String,
    pub name: String,
    pub settings: Value,
    pub enabled: bool,
}

/// Create an integration on `agent_id`, the only agent it will ever belong to.
pub async fn create(
    deps: &VizierDependencies,
    actor: &Actor<'_>,
    agent_id: &str,
    agent: &AgentConfig,
    new: NewIntegration,
) -> ServiceResult<Integration> {
    let kind = deps
        .integration_kinds
        .get(&new.kind)
        .ok_or_else(|| ServiceError::Invalid(format!("unknown kind '{}'", new.kind)))?;
    match actor {
        Actor::Person(caller) => {
            if !access::can_manage_on_agent(*caller, agent) {
                return Err(ServiceError::Forbidden(
                    "you may not add integrations to this agent".into(),
                ));
            }
        }
        Actor::Agent { agent_id: actor_agent, granted, .. } => {
            if *actor_agent != agent_id {
                return Err(ServiceError::Forbidden("an agent may only add integrations to itself".into()));
            }
            if !granted.contains(&kind.family()) {
                return Err(not_enabled(kind.family()));
            }
        }
    }
    persist_new(deps, &actor.requester(), actor.draft(EventAction::Created), agent_id, kind.as_ref(), new).await
}

/// Validate, save with its change record, notify and publish: what every new integration
/// goes through, whoever asked for it. Access is decided by the caller.
async fn persist_new(
    deps: &VizierDependencies,
    requester: &Requester,
    draft: EventDraft,
    agent_id: &str,
    kind: &dyn IntegrationKind,
    new: NewIntegration,
) -> ServiceResult<Integration> {
    let name = new.name.trim().to_string();
    if name.is_empty() {
        return Err(ServiceError::Invalid("a name is required".into()));
    }
    kind.validate(&name, &new.settings).map_err(ServiceError::Invalid)?;

    let now = Utc::now();
    let mut integration = Integration {
        id: uuid::Uuid::new_v4().to_string(),
        kind: kind.id().to_string(),
        family: kind.family(),
        name,
        settings: new.settings,
        enabled: new.enabled,
        creator: Some(requester.clone()),
        agents: vec![],
        created_at: now,
        updated_at: now,
    };
    let affected = deps
        .storage
        .save_integration(&integration, Some(agent_id), &[draft])
        .await
        .map_err(ServiceError::from_storage)?;
    let at = Utc::now();
    deps.notifications
        .integration_event(
            &integration.id,
            &integration.name,
            integration.family,
            requester,
            EventAction::Created,
            0,
            &affected,
            at,
        )
        .await;
    publish(&deps.transport, &integration, affected);
    integration.agents = vec![agent_id.to_string()];
    Ok(integration)
}

/// Install a built-in default on `agent_id` as a system action (specs/036-office-addon): no
/// permission check, the same validation, record, notification and event as `create`. The
/// actor is the agent's owner, or `system` when it has none. An agent that already has the
/// addon is `Ok(None)`, not an error, so a seed can be repeated.
pub async fn seed_default(
    deps: &VizierDependencies,
    agent_id: &str,
    owner: Option<&str>,
    kind_id: &str,
    name: &str,
) -> ServiceResult<Option<Integration>> {
    let kind = deps
        .integration_kinds
        .get(kind_id)
        .ok_or_else(|| ServiceError::Invalid(format!("unknown kind '{kind_id}'")))?;
    let requester = Requester::User(owner.unwrap_or("system").to_string());
    let draft = EventDraft::new(requester.clone(), EventAction::Created);
    let new = NewIntegration {
        kind: kind_id.to_string(),
        name: name.to_string(),
        settings: serde_json::json!({}),
        enabled: true,
    };
    match persist_new(deps, &requester, draft, agent_id, kind.as_ref(), new).await {
        Ok(integration) => Ok(Some(integration)),
        Err(ServiceError::Conflict(IntegrationConflict::AddonInstalled { .. })) => Ok(None),
        Err(e) => Err(e),
    }
}

#[derive(Default)]
pub struct IntegrationPatch {
    pub name: Option<String>,
    pub settings: Option<Value>,
    pub enabled: Option<bool>,
}

/// Top-level setting keys whose value changed: names only, never values.
fn changed_keys(old: &Value, new: &Value) -> Vec<String> {
    let empty = serde_json::Map::new();
    let old = old.as_object().unwrap_or(&empty);
    let new = new.as_object().unwrap_or(&empty);
    let mut keys: Vec<String> = old
        .keys()
        .chain(new.keys())
        .filter(|key| old.get(*key) != new.get(*key))
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Edit, enable or disable. Returns the integration as saved and the agents affected.
pub async fn update(
    deps: &VizierDependencies,
    actor: &Actor<'_>,
    id: &str,
    patch: IntegrationPatch,
    can_view: &(dyn Fn(&AgentConfig) -> bool + Sync),
) -> ServiceResult<(Integration, Vec<AgentId>)> {
    let mut integration = editable(deps, actor, id, can_view).await?;
    let kind = deps
        .integration_kinds
        .get(&integration.kind)
        .ok_or_else(|| ServiceError::Invalid(format!("unknown kind '{}'", integration.kind)))?;

    let before = integration.clone();
    if let Some(name) = patch.name {
        integration.name = name.trim().to_string();
    }
    if let Some(settings) = patch.settings {
        integration.settings = kind.merge_secrets(&integration.settings, settings);
    }
    if let Some(enabled) = patch.enabled {
        integration.enabled = enabled;
    }
    kind.validate(&integration.name, &integration.settings).map_err(ServiceError::Invalid)?;
    integration.updated_at = Utc::now();

    let mut fields = changed_keys(&before.settings, &integration.settings);
    if before.name != integration.name {
        fields.insert(0, "name".into());
    }
    let mut events = vec![];
    let toggled = before.enabled != integration.enabled;
    if !fields.is_empty() || !toggled {
        let mut edited = actor.draft(EventAction::Edited);
        edited.fields = fields;
        events.push(edited);
    }
    if toggled {
        events.push(actor.draft(if integration.enabled {
            EventAction::Enabled
        } else {
            EventAction::Disabled
        }));
    }
    let affected = deps
        .storage
        .save_integration(&integration, None, &events)
        .await
        .map_err(ServiceError::from_storage)?;
    let at = Utc::now();
    for (ordinal, event) in events.iter().enumerate() {
        deps.notifications
            .integration_event(
                &integration.id,
                &integration.name,
                integration.family,
                &actor.requester(),
                event.action,
                ordinal,
                &affected,
                at,
            )
            .await;
    }
    publish(&deps.transport, &integration, affected.clone());
    Ok((integration, affected))
}

/// Delete it, or, for a legacy row linked to several agents, unlink it from `from_agent` only
/// (FR-023). Returns the agents affected.
pub async fn delete(
    deps: &VizierDependencies,
    actor: &Actor<'_>,
    id: &str,
    from_agent: Option<&str>,
    can_view: &(dyn Fn(&AgentConfig) -> bool + Sync),
) -> ServiceResult<Vec<AgentId>> {
    let integration = editable(deps, actor, id, can_view).await?;
    if let Some(agent_id) = from_agent
        && !integration.agents.iter().any(|a| a == agent_id)
    {
        return Err(ServiceError::NotFound);
    }
    let (action, affected) = match from_agent {
        Some(agent_id) if integration.agents.len() > 1 => (
            EventAction::Unlinked,
            deps.storage
                .unlink(id, agent_id, &actor.draft(EventAction::Unlinked))
                .await
                .map_err(ServiceError::from_storage)?,
        ),
        _ => (
            EventAction::Deleted,
            deps.storage
                .delete_integration(id, &actor.draft(EventAction::Deleted))
                .await
                .map_err(ServiceError::from_storage)?,
        ),
    };
    let at = Utc::now();
    deps.notifications
        .integration_event(
            &integration.id,
            &integration.name,
            integration.family,
            &actor.requester(),
            action,
            0,
            &affected,
            at,
        )
        .await;
    publish(&deps.transport, &integration, affected.clone());
    Ok(affected)
}

/// Switch self-management on or off for some families of `agent_id` (FR-001–FR-006). Only
/// someone who manages that agent's integrations may; nothing restarts.
pub async fn set_grants(
    deps: &VizierDependencies,
    caller: Caller<'_>,
    agent_id: &str,
    agent: &AgentConfig,
    changes: &[(IntegrationFamily, bool)],
) -> ServiceResult<()> {
    if !access::can_manage_on_agent(caller, agent) {
        return Err(ServiceError::Forbidden(format!(
            "changing what this agent may manage itself needs {} or {} on an agent you own",
            access::MANAGE_ALL,
            access::MANAGE_OWNED
        )));
    }
    for (family, on) in changes {
        let changed = deps
            .storage
            .set_grant(agent_id, *family, *on, caller.user_id)
            .await
            .map_err(ServiceError::Storage)?;
        if changed {
            deps.notifications
                .integration_grant(
                    agent_id,
                    *family,
                    *on,
                    &Requester::User(caller.user_id.to_string()),
                    Utc::now(),
                )
                .await;
            deps.transport.publish_integration_change(IntegrationChanged {
                integration_id: String::new(),
                family: *family,
                agents: vec![agent_id.to_string()],
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{changed_keys, seed_default};
    use crate::{
        config::VizierConfig,
        dependencies::VizierDependencies,
        schema::{EventAction, Requester},
        storage::{agent::AgentStorage, integration::IntegrationStorage},
        tests_support::agent_config,
    };

    async fn deps() -> (VizierDependencies, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = VizierConfig { workspace: dir.path().to_string_lossy().to_string(), ..Default::default() };
        (VizierDependencies::new(config).await.unwrap(), dir)
    }

    #[cfg(feature = "documents")]
    #[tokio::test]
    async fn seeding_adds_the_addon_once_with_its_event_and_needs_no_permission() {
        let (deps, _dir) = deps().await;
        let mut config = agent_config("a");
        config.owner_id = Some("owner-1".into());
        deps.storage.create_agent("a", &config).await.unwrap();

        let first = seed_default(&deps, "a", Some("owner-1"), "office", "Office").await.unwrap();
        assert!(first.is_some_and(|i| i.enabled && i.kind == "office"));
        // The addon_installed conflict counts as success.
        assert!(seed_default(&deps, "a", Some("owner-1"), "office", "Office").await.unwrap().is_none());

        let listed = deps.storage.list_agent_integrations("a").await.unwrap();
        assert_eq!(listed.len(), 1);
        let events = deps.storage.list_events("a", None, None, 10).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, EventAction::Created);
        assert_eq!(events[0].actor, Requester::User("owner-1".into()));
    }

    #[cfg(feature = "documents")]
    #[tokio::test]
    async fn seeding_announces_the_change_so_running_agents_recompose() {
        let (deps, _dir) = deps().await;
        deps.storage.create_agent("c", &agent_config("c")).await.unwrap();
        let mut changes = deps.transport.subscribe_integration_changes();

        seed_default(&deps, "c", None, "office", "Office").await.unwrap();
        let change = changes.try_recv().expect("the seed publishes IntegrationChanged");
        assert_eq!(change.family, crate::schema::IntegrationFamily::Addon);
        assert_eq!(change.agents, vec!["c".to_string()]);
    }

    #[cfg(feature = "documents")]
    #[tokio::test]
    async fn turning_the_addon_off_and_on_publishes_without_touching_the_agent() {
        use super::{Actor, IntegrationPatch, update};
        use crate::access::Caller;

        let (deps, _dir) = deps().await;
        deps.storage.create_agent("e", &agent_config("e")).await.unwrap();
        let office = seed_default(&deps, "e", None, "office", "Office").await.unwrap().unwrap();
        let actor = Actor::Person(Caller { user_id: "root", is_system: true, permissions: &[] });
        let mut changes = deps.transport.subscribe_integration_changes();
        let _seeded = changes.try_recv();

        for enabled in [false, true] {
            let patch = IntegrationPatch { enabled: Some(enabled), ..Default::default() };
            let (saved, affected) = update(&deps, &actor, &office.id, patch, &|_| true).await.unwrap();
            assert_eq!(saved.enabled, enabled);
            assert_eq!(affected, vec!["e".to_string()]);
            let change = changes.try_recv().expect("each toggle publishes IntegrationChanged");
            assert_eq!(change.integration_id, office.id);
            let stored = deps.storage.list_agent_integrations("e").await.unwrap();
            assert_eq!(stored.len(), 1);
            assert_eq!(stored[0].enabled, enabled);
        }
    }

    /// A person turned the addon off: seeding again (as a restart or an update would, were
    /// either to seed) neither turns it on nor adds a second one.
    #[cfg(feature = "documents")]
    #[tokio::test]
    async fn a_disabled_addon_is_not_re_enabled_or_duplicated_by_seeding() {
        let (deps, _dir) = deps().await;
        deps.storage.create_agent("d", &agent_config("d")).await.unwrap();
        let mut office = seed_default(&deps, "d", None, "office", "Office").await.unwrap().unwrap();
        office.enabled = false;
        let draft = crate::schema::EventDraft::new(Requester::User("p".into()), EventAction::Disabled);
        deps.storage.save_integration(&office, None, &[draft]).await.unwrap();

        assert!(seed_default(&deps, "d", None, "office", "Office").await.unwrap().is_none());
        let listed = deps.storage.list_agent_integrations("d").await.unwrap();
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].enabled);
    }

    #[cfg(feature = "documents")]
    #[tokio::test]
    async fn an_agent_without_an_owner_is_seeded_as_system() {
        let (deps, _dir) = deps().await;
        deps.storage.create_agent("b", &agent_config("b")).await.unwrap();
        seed_default(&deps, "b", None, "office", "Office").await.unwrap();
        let events = deps.storage.list_events("b", None, None, 10).await.unwrap();
        assert_eq!(events[0].actor, Requester::User("system".into()));
    }

    #[test]
    fn changed_keys_names_only_what_differs() {
        let old = json!({"transport": "http", "uri": "a", "env": {"K": "1"}});
        let new = json!({"transport": "http", "uri": "b", "args": []});
        assert_eq!(changed_keys(&old, &new), ["args", "env", "uri"]);
        assert!(changed_keys(&old, &old).is_empty());
    }
}
