use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// A provider's id: a catalogue provider (`anthropic`, `google`, …), a built-in (`ollama`,
/// `dummyplug`, …) or an operator-defined custom provider. Replaces the `ProviderVariant` enum
/// (data-model §1).
///
/// Format `^[a-z0-9][a-z0-9._-]{0,63}$`. Serialised as a plain string; deserialising maps the
/// legacy enum names (`gemini` → `google`, `opencode_zen` → `opencode`, …) through
/// [`ProviderId::from_legacy`], so old agent JSON, provider rows and API clients keep working.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderId(Cow<'static, str>);

impl ProviderId {
    pub const OPENAI: ProviderId = ProviderId::new_static("openai");
    pub const ANTHROPIC: ProviderId = ProviderId::new_static("anthropic");
    pub const GOOGLE: ProviderId = ProviderId::new_static("google");
    pub const OPENROUTER: ProviderId = ProviderId::new_static("openrouter");
    pub const DEEPSEEK: ProviderId = ProviderId::new_static("deepseek");
    pub const GROQ: ProviderId = ProviderId::new_static("groq");
    pub const MISTRAL: ProviderId = ProviderId::new_static("mistral");
    pub const XAI: ProviderId = ProviderId::new_static("xai");
    pub const TOGETHERAI: ProviderId = ProviderId::new_static("togetherai");
    pub const COHERE: ProviderId = ProviderId::new_static("cohere");
    pub const HUGGINGFACE: ProviderId = ProviderId::new_static("huggingface");
    pub const GITHUB_COPILOT: ProviderId = ProviderId::new_static("github-copilot");
    pub const AZURE: ProviderId = ProviderId::new_static("azure");
    pub const OPENCODE_GO: ProviderId = ProviderId::new_static("opencode-go");
    pub const OLLAMA: ProviderId = ProviderId::new_static("ollama");
    pub const LLAMA_CPP: ProviderId = ProviderId::new_static("llama-cpp");
    pub const CHATGPT: ProviderId = ProviderId::new_static("chatgpt");
    pub const DUMMYPLUG: ProviderId = ProviderId::new_static("dummyplug");
    pub const ELEVENLABS: ProviderId = ProviderId::new_static("elevenlabs");
    pub const VOYAGEAI: ProviderId = ProviderId::new_static("voyageai");
    pub const HYPERBOLIC: ProviderId = ProviderId::new_static("hyperbolic");
    pub const CUSTOM: ProviderId = ProviderId::new_static("custom");

    /// For ids known to be valid at compile time.
    pub const fn new_static(id: &'static str) -> Self {
        ProviderId(Cow::Borrowed(id))
    }

    /// A validated id. Legacy names are *not* mapped here; use [`ProviderId::parse`] for input.
    pub fn new(id: impl Into<String>) -> crate::Result<Self> {
        let id = id.into();
        if Self::is_valid(&id) {
            Ok(ProviderId(Cow::Owned(id)))
        } else {
            Err(crate::VizierError(format!(
                "invalid provider id {id:?}: use 1-64 lowercase letters, digits, '.', '_' or \
                 '-', starting with a letter or digit"
            )))
        }
    }

    /// Input from a person, a stored row or an old client: legacy names are mapped, then the
    /// result is validated.
    pub fn parse(id: &str) -> crate::Result<Self> {
        match Self::from_legacy(id) {
            Some(mapped) => Ok(mapped),
            None => Self::new(id),
        }
    }

    pub fn is_valid(id: &str) -> bool {
        let bytes = id.as_bytes();
        !bytes.is_empty()
            && bytes.len() <= 64
            && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
            && bytes.iter().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            })
    }

    /// The legacy `ProviderVariant` name → new id map (contract `provider-id-migration.md`).
    /// Names that are unchanged, and `galadriel`/`mira` (kept as given, then refused at build),
    /// return `None`.
    pub fn from_legacy(name: &str) -> Option<Self> {
        let mapped = match name {
            "gemini" => "google",
            "mimo" => "xiaomi",
            "moonshot" => "moonshotai",
            "together" => "togetherai",
            "copilot" => "github-copilot",
            "opencode_zen" => "opencode",
            "opencode_go" => "opencode-go",
            "llama_cpp" => "llama-cpp",
            _ => return None,
        };
        Some(ProviderId::new_static(mapped))
    }

    /// The legacy name this id had, for the ids that were renamed.
    pub fn legacy_name(&self) -> Option<&'static str> {
        Some(match self.as_str() {
            "google" => "gemini",
            "xiaomi" => "mimo",
            "moonshotai" => "moonshot",
            "togetherai" => "together",
            "github-copilot" => "copilot",
            "opencode" => "opencode_zen",
            "opencode-go" => "opencode_go",
            "llama-cpp" => "llama_cpp",
            _ => return None,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ProviderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for ProviderId {
    type Err = crate::VizierError;
    fn from_str(s: &str) -> crate::Result<Self> {
        Self::parse(s)
    }
}

impl Serialize for ProviderId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ProviderId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        ProviderId::parse(&raw).map_err(|e| serde::de::Error::custom(e.0))
    }
}

