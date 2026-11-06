//! What the WebUI polls to show an agent's build progress (model downloads included).

use crate::{
    access::can_view_agent,
    auth::Principal,
    schema::AgentConfig,
    service::{ServiceError, Services},
    setup::SetupStates,
    storage::agent::AgentStorage,
};

/// Every setup in progress, plus recently finished ones, of the agents `user` can view.
///
/// The setups this process holds come from its own board, which is the live progress. The
/// setups of agents another node holds come from the published views
/// (`specs/048-agent-service-seam` contracts/shared-state-views.md). In one node the board
/// holds everything, so what is shown is exactly what it was before.
pub async fn states(services: &Services, user: &Principal) -> Result<SetupStates, ServiceError> {
    let deps = services.deps();
    let mut setups = deps.setup.snapshot();
    let agents = deps
        .storage
        .list_agents()
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?;
    for (id, _) in &agents {
        if setups.iter().any(|setup| &setup.agent_id == id) {
            continue;
        }
        if let Some(view) = crate::agents::views::read_setup(deps, id).await {
            setups.push(view);
        }
    }
    if setups.is_empty() {
        return Ok(SetupStates { setups });
    }
    // Only the agents this person may see: build errors can carry provider details.
    let setups = setups
        .into_iter()
        .filter(|setup| can_see_setup(user, &agents, &setup.agent_id, setup.owner_id.as_deref()))
        .collect();
    Ok(SetupStates { setups })
}

/// Whether `user` may see the setup of `agent_id`: the agents they can view, and for an agent
/// still being created (not stored yet) its owner and whoever may view every agent.
///
/// The one rule behind both `states` and the inbox stream (`specs/051-realtime-notifications`),
/// so a setup shown by polling and one pushed live can never disagree.
pub fn can_see_setup(
    user: &Principal,
    agents: &[(String, AgentConfig)],
    agent_id: &str,
    owner_id: Option<&str>,
) -> bool {
    match agents.iter().find(|(id, _)| id == agent_id) {
        Some((_, config)) => can_view_agent(user, config),
        None => {
            user.role.is_system
                || user.permissions.iter().any(|p| p == "all_agents:view")
                || owner_id == Some(user.user_id.as_str())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthMethod;
    use crate::storage::user::Role;

    fn principal(id: &str, system: bool, permissions: &[&str]) -> Principal {
        Principal {
            user_id: id.into(),
            username: id.into(),
            role: Role {
                role_id: "r".into(),
                name: "role".into(),
                permissions: vec![],
                is_system: system,
                created_at: chrono::Utc::now(),
            },
            permissions: permissions.iter().map(|p| p.to_string()).collect(),
            method: AuthMethod::Jwt,
        }
    }

    #[test]
    fn an_agent_being_created_is_seen_by_its_owner_and_those_who_view_everything_only() {
        let agents: Vec<(String, AgentConfig)> = vec![];
        let owner = principal("u1", false, &[]);
        let stranger = principal("u2", false, &[]);
        let viewer = principal("u3", false, &["all_agents:view"]);
        let system = principal("u4", true, &[]);
        assert!(can_see_setup(&owner, &agents, "scout", Some("u1")));
        assert!(!can_see_setup(&stranger, &agents, "scout", Some("u1")));
        assert!(!can_see_setup(&stranger, &agents, "scout", None));
        assert!(can_see_setup(&viewer, &agents, "scout", Some("u1")));
        assert!(can_see_setup(&system, &agents, "scout", None));
    }
}
