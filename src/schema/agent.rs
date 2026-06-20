use std::collections::HashMap;

use duration_string::DurationString;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderVariant;
use crate::config::shell::ShellConfig;
use crate::config::tools::mcp::McpClientConfig;

pub type AgentConfigs = HashMap<String, AgentConfig>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_to: Vec<String>,
    pub system_prompt: Option<String>,
    pub description: Option<String>,
    pub provider: ProviderVariant,
    pub model: String,
    pub thinking_depth: usize,
    #[serde(default = "default_checkpoint_threshold")]
    pub checkpoint_threshold: f64,
    pub tools: AgentToolsConfig,
    pub silent_read_initiative_chance: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_documents: Option<Vec<String>>,
    pub prompt_timeout: DurationString,
    #[serde(skip)]
    pub documents: Vec<String>,
    pub heartbeat_interval: DurationString,
    #[serde(default)]
    pub dream_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dream_schedule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dream_provider: Option<ProviderVariant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dream_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telegram_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<EmbeddingConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexer: Option<IndexerConfig>,
}

fn default_checkpoint_threshold() -> f64 {
    0.8
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AgentToolsConfig {
    pub timeout: DurationString,
    #[serde(default)]
    pub shell: Option<ShellConfig>,
    #[serde(default)]
    pub brave_search: ToolConfig<BraveSearchToolSettings>,
    #[serde(default)]
    pub discord: ToolConfig<()>,
    #[serde(default)]
    pub telegram: ToolConfig<()>,
    #[serde(default)]
    pub fetch: ToolConfig<()>,
    #[serde(default)]
    pub http_client: ToolConfig<()>,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpClientConfig>,
    #[serde(default)]
    pub tts: ToolConfig<TtsToolSettings>,
    #[serde(default)]
    pub stt: ToolConfig<SttToolSettings>,
    #[serde(default)]
    pub read_image: ToolConfig<ReadImageToolSettings>,
    #[serde(default)]
    pub image_gen: ToolConfig<ImageGenToolSettings>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolConfig<Settings> {
    pub enabled: bool,
    pub settings: Settings,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, utoipa::ToSchema, JsonSchema)]
pub struct EmbeddingConfig {
    pub provider: EmbeddingProvider,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default, utoipa::ToSchema, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProvider {
    #[default]
    Local,
    Openrouter,
    Ollama,
    Openai,
    Gemini,
    Voyageai,
    Mistral,
    Together,
    Cohere,
    Copilot,
}

impl EmbeddingProvider {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Openrouter => "openrouter",
            Self::Ollama => "ollama",
            Self::Openai => "openai",
            Self::Gemini => "gemini",
            Self::Voyageai => "voyageai",
            Self::Mistral => "mistral",
            Self::Together => "together",
            Self::Cohere => "cohere",
            Self::Copilot => "copilot",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, utoipa::ToSchema, JsonSchema)]
pub struct IndexerConfig {
    #[serde(default)]
    pub kind: IndexerKind,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default, utoipa::ToSchema, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IndexerKind {
    #[default]
    Sqlite,
}

impl IndexerKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, utoipa::ToSchema)]
pub struct BraveSearchToolSettings {
    pub api_key: Option<String>,
    pub safesearch: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TtsProvider {
    #[default]
    Openai,
    Openrouter,
    Elevenlabs,
    Xai,
    Hyperbolic,
    Kokoro,
}

impl TtsProvider {
    pub fn default_voice(&self) -> &str {
        match self {
            Self::Openai | Self::Openrouter => "alloy",
            Self::Elevenlabs => "pqHfZKP75CvOlQylNhV4",
            Self::Xai | Self::Hyperbolic => "default",
            Self::Kokoro => "af",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, utoipa::ToSchema)]
#[serde(default)]
pub struct TtsToolSettings {
    pub provider: TtsProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SttProvider {
    #[default]
    Openai,
    Elevenlabs,
    Groq,
    Mistral,
    Huggingface,
    Gemini,
}

impl SttProvider {
    pub fn default_model(&self) -> &str {
        match self {
            Self::Openai => "whisper-1",
            Self::Elevenlabs => "scribe_v1",
            Self::Groq => "whisper-large-v3",
            Self::Mistral => "voxtral-mini-2507",
            Self::Huggingface => "openai/whisper-large-v3",
            Self::Gemini => "gemini-1.5-flash",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, utoipa::ToSchema)]
#[serde(default)]
pub struct SttToolSettings {
    pub provider: SttProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, utoipa::ToSchema)]
#[serde(default)]
pub struct ReadImageToolSettings {
    pub provider: Option<ProviderVariant>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenProvider {
    #[default]
    Openai,
    Xai,
    Huggingface,
    Hyperbolic,
}

impl ImageGenProvider {
    pub fn default_model(&self) -> &str {
        match self {
            Self::Openai => "dall-e-3",
            Self::Xai => "grok-2-image-1212",
            Self::Huggingface => "stabilityai/stable-diffusion-xl-base-1.0",
            Self::Hyperbolic => "SDXL1.0-base",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, utoipa::ToSchema)]
#[serde(default)]
pub struct ImageGenToolSettings {
    pub provider: ImageGenProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}
