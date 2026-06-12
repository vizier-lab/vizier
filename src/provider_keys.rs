use std::sync::Arc;

use crate::{
    VizierError,
    config::provider::ProviderVariant,
    schema::ProviderEntryConfig,
    storage::{VizierStorage, provider::ProviderStorage},
};

/// Result of resolving a provider's API key and optional base URL.
pub struct ResolvedProvider {
    pub api_key: String,
    pub base_url: Option<String>,
}

/// Resolve a provider's API key by looking up, in order:
/// 1. WebUI-managed storage entry (`storage.get_provider(&variant)`)
/// 2. Environment variable named `env_var_name`
///
/// Returns a clear `VizierError` with remediation instructions if all sources are empty.
pub async fn resolve_provider_key(
    storage: &Arc<VizierStorage>,
    variant: ProviderVariant,
    env_var_name: &'static str,
) -> Result<ResolvedProvider, VizierError> {
    if let Ok(Some(entry)) = storage.get_provider(&variant).await {
        let resolved = match entry.config {
            ProviderEntryConfig::Openai { api_key, base_url } => Some((api_key, base_url)),
            ProviderEntryConfig::Openrouter { api_key } => Some((api_key, None)),
            ProviderEntryConfig::Elevenlabs { api_key } => Some((api_key, None)),
            _ => None,
        };
        if let Some((api_key, base_url)) = resolved
            && !api_key.is_empty()
        {
            return Ok(ResolvedProvider { api_key, base_url });
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
