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
    Mistralrs {
        enabled: bool,
    },
}
