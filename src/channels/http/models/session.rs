use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SessionResponse {
    pub agent_id: String,
    pub session_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatRequest {
    pub user: String,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ChatResponse {
    pub content: String,
    pub thinking: bool,
}
