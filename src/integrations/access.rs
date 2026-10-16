//! Who may see, edit and attach an integration (research R6). Every handler asks here and
//! nowhere else.

use crate::schema::{AgentConfig, Integration, IntegrationFamily, IntegrationScope};

pub const MANAGE_ALL: &str = "all_agents:integrations:manage";
pub const MANAGE_OWNED: &str = "owned_agents:integrations:manage";

/// The signed-in user, as far as integration access is concerned.
#[derive(Debug, Clone, Copy)]
pub struct Caller<'a> {
    pub user_id: &'a str,
    pub is_system: bool,
    pub permissions: &'a [String],
}

impl<'a> From<&'a crate::channels::http::auth::AuthenticatedUser> for Caller<'a> {
    fn from(user: &'a crate::channels::http::auth::AuthenticatedUser) -> Self {
        Self {
            user_id: &user.user_id,
            is_system: user.role.is_system,
            permissions: &user.permissions,
        }
    }
}

impl Caller<'_> {
    fn has(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }

    /// May manage every integration on every agent.
    pub fn manages_all(&self) -> bool {
        self.is_system || self.has(MANAGE_ALL)
    }

    /// May manage their own integrations on the agents they own.
    pub fn manages_owned(&self) -> bool {
        self.has(MANAGE_OWNED)
    }

    fn owns_agent(&self, agent: &AgentConfig) -> bool {
        agent.owner_id.as_deref() == Some(self.user_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Access {
    pub see: bool,
    pub edit: bool,
    pub attach: bool,
}

/// What `caller` may do with `integration` on the page of agent `agent_id`. The caller is
/// assumed able to view that agent (the endpoint checks that first).
pub fn integration_access(
    caller: Caller<'_>,
    integration: &Integration,
    agent_id: &str,
    agent: &AgentConfig,
) -> Access {
    if caller.manages_all() {
        return Access { see: true, edit: true, attach: true };
    }
    let mine = integration.created_by.as_deref() == Some(caller.user_id);
    let usable = mine || integration.scope == IntegrationScope::Shared;
    let owned = caller.manages_owned();
    let attached_here = integration.agents.iter().any(|a| a == agent_id);
    // A bot belongs to its agent: whoever manages that agent's integrations manages it.
    let agents_bot = integration.family == IntegrationFamily::Channel
        && attached_here
        && can_manage_on_agent(caller, agent);
    Access {
        // Anyone who can view the agent sees what is attached to it, read-only.
        see: (owned && usable) || attached_here,
        edit: (owned && mine) || agents_bot,
        attach: (owned && usable && caller.owns_agent(agent)) || agents_bot,
    }
}

/// May add integrations to, or remove them from, this agent at all.
pub fn can_manage_on_agent(caller: Caller<'_>, agent: &AgentConfig) -> bool {
    caller.manages_all() || (caller.manages_owned() && caller.owns_agent(agent))
}

/// May edit or delete the integration. `owner` is the agent a channel connection belongs to,
/// whose integration managers manage its bot whoever created it.
pub fn can_edit(caller: Caller<'_>, integration: &Integration, owner: Option<&AgentConfig>) -> bool {
    caller.manages_all()
        || (caller.manages_owned() && integration.created_by.as_deref() == Some(caller.user_id))
        || (integration.family == IntegrationFamily::Channel
            && owner.is_some_and(|agent| can_manage_on_agent(caller, agent)))
}

/// The scope a new integration gets from its creator, or `None` if they may not create one.
pub fn scope_for_creator(caller: Caller<'_>) -> Option<IntegrationScope> {
    if caller.manages_all() {
        Some(IntegrationScope::Shared)
    } else if caller.manages_owned() {
        Some(IntegrationScope::Owned)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::schema::IntegrationFamily;

    fn agent(owner: &str) -> AgentConfig {
        serde_json::from_value(serde_json::json!({
            "name": "a", "owner_id": owner, "system_prompt": null, "description": null,
            "provider": "dummyplug", "model": "m", "thinking_depth": 1,
            "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
            "prompt_timeout": "1m", "heartbeat_interval": "1m"
        }))
        .unwrap()
    }

    fn integration(created_by: &str, scope: IntegrationScope, agents: &[&str]) -> Integration {
        Integration {
            id: "i".into(),
            kind: "mcp".into(),
            family: IntegrationFamily::Mcp,
            name: "fs".into(),
            settings: serde_json::json!({}),
            enabled: true,
            scope,
            created_by: Some(created_by.into()),
            agents: agents.iter().map(|a| a.to_string()).collect(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn caller<'a>(user_id: &'a str, permissions: &'a [String]) -> Caller<'a> {
        Caller { user_id, is_system: false, permissions }
    }

    #[test]
    fn all_scope_can_do_everything() {
        let perms = vec![MANAGE_ALL.to_string()];
        let access = integration_access(
            caller("admin", &perms),
            &integration("alice", IntegrationScope::Owned, &[]),
            "dp-a",
            &agent("bob"),
        );
        assert_eq!(access, Access { see: true, edit: true, attach: true });
        assert_eq!(scope_for_creator(caller("admin", &perms)), Some(IntegrationScope::Shared));
        let system = Caller { user_id: "root", is_system: true, permissions: &[] };
        assert!(system.manages_all());
    }

    #[test]
    fn owned_scope_on_own_and_someone_elses_agent() {
        let perms = vec![MANAGE_OWNED.to_string()];
        let alice = caller("alice", &perms);
        let mine = integration("alice", IntegrationScope::Owned, &[]);

        let on_own = integration_access(alice, &mine, "dp-b", &agent("alice"));
        assert_eq!(on_own, Access { see: true, edit: true, attach: true });

        let on_other = integration_access(alice, &mine, "dp-a", &agent("admin"));
        assert_eq!(on_other, Access { see: true, edit: true, attach: false });
        assert!(can_manage_on_agent(alice, &agent("alice")));
        assert!(!can_manage_on_agent(alice, &agent("admin")));
        assert_eq!(scope_for_creator(alice), Some(IntegrationScope::Owned));
    }

    #[test]
    fn shared_versus_another_owned_users_integration() {
        let perms = vec![MANAGE_OWNED.to_string()];
        let bob = caller("bob", &perms);

        let shared = integration("admin", IntegrationScope::Shared, &[]);
        assert_eq!(
            integration_access(bob, &shared, "dp-c", &agent("bob")),
            Access { see: true, edit: false, attach: true }
        );

        let alices = integration("alice", IntegrationScope::Owned, &[]);
        assert_eq!(
            integration_access(bob, &alices, "dp-c", &agent("bob")),
            Access::default()
        );
        assert!(!can_edit(bob, &alices, None));
    }

    #[test]
    fn an_agents_manager_manages_its_bot_whoever_created_it() {
        let perms = vec![MANAGE_OWNED.to_string()];
        let alice = caller("alice", &perms);
        let mut bot = integration("admin", IntegrationScope::Shared, &["dp-b"]);
        bot.family = IntegrationFamily::Channel;
        assert_eq!(
            integration_access(alice, &bot, "dp-b", &agent("alice")),
            Access { see: true, edit: true, attach: true }
        );
        assert!(can_edit(alice, &bot, Some(&agent("alice"))));
        assert!(!can_edit(alice, &bot, Some(&agent("admin"))));
    }

    #[test]
    fn a_viewer_sees_only_what_is_attached() {
        let viewer = caller("carol", &[]);
        let attached = integration("admin", IntegrationScope::Shared, &["dp-a"]);
        assert_eq!(
            integration_access(viewer, &attached, "dp-a", &agent("admin")),
            Access { see: true, edit: false, attach: false }
        );
        let elsewhere = integration("admin", IntegrationScope::Shared, &["dp-b"]);
        assert_eq!(
            integration_access(viewer, &elsewhere, "dp-a", &agent("admin")),
            Access::default()
        );
        assert_eq!(scope_for_creator(viewer), None);
    }
}
