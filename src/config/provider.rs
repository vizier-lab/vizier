use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// A provider's id: a catalogue provider (`anthropic`, `google`, …), a built-in (`ollama`,
/// `dummyplug`, …) or an operator-defined custom provider. Replaces the `ProviderVariant` enum
/// (data-model §1).
///
/// Format `^[a-z0-9][a-z0-9._-]{0,63}$`. Serialised as a plain string; deserialising maps the
/// legacy enum names (`gemini` → `google`, `opencode_zen` → `opencode`, …) through
/// [`ProviderId::from_legacy`], so old agent JSON, provider rows and API clients keep working.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderId(Cow<'static, str>);

impl ProviderId {
    pub const OPENAI: ProviderId = ProviderId::new_static("openai");
    pub const ANTHROPIC: ProviderId = ProviderId::new_static("anthropic");
    pub const GOOGLE: ProviderId = ProviderId::new_static("google");
    pub const OPENROUTER: ProviderId = ProviderId::new_static("openrouter");
    pub const DEEPSEEK: ProviderId = ProviderId::new_static("deepseek");
    pub const GROQ: ProviderId = ProviderId::new_static("groq");
    pub const MISTRAL: ProviderId = ProviderId::new_static("mistral");
    pub const XAI: ProviderId = ProviderId::new_static("xai");
    pub const TOGETHERAI: ProviderId = ProviderId::new_static("togetherai");
    pub const COHERE: ProviderId = ProviderId::new_static("cohere");
    pub const HUGGINGFACE: ProviderId = ProviderId::new_static("huggingface");
    pub const GITHUB_COPILOT: ProviderId = ProviderId::new_static("github-copilot");
    pub const AZURE: ProviderId = ProviderId::new_static("azure");
    pub const OPENCODE_GO: ProviderId = ProviderId::new_static("opencode-go");
    pub const OLLAMA: ProviderId = ProviderId::new_static("ollama");
    pub const LLAMA_CPP: ProviderId = ProviderId::new_static("llama-cpp");
    pub const CHATGPT: ProviderId = ProviderId::new_static("chatgpt");
    pub const DUMMYPLUG: ProviderId = ProviderId::new_static("dummyplug");
    pub const ELEVENLABS: ProviderId = ProviderId::new_static("elevenlabs");
    pub const VOYAGEAI: ProviderId = ProviderId::new_static("voyageai");
    pub const HYPERBOLIC: ProviderId = ProviderId::new_static("hyperbolic");
    pub const CUSTOM: ProviderId = ProviderId::new_static("custom");

    /// For ids known to be valid at compile time.
    pub const fn new_static(id: &'static str) -> Self {
        ProviderId(Cow::Borrowed(id))
    }

    /// A validated id. Legacy names are *not* mapped here; use [`ProviderId::parse`] for input.
    pub fn new(id: impl Into<String>) -> crate::Result<Self> {
        let id = id.into();
        if Self::is_valid(&id) {
            Ok(ProviderId(Cow::Owned(id)))
        } else {
            Err(crate::VizierError(format!(
                "invalid provider id {id:?}: use 1-64 lowercase letters, digits, '.', '_' or \
                 '-', starting with a letter or digit"
            )))
        }
    }

    /// Input from a person, a stored row or an old client: legacy names are mapped, then the
    /// result is validated.
    pub fn parse(id: &str) -> crate::Result<Self> {
        match Self::from_legacy(id) {
            Some(mapped) => Ok(mapped),
            None => Self::new(id),
        }
    }

