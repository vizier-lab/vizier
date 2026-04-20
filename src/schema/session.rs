use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

pub type AgentId = String;

pub type TopicId = String;

#[derive(
    Debug,
    Clone,
    Hash,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    SurrealValue,
    JsonSchema,
    utoipa::ToSchema,
)]
pub struct VizierSession(pub AgentId, pub VizierChannelId, pub Option<TopicId>);

impl VizierSession {
    pub fn to_slug(&self) -> String {
        format!(
            "{}__{}__{}",
            self.0,
            self.1.to_slug(),
            self.2.clone().unwrap_or("DEFAULT".to_string())
        )
    }
}

#[derive(
    Debug,
    Clone,
    Hash,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    SurrealValue,
    JsonSchema,
    utoipa::ToSchema,
)]
pub enum VizierChannelId {
    DiscordChanel(u64),
    TelegramChannel(i64),
    HTTP(String),
    Task(String, DateTime<Utc>),
    InterAgent(Vec<String>),
    Heartbeat(DateTime<Utc>),
    System,
    Subagent,
    Dream(Box<VizierSession>),
}

impl VizierChannelId {
    pub fn to_slug(&self) -> String {
        match self {
            Self::DiscordChanel(id) => format!("discord__{}", id),
            Self::TelegramChannel(id) => format!("telegram__{}", id),
            Self::HTTP(id) => format!("http__{}", id),
            Self::Task(id, datetime) => {
                format!("task__{}__{}", id, datetime.timestamp_subsec_nanos())
            }
            Self::InterAgent(set) => {
                let participants = set.join("__");

                format!("inter_agent__[{participants}]")
            }
            Self::Heartbeat(datetime) => format!("heartbeat__{}", datetime.to_rfc3339()),
            Self::System => "SYSTEM".into(),
            Self::Dream(session) => format!("DREAM__{}", session.to_slug()),
            Self::Subagent => "SUBAGENT".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct VizierSessionDetail {
    pub agent_id: AgentId,
    pub channel: VizierChannelId,
    pub topic: Option<TopicId>,
    pub title: String,
}