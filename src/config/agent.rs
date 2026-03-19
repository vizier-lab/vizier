use std::collections::HashMap;

use duration_string::DurationString;
use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderVariant;

pub type AgentConfigs = HashMap<String, AgentConfig>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub preamble: Option<String>,
    pub description: Option<String>,
    pub provider: ProviderVariant,
    pub model: String,
    pub session_ttl: DurationString,
    pub memory: MemoryConfig,
    pub turn_depth: usize,
    pub tools: AgentToolsConfig,
    pub silent_read_initiative_chance: f32,
    pub show_thinking: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryConfig {
    pub session_memory_recall_depth: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AgentToolsConfig {
    pub python_interpreter: bool,
    pub cli_access: bool,
    pub brave_search: ToolConfig,
    pub vector_memory: ToolConfig,
    pub discord: ToolConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolConfig {
    pub enabled: bool,
    pub programmatic_tool_call: bool,
}

impl ToolConfig {
    pub fn is_programatically_enabled(&self) -> bool {
        self.enabled && self.programmatic_tool_call
    }
}