    pub fn is_valid(id: &str) -> bool {
        let bytes = id.as_bytes();
        !bytes.is_empty()
            && bytes.len() <= 64
            && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
            && bytes.iter().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            })
    }

    /// The legacy `ProviderVariant` name → new id map (contract `provider-id-migration.md`).
    /// Names that are unchanged, and `galadriel`/`mira` (kept as given, then refused at build),
    /// return `None`.
    pub fn from_legacy(name: &str) -> Option<Self> {
        let mapped = match name {
            "gemini" => "google",
            "mimo" => "xiaomi",
            "moonshot" => "moonshotai",
            "together" => "togetherai",
            "copilot" => "github-copilot",
            "opencode_zen" => "opencode",
            "opencode_go" => "opencode-go",
            "llama_cpp" => "llama-cpp",
            _ => return None,
        };
        Some(ProviderId::new_static(mapped))
    }

    /// The legacy name this id had, for the ids that were renamed.
    pub fn legacy_name(&self) -> Option<&'static str> {
        Some(match self.as_str() {
            "google" => "gemini",
            "xiaomi" => "mimo",
            "moonshotai" => "moonshot",
            "togetherai" => "together",
            "github-copilot" => "copilot",
            "opencode" => "opencode_zen",
            "opencode-go" => "opencode_go",
            "llama-cpp" => "llama_cpp",
            _ => return None,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ProviderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for ProviderId {
    type Err = crate::VizierError;
    fn from_str(s: &str) -> crate::Result<Self> {
        Self::parse(s)
    }
}

impl Serialize for ProviderId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ProviderId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        ProviderId::parse(&raw).map_err(|e| serde::de::Error::custom(e.0))
    }
}

impl utoipa::PartialSchema for ProviderId {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::String)
            .description(Some(
                "A provider id: a catalogue provider, a built-in or a custom provider",
            ))
            .into()
    }
}

impl utoipa::ToSchema for ProviderId {}

impl schemars::JsonSchema for ProviderId {
    fn schema_name() -> Cow<'static, str> {
        "ProviderId".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(generator)
    }
}

#[cfg(test)]
mod provider_id_tests {
    use super::*;

    #[test]
    fn every_legacy_name_maps_as_the_contract_says() {
        let table = [
            ("openai", "openai"),
            ("anthropic", "anthropic"),
            ("openrouter", "openrouter"),
            ("deepseek", "deepseek"),
            ("gemini", "google"),
            ("mimo", "xiaomi"),
            ("groq", "groq"),
            ("mistral", "mistral"),
            ("xai", "xai"),
            ("perplexity", "perplexity"),
            ("moonshot", "moonshotai"),
            ("zai", "zai"),
            ("minimax", "minimax"),
            ("together", "togetherai"),
            ("cohere", "cohere"),
            ("huggingface", "huggingface"),
            ("copilot", "github-copilot"),
            ("azure", "azure"),
            ("opencode_zen", "opencode"),
            ("opencode_go", "opencode-go"),
            ("ollama", "ollama"),
            ("llama_cpp", "llama-cpp"),
            ("chatgpt", "chatgpt"),
            ("dummyplug", "dummyplug"),
            ("elevenlabs", "elevenlabs"),
            ("voyageai", "voyageai"),
            ("hyperbolic", "hyperbolic"),
            ("custom", "custom"),
            // Removed providers are kept as given, then refused when an agent is built.
            ("galadriel", "galadriel"),
            ("mira", "mira"),
        ];
        for (legacy, new) in table {
            let parsed: ProviderId = serde_json::from_value(serde_json::json!(legacy)).unwrap();
            assert_eq!(parsed.as_str(), new, "{legacy}");
            if legacy != new {
                assert_eq!(parsed.legacy_name(), Some(legacy));
            }
        }
    }

    #[test]
    fn serialises_as_a_plain_string() {
        assert_eq!(
            serde_json::to_value(ProviderId::GOOGLE).unwrap(),
            serde_json::json!("google")
        );
    }

    #[test]
    fn refuses_invalid_ids() {
        for bad in ["", "Upper", "-lead", "has space", "a/b", &"x".repeat(65)] {
            assert!(ProviderId::parse(bad).is_err(), "{bad:?}");
            assert!(serde_json::from_value::<ProviderId>(serde_json::json!(bad)).is_err());
        }
        for good in ["a", "302ai", "moonshotai-cn", "llama-cpp", "x.y_z", &"x".repeat(64)] {
            assert!(ProviderId::parse(good).is_ok(), "{good:?}");
        }
    }
}
