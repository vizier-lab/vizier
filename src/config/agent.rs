use std::collections::HashMap;

use duration_string::DurationString;
use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderVariant;

pub type AgentConfigs = HashMap<String, AgentConfig>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub description: Option<String>,
    pub provider: ProviderVariant,
    pub model: String,
    pub session_ttl: DurationString,
    pub memory: MemoryConfig,
    pub turn_depth: usize,
    pub tools: AgentToolsConfig,
    pub silent_read_initiative_chance: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryConfig {
    pub session_memory_recall_depth: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AgentToolsConfig {
    pub enable_python_interpreter: bool,
    pub enable_brave_search: bool,
    pub enable_cli_access: bool,
    pub enable_vector_memory: bool,
}
