use anyhow::Result;
use chrono::{DateTime, Utc};
use rig::{
    OneOrMany,
    message::{Message, ToolResultContent, UserContent},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

use crate::error::VizierError;

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
    ToolResponse {
        response: serde_json::Value,
    },
    Message {
        content: String,
        stats: Option<VizierResponseStats>,
    },
    Empty,
    Abort,
}

impl VizierResponse {
    pub fn to_tool_response_content(
        &self,
        id: String,
        call_id: Option<String>,
    ) -> Result<UserContent> {
        let tool_contents = match &self.content {
            VizierResponseContent::Message { content, stats } => {
                vec![ToolResultContent::text(content)]
            }

            VizierResponseContent::ToolResponse { response } => {
                vec![ToolResultContent::text(serde_json::to_string(&response)?)]
            }

            _ => return Err(VizierError("unimplemented".into()).into()),
        };

        let res = if let Some(call_id) = call_id {
            UserContent::tool_result_with_call_id(id, call_id, OneOrMany::many(tool_contents)?)
        } else {
            UserContent::tool_result(id, OneOrMany::many(tool_contents)?)
        };

        Ok(res)
    }
}

