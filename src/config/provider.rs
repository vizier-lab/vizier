use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[allow(non_camel_case_types)]
pub enum ProviderVariant {
    deepseek,
    openrouter,
    ollama,
    gemini,
    openai,
    anthropic,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<AnthropicProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai: Option<OpenAIProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gemini: Option<GeminiProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deepseek: Option<DeepseekProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openrouter: Option<OpenRouterProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama: Option<OllamaProviderConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnthropicProviderConfig {
    pub api_key: String,
}

impl Default for AnthropicProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${ANTROPHIC_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAIProviderConfig {
    pub base_url: Option<String>,
    pub api_key: String,
}

impl Default for OpenAIProviderConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            api_key: "${OPENAI_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiProviderConfig {
    pub api_key: String,
}

impl Default for GeminiProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${GEMINI_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OllamaProviderConfig {
    pub base_url: String,
}

impl Default for OllamaProviderConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeepseekProviderConfig {
    pub api_key: String,
}

impl Default for DeepseekProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${DEEPSEEK_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenRouterProviderConfig {
    pub api_key: String,
}

impl Default for OpenRouterProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${OPENROUTER_API_KEY}".into(),
        }
    }
}