impl utoipa::PartialSchema for ProviderId {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::String)
            .description(Some(
                "A provider id: a catalogue provider, a built-in or a custom provider",
            ))
            .into()
    }
}

impl utoipa::ToSchema for ProviderId {}

impl schemars::JsonSchema for ProviderId {
    fn schema_name() -> Cow<'static, str> {
        "ProviderId".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(generator)
    }
}

#[cfg(test)]
mod provider_id_tests {
    use super::*;

    #[test]
    fn every_legacy_name_maps_as_the_contract_says() {
        let table = [
            ("openai", "openai"),
            ("anthropic", "anthropic"),
            ("openrouter", "openrouter"),
            ("deepseek", "deepseek"),
            ("gemini", "google"),
            ("mimo", "xiaomi"),
            ("groq", "groq"),
            ("mistral", "mistral"),
            ("xai", "xai"),
            ("perplexity", "perplexity"),
            ("moonshot", "moonshotai"),
            ("zai", "zai"),
            ("minimax", "minimax"),
            ("together", "togetherai"),
            ("cohere", "cohere"),
            ("huggingface", "huggingface"),
            ("copilot", "github-copilot"),
            ("azure", "azure"),
            ("opencode_zen", "opencode"),
            ("opencode_go", "opencode-go"),
            ("ollama", "ollama"),
            ("llama_cpp", "llama-cpp"),
            ("chatgpt", "chatgpt"),
            ("dummyplug", "dummyplug"),
            ("elevenlabs", "elevenlabs"),
            ("voyageai", "voyageai"),
            ("hyperbolic", "hyperbolic"),
            ("custom", "custom"),
            // Removed providers are kept as given, then refused when an agent is built.
            ("galadriel", "galadriel"),
            ("mira", "mira"),
        ];
        for (legacy, new) in table {
            let parsed: ProviderId = serde_json::from_value(serde_json::json!(legacy)).unwrap();
            assert_eq!(parsed.as_str(), new, "{legacy}");
            if legacy != new {
                assert_eq!(parsed.legacy_name(), Some(legacy));
            }
        }
    }

    #[test]
    fn serialises_as_a_plain_string() {
        assert_eq!(
            serde_json::to_value(ProviderId::GOOGLE).unwrap(),
            serde_json::json!("google")
        );
    }

    #[test]
    fn refuses_invalid_ids() {
        for bad in ["", "Upper", "-lead", "has space", "a/b", &"x".repeat(65)] {
            assert!(ProviderId::parse(bad).is_err(), "{bad:?}");
            assert!(serde_json::from_value::<ProviderId>(serde_json::json!(bad)).is_err());
        }
        for good in ["a", "302ai", "moonshotai-cn", "llama-cpp", "x.y_z", &"x".repeat(64)] {
            assert!(ProviderId::parse(good).is_ok(), "{good:?}");
        }
    }
}


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, utoipa::ToSchema)]
#[allow(non_camel_case_types)]
pub enum ProviderVariant {
    deepseek,
    openrouter,
    ollama,
    gemini,
    openai,
    anthropic,
    mimo,
    llama_cpp,
    elevenlabs,
    groq,
    mistral,
    xai,
    perplexity,
    moonshot,
    zai,
    minimax,
    together,
    cohere,
    huggingface,
    hyperbolic,
    voyageai,
    galadriel,
    mira,
    chatgpt,
    copilot,
    azure,
    custom,
    opencode_zen,
    opencode_go,
    dummyplug,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mimo: Option<MimoProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llama_cpp: Option<LlamaCppProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevenlabs: Option<ElevenLabsProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groq: Option<GroqProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mistral: Option<MistralProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xai: Option<XaiProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perplexity: Option<PerplexityProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moonshot: Option<MoonshotProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zai: Option<ZaiProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimax: Option<MinimaxProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub together: Option<TogetherProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohere: Option<CohereProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub huggingface: Option<HuggingfaceProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperbolic: Option<HyperbolicProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voyageai: Option<VoyageaiProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub galadriel: Option<GaladrielProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mira: Option<MiraProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chatgpt: Option<ChatGptProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copilot: Option<CopilotProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure: Option<AzureProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<CustomProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opencode_zen: Option<OpencodeZenProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opencode_go: Option<OpencodeGoProviderConfig>,
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
    pub api_key: String,
}

