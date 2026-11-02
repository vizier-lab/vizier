//! Providers vizier defines in code because the catalogue doesn't have them (data-model §3).
//! They have the same shape the pickers and resolve read for catalogue providers.

use super::data::ProviderKind;
use crate::config::provider::ProviderId;
use crate::llm::dialect::DialectKind;

/// A credential field a provider's connect form asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CredentialField {
    ApiKey,
    Endpoint,
    BaseUrl,
    AccessToken,
    AccountId,
    /// AWS access key id; an identifier, not a secret.
    AccessKeyId,
    SecretAccessKey,
    /// Temporary-credential session token.
    SessionToken,
    /// AWS region (also a Bedrock chat model's region).
    Region,
    /// The IAM Identity Center portal URL (Bedrock over SSO).
    StartUrl,
    /// The region the Identity Center instance lives in — not the Bedrock region.
    SsoRegion,
    /// The role chosen from the portal.
    RoleName,
    /// The non-secret handle the stored session is keyed by.
    SsoProfile,
}

#[derive(Debug, Clone)]
pub struct BuiltinProvider {
    pub id: ProviderId,
    pub name: &'static str,
    pub kind: ProviderKind,
    /// `None`: not a chat provider (media-only).
    pub dialect: Option<DialectKind>,
    pub fields: &'static [CredentialField],
    /// Env vars, in lookup order, matching `fields`.
    pub env: &'static [&'static str],
    pub default_base_url: Option<&'static str>,
    /// The catalogue provider whose models this entry serves, when it is the same list under a
    /// second identity. The SSO Bedrock provider is the only one today: it answers with
    /// `amazon-bedrock`'s models rather than duplicating them (spec 044, FR-030, research D2).
    pub model_source: Option<ProviderId>,
}

pub const CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";

pub fn builtins() -> &'static [BuiltinProvider] {
    use CredentialField::*;
    static BUILTINS: std::sync::LazyLock<Vec<BuiltinProvider>> = std::sync::LazyLock::new(|| {
        vec![
            BuiltinProvider {
                id: ProviderId::OLLAMA,
                name: "Ollama",
                kind: ProviderKind::Local,
                dialect: Some(DialectKind::Ollama),
                fields: &[BaseUrl],
                env: &["OLLAMA_BASE_URL"],
                default_base_url: Some("http://localhost:11434"),
                model_source: None,
            },
            BuiltinProvider {
                id: ProviderId::LLAMA_CPP,
                name: "llama.cpp",
                kind: ProviderKind::Local,
                dialect: Some(DialectKind::OpenAiChat),
                fields: &[BaseUrl],
                env: &["LLAMA_CPP_BASE_URL"],
                default_base_url: Some("http://localhost:8080"),
                model_source: None,
            },
            BuiltinProvider {
                id: ProviderId::CHATGPT,
                name: "ChatGPT (subscription)",
                kind: ProviderKind::Cloud,
                dialect: Some(DialectKind::OpenAiResponses),
                fields: &[AccessToken, AccountId, BaseUrl],
                env: &["CHATGPT_ACCESS_TOKEN", "CHATGPT_ACCOUNT_ID", "CHATGPT_API_BASE"],
                default_base_url: Some(CHATGPT_BASE_URL),
                model_source: None,
            },
            BuiltinProvider {
                id: ProviderId::DUMMYPLUG,
                name: "dummyplug (testing)",
                kind: ProviderKind::Local,
                dialect: Some(DialectKind::OpenAiChat),
                fields: &[BaseUrl],
                env: &["DUMMYPLUG_URL"],
                default_base_url: Some("http://127.0.0.1:9998/v1"),
                model_source: None,
            },
            BuiltinProvider {
                id: ProviderId::ELEVENLABS,
                name: "ElevenLabs",
                kind: ProviderKind::Media,
                dialect: None,
                fields: &[ApiKey],
                env: &["ELEVENLABS_API_KEY"],
                default_base_url: None,
                model_source: None,
            },
            BuiltinProvider {
                id: ProviderId::VOYAGEAI,
                name: "Voyage AI",
                kind: ProviderKind::Media,
                dialect: None,
                fields: &[ApiKey],
                env: &["VOYAGE_API_KEY"],
                default_base_url: Some("https://api.voyageai.com/v1"),
                model_source: None,
            },
            BuiltinProvider {
                id: ProviderId::HYPERBOLIC,
                name: "Hyperbolic",
                kind: ProviderKind::Media,
                dialect: None,
                fields: &[ApiKey],
                env: &["HYPERBOLIC_API_KEY"],
                default_base_url: Some(crate::llm::media::hyperbolic::HYPERBOLIC_BASE_URL),
                model_source: None,
            },
            sso_bedrock(),
        ]
    });
    &BUILTINS
}

pub fn builtin(id: &ProviderId) -> Option<&'static BuiltinProvider> {
    builtins().iter().find(|b| &b.id == id)
}

/// The SSO identity of [`ProviderId::AMAZON_BEDROCK`] (spec 044, FR-001): the same models under a
/// second id, reached with an IAM Identity Center sign-in instead of an access key pair. Its
/// `model_source` is what makes the catalogue answer with `amazon-bedrock`'s list (FR-030).
pub fn sso_bedrock() -> BuiltinProvider {
    use CredentialField::*;
    BuiltinProvider {
        id: ProviderId::AMAZON_BEDROCK_SSO,
        name: crate::config::provider::AMAZON_BEDROCK_SSO_NAME,
        kind: ProviderKind::Cloud,
        dialect: Some(DialectKind::Bedrock),
        // The five a connection needs, then the Bedrock region, not the SSO region
        // (contract `provider-credentials-api.md` 1).
        fields: &[StartUrl, SsoRegion, AccountId, RoleName, SsoProfile, Region],
        // A sign-in lives in the session store, so there is no environment fallback: naming
        // variables that would not work is worse than naming none.
        env: &[],
        default_base_url: None,
        model_source: Some(ProviderId::AMAZON_BEDROCK),
    }
}

/// The catalogue provider whose models `id` serves, when that id is a second identity of one
/// (`model_source`, FR-030, research D2). The one alias function the catalogue's `provider` and
/// `model` lookups and the resolver go through, so a second id never means a second model list
/// (plan.md, risk "second ProviderId").
pub fn model_source(id: &ProviderId) -> Option<ProviderId> {
    builtin(id).and_then(|b| b.model_source.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_found_by_id() {
        assert_eq!(builtin(&ProviderId::OLLAMA).unwrap().kind, ProviderKind::Local);
        assert_eq!(builtin(&ProviderId::DUMMYPLUG).unwrap().dialect, Some(DialectKind::OpenAiChat));
        assert!(builtin(&ProviderId::OPENAI).is_none());
        let mut ids: Vec<_> = builtins().iter().map(|b| b.id.as_str()).collect();
        ids.dedup();
        assert_eq!(ids.len(), builtins().len());
    }
}
