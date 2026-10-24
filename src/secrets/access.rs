//! Who may manage an agent's secrets (`specs/027-agent-secrets` research R5, FR-014): every
//! agent with `all_agents:secret`, the agents they own with `owned_agents:secret`, and the
//! system role as always. There is no read-only secret permission — seeing a name is managing
//! it.

use crate::{access::Caller, schema::AgentConfig};

pub const MANAGE_ALL: &str = "all_agents:secret";
pub const MANAGE_OWNED: &str = "owned_agents:secret";

/// May `caller` see and change `agent`'s secrets?
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
    fn the_system_role_manages_every_agent() {
        let system = Caller { user_id: "root", is_system: true, permissions: &[] };
        assert!(can_manage(system, &agent(Some("alice"))));
        assert!(can_manage(system, &agent(None)));
    }

    #[test]
    fn the_all_permission_manages_every_agent() {
        let perms = vec![MANAGE_ALL.to_string()];
        let admin = caller("admin", &perms);
        assert!(can_manage(admin, &agent(Some("alice"))));
        assert!(can_manage(admin, &agent(None)));
    }

    #[test]
    fn the_owned_permission_manages_only_owned_agents() {
        let perms = vec![MANAGE_OWNED.to_string()];
        let alice = caller("alice", &perms);
        assert!(can_manage(alice, &agent(Some("alice"))));
        assert!(!can_manage(alice, &agent(Some("bob"))));
        assert!(!can_manage(alice, &agent(None)));
    }

    #[test]
    fn neither_permission_manages_nothing_even_when_owning() {
        let carol = caller("carol", &[]);
        assert!(!can_manage(carol, &agent(Some("carol"))));
    }

    /// A permission that was revoked is simply absent from the next request's list.
    #[test]
    fn a_revoked_permission_is_not_carried_over() {
        let granted = vec![MANAGE_ALL.to_string()];
        assert!(can_manage(caller("admin", &granted), &agent(Some("a"))));
        let revoked: Vec<String> = vec![];
        assert!(!can_manage(caller("admin", &revoked), &agent(Some("a"))));
    }

    /// The two secret permissions are the only ones that count; integrations' do not leak in.
    #[test]
    fn integration_permissions_do_not_grant_secret_access() {
        let perms = vec![
            "all_agents:integrations:manage".to_string(),
            "owned_agents:integrations:manage".to_string(),
        ];
        assert!(!can_manage(caller("alice", &perms), &agent(Some("alice"))));
    }
}
