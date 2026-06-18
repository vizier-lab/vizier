use serde::{Deserialize, Serialize};

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
