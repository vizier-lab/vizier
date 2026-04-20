use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct VizierResponseStats {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub total_cached_input_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub duration: tokio::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct VizierResponse {
    pub timestamp: DateTime<Utc>,
    pub content: VizierResponseContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VizierResponseContent {
    ThinkingStart,
    Thinking(String),
    ToolChoice {
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