use serde::{Deserialize, Serialize};

use crate::schema::{AgentId, Memory, MemoryGraph, MemoryQueryParams, MemoryVisibility, VizierAttachment, agent::AgentConfig, file::FileRecord};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AgentHealthStatus {
    pub agent_id: AgentId,
    pub alive: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandRequest {
    Exit,
    Status,
    HealthCheck,
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
    HealthCheck {
        resp: tokio::sync::oneshot::Sender<Vec<AgentHealthStatus>>,
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

#[derive(Debug, Clone, Serialize)]
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

pub enum FileCommand {
    Upload {
        filename: String,
        content: Vec<u8>,
        response: tokio::sync::oneshot::Sender<anyhow::Result<FileRecord>>,
    },
    Resolve {
        attachment: VizierAttachment,
        response: tokio::sync::oneshot::Sender<anyhow::Result<Vec<u8>>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryOpRequest {
    Write {
        slug: Option<String>,
        title: String,
        content: String,
        visibility: MemoryVisibility,
        shared_to: Vec<String>,
        tags: Vec<String>,
        attachments: Vec<VizierAttachment>,
    },
    Query {
        query: String,
        limit: usize,
        threshold: f64,
    },
    GetById {
        slug: String,
    },
    List {
        params: MemoryQueryParams,
    },
    GetRelated {
        slug: String,
    },
    GetGraph,
    Delete {
        slug: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MemoryOpResponse {
    Memory(Memory),
    MemoryList(Vec<Memory>),
    MemoryOption(Option<Memory>),
    Paginated(crate::schema::PaginatedMemory),
    Graph(MemoryGraph),
    Unit,
}

pub struct MemoryOpEnvelope {
    pub op: MemoryOpRequest,
    pub response: tokio::sync::oneshot::Sender<anyhow::Result<MemoryOpResponse>>,
}
