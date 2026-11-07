//! Media and embedding clients. They implement vizier's existing `Vizier*Model` traits
//! directly, over the same transport as completions (contract S9).

pub mod bedrock;
pub mod cohere;
pub mod gemini;
pub mod huggingface;
pub mod hyperbolic;
pub mod ollama;
pub mod openai_shaped;

pub use openai_shaped::Endpoint;

#[cfg(test)]
pub(crate) mod golden_tests;

use crate::config::provider::ProviderId;
use crate::dependencies::VizierDependencies;
use crate::llm::auth::{CopilotExchange, CredentialSource, KeyStyle, StaticKey};
use crate::VizierError;

/// The API root each curated media provider is called at (contract `provider-layer.md` §
/// Media). Media providers stay curated (FR-016), so this is a table, not catalogue data.
pub fn media_base_url(id: &ProviderId) -> &'static str {
    match id.as_str() {
        "openai" => "https://api.openai.com/v1",
        "groq" => "https://api.groq.com/openai/v1",
        "mistral" => "https://api.mistral.ai/v1",
        "google" => "https://generativelanguage.googleapis.com/v1beta",
        "huggingface" => huggingface::HUGGINGFACE_INFERENCE_URL,
        "openrouter" => "https://openrouter.ai/api/v1",
        "xai" => "https://api.x.ai/v1",
        "hyperbolic" => hyperbolic::HYPERBOLIC_BASE_URL,
        "togetherai" => "https://api.together.xyz/v1",
        "voyageai" => "https://api.voyageai.com/v1",
        "cohere" => cohere::COHERE_EMBED_BASE_URL,
        "github-copilot" => crate::llm::auth::COPILOT_API_BASE_URL,
        "ollama" => "http://localhost:11434",
        _ => "",
    }
}

/// A provider's API key for a media feature, from Settings › Providers or its env vars.
pub async fn api_key(deps: &VizierDependencies, id: &ProviderId) -> crate::Result<String> {
    let env = crate::llm::resolve::provider_env(&deps.catalogue, id);
    crate::provider_keys::resolve_credentials(&deps.storage, id, &env)
        .await
        .and_then(|(c, _)| c.api_key)
        .filter(|k| !k.is_empty())
        .ok_or_else(|| {
            let hint = env
                .iter()
                .find(|e| e.contains("KEY") || e.contains("TOKEN"))
                .map(|e| format!(" or set {e}"))
                .unwrap_or_default();
            VizierError(format!(
                "no API key found for provider \"{id}\": add one in Settings › Providers{hint}"
            ))
        })
}

/// The endpoint a media client of provider `id` calls, with its credential.
pub async fn endpoint(
    deps: &VizierDependencies,
    id: &ProviderId,
    model: String,
) -> crate::Result<Endpoint> {
    if *id == ProviderId::AMAZON_BEDROCK || *id == ProviderId::AMAZON_BEDROCK_SSO {
        // One saved connection serves embeddings and images with the same region rules as chat
        // (research R6/R7). The credential comes from the shared helper, so the static pair and
        // the SSO session differ in exactly one place (FR-002, FR-009d).
        let bedrock = crate::llm::resolve::bedrock_role_credentials(deps, id, None).await?;
        return Ok(Endpoint {
            http: deps.http.clone(),
            base_url: crate::llm::bedrock::runtime_endpoint(&bedrock.region),
            credential: bedrock.credential,
            model,
        });
    }
    let base_url = media_base_url(id).to_string();
    if *id == ProviderId::OLLAMA {
        let env = crate::llm::resolve::provider_env(&deps.catalogue, id);
        let base = crate::provider_keys::resolve_credentials(&deps.storage, id, &env)
            .await
            .and_then(|(c, _)| c.base_url)
            .filter(|u| !u.is_empty())
            .unwrap_or(base_url);
        return Ok(Endpoint {
            http: deps.http.clone(),
            base_url: base,
            credential: StaticKey::none(),
            model,
        });
    }
    let key = api_key(deps, id).await?;
    let credential: std::sync::Arc<dyn CredentialSource> = if *id == ProviderId::GITHUB_COPILOT {
        CopilotExchange::new(deps.http.clone(), key, base_url.clone())
    } else if *id == ProviderId::GOOGLE {
        StaticKey::shared(KeyStyle::Header("x-goog-api-key"), Some(key))
    } else {
        StaticKey::shared(KeyStyle::Bearer, Some(key))
    };
    Ok(Endpoint {
        http: deps.http.clone(),
        base_url,
        credential,
        model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VizierConfig;
    use crate::schema::{ProviderCredentials, ProviderEntry};
    use crate::storage::provider::ProviderStorage;

    async fn deps() -> (VizierDependencies, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        (VizierDependencies::new(config).await.unwrap(), dir)
    }

    fn sso_row() -> ProviderEntry {
        ProviderEntry {
            id: ProviderId::AMAZON_BEDROCK_SSO,
            credentials: ProviderCredentials {
                start_url: Some("https://acme.awsapps.com/start".into()),
                sso_region: Some("us-east-1".into()),
                account_id: Some("123456789012".into()),
                role_name: Some("BedrockOperator".into()),
                sso_profile: Some("acme-bedrock".into()),
                region: Some("eu-west-1".into()),
                ..Default::default()
            },
        }
    }

    fn static_row() -> ProviderEntry {
        ProviderEntry {
            id: ProviderId::AMAZON_BEDROCK,
            credentials: ProviderCredentials {
                region: Some("eu-west-1".into()),
                access_key_id: Some("AKIAEXAMPLE".into()),
                secret_access_key: Some("secret".into()),
                ..Default::default()
            },
        }
    }

    /// FR-002/FR-033: both Bedrock connections reach the runtime endpoint through the one
    /// credential helper, so media never signs in a second time — for the SSO row there is no
    /// API key and no stored session here, and building the endpoint still succeeds.
    #[tokio::test]
    async fn both_bedrock_connections_build_their_endpoint_from_the_shared_helper() {
        let (deps, _dir) = deps().await;
        deps.storage.upsert_provider(&sso_row()).await.unwrap();
        deps.storage.upsert_provider(&static_row()).await.unwrap();

        let model = crate::llm::media::bedrock::DEFAULT_EMBEDDING_MODEL.to_string();
        for id in [ProviderId::AMAZON_BEDROCK_SSO, ProviderId::AMAZON_BEDROCK] {
            let endpoint = endpoint(&deps, &id, model.clone())
                .await
                .unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(endpoint.model, model);
            assert_eq!(
                endpoint.base_url,
                crate::llm::bedrock::runtime_endpoint("eu-west-1"),
                "{id}: the Bedrock region, not the SSO region"
            );
        }
    }

    /// A missing connection is still named as a connection problem, not as a missing API key.
    ///
    /// Blocking rather than `#[tokio::test]`, because the region falls back to `AWS_REGION` and
    /// another test sets it for its own turn: the environment lock has to be held for the whole
    /// lookup, and a guard cannot be held across an `await`.
    #[test]
    fn an_unconfigured_sso_connection_is_not_reported_as_a_missing_key() {
        let _env = crate::tests_support::env_lock();
        let error = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let (deps, _dir) = deps().await;
                endpoint(
                    &deps,
                    &ProviderId::AMAZON_BEDROCK_SSO,
                    crate::llm::media::bedrock::DEFAULT_EMBEDDING_MODEL.to_string(),
                )
                .await
                .err()
                .expect("an unconfigured connection is refused")
                .0
            });
        assert!(error.contains("amazon-bedrock-sso"), "{error}");
        assert!(!error.contains("API key"), "{error}");
    }
}
