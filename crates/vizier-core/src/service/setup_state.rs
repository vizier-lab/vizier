//! What the WebUI polls to show an agent's build progress (model downloads included).

use crate::{
    access::can_view_agent,
    auth::Principal,
    service::{ServiceError, Services},
    setup::SetupStates,
    storage::agent::AgentStorage,
};

/// Every setup in progress, plus recently finished ones, of the agents `user` can view.
pub async fn states(services: &Services, user: &Principal) -> Result<SetupStates, ServiceError> {
    let deps = services.deps();
    let setups = deps.setup.snapshot();
    if setups.is_empty() {
        return Ok(SetupStates { setups });
    }
    // Only the agents this person may see: build errors can carry provider details.
    let agents = deps
        .storage
        .list_agents()
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?;
    let setups = setups
        .into_iter()
        .filter(|setup| match agents.iter().find(|(id, _)| *id == setup.agent_id) {
            Some((_, config)) => can_view_agent(user, config),
            // Still being created: its owner, and whoever may view every agent.
            None => {
                user.role.is_system
                    || user.permissions.iter().any(|p| p == "all_agents:view")
                    || setup.owner_id.as_deref() == Some(user.user_id.as_str())
            }
        })
        .collect();
    Ok(SetupStates { setups })
}
