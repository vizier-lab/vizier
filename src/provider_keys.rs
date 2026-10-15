//! Where a provider's credentials come from (data-model §4, FR-009, FR-016a): the saved
//! `provider_config` row, then the catalogue's env vars, then the env vars vizier read before
//! the catalogue, so existing deployments keep working.

use std::sync::Arc;

use crate::config::provider::ProviderId;
use crate::schema::provider::ProviderCredentials;
use crate::storage::{VizierStorage, provider::ProviderStorage};


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

/// A provider's credentials: the saved row (its set fields win), with anything it leaves out
/// filled from the environment. `None` when there is neither.
pub async fn resolve_credentials(
    storage: &Arc<VizierStorage>,
    id: &ProviderId,
    catalogue_env: &[String],
) -> Option<(ProviderCredentials, CredentialOrigin)> {
    let saved = storage
        .get_provider(id)
        .await
        .ok()
        .flatten()
        .map(|e| e.credentials)
        .filter(|c| !c.is_empty());
    let env = credentials_from_env(id, catalogue_env);
    match (saved, env) {
        (Some(saved), Some((env, _))) => Some((env.merged_with(&saved), CredentialOrigin::Saved)),
        (Some(saved), None) => Some((saved, CredentialOrigin::Saved)),
        (None, env) => env,
    }
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
