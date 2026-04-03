use axum::{
    Router,
    extract::{Path, State},
    routing::get,
};
use reqwest::StatusCode;
use serde_json::json;

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response},
        },
        state::HTTPState,
    },
    config::VizierConfig,
};

mod channel;
mod memory;
mod session;
mod task;

use channel::channel;
use memory::memory;
use session::session;
use task::task;

impl VizierConfig {
    fn is_agent_exists(&self, agent_id: &String) -> bool {
        self.agents.get(agent_id).is_some()
    }
}

pub fn agents() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_agents))
        .route("/{agent_id}", get(agent_detail))
        .nest("/{agent_id}/channel", channel())
        .nest("/{agent_id}/memory", memory())
        .nest("/{agent_id}/session", session())
        .nest("/{agent_id}/tasks", task())
}

async fn list_agents(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<serde_json::Value>> {
    let res = state
        .config
        .agents
        .iter()
        .map(|(key, config)| {
            json!({
                "agent_id": key.clone(),
                "name": config.name.clone(),
                "description": config.description.clone(),
            })
        })
        .collect();

    api_response(StatusCode::OK, res)
}

async fn agent_detail(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<serde_json::Value> {
    let res = state.config.agents.get(&agent_id.clone()).map(|config| {
        json!({
            "agent_id": agent_id.clone(),
            "name": config.name.clone(),
            "description": config.description.clone(),
        })
    });

    if res.is_none() {
        err_response(StatusCode::NOT_FOUND, "not found".into())
    } else {
        api_response(StatusCode::OK, res.unwrap())
    }
}
