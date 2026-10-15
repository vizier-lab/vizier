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
/// Covers moonshot, zai, minimax, opencode_zen, opencode_go (and any future
/// openai-compat provider that exposes a `base_url` builder). Storage entry
/// takes precedence; env var is the fallback. If neither has a base URL,
/// `default_base_url` is returned instead (pass `None` for providers whose
/// rig-core client already bakes in its own hardcoded default; pass
/// `Some(url)` for providers built on a generic client, like
/// `openai::CompletionsClient`, that has no opinion of its own).
pub async fn resolve_provider_with_base_url(
    storage: &Arc<VizierStorage>,
    variant: ProviderVariant,
    env_var_name: &'static str,
    default_base_url: Option<&'static str>,
) -> Result<ResolvedProvider, VizierError> {
    if let Ok(Some(entry)) = storage.get_provider(&variant).await {
        let (api_key, base_url) = match entry.config {
            ProviderEntryConfig::Moonshot { api_key, base_url } => (api_key, base_url),
            ProviderEntryConfig::Zai { api_key, base_url } => (api_key, base_url),
            ProviderEntryConfig::Minimax { api_key, base_url } => (api_key, base_url),
            ProviderEntryConfig::OpencodeZen { api_key, base_url } => (api_key, base_url),
            ProviderEntryConfig::OpencodeGo { api_key, base_url } => (api_key, base_url),
            _ => (String::new(), None),
        };
        if !api_key.is_empty() {
            return Ok(ResolvedProvider {
                api_key,
                base_url: base_url.or_else(|| default_base_url.map(str::to_string)),
            });
        }
    }

    if let Ok(key) = std::env::var(env_var_name)
        && !key.is_empty()
    {
        let base_url = std::env::var(format!("{}_API_BASE", env_var_name_prefix(env_var_name)))
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| default_base_url.map(str::to_string));
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

/// Resolve Custom (OpenAI-compatible) provider credentials: base_url + api_key.
///
/// Looks up the WebUI-managed storage entry first, then falls back to
/// `CUSTOM_BASE_URL` and `CUSTOM_API_KEY` environment variables. The
/// `base_url` is **required**: callers must point at an OpenAI Chat
/// Completions-compatible endpoint (LM Studio, vLLM, llama.cpp server, etc.).
pub async fn resolve_custom_provider(
    storage: &Arc<VizierStorage>,
) -> Result<ResolvedProvider, VizierError> {
    if let Ok(Some(entry)) = storage.get_provider(&ProviderVariant::custom).await {
        if let ProviderEntryConfig::Custom { api_key, base_url } = entry.config
            && !base_url.is_empty()
        {
            return Ok(ResolvedProvider {
                api_key,
                base_url: Some(base_url),
            });
        }
    }

    if let Ok(base_url) = std::env::var("CUSTOM_BASE_URL")
        && !base_url.is_empty()
    {
        let api_key = std::env::var("CUSTOM_API_KEY").unwrap_or_default();
        return Ok(ResolvedProvider {
            api_key,
            base_url: Some(base_url),
        });
    }

    Err(VizierError(format!(
        "no base_url found for provider {:?}. Configure it via the WebUI \
         (PUT /api/v1/providers/custom) or set the CUSTOM_BASE_URL \
         environment variable.",
        ProviderVariant::custom
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

// Generic credential lookup (spec 015, data-model §4) ------------------------------------------

use crate::config::provider::ProviderId;
use crate::schema::provider::ProviderCredentials;

/// Where a provider's credentials came from, as the providers API reports it (FR-010a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialOrigin {
    Saved,
    Env(String),
}

impl CredentialOrigin {
    pub fn label(&self) -> String {
        match self {
            CredentialOrigin::Saved => "saved".into(),
            CredentialOrigin::Env(name) => format!("env:{name}"),
        }
    }
}

/// Which credential field an environment variable fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    ApiKey,
    Endpoint,
    BaseUrl,
    AccessToken,
    AccountId,
}

/// Env vars vizier read before the catalogue, still honoured so existing deployments keep
/// working (contract `provider-id-migration.md`). Checked after the catalogue's own `env[]`.
fn legacy_env(id: &ProviderId) -> &'static [(Field, &'static str)] {
    use Field::*;
    match id.as_str() {
        "openai" => &[(ApiKey, "OPENAI_API_KEY")],
        "anthropic" => &[(ApiKey, "ANTHROPIC_API_KEY")],
        "openrouter" => &[(ApiKey, "OPENROUTER_API_KEY")],
        "deepseek" => &[(ApiKey, "DEEPSEEK_API_KEY")],
        "google" => &[(ApiKey, "GEMINI_API_KEY")],
        "xiaomi" => &[(ApiKey, "XIAOMI_MIMO_API_KEY")],
        "groq" => &[(ApiKey, "GROQ_API_KEY")],
        "mistral" => &[(ApiKey, "MISTRAL_API_KEY")],
        "xai" => &[(ApiKey, "XAI_API_KEY")],
        "perplexity" => &[(ApiKey, "PERPLEXITY_API_KEY")],
        "moonshotai" | "moonshotai-cn" => &[(ApiKey, "MOONSHOT_API_KEY")],
        "zai" | "zhipuai" => &[(ApiKey, "ZAI_API_KEY")],
        "minimax" | "minimax-cn" => &[(ApiKey, "MINIMAX_API_KEY")],
        "togetherai" => &[(ApiKey, "TOGETHER_API_KEY")],
        "cohere" => &[(ApiKey, "COHERE_API_KEY")],
        "huggingface" => &[(ApiKey, "HUGGINGFACE_API_KEY")],
        "github-copilot" => &[(ApiKey, "COPILOT_API_KEY")],
        "azure" => &[(ApiKey, "AZURE_API_KEY"), (Endpoint, "AZURE_ENDPOINT")],
        "opencode" => &[(ApiKey, "OPENCODE_ZEN_API_KEY")],
        "opencode-go" => &[(ApiKey, "OPENCODE_GO_API_KEY")],
        "ollama" => &[(BaseUrl, "OLLAMA_BASE_URL")],
        "llama-cpp" => &[(BaseUrl, "LLAMA_CPP_BASE_URL")],
        "chatgpt" => &[
            (AccessToken, "CHATGPT_ACCESS_TOKEN"),
            (AccountId, "CHATGPT_ACCOUNT_ID"),
            (BaseUrl, "CHATGPT_API_BASE"),
        ],
        "elevenlabs" => &[(ApiKey, "ELEVENLABS_API_KEY")],
        "voyageai" => &[(ApiKey, "VOYAGE_API_KEY")],
        "hyperbolic" => &[(ApiKey, "HYPERBOLIC_API_KEY")],
        _ => &[],
    }
}

/// A catalogue `env[]` entry that holds a secret (`*_API_KEY`, `HF_TOKEN`, `GITHUB_TOKEN`…).
/// The others (`AZURE_RESOURCE_NAME`, regions, projects) are only for `${VAR}` expansion.
fn is_secret_var(name: &str) -> bool {
    name.contains("KEY") || name.contains("TOKEN")
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Credentials from the environment: the catalogue's `env[]` (secrets only) in order, then the
/// legacy names. `None` when nothing is set. The origin names the first variable used.
pub fn credentials_from_env(
    id: &ProviderId,
    catalogue_env: &[String],
) -> Option<(ProviderCredentials, CredentialOrigin)> {
    let mut creds = ProviderCredentials::default();
    let mut origin = None;
    let mut set = |field: Field, name: &str, creds: &mut ProviderCredentials| {
        let Some(value) = env_value(name) else {
            return;
        };
        let slot = match field {
            Field::ApiKey => &mut creds.api_key,
            Field::Endpoint => &mut creds.endpoint,
            Field::BaseUrl => &mut creds.base_url,
            Field::AccessToken => &mut creds.access_token,
            Field::AccountId => &mut creds.account_id,
        };
        if slot.is_none() {
            *slot = Some(value);
            origin.get_or_insert_with(|| CredentialOrigin::Env(name.to_string()));
        }
    };
    for name in catalogue_env.iter().filter(|n| is_secret_var(n)) {
        set(Field::ApiKey, name, &mut creds);
    }
    for (field, name) in legacy_env(id) {
        set(*field, name, &mut creds);
    }
    let origin = origin?;
    Some((creds, origin))
}

#[cfg(test)]
mod credential_tests {
    use super::*;

    #[test]
    fn catalogue_env_comes_first_then_legacy_names_and_non_secrets_are_skipped() {
        // SAFETY: test-only env vars with names no other test uses.
        unsafe {
            std::env::set_var("VZT_TEST_RESOURCE_NAME", "res");
            std::env::set_var("VZT_TEST_API_KEY", "k1");
        }
        let id = ProviderId::new("vzt-test".to_string()).unwrap();
        let env = vec!["VZT_TEST_RESOURCE_NAME".to_string(), "VZT_TEST_API_KEY".to_string()];
        let (creds, origin) = credentials_from_env(&id, &env).unwrap();
        assert_eq!(creds.api_key.as_deref(), Some("k1"));
        assert_eq!(origin, CredentialOrigin::Env("VZT_TEST_API_KEY".into()));
        assert!(credentials_from_env(&id, &[]).is_none());
        unsafe {
            std::env::remove_var("VZT_TEST_RESOURCE_NAME");
            std::env::remove_var("VZT_TEST_API_KEY");
        }
    }

    #[test]
    fn origin_labels() {
        assert_eq!(CredentialOrigin::Saved.label(), "saved");
        assert_eq!(CredentialOrigin::Env("X".into()).label(), "env:X");
    }
}
