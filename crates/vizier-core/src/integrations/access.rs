//! Who may see and change an integration (specs/017-agent-self-integrations research R3). Every
//! handler and agent tool asks here and nowhere else.
//!
//! Integrations belong to one agent each. Who may manage them follows only from which agents a
//! person may manage: every agent with `all_agents:integrations:manage`, the agents they own
//! with `owned_agents:integrations:manage`. Who created an integration grants nothing. Legacy
//! rows linked to several agents (FR-023) are for all-agents managers only.

use std::collections::HashMap;

use crate::schema::{AgentConfig, AgentId, Integration, IntegrationFamily};

pub use crate::access::Caller;

pub const MANAGE_ALL: &str = "all_agents:integrations:manage";
pub const MANAGE_OWNED: &str = "owned_agents:integrations:manage";

impl Caller<'_> {
    /// May manage the integrations of every agent.
    pub fn manages_all(&self) -> bool {
        self.is_system || self.has(MANAGE_ALL)
    }

    /// May manage the integrations of the agents they own.
    pub fn manages_owned(&self) -> bool {
        self.has(MANAGE_OWNED)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Access {
    pub see: bool,
    pub edit: bool,
}

/// May manage this agent's integrations, and so switch its self-management on or off.
pub fn can_manage_on_agent(caller: Caller<'_>, agent: &AgentConfig) -> bool {
    caller.manages(agent, MANAGE_ALL, MANAGE_OWNED)
}

/// What `caller` may do with `integration`. `agents` resolves its linked agents; `can_view`
/// says whether the caller may view an agent at all.
pub fn integration_access(
    caller: Caller<'_>,
    integration: &Integration,
    agents: &HashMap<AgentId, AgentConfig>,
    can_view: impl Fn(&AgentConfig) -> bool,
) -> Access {
    if caller.manages_all() {
        return Access { see: true, edit: true };
    }
    let edit = integration
        .owner_agent()
        .and_then(|id| agents.get(id))
        .is_some_and(|agent| can_manage_on_agent(caller, agent));
    let see = edit
        || integration
            .agents
            .iter()
            .any(|id| agents.get(id).is_some_and(&can_view));
    Access { see, edit }
}

/// May agent `agent_id` change `integration` itself: it is that agent's own (not a legacy row)
/// and the agent's switch for its family is on.
pub fn agent_may(agent_id: &str, integration: &Integration, granted: &[IntegrationFamily]) -> bool {
    integration.owner_agent().is_some_and(|owner| owner == agent_id)
        && granted.contains(&integration.family)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn agent(owner: &str) -> AgentConfig {
        serde_json::from_value(serde_json::json!({
            "name": "a", "owner_id": owner, "system_prompt": null, "description": null,
            "provider": "dummyplug", "model": "m", "thinking_depth": 1,
            "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
            "prompt_timeout": "1m", "heartbeat_interval": "1m"
        }))
        .unwrap()
    }

    /// `dp-a` owned by alice, `dp-b` owned by bob.
    fn agents() -> HashMap<AgentId, AgentConfig> {
        HashMap::from([("dp-a".to_string(), agent("alice")), ("dp-b".to_string(), agent("bob"))])
    }

    fn integration(family: IntegrationFamily, agents: &[&str]) -> Integration {
        Integration {
            id: "i".into(),
            kind: "mcp".into(),
            family,
            name: "fs".into(),
            settings: serde_json::json!({}),
            enabled: true,
            creator: Some(crate::schema::Requester::User("bob".into())),
            agents: agents.iter().map(|a| a.to_string()).collect(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn caller<'a>(user_id: &'a str, permissions: &'a [String]) -> Caller<'a> {
        Caller { user_id, is_system: false, permissions }
    }

    /// Views only the agents they own.
    fn owner_view(user: &str) -> impl Fn(&AgentConfig) -> bool + '_ {
        move |a: &AgentConfig| a.owner_id.as_deref() == Some(user)
    }

    #[test]
    fn all_agents_manager_does_everything_including_legacy_rows() {
        let perms = vec![MANAGE_ALL.to_string()];
        let admin = caller("admin", &perms);
        let legacy = integration(IntegrationFamily::Mcp, &["dp-a", "dp-b"]);
        assert_eq!(
            integration_access(admin, &legacy, &agents(), |_| false),
            Access { see: true, edit: true }
        );
        let system = Caller { user_id: "root", is_system: true, permissions: &[] };
        assert!(system.manages_all());
    }

    #[test]
    fn owned_manager_on_own_agent_whoever_created_it() {
        let perms = vec![MANAGE_OWNED.to_string()];
        let alice = caller("alice", &perms);
        // bob created it, but it is on alice's agent.
        let on_mine = integration(IntegrationFamily::Mcp, &["dp-a"]);
        assert_eq!(
            integration_access(alice, &on_mine, &agents(), owner_view("alice")),
            Access { see: true, edit: true }
        );
        assert!(can_manage_on_agent(alice, &agent("alice")));
        assert!(!can_manage_on_agent(alice, &agent("bob")));
    }

    #[test]
    fn owned_manager_on_someone_elses_agent() {
        let perms = vec![MANAGE_OWNED.to_string()];
        let alice = caller("alice", &perms);
        let on_bobs = integration(IntegrationFamily::Mcp, &["dp-b"]);
        assert_eq!(
            integration_access(alice, &on_bobs, &agents(), owner_view("alice")),
            Access::default()
        );
    }

    #[test]
    fn owned_manager_only_sees_a_legacy_row() {
        let perms = vec![MANAGE_OWNED.to_string()];
        let alice = caller("alice", &perms);
        let legacy = integration(IntegrationFamily::Addon, &["dp-a", "dp-b"]);
        assert_eq!(
            integration_access(alice, &legacy, &agents(), owner_view("alice")),
            Access { see: true, edit: false }
        );
    }

    #[test]
    fn a_viewer_sees_but_does_not_edit() {
        let carol = caller("carol", &[]);
        let on_a = integration(IntegrationFamily::Channel, &["dp-a"]);
        assert_eq!(integration_access(carol, &on_a, &agents(), |_| true), Access { see: true, edit: false });
        assert_eq!(integration_access(carol, &on_a, &agents(), |_| false), Access::default());
    }

    #[test]
    fn an_agent_changes_only_its_own_in_granted_families() {
        let mcp = [IntegrationFamily::Mcp];
        let own = integration(IntegrationFamily::Mcp, &["dp-a"]);
        assert!(agent_may("dp-a", &own, &mcp));
        assert!(!agent_may("dp-a", &own, &[IntegrationFamily::Addon]));
        assert!(!agent_may("dp-b", &own, &mcp));
        let legacy = integration(IntegrationFamily::Mcp, &["dp-a", "dp-b"]);
        assert!(!agent_may("dp-a", &legacy, &mcp));
    }
}
