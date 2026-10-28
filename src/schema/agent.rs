use std::collections::HashMap;

use duration_string::DurationString;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::ChunkLimits;
use crate::config::provider::ProviderId;
use crate::config::shell::ShellConfig;

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
    pub provider: ProviderId,
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
    pub dream_provider: Option<ProviderId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dream_model: Option<String>,
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
    /// How a failed completion call is retried, and how long one attempt may take (#41).
    #[serde(default)]
    pub completion_retry: CompletionRetryConfig,
}

fn default_checkpoint_threshold() -> f64 {
    0.8
}

/// Retries for transient provider failures (rate limits, 5xx, dropped connections). Never
/// applied to errors that would fail the same way again, such as a bad key or a bad request.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompletionRetryConfig {
    /// Attempts in all, the first included. 1 disables retrying.
    #[serde(default = "default_retry_max_attempts")]
    pub max_attempts: u32,
    /// Wait before the first retry; doubled for each later one, with jitter.
    #[serde(default = "default_retry_base_delay")]
    pub base_delay: DurationString,
    #[serde(default = "default_retry_max_delay")]
    pub max_delay: DurationString,
    /// How long one attempt may take before it is abandoned and retried. A turn's
    /// `prompt_timeout` still bounds the attempts together.
    #[serde(default = "default_retry_attempt_timeout")]
    pub attempt_timeout: DurationString,
}

impl Default for CompletionRetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_retry_max_attempts(),
            base_delay: default_retry_base_delay(),
            max_delay: default_retry_max_delay(),
            attempt_timeout: default_retry_attempt_timeout(),
        }
    }
}

fn default_retry_max_attempts() -> u32 {
    4
}

fn default_retry_base_delay() -> DurationString {
    std::time::Duration::from_secs(1).into()
}

fn default_retry_max_delay() -> DurationString {
    std::time::Duration::from_secs(30).into()
}

