use std::collections::HashMap;

use duration_string::DurationString;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::ChunkLimits;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core: Option<String>,
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
    /// Passage sizing bounds used when this agent's memories are chunked (FR-041).
    #[serde(default)]
    pub chunking: ChunkLimits,
    /// Budget for the related-memory block injected into each turn (FR-022, FR-023, FR-030).
    #[serde(default)]
    pub auto_context: AutoContextConfig,
}

fn default_checkpoint_threshold() -> f64 {
    0.8
}

/// What the per-turn related-memory lookup is allowed to spend, budgeted separately per request
/// kind. `SilentRead` fires for every non-mention message in a Discord guild channel and every
/// Telegram group message, so its cost scales with channel traffic rather than with conversation
/// volume — it defaults to zero and `Chat` does not (FR-030, research Decision 9).
#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema, JsonSchema)]
pub struct AutoContextConfig {
    /// Passages injected on the `Chat` / `AudioChat` path.
    #[serde(default = "default_auto_context_chat_passages")]
    pub chat_passages: usize,
    /// Passages injected on the `SilentRead` path. Zero disables retrieval there entirely.
    #[serde(default)]
    pub silent_read_passages: usize,
    /// Relevance floor for automatic context, independent of and stricter than the search
    /// threshold (FR-023).
    #[serde(default = "default_auto_context_threshold")]
    pub threshold: f64,
    /// Total byte budget for the assembled block, spent in rank order (FR-022, FR-028).
    #[serde(default = "default_auto_context_size_cap")]
    pub size_cap: usize,
    /// How many passages one document may contribute to a single turn (FR-024).
    #[serde(default = "default_auto_context_per_document")]
    pub per_document: usize,
}

fn default_auto_context_chat_passages() -> usize {
    5
}

/// **0.45**, revised down from a provisional 0.6 after measuring a real index (quickstart Step 3).
///
/// Against fastembed `all-MiniLM-L6-v2`, genuine topical matches scored 0.30–0.49 and unrelated
/// queries 0.04–0.08. **Nothing in that corpus reached 0.6 at all**, so 0.6 would have left this
/// block empty on every turn — safe, because FR-026 omits the section entirely rather than filling
/// it, but it would have meant the per-turn half of this feature never fired in practice while
/// looking configured.
///
/// 0.45 fires only on the strong end of the observed match range, which is the intent: five
/// passages cost roughly 15x the ten titles they replaced, so the saving comes entirely from how
/// often the block is *correctly empty* (SC-010 wants that on at least 70% of real messages). It
/// stays stricter than `SEARCH_THRESHOLD` (0.20) as FR-023 requires.
///
/// This is calibrated to one embedding model on one corpus. Deriving it per deployment by replaying
/// stored session history (research Decision 10, task T051) is still the right way to settle it, and
/// `HistoryStorage` already persists what that needs.
fn default_auto_context_threshold() -> f64 {
    0.45
}

fn default_auto_context_size_cap() -> usize {
    6000
}

fn default_auto_context_per_document() -> usize {
    2
}

impl Default for AutoContextConfig {
    fn default() -> Self {
        Self {
            chat_passages: default_auto_context_chat_passages(),
            silent_read_passages: 0,
            threshold: default_auto_context_threshold(),
            size_cap: default_auto_context_size_cap(),
            per_document: default_auto_context_per_document(),
        }
    }
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
    #[serde(default)]
    pub python: PythonSandboxConfig,
}

/// The two Python switches: `enabled` gives the agent `execute_python`; `code_mode`
/// (valid only with `enabled`) makes its other tools callable from scripts only.
#[derive(Debug, Serialize, Deserialize, Clone, Default, JsonSchema, utoipa::ToSchema)]
pub struct PythonSandboxConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub code_mode: bool,
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
    Whisper,
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
            Self::Whisper => "large-v3",
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

#[cfg(test)]
mod python_sandbox_config_tests {
    use super::*;

    #[test]
    fn records_saved_before_the_python_switches_load_with_both_off() {
        let tools: AgentToolsConfig = serde_json::from_str(r#"{"timeout": "30s"}"#).unwrap();
        assert!(!tools.python.enabled);
        assert!(!tools.python.code_mode);
        let python: PythonSandboxConfig = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert!(python.enabled && !python.code_mode);
    }
}
