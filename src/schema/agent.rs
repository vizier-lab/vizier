use std::collections::HashMap;

use duration_string::DurationString;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderVariant;
use crate::config::shell::ShellConfig;
use crate::config::tools::mcp::McpClientConfig;
use crate::schema::provider::Quantization;


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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<Quantization>,
    pub session_memory: MemoryConfig,
    pub thinking_depth: usize,
    pub tools: AgentToolsConfig,
    pub silent_read_initiative_chance: f32,
    pub show_thinking: Option<bool>,
    pub show_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
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
    pub embedding: Option<EmbeddingToolSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexer: Option<IndexerConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryConfig {
    pub max_capacity: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AgentToolsConfig {
    pub timeout: DurationString,
    #[serde(default)]
    pub programmatic_sandbox: bool,
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
pub struct EmbeddingToolSettings {
    pub provider: EmbeddingProvider,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default, utoipa::ToSchema, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProvider {
    #[default]
    Local,
    Openrouter,
    Ollama,
    Openai,
    Gemini,
}

impl EmbeddingProvider {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Openrouter => "openrouter",
            Self::Ollama => "ollama",
            Self::Openai => "openai",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, utoipa::ToSchema, JsonSchema)]
pub struct IndexerConfig {
    #[serde(default)]
    pub kind: IndexerKind,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default, utoipa::ToSchema, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IndexerKind {
    #[default]
    Surreal,
}

impl IndexerKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Surreal => "surreal",
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
    Piper,
    Kitten,
    Openai,
    Openrouter,
    Elevenlabs,
}

impl TtsProvider {
    pub fn default_voice(&self) -> &str {
        match self {
            Self::Piper | Self::Kitten => "0",
            Self::Openai | Self::Openrouter => "alloy",
            Self::Elevenlabs => "pqHfZKP75CvOlQylNhV4",
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
    SenseVoice,
    Openai,
    Elevenlabs,
}

impl SttProvider {
    pub fn default_model(&self) -> &str {
        match self {
            Self::SenseVoice => "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
            Self::Openai => "whisper-1",
            Self::Elevenlabs => "scribe_v1",
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
}

impl ImageGenProvider {
    pub fn default_model(&self) -> &str {
        match self {
            Self::Openai => "dall-e-3",
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
