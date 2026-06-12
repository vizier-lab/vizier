use std::sync::Arc;

use crate::{
    VizierError,
    config::provider::ProviderVariant,
    schema::ProviderEntryConfig,
    storage::{VizierStorage, provider::ProviderStorage},
};

/// Result of resolving a provider's credentials.
///
/// - Remote providers populate `api_key`; `base_url` is always `None`.
/// - Local providers populate `base_url`; `api_key` is empty.
/// - Azure populates both `endpoint` (via `base_url`) and `api_key`.
/// - ChatGPT populates `api_key` (the access token) and `base_url` (the account id).
pub struct ResolvedProvider {
    pub api_key: String,
    pub base_url: Option<String>,
}

/// Resolve a remote (API-key) provider by looking up, in order:
/// 1. WebUI-managed storage entry (`storage.get_provider(&variant)`)
/// 2. Environment variable named `env_var_name`
///
/// Covers all remote variants: openai, anthropic, openrouter, deepseek,
/// gemini, mimo, elevenlabs, groq, mistral, xai, perplexity, together,
/// cohere, huggingface, hyperbolic, voyageai, galadriel, mira, copilot.
///
/// `base_url` is intentionally not overridable for these variants — proxies
/// are not supported.
pub async fn resolve_provider_key(
    storage: &Arc<VizierStorage>,
    variant: ProviderVariant,
    env_var_name: &'static str,
) -> Result<ResolvedProvider, VizierError> {
    if let Ok(Some(entry)) = storage.get_provider(&variant).await {
        let api_key = match entry.config {
            ProviderEntryConfig::Openai { api_key } => Some(api_key),
            ProviderEntryConfig::Anthropic { api_key } => Some(api_key),
            ProviderEntryConfig::Openrouter { api_key } => Some(api_key),
            ProviderEntryConfig::Deepseek { api_key } => Some(api_key),
            ProviderEntryConfig::Gemini { api_key } => Some(api_key),
            ProviderEntryConfig::Mimo { api_key } => Some(api_key),
            ProviderEntryConfig::Elevenlabs { api_key } => Some(api_key),
            ProviderEntryConfig::Groq { api_key } => Some(api_key),
            ProviderEntryConfig::Mistral { api_key } => Some(api_key),
            ProviderEntryConfig::Xai { api_key } => Some(api_key),
            ProviderEntryConfig::Perplexity { api_key } => Some(api_key),
            ProviderEntryConfig::Together { api_key } => Some(api_key),
            ProviderEntryConfig::Cohere { api_key } => Some(api_key),
            ProviderEntryConfig::Huggingface { api_key } => Some(api_key),
            ProviderEntryConfig::Hyperbolic { api_key } => Some(api_key),
            ProviderEntryConfig::Voyageai { api_key } => Some(api_key),
            ProviderEntryConfig::Galadriel { api_key } => Some(api_key),
            ProviderEntryConfig::Mira { api_key } => Some(api_key),
            ProviderEntryConfig::Copilot { api_key } => Some(api_key),
            _ => None,
        };
        if let Some(api_key) = api_key
            && !api_key.is_empty()
        {
            return Ok(ResolvedProvider {
                api_key,
                base_url: None,
            });
        }
    }

    if let Ok(key) = std::env::var(env_var_name)
        && !key.is_empty()
    {
        return Ok(ResolvedProvider {
            api_key: key,
            base_url: None,
        });
    }

    Err(VizierError(format!(
        "no API key found for provider {:?}. Configure it via the WebUI \
         (PUT /api/v1/providers/{:?}) or set the {:?} environment variable.",
        variant, variant, env_var_name
    )))
}

/// Resolve a provider that supports both API key and optional base URL override.
///
/// Covers moonshot, zai, minimax (and any future openai-compat provider that
/// exposes a `base_url` builder). Storage entry takes precedence; env var is
/// the fallback. If neither has a base URL, `None` is returned (the provider
/// client will use its hardcoded default).
pub async fn resolve_provider_with_base_url(
    storage: &Arc<VizierStorage>,
    variant: ProviderVariant,
    env_var_name: &'static str,
) -> Result<ResolvedProvider, VizierError> {
    if let Ok(Some(entry)) = storage.get_provider(&variant).await {
        let (api_key, base_url) = match entry.config {
            ProviderEntryConfig::Moonshot { api_key, base_url } => (api_key, base_url),
            ProviderEntryConfig::Zai { api_key, base_url } => (api_key, base_url),
            ProviderEntryConfig::Minimax { api_key, base_url } => (api_key, base_url),
            _ => (String::new(), None),
        };
        if !api_key.is_empty() {
            return Ok(ResolvedProvider {
                api_key,
                base_url,
            });
        }
    }

    if let Ok(key) = std::env::var(env_var_name)
        && !key.is_empty()
    {
        let base_url = std::env::var(format!("{}_API_BASE", env_var_name_prefix(env_var_name)))
            .ok()
            .filter(|s| !s.is_empty());
        return Ok(ResolvedProvider {
            api_key: key,
            base_url,
        });
    }

    Err(VizierError(format!(
        "no API key found for provider {:?}. Configure it via the WebUI \
         (PUT /api/v1/providers/{:?}) or set the {:?} environment variable.",
        variant, variant, env_var_name
    )))
}

