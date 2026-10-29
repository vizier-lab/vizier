use anyhow::Result;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::agent::model::{PROMPT_TIMED_OUT, PROVIDER_UNAVAILABLE, RATE_LIMITED},
    error::VizierError,
    llm::types::{Message, Part},
    schema::VizierAttachment,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct VizierResponseStats {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub total_cached_input_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub duration: tokio::time::Duration,
    pub cache_creation_input_tokens: u64,
    pub total_cache_creation_input_tokens: u64,
    pub current_context_size: Option<u64>,
    pub context_window: Option<u64>,
    /// The turn reached `thinking_depth`, and the reply is what it had by then (#48).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub depth_exhausted: bool,
    /// The turn ended because this turn-ending tool succeeded, not because the model answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_by_tool: Option<String>,
    /// The provider and model that answered, for the usage view's cost estimate (FR-021).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>)]
    pub provider: Option<crate::config::provider::ProviderId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct VizierResponse {
    pub timestamp: DateTime<Utc>,
    pub content: VizierResponseContent,
    pub attachments: Vec<VizierAttachment>,
    /// The `session_history` uid this response was saved as. Set by whoever saved it,
    /// after the save, so it is never part of the stored `data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_uid: Option<String>,
    /// The turn was interrupted by a restart and carried on afterwards
    /// (`specs/023-persist-turn-activity`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resumed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Completion,
    ToolTimeout,
    PromptTimeout,
    /// The provider kept rate limiting after every retry.
    RateLimited,
    /// The provider kept failing transiently (5xx, dropped connection, no answer) after every
    /// retry.
    ProviderUnavailable,
    /// The agent itself is down: it failed to start and is waiting to be restarted.
    AgentUnavailable,
}

impl ErrorKind {
    /// Classify an error string into the appropriate `ErrorKind`.
    /// Used by the agent when persisting errors and by the transport
    /// when forwarding errors to live channels, so the two paths stay
    /// in sync.
    pub fn classify(err_str: &str) -> Self {
        if err_str.contains("timed out") && err_str.contains("Tool") {
            ErrorKind::ToolTimeout
        } else if err_str.contains("thinking depth") || err_str.contains(PROMPT_TIMED_OUT) {
            ErrorKind::PromptTimeout
        } else if err_str.contains(RATE_LIMITED) {
            ErrorKind::RateLimited
        } else if err_str.contains(PROVIDER_UNAVAILABLE) {
            ErrorKind::ProviderUnavailable
        } else {
            ErrorKind::Completion
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorKind::Completion => "completion",
            ErrorKind::ToolTimeout => "tool_timeout",
            ErrorKind::PromptTimeout => "prompt_timeout",
            ErrorKind::RateLimited => "rate_limited",
            ErrorKind::ProviderUnavailable => "provider_unavailable",
            ErrorKind::AgentUnavailable => "agent_unavailable",
        }
    }

    /// How a chat channel titles the error for a person.
    pub fn label(&self) -> &'static str {
        match self {
            ErrorKind::Completion => "Completion Error",
            ErrorKind::ToolTimeout => "Tool Timeout",
            ErrorKind::PromptTimeout => "Prompt Timeout",
            ErrorKind::RateLimited => "Rate Limited",
            ErrorKind::ProviderUnavailable => "Provider Unavailable",
            ErrorKind::AgentUnavailable => "Agent Unavailable",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
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
    AudioReply(
        VizierAttachment,
        Option<String>,
        Option<VizierResponseStats>,
    ),
    Error {
        kind: ErrorKind,
        message: String,
    },
    #[default]
    Empty,
    Abort,
    Checkpoint {
        handover: Option<String>,
    },
}

impl VizierResponse {
    /// This tool response as the result of call `call_id`.
    pub fn to_tool_result(
        &self,
        call_id: String,
        workspace: &str,
        resolver: &dyn crate::schema::request::AttachmentResolver,
    ) -> Result<Message> {
        let text = match &self.content {
            VizierResponseContent::Message { content, stats: _ } => content.clone(),
            VizierResponseContent::AudioReply(_, Some(content), _) => content.clone(),
            VizierResponseContent::AudioReply(_, None, _) => "[Audio reply]".to_string(),
            VizierResponseContent::ToolResponse { response } => serde_json::to_string(&response)?,
            VizierResponseContent::Error { kind, message } => {
                format!("[Error: {}] {}", kind.as_str(), message)
            }
            _ => return Err(VizierError("unimplemented".into()).into()),
        };
        let mut content = vec![Part::Text(text)];
        for attachment in &self.attachments {
            content.push(attachment.to_part(workspace, resolver)?);
        }
        Ok(Message::ToolResult { call_id, content })
    }
}
