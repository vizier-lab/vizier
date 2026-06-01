use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderVariant;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderEntry {
    pub variant: ProviderVariant,
    pub config: ProviderEntryConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ProviderEntryConfig {
    Ollama {
        base_url: String,
    },
    Openai {
        api_key: String,
        base_url: Option<String>,
    },
    Anthropic {
        api_key: String,
        base_url: Option<String>,
    },
    Deepseek {
        api_key: String,
    },
    Openrouter {
        api_key: String,
    },
    Gemini {
        api_key: String,
    },
    Mimo {
        api_key: String,
    },
    LlamaCpp {
        base_url: String,
    },
}
