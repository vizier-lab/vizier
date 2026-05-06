use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderVariant;

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct AgentDefinition {
    pub agent_id: String,
    pub name: String,
    pub description: Option<String>,
    pub agent_behaviour: AgentBehaviour,
    pub model_provider: ModelProvider,
    pub session_memory: AgentSessionMemory,
    pub tools: AgentTools,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct AgentBehaviour {
    pub thinking_depth: usize,
    pub silent_read_initiative_chance: f32,
    pub show_thinking: Option<bool>,
    pub show_tool_calls: Option<bool>,
    pub prompt_timeout: u32,
    pub heartbeat_interval: u32,
    pub dream_interval: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ModelProvider {
    pub provider: ProviderVariant,
    pub model: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct AgentSessionMemory {
    pub max_capacity: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, JsonSchema)]
pub struct AgentTools {
    pub timeout: u32,
    #[serde(default)]
    pub programmatic_sandbox: AgentTool<()>,
    #[serde(default)]
    pub shell_access: AgentTool<()>,
    #[serde(default)]
    pub brave_search: AgentTool<()>,
    #[serde(default)]
    pub vector_memory: AgentTool<()>,
    #[serde(default)]
    pub discord: AgentTool<()>,
    #[serde(default)]
    pub telegram: AgentTool<()>,
    #[serde(default)]
    pub notify_primary_user: AgentTool<()>,
    #[serde(default)]
    pub fetch: AgentTool<()>,
    #[serde(default)]
    pub http_client: AgentTool<()>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, JsonSchema)]
pub struct AgentTool<Settings> {
    pub enabled: bool,
    pub settings: Settings,
}
