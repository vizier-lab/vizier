use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

use super::{AgentId, DreamStage};
use crate::utils::markdown::MarkdownDoc;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, utoipa::ToSchema, MarkdownDoc)]
pub struct DreamJournalEntry {
    pub id: String,
    pub dream_cycle_id: String,
    pub agent_id: AgentId,
    pub timestamp: DateTime<Utc>,
    pub stage: DreamStage,
    pub source_sessions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_context: Option<String>,
    #[markdown(content)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_used: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_used: Option<String>,
}
