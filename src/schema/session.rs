use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


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
    JsonSchema,
    utoipa::ToSchema,
)]
pub enum VizierChannelId {
    DiscordChanel(u64),
    TelegramChannel(i64),
    HTTP(String, String),
    Task(String, DateTime<Utc>),
    InterAgent(Vec<String>),
    Heartbeat(DateTime<Utc>),
    System,
    Subagent,
    Dream(Box<VizierSession>, DreamStage),
}

impl VizierChannelId {
    pub fn to_slug(&self) -> String {
        match self {
            Self::DiscordChanel(id) => format!("discord__{}", id),
            Self::TelegramChannel(id) => format!("telegram__{}", id),
            Self::HTTP(user, id) => format!("http__{}__{}", user, id),
            Self::Task(id, datetime) => {
                format!("task__{}__{}", id, datetime.timestamp_subsec_nanos())
            }
            Self::InterAgent(set) => {
                let participants = set.join("__");

                format!("inter_agent__[{participants}]")
            }
            Self::Heartbeat(datetime) => format!("heartbeat__{}", datetime.to_rfc3339()),
            Self::System => "SYSTEM".into(),
            Self::Dream(session, stage) => {
                let stage_str = match stage {
                    DreamStage::Extraction => "extraction",
                    DreamStage::Consolidation => "consolidation",
                };
                format!("DREAM__{}__{}", session.to_slug(), stage_str)
            }
            Self::Subagent => "SUBAGENT".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VizierSessionDetail {
    pub agent_id: AgentId,
    pub channel: VizierChannelId,
    pub topic: Option<TopicId>,
    pub title: String,
    #[serde(default)]
    pub is_thinking: bool,
}

#[derive(
    Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DreamStage {
    Extraction,
    Consolidation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DreamStatus {
    Idle,
    Extracting {
        started_at: DateTime<Utc>,
        cycle_id: String,
        total_sessions: usize,
        completed_sessions: usize,
    },
    Consolidating {
        started_at: DateTime<Utc>,
        cycle_id: String,
    },
}