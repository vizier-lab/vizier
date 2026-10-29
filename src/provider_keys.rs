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
    AccessKeyId,
    SecretAccessKey,
    SessionToken,
    Region,
}

/// Extra environment variables a provider reads that the catalogue's `env[]` does not list.
/// Appended by both the catalogue listing (the form and connection state) and `provider_env`
/// (resolution), so the two always agree.
pub fn extra_env(id: &ProviderId) -> &'static [&'static str] {
    match id.as_str() {
        "amazon-bedrock" => &["AWS_SESSION_TOKEN", "AWS_DEFAULT_REGION"],
        _ => &[],
    }
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

/// Match a catalogue environment variable to the provider field it configures. AWS names are
/// explicit and checked first: the generic `KEY`/`TOKEN` heuristics would otherwise map both
/// Bedrock key variables to `api_key` and lose the region.
fn catalogue_env_field(name: &str) -> Option<Field> {
    match name {
        "AWS_ACCESS_KEY_ID" => return Some(Field::AccessKeyId),
        "AWS_SECRET_ACCESS_KEY" => return Some(Field::SecretAccessKey),
        "AWS_SESSION_TOKEN" => return Some(Field::SessionToken),
        "AWS_REGION" | "AWS_DEFAULT_REGION" => return Some(Field::Region),
        // Recognised only so the resolver can say Bedrock API keys are unsupported.
        "AWS_BEARER_TOKEN_BEDROCK" => return Some(Field::ApiKey),
        _ => {}
    }
    if name.contains("KEY") {
        Some(Field::ApiKey)
    } else if name.ends_with("_ACCESS_TOKEN") {
        Some(Field::AccessToken)
    } else if name.ends_with("_ENDPOINT") {
        Some(Field::Endpoint)
    } else if name.ends_with("_ACCOUNT_ID") {
        Some(Field::AccountId)
    } else if name.ends_with("_BASE_URL") || name.ends_with("_URL") || name.ends_with("_API_BASE") {
        Some(Field::BaseUrl)
    } else if name.contains("TOKEN") {
        Some(Field::ApiKey)
    } else {
        None
    }
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Credentials from the environment: recognized catalogue `env[]` fields in order, then the
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
            Field::AccessKeyId => &mut creds.access_key_id,
            Field::SecretAccessKey => &mut creds.secret_access_key,
            Field::SessionToken => &mut creds.session_token,
            Field::Region => &mut creds.region,
        };
        if slot.is_none() {
            *slot = Some(value);
            origin.get_or_insert_with(|| CredentialOrigin::Env(name.to_string()));
        }
    };
    for name in catalogue_env {
        if let Some(field) = catalogue_env_field(name) {
            set(field, name, &mut creds);
        }
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

    #[test]
    fn catalogue_base_url_variable_fills_base_url() {
        // SAFETY: unique test variable, removed immediately after the assertion.
        unsafe { std::env::set_var("VZT_TEST_URL", "http://127.0.0.1:9998/v1"); }
        let id = ProviderId::new("vzt-test-url".to_string()).unwrap();
        let (creds, origin) = credentials_from_env(&id, &["VZT_TEST_URL".to_string()]).unwrap();
        assert_eq!(creds.base_url.as_deref(), Some("http://127.0.0.1:9998/v1"));
        assert_eq!(origin, CredentialOrigin::Env("VZT_TEST_URL".into()));
        unsafe { std::env::remove_var("VZT_TEST_URL"); }
    }

    #[test]
    fn aws_names_map_explicitly_and_region_precedes_the_default_region() {
        let _env = crate::tests_support::env_lock();
        // SAFETY: test-only names no other test reads.
        unsafe {
            std::env::set_var("AWS_ACCESS_KEY_ID", "AKIATEST");
            std::env::set_var("AWS_SECRET_ACCESS_KEY", "secret");
            std::env::set_var("AWS_SESSION_TOKEN", "token");
            std::env::set_var("AWS_REGION", "eu-west-1");
            std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");
        }
        let id = ProviderId::AMAZON_BEDROCK;
        let names: Vec<String> = ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_REGION"]
            .into_iter()
            .chain(extra_env(&id).iter().copied())
            .map(str::to_string)
            .collect();
        let (creds, origin) = credentials_from_env(&id, &names).unwrap();
        assert_eq!(creds.access_key_id.as_deref(), Some("AKIATEST"));
        assert_eq!(creds.secret_access_key.as_deref(), Some("secret"));
        assert_eq!(creds.session_token.as_deref(), Some("token"));
        assert_eq!(creds.region.as_deref(), Some("eu-west-1"), "AWS_REGION wins");
        assert_eq!(creds.api_key, None);
        assert_eq!(origin, CredentialOrigin::Env("AWS_ACCESS_KEY_ID".into()));

        // A saved row wins field by field; the environment fills only what it leaves unset.
        let saved = ProviderCredentials {
            secret_access_key: Some("saved-secret".into()),
            region: Some("us-west-2".into()),
            ..Default::default()
        };
        let merged = creds.merged_with(&saved);
        assert_eq!(merged.access_key_id.as_deref(), Some("AKIATEST"));
        assert_eq!(merged.secret_access_key.as_deref(), Some("saved-secret"));
        assert_eq!(merged.region.as_deref(), Some("us-west-2"));

        unsafe {
            std::env::remove_var("AWS_ACCESS_KEY_ID");
            std::env::remove_var("AWS_SECRET_ACCESS_KEY");
            std::env::remove_var("AWS_SESSION_TOKEN");
            std::env::remove_var("AWS_REGION");
            std::env::remove_var("AWS_DEFAULT_REGION");
        }
    }

    #[test]
    fn bedrock_bearer_token_maps_to_api_key() {
        assert_eq!(
            catalogue_env_field("AWS_BEARER_TOKEN_BEDROCK"),
            Some(Field::ApiKey)
        );
        assert_eq!(catalogue_env_field("AWS_ACCESS_KEY_ID"), Some(Field::AccessKeyId));
        assert_eq!(catalogue_env_field("AWS_DEFAULT_REGION"), Some(Field::Region));
    }
}
