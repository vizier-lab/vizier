use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::schema::VizierResponse;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatRequest {
    pub user: String,
    pub content: String,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ChoiceResponse {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ChatResponse {
    pub content: Option<String>,
    pub choice: Option<ChoiceResponse>,
    pub thinking: bool,
    pub timestamp: Option<DateTime<Utc>>,
}

impl From<VizierResponse> for ChatResponse {
    fn from(value: VizierResponse) -> Self {
        match value {
            VizierResponse::ThinkingProgress => Self {
                content: None,
                thinking: true,
                timestamp: Some(Utc::now()),
                choice: None,
            },
            VizierResponse::Message { content, stats: _ } => Self {
                content: Some(content),
                thinking: false,
                timestamp: Some(Utc::now()),
                choice: None,
            },
            VizierResponse::Thinking { name, args } => Self {
                content: None,
                thinking: false,
                timestamp: Some(Utc::now()),
                choice: Some(ChoiceResponse { name, args }),
            },
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum ChatHistory {
    request(ChatRequest),
    response(ChatResponse),
}
