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
    if *id == ProviderId::AMAZON_BEDROCK {
        // One saved connection (access key pair + region) serves embeddings and images; the
        // same region/pair rules as the chat resolver decide the host (research R6/R7).
        let env = crate::llm::resolve::provider_env(&deps.catalogue, id);
        let credentials = crate::provider_keys::resolve_credentials(&deps.storage, id, &env)
            .await
            .map(|(c, _)| c)
            .unwrap_or_default();
        let region = crate::llm::resolve::bedrock_region(&credentials)?;
        let (access_key_id, secret_access_key, session_token) =
            crate::llm::resolve::bedrock_keys(&credentials)?;
        return Ok(Endpoint {
            http: deps.http.clone(),
            base_url: crate::llm::bedrock::runtime_endpoint(&region),
            credential: crate::llm::auth::SigV4Credential::bedrock(
                access_key_id,
                secret_access_key,
                session_token,
                region,
            ),
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
