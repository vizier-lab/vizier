//! The four artifact permissions (`specs/035-add-artifacts` R11).
//!
//! `all_agents`/`owned_agents` × `view_artifacts`/`manage_artifacts`. `manage` implies `view`,
//! `owned` requires ownership and the system role bypasses. These permissions only gate the
//! artifact gallery/content/activity and promotion/deletion; they never grant the agent's other
//! pages (`crate::access::Caller` is the same ownership rule used by secrets/integrations).

use crate::access::Caller;
use crate::schema::AgentConfig;

pub const ALL_AGENTS_VIEW: &str = "all_agents:view_artifacts";
pub const OWNED_AGENTS_VIEW: &str = "owned_agents:view_artifacts";
pub const ALL_AGENTS_MANAGE: &str = "all_agents:manage_artifacts";
pub const OWNED_AGENTS_MANAGE: &str = "owned_agents:manage_artifacts";

/// May see the gallery, content, activity and artifacts attached to messages/memories.
pub fn can_view(caller: &Caller<'_>, agent: &AgentConfig) -> bool {
    caller.manages(agent, ALL_AGENTS_VIEW, OWNED_AGENTS_VIEW)
        || caller.manages(agent, ALL_AGENTS_MANAGE, OWNED_AGENTS_MANAGE)
}

/// May promote and delete artifacts.
pub fn can_manage(caller: &Caller<'_>, agent: &AgentConfig) -> bool {
    caller.manages(agent, ALL_AGENTS_MANAGE, OWNED_AGENTS_MANAGE)
}

#[cfg(test)]
mod tests {
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

    fn caller<'a>(user_id: &'a str, permissions: &'a [String]) -> Caller<'a> {
        Caller {
            user_id,
            is_system: false,
            permissions,
        }
    }

    #[test]
    fn all_agents_view_covers_every_agent() {
        let perms = vec![ALL_AGENTS_VIEW.to_string()];
        let admin = caller("admin", &perms);
        assert!(can_view(&admin, &agent("alice")));
        assert!(can_view(&admin, &agent("bob")));
        assert!(!can_manage(&admin, &agent("alice")));
    }

    #[test]
    fn owned_agents_view_needs_ownership() {
        let perms = vec![OWNED_AGENTS_VIEW.to_string()];
        let alice = caller("alice", &perms);
        assert!(can_view(&alice, &agent("alice")));
        assert!(!can_view(&alice, &agent("bob")));
        assert!(!can_manage(&alice, &agent("alice")));
    }

    #[test]
    fn manage_implies_view() {
        for perms in [
            vec![ALL_AGENTS_MANAGE.to_string()],
            vec![OWNED_AGENTS_MANAGE.to_string()],
        ] {
            let alice = caller("alice", &perms);
            assert!(can_manage(&alice, &agent("alice")));
            assert!(can_view(&alice, &agent("alice")));
        }
    }

    #[test]
    fn a_system_role_bypasses_every_rule() {
        let system = Caller {
            user_id: "root",
            is_system: true,
            permissions: &[],
        };
        assert!(can_view(&system, &agent("bob")));
        assert!(can_manage(&system, &agent("bob")));
    }

    #[test]
    fn no_permission_sees_nothing() {
        let carol = caller("carol", &[]);
        assert!(!can_view(&carol, &agent("carol")));
        assert!(!can_manage(&carol, &agent("carol")));
    }

    #[test]
    fn a_revoked_permission_is_gone_from_the_next_callers_list() {
        let granted = vec![ALL_AGENTS_VIEW.to_string()];
        assert!(can_view(&caller("admin", &granted), &agent("a")));
        let revoked: Vec<String> = vec![];
        assert!(!can_view(&caller("admin", &revoked), &agent("a")));
    }
}
