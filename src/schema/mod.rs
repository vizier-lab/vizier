use std::fmt::Display;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb_types::SurrealValue;

pub type AgentId = String;

pub type TopicId = String;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
pub struct VizierSession(pub AgentId, pub VizierChannelId, pub Option<TopicId>);

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
pub enum VizierChannelId {
    DiscordChanel(u64),
    HTTP(String),
    Task(String, DateTime<Utc>),
    Socket(String),
    InterAgent(Vec<String>),
    System,
}

impl VizierChannelId {
    pub fn to_slug(&self) -> String {
        match self {
            Self::DiscordChanel(id) => format!("discord__{}", id),
            Self::HTTP(id) => format!("http__{}", id),
            Self::Task(id, datetime) => {
                format!("task__{}__{}", id, datetime.timestamp_subsec_nanos())
            }
            Self::Socket(id) => format!("socket__{}", id),
            Self::InterAgent(set) => {
                let participants = set.join("__");

                format!("inter_agent__[{participants}]")
            }
            Self::System => "SYSTEM".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct VizierResponseStats {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub total_cached_input_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
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
    Abort,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub enum VizierRequestContent {
    Chat(String),
    Prompt(String),
    SilentRead(String),
    Task(String),
    Command(String),
}

impl Display for VizierRequestContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Chat(content) => content,
                Self::Prompt(content) => content,
                Self::SilentRead(content) => content,
                Self::Task(content) => content,
                Self::Command(content) => content,
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct VizierRequest {
    pub user: String,
    pub content: VizierRequestContent,
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
    pub vizier_session: VizierSession,
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

// is an indexed document
#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct Skill {
    pub name: String,
    pub agent_id: Option<AgentId>,
    pub author: String,
    pub description: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct VizierSessionDetail {
    pub agent_id: AgentId,
    pub channel: VizierChannelId,
    pub topic: Option<TopicId>,
    pub title: String,
}
