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
pub struct ResolvedProvider {
    pub api_key: String,
    pub base_url: Option<String>,
}

/// Resolve a remote (API-key) provider by looking up, in order:
/// 1. WebUI-managed storage entry (`storage.get_provider(&variant)`)
/// 2. Environment variable named `env_var_name`
///
/// Covers all remote variants: openai, anthropic, openrouter, deepseek,
/// gemini, mimo, elevenlabs.
///
/// `base_url` is intentionally not overridable for any variant — proxies
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
