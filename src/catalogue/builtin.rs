//! Providers vizier defines in code because the catalogue doesn't have them (data-model §3).
//! They have the same shape the pickers and resolve read for catalogue providers.

use super::data::ProviderKind;
use crate::config::provider::ProviderId;
use crate::llm::dialect::DialectKind;

/// A credential field a provider's connect form asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CredentialField {
    ApiKey,
    Endpoint,
    BaseUrl,
    AccessToken,
    AccountId,
}

#[derive(Debug, Clone)]
pub struct BuiltinProvider {
    pub id: ProviderId,
    pub name: &'static str,
    pub kind: ProviderKind,
    /// `None`: not a chat provider (media-only), or dummyplug, which has its own model.
    pub dialect: Option<DialectKind>,
    pub fields: &'static [CredentialField],
    /// Env vars, in lookup order, matching `fields`.
    pub env: &'static [&'static str],
    pub default_base_url: Option<&'static str>,
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
            },
            BuiltinProvider {
                id: ProviderId::LLAMA_CPP,
                name: "llama.cpp",
                kind: ProviderKind::Local,
                dialect: Some(DialectKind::OpenAiChat),
                fields: &[BaseUrl],
                env: &["LLAMA_CPP_BASE_URL"],
                default_base_url: Some("http://localhost:8080"),
            },
            BuiltinProvider {
                id: ProviderId::CHATGPT,
                name: "ChatGPT (subscription)",
                kind: ProviderKind::Cloud,
                dialect: Some(DialectKind::OpenAiResponses),
                fields: &[AccessToken, AccountId, BaseUrl],
                env: &["CHATGPT_ACCESS_TOKEN", "CHATGPT_ACCOUNT_ID", "CHATGPT_API_BASE"],
                default_base_url: Some(CHATGPT_BASE_URL),
            },
            BuiltinProvider {
                id: ProviderId::DUMMYPLUG,
                name: "dummyplug (testing)",
                kind: ProviderKind::Local,
                dialect: None,
                fields: &[],
                env: &[],
                default_base_url: None,
            },
            BuiltinProvider {
                id: ProviderId::ELEVENLABS,
                name: "ElevenLabs",
                kind: ProviderKind::Media,
                dialect: None,
                fields: &[ApiKey],
                env: &["ELEVENLABS_API_KEY"],
                default_base_url: None,
            },
            BuiltinProvider {
                id: ProviderId::VOYAGEAI,
                name: "Voyage AI",
                kind: ProviderKind::Media,
                dialect: None,
                fields: &[ApiKey],
                env: &["VOYAGE_API_KEY"],
                default_base_url: Some("https://api.voyageai.com/v1"),
            },
            BuiltinProvider {
                id: ProviderId::HYPERBOLIC,
                name: "Hyperbolic",
                kind: ProviderKind::Media,
                dialect: None,
                fields: &[ApiKey],
                env: &["HYPERBOLIC_API_KEY"],
                default_base_url: Some(crate::llm::media::hyperbolic::HYPERBOLIC_BASE_URL),
            },
        ]
    });
    &BUILTINS
}

pub fn builtin(id: &ProviderId) -> Option<&'static BuiltinProvider> {
    builtins().iter().find(|b| &b.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_found_by_id() {
        assert_eq!(builtin(&ProviderId::OLLAMA).unwrap().kind, ProviderKind::Local);
        assert_eq!(builtin(&ProviderId::DUMMYPLUG).unwrap().dialect, None);
        assert!(builtin(&ProviderId::OPENAI).is_none());
        let mut ids: Vec<_> = builtins().iter().map(|b| b.id.as_str()).collect();
        ids.dedup();
        assert_eq!(ids.len(), builtins().len());
    }
}
