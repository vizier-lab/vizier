use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::shell::ShellConfig;
use crate::config::tools::mcp::McpClientConfig;
use crate::schema::agent::AgentConfig;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandRequest {
    Exit,
    Status,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandResponse {
    Ok(String),
    Error(String),
}

impl std::fmt::Display for CommandResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandResponse::Ok(s) => write!(f, "{s}"),
            CommandResponse::Error(s) => write!(f, "{s}"),
        }
    }
}

pub enum AgentCommand {
    Create {
        agent_id: String,
        config: AgentConfig,
        resp: tokio::sync::oneshot::Sender<AgentCommandResult>,
    },
    Update {
        agent_id: String,
        config: AgentConfig,
        resp: tokio::sync::oneshot::Sender<AgentCommandResult>,
    },
    Delete {
        agent_id: String,
        delete_workspace: bool,
        resp: tokio::sync::oneshot::Sender<AgentCommandResult>,
    },
}

pub enum AgentCommandResult {
    Ok(AgentSummary),
    Error(String),
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AgentSummary {
    pub agent_id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar_url: Option<String>,
    pub owner_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_to: Vec<String>,
}

pub enum ChannelCommand {
    AgentCreated {
        agent_id: String,
        config: AgentConfig,
    },
    AgentUpdated {
        agent_id: String,
        config: AgentConfig,
    },
    AgentDeleted {
        agent_id: String,
    },
}

pub enum GlobalCommand {
    ReloadMcp {
        config: HashMap<String, McpClientConfig>,
        resp: tokio::sync::oneshot::Sender<GlobalCommandResult>,
    },
    ReloadShell {
        config: ShellConfig,
        resp: tokio::sync::oneshot::Sender<GlobalCommandResult>,
    },
}

pub enum GlobalCommandResult {
    Ok(String),
    Error(String),
}