impl Default for OpenAIProviderConfig {
    fn default() -> Self {
        Self {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MimoProviderConfig {
    pub api_key: String,
}

impl Default for MimoProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${XIAOMI_MIMO_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlamaCppProviderConfig {
    pub base_url: String,
}

impl Default for LlamaCppProviderConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ElevenLabsProviderConfig {
    pub api_key: String,
}

impl Default for ElevenLabsProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${ELEVENLABS_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GroqProviderConfig {
    pub api_key: String,
}

impl Default for GroqProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${GROQ_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MistralProviderConfig {
    pub api_key: String,
}

impl Default for MistralProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${MISTRAL_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct XaiProviderConfig {
    pub api_key: String,
}

impl Default for XaiProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${XAI_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PerplexityProviderConfig {
    pub api_key: String,
}

impl Default for PerplexityProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${PERPLEXITY_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoonshotProviderConfig {
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for MoonshotProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${MOONSHOT_API_KEY}".into(),
            base_url: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZaiProviderConfig {
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for ZaiProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${ZAI_API_KEY}".into(),
            base_url: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MinimaxProviderConfig {
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for MinimaxProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${MINIMAX_API_KEY}".into(),
            base_url: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TogetherProviderConfig {
    pub api_key: String,
}

impl Default for TogetherProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${TOGETHER_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CohereProviderConfig {
    pub api_key: String,
}

impl Default for CohereProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${COHERE_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HuggingfaceProviderConfig {
    pub api_key: String,
}

impl Default for HuggingfaceProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${HUGGINGFACE_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HyperbolicProviderConfig {
    pub api_key: String,
}

impl Default for HyperbolicProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${HYPERBOLIC_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VoyageaiProviderConfig {
    pub api_key: String,
}

impl Default for VoyageaiProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${VOYAGE_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GaladrielProviderConfig {
    pub api_key: String,
}

impl Default for GaladrielProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${GALADRIEL_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiraProviderConfig {
    pub api_key: String,
}

impl Default for MiraProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${MIRA_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatGptProviderConfig {
    pub access_token: String,
    pub account_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for ChatGptProviderConfig {
    fn default() -> Self {
        Self {
            access_token: "${CHATGPT_ACCESS_TOKEN}".into(),
            account_id: "${CHATGPT_ACCOUNT_ID}".into(),
            base_url: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CopilotProviderConfig {
    pub api_key: String,
}

impl Default for CopilotProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${COPILOT_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AzureProviderConfig {
    pub endpoint: String,
    pub api_key: String,
}

impl Default for AzureProviderConfig {
    fn default() -> Self {
        Self {
            endpoint: "${AZURE_ENDPOINT}".into(),
            api_key: "${AZURE_API_KEY}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomProviderConfig {
    pub api_key: String,
    pub base_url: String,
}

impl Default for CustomProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${CUSTOM_API_KEY}".into(),
            base_url: "${CUSTOM_BASE_URL}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpencodeZenProviderConfig {
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for OpencodeZenProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${OPENCODE_ZEN_API_KEY}".into(),
            base_url: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpencodeGoProviderConfig {
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for OpencodeGoProviderConfig {
    fn default() -> Self {
        Self {
            api_key: "${OPENCODE_GO_API_KEY}".into(),
            base_url: None,
        }
    }
}
