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
    },
    Anthropic {
        api_key: String,
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
    Elevenlabs {
        api_key: String,
    },
    Groq {
        api_key: String,
    },
    Mistral {
        api_key: String,
    },
    Xai {
        api_key: String,
    },
    Perplexity {
        api_key: String,
    },
    Moonshot {
        api_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    Zai {
        api_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    Minimax {
        api_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    Together {
        api_key: String,
    },
    Cohere {
        api_key: String,
    },
    Huggingface {
        api_key: String,
    },
    Hyperbolic {
        api_key: String,
    },
    Voyageai {
        api_key: String,
    },
    Galadriel {
        api_key: String,
    },
    Mira {
        api_key: String,
    },
    Chatgpt {
        access_token: String,
        account_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    Copilot {
        api_key: String,
    },
    Azure {
        endpoint: String,
        api_key: String,
    },
    Custom {
        api_key: String,
        base_url: String,
    },
}
