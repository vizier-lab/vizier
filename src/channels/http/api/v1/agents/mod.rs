use axum::{
    Router,
    extract::{Path, Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use schemars::schema_for;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    config::VizierConfig,
    schema::{AgentDefinition, AgentUsageStats},
    storage::{VizierStorage, history::HistoryStorage},
};

pub mod channel;
pub mod documents;
pub mod memory;
pub mod task;

use channel::channel;
use documents::documents;
use memory::memory;
use task::task;

impl VizierConfig {
    fn is_agent_exists(&self, agent_id: &String) -> bool {
        self.agents.get(agent_id).is_some()
    }
}

pub fn agents() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_agents))
        .route("/schema", get(agent_schema))
        .route("/{agent_id}", get(agent_detail))
        .route("/{agent_id}/usage", get(agent_usage))
        .nest("/{agent_id}/channel", channel())
        .nest("/{agent_id}/documents", documents())
        .nest("/{agent_id}/memory", memory())
        .nest("/{agent_id}/tasks", task())
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AgentSummary {
    pub agent_id: String,
    pub name: String,
    pub description: Option<String>,
}

#[utoipa::path(
    get,
    path = "/agents",
    responses(
        (status = 200, description = "List of agents", body = APIResponse<Vec<AgentSummary>>)
    )
)]
async fn list_agents(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<AgentSummary>> {
    let res: Vec<AgentSummary> = state
        .config
        .agents
        .iter()
        .map(|(key, config)| AgentSummary {
            agent_id: key.clone(),
            name: config.name.clone(),
            description: config.description.clone(),
        })
        .collect();

    api_response(StatusCode::OK, res)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Agent details", body = APIResponse<AgentSummary>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
async fn agent_detail(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<AgentSummary> {
    let res = state
        .config
        .agents
        .get(&agent_id)
        .map(|config| AgentSummary {
            agent_id: agent_id.clone(),
            name: config.name.clone(),
            description: config.description.clone(),
        });

    if res.is_none() {
        err_response(StatusCode::NOT_FOUND, "not found".into())
    } else {
        api_response(StatusCode::OK, res.unwrap())
    }
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct UsageQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/schema",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Agent Definition Schema", body = APIResponse<serde_json::Value>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
async fn agent_schema(
    State(state): State<HTTPState>,
) -> models::response::Response<serde_json::Value> {
    let res = serde_json::to_value(schema_for!(AgentDefinition)).unwrap();
    api_response(StatusCode::OK, res)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/usage",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = UsageQuery,
    responses(
        (status = 200, description = "Agent usage statistics", body = APIResponse<AgentUsageStats>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
async fn agent_usage(
    Path(agent_id): Path<String>,
    Query(query): Query<UsageQuery>,
    State(state): State<HTTPState>,
) -> models::response::Response<AgentUsageStats> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, "agent not found".into());
    }

    let start_date = query.start_date.as_ref().and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    let end_date = query.end_date.as_ref().and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    let storage: &VizierStorage = &state.storage;
    let usage_result = storage
        .aggregate_usage(&agent_id, start_date, end_date)
        .await;

    match usage_result {
        Ok(stats) => api_response(StatusCode::OK, stats),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

