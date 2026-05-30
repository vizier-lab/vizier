use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::shell::ShellConfig;
use crate::config::tools::mcp::McpClientConfig;

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
#[serde(tag = "type", content = "data")]
pub enum GlobalConfigValue {
    McpServers(HashMap<String, McpClientConfig>),
    Shell(ShellConfig),
}

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
pub struct GlobalConfigEntry {
    pub key: String,
    pub value: GlobalConfigValue,
}
