use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "host", rename_all = "snake_case")]
pub enum McpClientConfig {
    Local {
        command: String,
        args: Vec<String>,
        env: Option<HashMap<String, String>>,
    },
    Http {
        uri: String,
    },
}
