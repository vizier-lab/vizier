//! Utility endpoints. `setup-state` is what the WebUI polls to show an agent's build
//! progress (model downloads included) as a single spinner and message.

use axum::{Extension, Router, extract::State, routing::get};
use reqwest::StatusCode;

use crate::{
    channels::http::{
        api::v1::agents::user_can_view_agent,
        auth::AuthenticatedUser,
        models::response::{APIResponse, api_response, err_response},
        state::HTTPState,
    },
    setup::SetupStates,
    storage::agent::AgentStorage,
};

pub fn utils() -> Router<HTTPState> {
    Router::new().route("/setup-state", get(setup_state))
}

#[utoipa::path(
    get,
    path = "/utils/setup-state",
    responses(
        (status = 200, description = "Every setup in progress, plus recently finished ones, of the agents the caller can view", body = APIResponse<SetupStates>)
    )
)]
pub async fn setup_state(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> (StatusCode, axum::Json<APIResponse<SetupStates>>) {
    let setups = state.deps.setup.snapshot();
    if setups.is_empty() {
        return api_response(StatusCode::OK, SetupStates { setups });
    }
    // Only the agents this person may see: build errors can carry provider details.
    let agents = match state.storage.list_agents().await {
        Ok(agents) => agents,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let setups = setups
        .into_iter()
        .filter(|setup| match agents.iter().find(|(id, _)| *id == setup.agent_id) {
            Some((_, config)) => user_can_view_agent(&user, config),
            // Still being created: its owner, and whoever may view every agent.
            None => {
                user.role.is_system
                    || user.permissions.iter().any(|p| p == "all_agents:view")
                    || setup.owner_id.as_deref() == Some(user.user_id.as_str())
            }
        })
        .collect();
    api_response(StatusCode::OK, SetupStates { setups })
}