fn default_retry_attempt_timeout() -> DurationString {
    std::time::Duration::from_secs(300).into()
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
    pub fetch: ToolConfig<()>,
    #[serde(default)]
    pub http_client: ToolConfig<()>,
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
    /// Whose saved credentials this feature uses (data-model §8).
    pub fn credentials(&self) -> Option<ProviderId> {
        Some(match self {
            Self::Local => return None,
            Self::Openrouter => ProviderId::OPENROUTER,
            Self::Ollama => ProviderId::OLLAMA,
            Self::Openai => ProviderId::OPENAI,
            Self::Gemini => ProviderId::GOOGLE,
            Self::Voyageai => ProviderId::VOYAGEAI,
            Self::Mistral => ProviderId::MISTRAL,
            Self::Together => ProviderId::TOGETHERAI,
            Self::Cohere => ProviderId::COHERE,
            Self::Copilot => ProviderId::GITHUB_COPILOT,
        })
    }

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TtsProvider {
    Openai,
    Openrouter,
    Elevenlabs,
    Xai,
    Hyperbolic,
    #[default]
    Kokoro,
}

impl TtsProvider {
    /// Whose saved credentials this feature uses (data-model §8).
    pub fn credentials(&self) -> Option<ProviderId> {
        Some(match self {
            Self::Openai => ProviderId::OPENAI,
            Self::Openrouter => ProviderId::OPENROUTER,
            Self::Elevenlabs => ProviderId::ELEVENLABS,
            Self::Xai => ProviderId::XAI,
            Self::Hyperbolic => ProviderId::HYPERBOLIC,
            Self::Kokoro => return None,
        })
    }

    pub fn default_voice(&self) -> &str {
        match self {
            Self::Openai => "alloy",
            // `alloy` is an OpenAI voice; Kokoro publishes `af_alloy` (research R2).
            Self::Openrouter => "af_alloy",
            Self::Elevenlabs => "pqHfZKP75CvOlQylNhV4",
            Self::Xai | Self::Hyperbolic => "default",
            Self::Kokoro => "af",
        }
    }

    /// The model used when the setting leaves it empty (spec 028, research R2).
    pub fn default_model(&self) -> &str {
        match self {
            Self::Openai => "tts-1",
            Self::Openrouter => "hexgrad/kokoro-82m",
            Self::Elevenlabs => "eleven_multilingual_v2",
            Self::Xai => "grok-2-tts",
            Self::Hyperbolic => "Melo-TTS",
            Self::Kokoro => "Kokoro-82M",
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
    Openai,
    Openrouter,
    Elevenlabs,
    Groq,
    Mistral,
    Huggingface,
    Gemini,
    #[default]
    Whisper,
}

impl SttProvider {
    /// Whose saved credentials this feature uses (data-model §8).
    pub fn credentials(&self) -> Option<ProviderId> {
        Some(match self {
            Self::Openai => ProviderId::OPENAI,
            Self::Openrouter => ProviderId::OPENROUTER,
            Self::Elevenlabs => ProviderId::ELEVENLABS,
            Self::Groq => ProviderId::GROQ,
            Self::Mistral => ProviderId::MISTRAL,
            Self::Huggingface => ProviderId::HUGGINGFACE,
            Self::Gemini => ProviderId::GOOGLE,
            Self::Whisper => return None,
        })
    }

    pub fn default_model(&self) -> &str {
        match self {
            Self::Openai => "whisper-1",
            Self::Openrouter => "openai/whisper-large-v3",
            Self::Elevenlabs => "scribe_v1",
            Self::Groq => "whisper-large-v3",
            Self::Mistral => "voxtral-mini-2507",
            Self::Huggingface => "openai/whisper-large-v3",
            Self::Gemini => "gemini-1.5-flash",
            Self::Whisper => "ggml-small",
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
    pub provider: Option<ProviderId>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenProvider {
    #[default]
    Openai,
    Openrouter,
    Xai,
    Huggingface,
    Hyperbolic,
}

impl ImageGenProvider {
    /// Whose saved credentials this feature uses (data-model §8).
    pub fn credentials(&self) -> Option<ProviderId> {
        Some(match self {
            Self::Openai => ProviderId::OPENAI,
            Self::Openrouter => ProviderId::OPENROUTER,
            Self::Xai => ProviderId::XAI,
            Self::Huggingface => ProviderId::HUGGINGFACE,
            Self::Hyperbolic => ProviderId::HYPERBOLIC,
        })
    }

    pub fn default_model(&self) -> &str {
        match self {
            Self::Openai => "dall-e-3",
            Self::Openrouter => "bytedance-seed/seedream-4.5",
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
mod removed_python_config_tests {
    use super::*;

    /// Integration settings moved off the agent (spec 016, research R7): a row saved before
    /// still loads, and the moved keys are dropped when it is next saved.
    #[test]
    fn records_saved_with_the_moved_integration_fields_still_load() {
        let config: AgentConfig = serde_json::from_value(serde_json::json!({
            "name": "a", "system_prompt": null, "description": null, "provider": "dummyplug",
            "model": "m", "thinking_depth": 1, "silent_read_initiative_chance": 0.0,
            "prompt_timeout": "1m", "heartbeat_interval": "1m",
            "discord_token": "x", "telegram_token": "y",
            "tools": {
                "timeout": "30s",
                "discord": {"enabled": true, "settings": null},
                "brave_search": {"enabled": true, "settings": {"api_key": "k"}},
                "mcp_servers": {"fs": {"host": "http", "uri": "http://x"}}
            }
        }))
        .unwrap();
        let saved = serde_json::to_string(&config).unwrap();
        for key in ["discord_token", "telegram_token", "brave_search", "mcp_servers", "\"discord\""] {
            assert!(!saved.contains(key), "{key} survived: {saved}");
        }
    }

    #[test]
    fn records_saved_with_the_removed_python_switches_still_load() {
        let tools: Result<AgentToolsConfig, _> = serde_json::from_str(
            r#"{"timeout": "30s", "python": {"enabled": false, "code_mode": false}}"#,
        );
        assert!(tools.is_ok(), "{tools:?}");
    }
}

#[cfg(test)]
mod media_provider_tests {
    use super::*;

    /// Spec 028 (data-model §7): the OpenRouter media variants and the defaults every empty
    /// override falls back to. The catalogue (`catalogue/media.rs`) delegates to these values.
    #[test]
    fn openrouter_is_selectable_for_every_media_capability() {
        assert_eq!(SttProvider::Openrouter.credentials(), Some(ProviderId::OPENROUTER));
        assert_eq!(ImageGenProvider::Openrouter.credentials(), Some(ProviderId::OPENROUTER));
        assert_eq!(TtsProvider::Openrouter.credentials(), Some(ProviderId::OPENROUTER));

        assert_eq!(SttProvider::Openrouter.default_model(), "openai/whisper-large-v3");
        assert_eq!(ImageGenProvider::Openrouter.default_model(), "bytedance-seed/seedream-4.5");
        assert_eq!(TtsProvider::Openrouter.default_model(), "hexgrad/kokoro-82m");
    }

    #[test]
    fn the_tts_default_model_keeps_every_provider() {
        let models: Vec<_> = [
            TtsProvider::Openai,
            TtsProvider::Openrouter,
            TtsProvider::Elevenlabs,
            TtsProvider::Xai,
            TtsProvider::Hyperbolic,
            TtsProvider::Kokoro,
        ]
        .into_iter()
        .map(|p| p.default_model().to_string())
        .collect();
        assert_eq!(
            models,
            vec![
                "tts-1",
                "hexgrad/kokoro-82m",
                "eleven_multilingual_v2",
                "grok-2-tts",
                "Melo-TTS",
                "Kokoro-82M",
            ]
        );
    }

    #[test]
    fn local_media_providers_use_the_whisper_rs_and_any_tts_artifacts() {
        assert_eq!(SttProvider::Whisper.default_model(), "ggml-small");
        assert_eq!(TtsProvider::Kokoro.default_model(), "Kokoro-82M");
        // A voice id saved for sherpa keeps its meaning at the provider layer.
        assert_eq!(TtsProvider::Kokoro.default_voice(), "af");
    }

    /// A saved agent keeps loading: the new variants are additive, and the old values mean
    /// what they did.
    #[test]
    fn media_settings_round_trip_with_openrouter() {
        let stt: SttToolSettings =
            serde_json::from_str(r#"{"provider": "openrouter", "model": null, "language": "en"}"#).unwrap();
        assert_eq!(stt.provider, SttProvider::Openrouter);
        let image: ImageGenToolSettings =
            serde_json::from_str(r#"{"provider": "openrouter", "model": "x", "size": "2K"}"#).unwrap();
        assert_eq!(image.provider, ImageGenProvider::Openrouter);
        let json = serde_json::to_string(&image).unwrap();
        assert!(json.contains(r#""provider":"openrouter""#), "{json}");
        assert_eq!(serde_json::from_str::<SttProvider>(r#""openai""#).unwrap(), SttProvider::Openai);
    }
}
