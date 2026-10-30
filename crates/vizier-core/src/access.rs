//! Who may manage an object that belongs to one agent (`specs/027-agent-secrets` research R5).
//!
//! The rule is the same for integrations and secrets: a system role bypasses it, an
//! all-agents permission covers every agent, and an owned-agents permission covers the agents
//! the person owns. Only the permission strings differ, so callers pass theirs in.

use crate::schema::AgentConfig;

/// The signed-in user, as far as ownership-based access is concerned.
#[derive(Debug, Clone, Copy)]
pub struct Caller<'a> {
    pub user_id: &'a str,
    pub is_system: bool,
    pub permissions: &'a [String],
}

use crate::auth::Principal;
use crate::service::error::ServiceError;

/// May `user` see this agent: system, `all_agents:view`, the owner, or shared to them.
pub fn can_view_agent(user: &Principal, config: &AgentConfig) -> bool {
    if user.role.is_system || user.permissions.iter().any(|p| p == "all_agents:view") {
        return true;
    }
    config.owner_id.as_deref() == Some(user.user_id.as_str())
        || config.shared_to.contains(&user.user_id)
}

/// May `user` edit this agent: system, `all_agents:edit`, or the owner.
pub fn can_edit_agent(user: &Principal, config: &AgentConfig) -> bool {
    if user.role.is_system || user.permissions.iter().any(|p| p == "all_agents:edit") {
        return true;
    }
    config.owner_id.as_deref() == Some(user.user_id.as_str())
}

/// Whether `user` may read this agent's Discord (`platform` = `"discord"`) or Telegram
/// (`"telegram"`) conversations.
pub fn can_show_platform_chats(user: &Principal, config: &AgentConfig, platform: &str) -> bool {
    let has = |scope: &str| {
        let wanted = format!("{scope}_agents:show_{platform}_chats");
        user.permissions.iter().any(|p| *p == wanted)
    };
    user.role.is_system
        || has("all")
        || (has("owned") && config.owner_id.as_deref() == Some(user.user_id.as_str()))
}

/// The platform a conversation slug belongs to, if it is a Discord or Telegram one.
pub fn platform_of_slug(channel: &str) -> Option<&'static str> {
    if channel.starts_with("discord__") {
        Some("discord")
    } else if channel.starts_with("telegram__") {
        Some("telegram")
    } else {
        None
    }
}

/// System bypasses; otherwise the principal needs `permission`.
pub fn require(user: &Principal, permission: &str) -> Result<(), ServiceError> {
    if user.role.is_system || user.permissions.iter().any(|p| p == permission) {
        Ok(())
    } else {
        Err(ServiceError::Forbidden("Forbidden: insufficient permissions".into()))
    }
}

impl<'a> From<&'a crate::auth::Principal> for Caller<'a> {
    fn from(user: &'a crate::auth::Principal) -> Self {
        Self {
            user_id: &user.user_id,
            is_system: user.role.is_system,
            permissions: &user.permissions,
        }
    }
}

impl Caller<'_> {
    pub fn has(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }

    /// Whether this person owns the agent.
    pub fn owns(&self, agent: &AgentConfig) -> bool {
        agent.owner_id.as_deref() == Some(self.user_id)
    }

    /// May manage `agent`'s objects: system, `all_permission`, or `owned_permission` plus
    /// ownership of the agent.
    pub fn manages(&self, agent: &AgentConfig, all_permission: &str, owned_permission: &str) -> bool {
        self.is_system || self.has(all_permission) || (self.has(owned_permission) && self.owns(agent))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::schema::Requester;

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
        Caller { user_id, is_system: false, permissions }
    }


    fn principal(system: bool, id: &str, perms: &[&str]) -> Principal {
        Principal {
            user_id: id.into(),
            username: id.into(),
            role: crate::auth::Role {
                role_id: "r".into(),
                name: "r".into(),
                permissions: perms.iter().map(|p| p.to_string()).collect(),
                is_system: system,
                created_at: Utc::now(),
            },
            permissions: perms.iter().map(|p| p.to_string()).collect(),
            method: crate::auth::AuthMethod::Jwt,
        }
    }

    #[test]
    fn agent_view_edit_and_require() {
        let a = agent("alice");
        assert!(can_view_agent(&principal(true, "x", &[]), &a));
        assert!(can_view_agent(&principal(false, "x", &["all_agents:view"]), &a));
        assert!(can_view_agent(&principal(false, "alice", &[]), &a));
        assert!(!can_view_agent(&principal(false, "bob", &[]), &a));
        assert!(can_edit_agent(&principal(false, "alice", &[]), &a));
        assert!(!can_edit_agent(&principal(false, "bob", &["all_agents:view"]), &a));
        assert!(require(&principal(false, "b", &["p"]), "p").is_ok());
        assert!(require(&principal(false, "b", &[]), "p").is_err());
        assert_eq!(platform_of_slug("discord__1"), Some("discord"));
        assert!(can_show_platform_chats(&principal(false, "alice", &["owned_agents:show_discord_chats"]), &a, "discord"));
    }

    #[test]
    fn a_system_role_bypasses_every_permission() {
        let system = Caller { user_id: "root", is_system: true, permissions: &[] };
        assert!(system.manages(&agent("alice"), "all_agents:x", "owned_agents:x"));
    }

    #[test]
    fn the_all_permission_covers_every_agent() {
        let perms = vec!["all_agents:x".to_string()];
        let admin = caller("admin", &perms);
        assert!(admin.manages(&agent("alice"), "all_agents:x", "owned_agents:x"));
        assert!(!admin.manages(&agent("alice"), "all_agents:y", "owned_agents:x"));
    }

    #[test]
    fn the_owned_permission_needs_ownership() {
        let perms = vec!["owned_agents:x".to_string()];
        let alice = caller("alice", &perms);
        assert!(alice.manages(&agent("alice"), "all_agents:x", "owned_agents:x"));
        assert!(!alice.manages(&agent("bob"), "all_agents:x", "owned_agents:x"));
        assert!(alice.owns(&agent("alice")));
        assert!(!alice.owns(&agent("bob")));
    }

    #[test]
    fn no_permission_manages_nothing_but_may_still_own() {
        let carol = caller("carol", &[]);
        assert!(!carol.manages(&agent("carol"), "all_agents:x", "owned_agents:x"));
        assert!(carol.owns(&agent("carol")));
    }

    #[test]
    fn an_agent_without_an_owner_is_never_owned() {
        let perms = vec!["owned_agents:x".to_string()];
        let carol = caller("carol", &perms);
        let mut unowned = agent("carol");
        unowned.owner_id = None;
        assert!(!carol.owns(&unowned));
        assert!(!carol.manages(&unowned, "all_agents:x", "owned_agents:x"));
    }

    /// The caller's permissions are borrowed, so a revoked one is gone on the next request.
    #[test]
    fn a_revoked_permission_is_absent_from_the_next_callers_list() {
        let granted = vec!["all_agents:x".to_string()];
        assert!(caller("admin", &granted).manages(&agent("a"), "all_agents:x", "owned_agents:x"));
        let revoked: Vec<String> = vec![];
        assert!(!caller("admin", &revoked).manages(&agent("a"), "all_agents:x", "owned_agents:x"));
    }

    #[test]
    fn requester_still_round_trips_through_serde() {
        let actor = Requester::Agent("dp-a".into());
        let json = serde_json::to_string(&actor).unwrap();
        assert_eq!(serde_json::from_str::<Requester>(&json).unwrap(), actor);
        let when = Utc::now();
        assert!(when.to_rfc3339().contains('T'));
    }
}
