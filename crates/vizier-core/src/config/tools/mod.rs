use serde::{Deserialize, Serialize};

pub mod mcp;

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
