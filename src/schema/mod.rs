use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb_types::SurrealValue;

pub type AgentId = String;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
pub struct VizierSession(pub AgentId, pub SessionId);

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
pub enum SessionId {
    DiscordChanel(u64),
    HTTP(String),
    Task(String),
    Socket(String),
}

impl SessionId {
    pub fn to_slug(&self) -> String {
        match self {
            Self::DiscordChanel(id) => format!("discord__{}", id),
            Self::HTTP(id) => format!("http__{}", id),
            Self::Task(id) => format!("task__{}", id),
            Self::Socket(id) => format!("socket__{}", id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct VizierResponseStats {
    pub duration: tokio::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub enum VizierResponse {
    ThinkingProgress,
    Thinking {
        name: String,
        args: serde_json::Value,
    },
    Message {
        content: String,
        stats: Option<VizierResponseStats>,
    },
    Empty,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, SurrealValue)]
pub struct VizierRequest {
    pub user: String,
    pub content: String,
    pub is_silent_read: bool,
    pub is_task: bool,
    pub metadata: serde_json::Value,
}

impl VizierRequest {
    pub fn to_prompt(&self) -> Result<String> {
        Ok(format!(
            "---\n{}\n---\n\n{}",
            self.generate_frontmatter()?,
            self.content
        ))
    }

    pub fn generate_frontmatter(&self) -> Result<String> {
        Ok(serde_yaml::to_string(&json!({
            "sender": self.user,
            "metadata": self.metadata,
        }))?)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
struct User {
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct Memory {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub embedding: Vec<f64>,
    pub agent_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct Task {
    pub slug: String,
    pub user: String,
    pub agent_id: String,
    pub title: String,
    pub instruction: String,
    pub is_active: bool,
    pub schedule: TaskSchedule,
    pub last_executed_at: Option<chrono::DateTime<Utc>>,
    pub timestamp: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub enum TaskSchedule {
    CronTask(String),
    OneTimeTask(chrono::DateTime<Utc>),
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct SessionHistory {
    pub uid: String,
    pub session: VizierSession,
    pub content: SessionHistoryContent,
    pub timestamp: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub enum SessionHistoryContent {
    Request(VizierRequest),
    Response(String, Option<VizierResponseStats>),
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct DocumentIndex {
    pub path: String,
    pub embedding: Vec<f64>,
    pub context: String,
}
