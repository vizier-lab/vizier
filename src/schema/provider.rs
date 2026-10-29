use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderId;

/// A provider's saved credentials, keyed by its id (`provider_config`).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ProviderEntry {
    pub id: ProviderId,
    pub credentials: ProviderCredentials,
}

/// A `provider_config` row as vizier stored it before spec 015: the legacy enum name and one
/// tagged variant per provider. Read only by `migrate_provider_ids_v1`, which rewrites every
/// row as a [`ProviderEntry`].
#[derive(Debug, Deserialize, Clone)]
pub struct LegacyProviderEntry {
    pub variant: String,
    pub config: LegacyProviderEntryConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum LegacyProviderEntryConfig {
    Ollama {
        base_url: String,
    },
    Openai {
        api_key: String,
    },
    Anthropic {
        api_key: String,
    },
    Deepseek {
        api_key: String,
    },
    Openrouter {
        api_key: String,
    },
    Gemini {
        api_key: String,
    },
    Mimo {
        api_key: String,
    },
    LlamaCpp {
        base_url: String,
    },
    Elevenlabs {
        api_key: String,
    },
    Groq {
        api_key: String,
    },
    Mistral {
        api_key: String,
    },
    Xai {
        api_key: String,
    },
    Perplexity {
        api_key: String,
    },
    Moonshot {
        api_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    Zai {
        api_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    Minimax {
        api_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    Together {
        api_key: String,
    },
    Cohere {
        api_key: String,
    },
    Huggingface {
        api_key: String,
    },
    Hyperbolic {
        api_key: String,
    },
    Voyageai {
        api_key: String,
    },
    Galadriel {
        api_key: String,
    },
    Mira {
        api_key: String,
    },
    Chatgpt {
        access_token: String,
        account_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    Copilot {
        api_key: String,
    },
    Azure {
        endpoint: String,
        api_key: String,
    },
    Custom {
        api_key: String,
        base_url: String,
    },
    OpencodeZen {
        api_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    OpencodeGo {
        api_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
}

/// A provider's stored credentials (data-model §4): one generic shape for every catalogue and
/// built-in provider, replacing the per-provider `ProviderEntryConfig` variants. Only the
/// fields a provider uses are accepted by the API.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, utoipa::ToSchema)]
pub struct ProviderCredentials {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Azure's resource endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Local providers only; refused for cloud providers (FR-027).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// ChatGPT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// ChatGPT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// AWS Bedrock access key id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,
    /// AWS Bedrock secret access key (encrypted at rest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
    /// AWS Bedrock temporary-credential session token (encrypted at rest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
    /// AWS region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

impl ProviderCredentials {
    pub fn is_empty(&self) -> bool {
        [
            &self.api_key,
            &self.endpoint,
            &self.base_url,
            &self.access_token,
            &self.account_id,
            &self.access_key_id,
            &self.secret_access_key,
            &self.session_token,
            &self.region,
        ]
        .iter()
        .all(|v| v.as_deref().is_none_or(str::is_empty))
    }

    /// `other`'s set fields over `self`'s.
    pub fn merged_with(&self, other: &ProviderCredentials) -> ProviderCredentials {
        let pick = |a: &Option<String>, b: &Option<String>| {
            b.clone().filter(|s| !s.is_empty()).or_else(|| a.clone())
        };
        ProviderCredentials {
            api_key: pick(&self.api_key, &other.api_key),
            endpoint: pick(&self.endpoint, &other.endpoint),
            base_url: pick(&self.base_url, &other.base_url),
            access_token: pick(&self.access_token, &other.access_token),
            account_id: pick(&self.account_id, &other.account_id),
            access_key_id: pick(&self.access_key_id, &other.access_key_id),
            secret_access_key: pick(&self.secret_access_key, &other.secret_access_key),
            session_token: pick(&self.session_token, &other.session_token),
            region: pick(&self.region, &other.region),
        }
    }
}

/// The wire protocol an operator-defined provider speaks.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CustomProtocol {
    OpenaiCompatible,
    Anthropic,
}

/// A model an operator listed for a custom provider; unset fields are filled from the
/// catalogue (FR-024).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, utoipa::ToSchema)]
pub struct CustomModel {
    pub id: String,
    #[serde(default)]
    pub context: Option<u64>,
    #[serde(default)]
    pub output: Option<u64>,
    #[serde(default)]
    pub tool_call: Option<bool>,
    #[serde(default)]
    pub image_input: Option<bool>,
    #[serde(default)]
    pub reasoning: Option<bool>,
    /// Set when the operator changed the row; a fetch never overwrites an edited row.
    #[serde(default)]
    pub edited: bool,
}

/// An operator-defined provider (data-model §5), stored in `custom_provider`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CustomProvider {
    pub id: ProviderId,
    pub name: String,
    pub protocol: CustomProtocol,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub models: Vec<CustomModel>,
    #[serde(default)]
    pub models_fetched_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(test)]
mod credential_tests {
    use super::*;

    #[test]
    fn bedrock_fields_take_part_in_is_empty_and_merge() {
        let empty = ProviderCredentials::default();
        assert!(empty.is_empty());
        let region_only = ProviderCredentials {
            region: Some("us-east-1".into()),
            ..Default::default()
        };
        assert!(!region_only.is_empty());

        let stored = ProviderCredentials {
            access_key_id: Some("AKIAOLD".into()),
            secret_access_key: Some("old-secret".into()),
            session_token: Some("old-token".into()),
            region: Some("us-east-1".into()),
            ..Default::default()
        };
        let update = ProviderCredentials {
            access_key_id: Some("AKIANEW".into()),
            // An empty value does not replace a stored one.
            secret_access_key: Some(String::new()),
            session_token: None,
            region: Some("eu-west-1".into()),
            ..Default::default()
        };
        let merged = stored.merged_with(&update);
        assert_eq!(merged.access_key_id.as_deref(), Some("AKIANEW"));
        assert_eq!(merged.secret_access_key.as_deref(), Some("old-secret"));
        assert_eq!(merged.session_token.as_deref(), Some("old-token"));
        assert_eq!(merged.region.as_deref(), Some("eu-west-1"));
    }

    #[test]
    fn records_saved_without_the_bedrock_fields_still_load() {
        let creds: ProviderCredentials =
            serde_json::from_value(serde_json::json!({"api_key": "k"})).unwrap();
        assert!(creds.access_key_id.is_none());
        assert_eq!(creds.api_key.as_deref(), Some("k"));
    }
}
