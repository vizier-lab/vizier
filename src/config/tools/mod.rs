use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::tools::mcp::McpClientConfig;

pub mod mcp;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolsConfig {
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub mcp_servers: HashMap<String, McpClientConfig>,
    pub brave_search: Option<BraveSearchConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BraveSearchConfig {
    pub api_key: String,
    pub safesearch: bool,
}

impl Default for BraveSearchConfig {
    fn default() -> Self {
        Self {
            api_key: "${BRAVE_API_KEY}".into(),
            safesearch: true,
        }
    }
}
