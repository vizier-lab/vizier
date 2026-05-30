use std::collections::HashMap;

use duration_string::DurationString;
use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderVariant;

pub type AgentConfigs = HashMap<String, AgentConfig>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub system_prompt: Option<String>,
    pub description: Option<String>,
    pub provider: ProviderVariant,
    pub model: String,
    pub session_memory: MemoryConfig,
    pub thinking_depth: usize,
    pub tools: AgentToolsConfig,
    pub silent_read_initiative_chance: f32,
    pub show_thinking: Option<bool>,
    pub show_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_documents: Option<Vec<String>>,
    pub prompt_timeout: DurationString,
    #[serde(skip)]
    pub documents: Vec<String>,
    pub heartbeat_interval: DurationString,
    pub dream_interval: DurationString,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryConfig {
    pub max_capacity: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AgentToolsConfig {
    pub timeout: DurationString,
    #[serde(default)]
    pub programmatic_sandbox: bool,
    #[serde(default)]
    pub shell_access: bool,
    #[serde(default)]
    pub brave_search: ToolConfig,
    #[serde(default)]
    pub vector_memory: ToolConfig,
    #[serde(default)]
    pub discord: ToolConfig,
    #[serde(default)]
    pub telegram: ToolConfig,
    #[serde(default)]
    pub notify_primary_user: ToolConfig,
    #[serde(default)]
    pub fetch: ToolConfig,
    #[serde(default)]
    pub http_client: ToolConfig,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolConfig {
    pub enabled: bool,
}