fn env_var_name_prefix(env_var_name: &str) -> String {
    env_var_name
        .strip_suffix("_API_KEY")
        .unwrap_or(env_var_name)
        .to_string()
}

/// Resolve Azure OpenAI credentials: endpoint + api_key.
///
/// Looks up the WebUI-managed storage entry first, then falls back to
/// `AZURE_ENDPOINT` and `AZURE_API_KEY` environment variables.
pub async fn resolve_azure_provider(
    storage: &Arc<VizierStorage>,
) -> Result<ResolvedProvider, VizierError> {
    if let Ok(Some(entry)) = storage.get_provider(&ProviderVariant::azure).await {
        if let ProviderEntryConfig::Azure { endpoint, api_key } = entry.config
            && !api_key.is_empty()
        {
            return Ok(ResolvedProvider {
                api_key,
                base_url: Some(endpoint),
            });
        }
    }

    let endpoint = std::env::var("AZURE_ENDPOINT").ok();
    if let Ok(api_key) = std::env::var("AZURE_API_KEY")
        && !api_key.is_empty()
    {
        return Ok(ResolvedProvider {
            api_key,
            base_url: endpoint.filter(|s| !s.is_empty()),
        });
    }

    Err(VizierError(format!(
        "no credentials found for azure. Configure it via the WebUI \
         (PUT /api/v1/providers/azure) or set AZURE_ENDPOINT and \
         AZURE_API_KEY environment variables."
    )))
}

/// Resolve ChatGPT backend credentials: access token + account id.
///
/// Looks up the WebUI-managed storage entry first, then falls back to
/// `CHATGPT_ACCESS_TOKEN` and `CHATGPT_ACCOUNT_ID` environment variables.
pub async fn resolve_chatgpt_provider(
    storage: &Arc<VizierStorage>,
) -> Result<ResolvedProvider, VizierError> {
    if let Ok(Some(entry)) = storage.get_provider(&ProviderVariant::chatgpt).await {
        if let ProviderEntryConfig::Chatgpt {
            access_token,
            account_id,
            base_url,
        } = entry.config
            && !access_token.is_empty()
        {
            return Ok(ResolvedProvider {
                api_key: format!("{}:{}", access_token, account_id),
                base_url,
            });
        }
    }

    if let Ok(access_token) = std::env::var("CHATGPT_ACCESS_TOKEN")
        && !access_token.is_empty()
        && let Ok(account_id) = std::env::var("CHATGPT_ACCOUNT_ID")
        && !account_id.is_empty()
    {
        let base_url = std::env::var("CHATGPT_API_BASE").ok();
        return Ok(ResolvedProvider {
            api_key: format!("{}:{}", access_token, account_id),
            base_url: base_url.filter(|s| !s.is_empty()),
        });
    }

    Err(VizierError(format!(
        "no credentials found for chatgpt. Configure it via the WebUI \
         (PUT /api/v1/providers/chatgpt) or set CHATGPT_ACCESS_TOKEN and \
         CHATGPT_ACCOUNT_ID environment variables."
    )))
}

/// Resolve a local (URL-only) provider by looking up, in order:
/// 1. WebUI-managed storage entry (`storage.get_provider(&variant)`)
/// 2. Environment variable named `env_var_name`
///
/// Covers ollama and llama_cpp. Returns `api_key: ""` and `base_url: Some(url)`.
pub async fn resolve_local_provider(
    storage: &Arc<VizierStorage>,
    variant: ProviderVariant,
    env_var_name: &'static str,
    default_base_url: &'static str,
) -> Result<ResolvedProvider, VizierError> {
    if let Ok(Some(entry)) = storage.get_provider(&variant).await {
        let base_url = match entry.config {
            ProviderEntryConfig::Ollama { base_url } => Some(base_url),
            ProviderEntryConfig::LlamaCpp { base_url } => Some(base_url),
            _ => None,
        };
        if let Some(base_url) = base_url
            && !base_url.is_empty()
        {
            return Ok(ResolvedProvider {
                api_key: String::new(),
                base_url: Some(base_url),
            });
        }
    }

    if let Ok(url) = std::env::var(env_var_name)
        && !url.is_empty()
    {
        return Ok(ResolvedProvider {
            api_key: String::new(),
            base_url: Some(url),
        });
    }

    Ok(ResolvedProvider {
        api_key: String::new(),
        base_url: Some(default_base_url.to_string()),
    })
}
