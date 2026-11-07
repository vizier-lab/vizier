use serde::{Deserialize, Serialize};

use crate::catalogue::data::Cost;
use crate::config::provider::ProviderId;

/// A provider's saved credentials, keyed by its id (`provider_config`).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
    // ---- Amazon Bedrock (SSO): the non-secret half (spec 044, data-model "SSO connection").
    // The session itself is a separate encrypted row (`bedrock_sso_session`), never a field here.
    /// The Identity Center start URL the person's organisation gave them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_url: Option<String>,
    /// The region the Identity Center instance lives in — not the Bedrock region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sso_region: Option<String>,
    /// The role chosen from the portal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    /// The non-secret handle the stored session is keyed by (FR-009e).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sso_profile: Option<String>,
    /// The saved Application Inference Profile entries, each with its optional pricing
    /// pairing (FR-020, FR-034a).
    #[serde(default)]
    pub profiles: Vec<ProfileEntry>,
}

impl ProviderCredentials {
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
            && [
                &self.api_key,
                &self.endpoint,
                &self.base_url,
                &self.access_token,
                &self.account_id,
                &self.access_key_id,
                &self.secret_access_key,
                &self.session_token,
                &self.region,
                &self.start_url,
                &self.sso_region,
                &self.role_name,
                &self.sso_profile,
            ]
            .iter()
            .all(|v| v.as_deref().is_none_or(str::is_empty))
    }

    /// `other`'s set fields over `self`'s. The inference-profile entry list is **not** taken from
    /// `other`: an entry list cannot tell "omitted" from "emptied", so the caller decides it with
    /// [`merged_with_profiles`](Self::merged_with_profiles). This keeps the environment and
    /// test-overlay callers (which never carry entries) from wiping a saved list.
    pub fn merged_with(&self, other: &ProviderCredentials) -> ProviderCredentials {
        self.merged_with_profiles(other, None)
    }

    /// [`merged_with`](Self::merged_with) with the entry list decided by the caller: `None` keeps
    /// the stored list, `Some(list)` replaces it — `Some(vec![])` included, which **clears** it.
    /// That is what `PUT /providers/{id} {"profiles": []}` means: an emptied list, not an omitted
    /// one (`contracts/provider-credentials-api.md` §4, FR-021).
    pub fn merged_with_profiles(
        &self,
        other: &ProviderCredentials,
        profiles: Option<Vec<ProfileEntry>>,
    ) -> ProviderCredentials {
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
            start_url: pick(&self.start_url, &other.start_url),
            sso_region: pick(&self.sso_region, &other.sso_region),
            role_name: pick(&self.role_name, &other.role_name),
            sso_profile: pick(&self.sso_profile, &other.sso_profile),
            // One operation, decided by the caller rather than guessed from the list: omitted
            // keeps the stored entries, an empty list clears them (FR-021).
            profiles: profiles.unwrap_or_else(|| self.profiles.clone()),
        }
    }
}

/// One Application Inference Profile a connection may chat through (spec 044, data-model
/// "profile entry"). Non-secret: the ARN as saved, an optional display name and optional
/// limits, plus the optional pricing pairing (`model` + `price`, FR-034a).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProfileEntry {
    /// The ARN exactly as saved; sent to the runtime unchanged (FR-027).
    pub arn: String,
    /// Shown instead of the profile id when set (FR-026).
    #[serde(default)]
    pub name: Option<String>,
    /// Context window; unset is unknown, never "unsupported" (FR-028).
    #[serde(default)]
    pub context: Option<u64>,
    /// Output limit; unset is unknown.
    #[serde(default)]
    pub output: Option<u64>,
    /// Whether the profile takes tools; unset is unknown.
    #[serde(default)]
    pub tool_call: Option<bool>,
    /// Whether the profile takes images; unset is unknown.
    #[serde(default)]
    pub image_input: Option<bool>,
    /// The pricing pairing: one catalogue Bedrock model id, re-resolved on every read so a
    /// catalogue refresh reprices the entry and an id that has left the catalogue stays saved
    /// (FR-034a.1, FR-034a.3).
    #[serde(default)]
    pub model: Option<String>,
    /// A hand-entered cost, in the catalogue's per-million rates; set means no catalogue lookup
    /// happens and a refresh never overwrites it (FR-034a.4).
    #[serde(default)]
    pub price: Option<Cost>,
    /// Which of the two limits the person set by hand, so a re-pairing refreshes only the others
    /// (FR-034a.7). **Server-owned**: recomputed on every write from the stored entry and the
    /// submitted numbers, and whatever the client sends here is ignored — that is what makes
    /// echoing a displayed value back a no-op. `None` on an entry saved before this rule reads as
    /// "true wherever a number is present", so no old number is mistaken for a fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits_edited: Option<LimitsEdited>,
    /// Set once a person changed the row, so a future fetch never overwrites it (the
    /// `CustomModel.edited` convention).
    #[serde(default)]
    pub edited: bool,
}

/// Which of an entry's two limits the person set by hand (FR-034a.7 rule 1). Two bits, written by
/// the server on every entry write: the numbers the person typed are never overwritten by the
/// limit fill, and the ones they did not type are the fill's to refresh when the pair changes.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct LimitsEdited {
    /// The context window is the person's, not the paired model's.
    pub context: bool,
    /// The output limit is the person's, not the paired model's.
    pub output: bool,
}

/// The wire protocol an operator-defined provider speaks.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CustomProtocol {
    OpenaiCompatible,
    Anthropic,
}

/// A model an operator listed for a custom provider; unset fields are filled from the
/// catalogue (FR-024).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
    pub models_fetched_at: Option<chrono::DateTime<chrono::Local>>,
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
    fn an_omitted_entry_list_keeps_the_stored_entries_and_an_empty_one_clears_them() {
        let entry = |arn: &str| ProfileEntry {
            arn: arn.into(),
            ..Default::default()
        };
        let stored = ProviderCredentials {
            sso_profile: Some("acme".into()),
            profiles: vec![entry("arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/a")],
            ..Default::default()
        };
        let other = ProviderCredentials {
            profiles: vec![entry("arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/b")],
            ..Default::default()
        };

        // Omitted keeps; `[]` clears; a list replaces.
        assert_eq!(stored.merged_with_profiles(&other, None).profiles, stored.profiles);
        assert!(stored.merged_with_profiles(&other, Some(Vec::new())).profiles.is_empty());
        assert_eq!(
            stored.merged_with_profiles(&other, Some(other.profiles.clone())).profiles,
            other.profiles
        );
        // The plain merge never guesses from the list, so an overlay without entries cannot wipe it.
        assert_eq!(stored.merged_with(&ProviderCredentials::default()).profiles, stored.profiles);
        assert_eq!(stored.merged_with(&other).profiles, stored.profiles);
    }

    #[test]
    fn records_saved_without_the_bedrock_fields_still_load() {
        let creds: ProviderCredentials =
            serde_json::from_value(serde_json::json!({"api_key": "k"})).unwrap();
        assert!(creds.access_key_id.is_none());
        assert_eq!(creds.api_key.as_deref(), Some("k"));
    }
}
