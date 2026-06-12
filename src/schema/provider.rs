use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderVariant;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, utoipa::ToSchema)]
pub enum Quantization {
    #[serde(rename = "auto_4")]
    Auto4,
    #[serde(rename = "auto_8")]
    Auto8,
    #[serde(rename = "q4_0")]
    Q4_0,
    #[serde(rename = "q4_1")]
    Q4_1,
    #[serde(rename = "q4k")]
    Q4K,
    #[serde(rename = "q5_0")]
    Q5_0,
    #[serde(rename = "q5_1")]
    Q5_1,
    #[serde(rename = "q5k")]
    Q5K,
    #[serde(rename = "q6k")]
    Q6K,
    #[serde(rename = "q8_0")]
    Q8_0,
    #[serde(rename = "q8_1")]
    Q8_1,
    #[serde(rename = "hqq4")]
    Hqq4,
    #[serde(rename = "hqq8")]
    Hqq8,
    #[serde(rename = "fp8")]
    Fp8,
}

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
    Mistralrs {
        enabled: bool,
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
}
