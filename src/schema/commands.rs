use serde::{Deserialize, Serialize};

use crate::schema::{
    AgentId, BundleSummary, ImportReport, Memory, MemoryGraph, MemoryQueryParams, MemoryRevision,
    PaginatedMemoryRevisions, RevisionDiff, RevisionOrigin, RollbackResponse, VizierAttachment,
    agent::AgentConfig, file::FileRecord,
};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AgentHealthStatus {
    pub agent_id: AgentId,
    /// `state == Running`, kept for clients that only read this.
    pub alive: bool,
    #[serde(default)]
    pub state: AgentRunState,
    /// Why the agent last failed to start or stopped running, while it is `Failed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Failed starts since it last ran for a while.
    #[serde(default)]
    pub failures: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_retry_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The agent's Discord and Telegram readers, while it is running (#50).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<ChannelHealth>,
    /// What the agent's model resolved to, once it has been built (spec 015, SC-004).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<AgentModelInfo>,
    /// Configuration that works but probably isn't what was meant (FR-033).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// The model an agent runs on, with its effective limits and where they came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AgentModelInfo {
    pub provider: crate::config::provider::ProviderId,
    pub model: String,
    pub context_window: Option<u64>,
    pub max_output: Option<u64>,
    pub context_source: crate::llm::ContextSource,
}

/// One of an agent's platform readers, as its health reports it.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChannelHealth {
    pub platform: ChannelPlatform,
    pub state: ChannelState,
    /// Why it last stopped, while it is reconnecting or failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Stops since it last ran for a while.
    #[serde(default)]
    pub failures: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_retry_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChannelPlatform {
    Discord,
    Telegram,
}

impl std::fmt::Display for ChannelPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Discord => "discord",
            Self::Telegram => "telegram",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChannelState {
    Running,
    /// Stopped, and started again at `next_retry_at`.
    Reconnecting,
    /// Stopped by an error retrying won't fix, such as a revoked token. Started again when
    /// the agent is updated.
    Failed,
}

/// Where an agent's supervisor is (#42).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunState {
    /// Being built: requests queue until it runs.
    #[default]
    Starting,
    Running,
    /// Failed to build, or stopped with an error. Restarted at `next_retry_at`; until then
    /// requests are answered with an error rather than queued.
    Failed,
    Stopped,
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
        bundle: Option<String>,
        path: Option<String>,
        create_only: bool,
        title: String,
        content: String,
        tags: Vec<String>,
        attachments: Vec<VizierAttachment>,
        origin: RevisionOrigin,
    },
    Query {
        bundle: Option<String>,
        query: String,
        limit: usize,
        threshold: f64,
    },
    GetById {
        bundle: Option<String>,
        path: String,
    },
    List {
        params: MemoryQueryParams,
    },
    GetRelated {
        bundle: Option<String>,
        path: String,
    },
    GetGraph {
        bundle: Option<String>,
        search: Option<String>,
    },
    Delete {
        bundle: Option<String>,
        path: String,
        origin: RevisionOrigin,
    },
    ListBundles,
    DeleteBundle {
        bundle: String,
        force: bool,
        origin: RevisionOrigin,
    },
    ExportBundle {
        bundle: String,
    },
    ImportBundle {
        bundle: String,
        zip_bytes: Vec<u8>,
        origin: RevisionOrigin,
    },
    // Version history (specs/006-memory-version-history). Reads go through the channel too,
    // so `memory.rs` keeps one access pattern (research Decision 6).
    ListRevisions {
        bundle: Option<String>,
        path: String,
        offset: usize,
        limit: usize,
    },
    GetRevision {
        bundle: Option<String>,
        path: String,
        seq: i64,
    },
    DiffRevisions {
        bundle: Option<String>,
        path: String,
        from: Option<i64>,
        to: i64,
    },
    Rollback {
        bundle: Option<String>,
        path: String,
        seq: i64,
        origin: RevisionOrigin,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MemoryOpResponse {
    Memory(Memory),
    MemoryList(Vec<Memory>),
    /// What a search returns: addressed passages, not whole document bodies (FR-008). Separate
    /// from `MemoryList` because link traversal and listing still answer in documents.
    PassageList(Vec<crate::schema::MemoryPassageResult>),
    MemoryOption(Option<Memory>),
    Paginated(crate::schema::PaginatedMemory),
    Graph(MemoryGraph),
    Bundles(Vec<BundleSummary>),
    Export(Vec<u8>),
    Import(ImportReport),
    Revisions(PaginatedMemoryRevisions),
    Revision(Option<MemoryRevision>),
    Diff(RevisionDiff),
    Rollback(RollbackResponse),
    Unit,
}

pub struct MemoryOpEnvelope {
    pub op: MemoryOpRequest,
    pub response: tokio::sync::oneshot::Sender<anyhow::Result<MemoryOpResponse>>,
}
