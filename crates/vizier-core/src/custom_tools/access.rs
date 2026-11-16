//! Who may change an agent's functions (its custom tools, `specs/061-agent-custom-tools`): every
//! agent with `all_agents:manage_functions`, the agents they own with
//! `owned_agents:manage_functions`, and the system role as always. Seeing them needs only the right
//! to view the agent.

use crate::{access::Caller, schema::AgentConfig};

pub const MANAGE_ALL: &str = "all_agents:manage_functions";
pub const MANAGE_OWNED: &str = "owned_agents:manage_functions";

/// May `caller` edit, switch off, roll back or delete `agent`'s functions and rename their
/// categories?
pub fn can_manage(caller: Caller<'_>, agent: &AgentConfig) -> bool {
    caller.manages(agent, MANAGE_ALL, MANAGE_OWNED)
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
    fn the_system_role_and_the_all_permission_manage_every_agent() {
        let system = Caller { user_id: "root", is_system: true, permissions: &[] };
        assert!(can_manage(system, &agent(Some("alice"))));
        let perms = vec![MANAGE_ALL.to_string()];
        assert!(can_manage(caller("admin", &perms), &agent(None)));
    }

    #[test]
    fn the_owned_permission_manages_only_owned_agents() {
        let perms = vec![MANAGE_OWNED.to_string()];
        let alice = caller("alice", &perms);
        assert!(can_manage(alice, &agent(Some("alice"))));
        assert!(!can_manage(alice, &agent(Some("bob"))));
    }

    #[test]
    fn editing_the_agent_is_not_enough() {
        let perms = vec!["all_agents:edit".to_string(), "owned_agents:edit".to_string()];
        assert!(!can_manage(caller("alice", &perms), &agent(Some("alice"))));
    }

    #[test]
    fn both_are_permissions_a_role_can_hold() {
        for p in [MANAGE_ALL, MANAGE_OWNED] {
            assert!(crate::storage::user::AVAILABLE_PERMISSIONS.contains(&p), "{p}");
        }
    }
}
