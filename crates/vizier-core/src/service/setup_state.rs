//! What the WebUI polls to show an agent's build progress (model downloads included).

use crate::{
    access::can_view_agent,
    auth::Principal,
    service::{ServiceError, Services},
    setup::SetupStates,
    storage::agent::AgentStorage,
};

/// Every setup in progress, plus recently finished ones, of the agents `user` can view.
///
/// The setups this process holds come from its own board, which is the live progress. The
/// setups of agents another node holds come from the published views
/// (`specs/163-agent-service-seam` contracts/shared-state-views.md). In one node the board
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
