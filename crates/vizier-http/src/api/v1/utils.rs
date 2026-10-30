//! Utility endpoints. `setup-state` is what the WebUI polls to show an agent's build
//! progress (model downloads included) as a single spinner and message.

use axum::{Extension, Router, extract::State, routing::get};
use reqwest::StatusCode;

use crate::{
    auth::Principal,
    models::response::{APIResponse, api_response},
    state::HTTPState,
};
use vizier_core::setup::SetupStates;

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
    Extension(user): Extension<Principal>,
) -> (StatusCode, axum::Json<APIResponse<SetupStates>>) {
    match vizier_core::service::setup_state::states(&super::auth::services(&state), &user).await {
        Ok(states) => api_response(StatusCode::OK, states),
        Err(e) => super::auth::err(e),
    }
}
