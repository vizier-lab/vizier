//! Who may do what with triggers (`specs/071-triggers` FR-038, research R8).
//!
//! Three permissions, each for every agent or only for agents the person owns. Being able to view
//! an agent, or having it shared with you, grants none of them: a monitor runs code with the
//! agent's secrets. Only the system role holds them by default.

use crate::{access::Caller, schema::AgentConfig};

pub const VIEW_ALL: &str = "all_agents:view_triggers";
pub const VIEW_OWNED: &str = "owned_agents:view_triggers";
pub const MANAGE_ALL: &str = "all_agents:manage_triggers";
pub const MANAGE_OWNED: &str = "owned_agents:manage_triggers";
pub const PUBLIC_WEBHOOKS_ALL: &str = "all_agents:public_webhooks";
pub const PUBLIC_WEBHOOKS_OWNED: &str = "owned_agents:public_webhooks";
/// The existing permission to choose a model, which also picks a triage model (FR-042).
pub const CHOOSE_MODEL: &str = "model_override";

pub const ALL: [&str; 6] =
    [VIEW_ALL, VIEW_OWNED, MANAGE_ALL, MANAGE_OWNED, PUBLIC_WEBHOOKS_ALL, PUBLIC_WEBHOOKS_OWNED];

/// May read the agent's triggers, events, runs and their conversations. Manage implies view.
pub fn can_view(caller: Caller<'_>, agent: &AgentConfig) -> bool {
    caller.manages(agent, VIEW_ALL, VIEW_OWNED) || can_manage(caller, agent)
}

/// May create, change, test, approve a monitor, deactivate and delete the agent's triggers.
pub fn can_manage(caller: Caller<'_>, agent: &AgentConfig) -> bool {
    caller.manages(agent, MANAGE_ALL, MANAGE_OWNED)
}

/// May create, activate or approve a webhook, or replace its secret: manage plus public webhooks.
pub fn can_publish_webhooks(caller: Caller<'_>, agent: &AgentConfig) -> bool {
    can_manage(caller, agent) && caller.manages(agent, PUBLIC_WEBHOOKS_ALL, PUBLIC_WEBHOOKS_OWNED)
}

/// May pick the model a trigger's triage uses.
pub fn can_choose_model(caller: Caller<'_>) -> bool {
    caller.is_system || caller.has(CHOOSE_MODEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(owner: Option<&str>) -> AgentConfig {
        let mut config: AgentConfig = serde_json::from_value(serde_json::json!({
            "name": "a", "system_prompt": null, "description": null,
            "provider": "dummyplug", "model": "m", "thinking_depth": 1,
            "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
            "prompt_timeout": "1m", "heartbeat_interval": "1m"
        }))
        .unwrap();
        config.owner_id = owner.map(str::to_string);
        config
    }

    fn caller<'a>(user_id: &'a str, permissions: &'a [String]) -> Caller<'a> {
        Caller { user_id, is_system: false, permissions }
    }

    #[test]
    fn the_system_role_may_do_everything() {
        let system = Caller { user_id: "root", is_system: true, permissions: &[] };
        let a = agent(Some("alice"));
        assert!(can_view(system, &a) && can_manage(system, &a) && can_publish_webhooks(system, &a));
        assert!(can_choose_model(system));
    }

    #[test]
    fn viewing_or_editing_the_agent_grants_nothing() {
        let perms: Vec<String> =
            ["all_agents:view", "all_agents:edit", "owned_agents:edit"].map(String::from).to_vec();
        let a = agent(Some("alice"));
        let alice = caller("alice", &perms);
        assert!(!can_view(alice, &a) && !can_manage(alice, &a));
    }

    #[test]
    fn manage_implies_view_and_owned_means_owned() {
        let perms = vec![MANAGE_OWNED.to_string()];
        let alice = caller("alice", &perms);
        assert!(can_view(alice, &agent(Some("alice"))));
        assert!(can_manage(alice, &agent(Some("alice"))));
        assert!(!can_manage(alice, &agent(Some("bob"))));
    }

    #[test]
    fn public_webhooks_need_manage_too() {
        let only_public = vec![PUBLIC_WEBHOOKS_ALL.to_string()];
        assert!(!can_publish_webhooks(caller("a", &only_public), &agent(None)));
        let both = vec![PUBLIC_WEBHOOKS_ALL.to_string(), MANAGE_ALL.to_string()];
        assert!(can_publish_webhooks(caller("a", &both), &agent(None)));
    }

    #[test]
    fn every_permission_is_one_a_role_can_hold() {
        for p in ALL {
            assert!(crate::storage::user::AVAILABLE_PERMISSIONS.contains(&p), "{p}");
        }
    }
}
