//! The one project permission matrix (`specs/040-projects-mode/contracts/session-and-permissions.md`
//! §3). No handler or UI helper decides this on its own; agents only translate the result.

use crate::{
    access::Caller,
    auth::AuthMethod,
    schema::{AgentConfig, Project},
};

/// Use projects on every agent the person can view; manage every project.
pub const USE_ALL: &str = "all_agents:projects";
/// Use projects on the agents the person owns.
pub const USE_OWNED: &str = "owned_agents:projects";

/// The existing agent visibility rule, in [`Caller`] form: system, `all_agents:view`, the owner,
/// or shared to the person.
fn can_view(caller: Caller<'_>, agent: &AgentConfig) -> bool {
    caller.is_system
        || caller.has("all_agents:view")
        || caller.owns(agent)
        || agent.shared_to.contains(&caller.user_id.to_string())
}

/// May the person use (see, open, read, run turns in) this agent's projects: the agent is
/// visible **and** one of the project permissions applies.
pub fn can_use(caller: Caller<'_>, agent: &AgentConfig) -> bool {
    can_view(caller, agent)
        && (caller.is_system
            || caller.has(USE_ALL)
            || (caller.has(USE_OWNED) && caller.owns(agent)))
}

/// May the person rename, remove or toggle a project: its owner, or the whole-workspace
/// permission.
pub fn can_manage(caller: Caller<'_>, project: &Project) -> bool {
    caller.is_system || project.owner_id == caller.user_id || caller.has(USE_ALL)
}

/// May the person open a folder as a project: [`can_use`] and a desktop principal.
pub fn can_create(caller: Caller<'_>, agent: &AgentConfig, method: AuthMethod) -> bool {
    method == AuthMethod::Desktop && can_use(caller, agent)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::auth::{Principal, Role};

    fn agent(owner: &str) -> AgentConfig {
        serde_json::from_value(serde_json::json!({
            "name": "a", "owner_id": owner, "system_prompt": null, "description": null,
            "provider": "dummyplug", "model": "m", "thinking_depth": 1,
            "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
            "prompt_timeout": "1m", "heartbeat_interval": "1m"
        }))
        .unwrap()
    }

    fn project(owner: &str) -> Project {
        Project {
            id: "p3xY9aQ2mZ".into(),
            agent_id: "dp-a".into(),
            name: "vizier".into(),
            folder: std::path::PathBuf::from("/tmp/vizier"),
            owner_id: owner.into(),
            shell_enabled: true,
            created_at: 0,
            removed_at: None,
        }
    }

    fn principal(system: bool, id: &str, perms: &[&str]) -> Principal {
        Principal {
            user_id: id.into(),
            username: id.into(),
            role: Role {
                role_id: "r".into(),
                name: "r".into(),
                permissions: perms.iter().map(|p| p.to_string()).collect(),
                is_system: system,
                created_at: Utc::now(),
            },
            permissions: perms.iter().map(|p| p.to_string()).collect(),
            method: AuthMethod::Jwt,
        }
    }

    fn caller(user: &crate::auth::Principal) -> Caller<'_> {
        Caller::from(user)
    }

    #[test]
    fn a_permitted_non_owner_can_use_but_not_manage() {
        let a = agent("alice");
        let bob = principal(false, "bob", &["all_agents:view", USE_ALL]);
        assert!(can_use(caller(&bob), &a));
        assert!(can_manage(caller(&bob), &project("alice")));
    }

    #[test]
    fn owned_agents_projects_needs_ownership_of_the_agent() {
        let a = agent("alice");
        let alice = principal(false, "alice", &[USE_OWNED]);
        let bob = principal(false, "bob", &[USE_OWNED, "all_agents:view"]);
        assert!(can_use(caller(&alice), &a));
        assert!(!can_use(caller(&bob), &a));
    }

    #[test]
    fn view_alone_is_not_enough() {
        let a = agent("alice");
        // The owner still needs the project permission (FR-018/FR-019).
        let owner = principal(false, "alice", &["owned_agents:view"]);
        assert!(!can_use(caller(&owner), &a));
        // A shared-to person needs it too.
        let mut shared = a.clone();
        shared.shared_to = vec!["carol".into()];
        let carol = principal(false, "carol", &["all_agents:view"]);
        assert!(!can_use(caller(&carol), &shared));
    }

    #[test]
    fn the_system_role_uses_and_manages_everything() {
        let system = principal(true, "root", &[]);
        assert!(can_use(caller(&system), &agent("alice")));
        assert!(can_manage(caller(&system), &project("alice")));
    }

    #[test]
    fn managing_needs_the_project_owner_or_the_all_permission() {
        let own = principal(false, "alice", &[USE_OWNED]);
        let other = principal(false, "bob", &[USE_OWNED]);
        assert!(can_manage(caller(&own), &project("alice")));
        assert!(!can_manage(caller(&other), &project("alice")));
    }

    #[test]
    fn creating_needs_a_desktop_principal() {
        let a = agent("alice");
        let alice = principal(false, "alice", &[USE_OWNED]);
        assert!(can_create(caller(&alice), &a, AuthMethod::Desktop));
        assert!(!can_create(caller(&alice), &a, AuthMethod::Jwt));
        assert!(!can_create(caller(&alice), &a, AuthMethod::ApiKey));
    }

    #[test]
    fn a_revoked_permission_is_absent_on_the_next_call() {
        let a = agent("alice");
        let granted = principal(false, "alice", &[USE_OWNED]);
        assert!(can_use(caller(&granted), &a));
        let revoked = principal(false, "alice", &[]);
        assert!(!can_use(caller(&revoked), &a));
    }
}
