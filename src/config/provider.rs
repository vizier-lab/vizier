use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(non_camel_case_types)]
pub enum ProviderVariant {
    deepseek,
    openrouter,
    ollama,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderConfig {
    pub ollama: Option<OllamaProviderConfig>,
    pub deepseek: Option<DeepseekProviderConfig>,
    pub openrouter: Option<OpenRouterProviderConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OllamaProviderConfig {
    pub base_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DeepseekProviderConfig {
    pub api_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OpenRouterProviderConfig {
    pub api_key: String,
}
