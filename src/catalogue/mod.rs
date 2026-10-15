//! The provider and model catalogue, from models.dev (spec 015): a seed embedded at build
//! time, a SQLite cache of lazy refreshes, and the in-memory copy every query reads.

use std::sync::Arc;

use parking_lot::RwLock;
use serde::Serialize;

pub mod builtin;
pub mod data;
pub mod seed;

use builtin::{BuiltinProvider, CredentialField, builtins};
use data::{CatalogueData, CatalogueModel, CatalogueProvider, ModelStatus, ProviderKind};

use crate::config::provider::ProviderId;
use crate::llm::dialect::DialectKind;
use crate::schema::provider::{CustomProtocol, CustomProvider};

/// The ids vizier supported before the catalogue, listed first among the unconnected
/// providers as "popular".
pub const POPULAR: &[&str] = &[
    "openai",
    "anthropic",
    "google",
    "openrouter",
    "deepseek",
    "ollama",
    "groq",
    "mistral",
    "xai",
    "github-copilot",
    "togetherai",
    "perplexity",
    "moonshotai",
    "zai",
    "minimax",
    "xiaomi",
    "cohere",
    "huggingface",
    "azure",
    "opencode",
    "opencode-go",
    "llama-cpp",
    "chatgpt",
];

/// The shared catalogue. Readers clone the inner `Arc`, so a refresh's `swap` never makes them
/// wait, and an agent keeps the data it was built with until its next build.
#[derive(Clone)]
pub struct Catalogue(Arc<RwLock<Arc<CatalogueData>>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOrigin {
    Catalogue,
    Builtin,
    Custom,
}

/// One selectable provider, as the pickers list it.
#[derive(Debug, Clone)]
pub struct ProviderListing {
    pub id: ProviderId,
    pub name: String,
    pub origin: ProviderOrigin,
    pub kind: ProviderKind,
    pub dialect: Option<DialectKind>,
    pub model_count: usize,
    pub env: Vec<String>,
    pub fields: Vec<CredentialField>,
}

impl Catalogue {
    pub fn new(data: CatalogueData) -> Self {
        Self(Arc::new(RwLock::new(Arc::new(data))))
    }

    pub fn snapshot(&self) -> Arc<CatalogueData> {
        self.0.read().clone()
    }

    pub fn swap(&self, data: CatalogueData) {
        *self.0.write() = Arc::new(data);
    }

    /// Every selectable provider: catalogue providers with a dialect vizier speaks (FR-008),
    /// the built-ins, and `custom`. A custom provider whose id equals a catalogue provider's
    /// hides it (spec edge case).
    pub fn providers(&self, custom: &[CustomProvider]) -> Vec<ProviderListing> {
        let data = self.snapshot();
        let mut out: Vec<ProviderListing> = custom.iter().map(custom_listing).collect();
        for b in builtins() {
            if !custom.iter().any(|c| c.id == b.id) {
                out.push(builtin_listing(b));
            }
        }
        for p in data.providers.values() {
            let hidden = custom.iter().any(|c| c.id == p.id) || builtin::builtin(&p.id).is_some();
            if !hidden && p.dialect.is_some() {
                out.push(catalogue_listing(p));
            }
        }
        out
    }

    pub fn provider(&self, id: &ProviderId) -> Option<CatalogueProvider> {
        self.snapshot().providers.get(id).cloned()
    }

    /// The FR-031 list: with `all`, every model newest first; otherwise only models that are
    /// not known to lack tool calling and are not deprecated. The first is the default.
    pub fn models(&self, provider: &ProviderId, all: bool) -> Vec<CatalogueModel> {
        let Some(p) = self.provider(provider) else {
            return vec![];
        };
        let mut models: Vec<CatalogueModel> = p
            .models
            .into_values()
            .filter(|m| all || (m.caps.tool_call != Some(false) && m.status != ModelStatus::Deprecated))
            .collect();
        sort_newest_first(&mut models);
        models
    }

    pub fn model(&self, provider: &ProviderId, model: &str) -> Option<CatalogueModel> {
        self.snapshot()
            .providers
            .get(provider)
            .and_then(|p| p.models.get(model).cloned())
    }

    /// The first catalogue model with this id, preferring a provider that speaks `prefer`
    /// (FR-024: a custom provider's model borrows its limits and capabilities).
    pub fn find_model_by_id(&self, model: &str, prefer: Option<DialectKind>) -> Option<CatalogueModel> {
        let data = self.snapshot();
        let mut fallback = None;
        for p in data.providers.values() {
            let Some(m) = p.models.get(model) else {
                continue;
            };
            let dialect = data::dialect_for(p, Some(m)).map(|(d, _)| d);
            if prefer.is_some() && dialect == prefer {
                return Some(m.clone());
            }
            fallback.get_or_insert_with(|| m.clone());
        }
        fallback
    }
}

pub fn sort_newest_first(models: &mut [CatalogueModel]) {
    models.sort_by(|a, b| {
        b.release_date
            .cmp(&a.release_date)
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn catalogue_listing(p: &CatalogueProvider) -> ProviderListing {
    use CredentialField::*;
    let mut fields = vec![ApiKey];
    if p.dialect == Some(DialectKind::OpenAiChat) && p.npm == "@ai-sdk/azure" {
        fields.push(Endpoint);
    }
    if p.kind == ProviderKind::Local {
        fields.push(BaseUrl);
    }
    ProviderListing {
        id: p.id.clone(),
        name: p.name.clone(),
        origin: ProviderOrigin::Catalogue,
        kind: p.kind,
        dialect: p.dialect,
        model_count: p.models.len(),
        env: p.env.clone(),
        fields,
    }
}

fn builtin_listing(b: &BuiltinProvider) -> ProviderListing {
    ProviderListing {
        id: b.id.clone(),
        name: b.name.to_string(),
        origin: ProviderOrigin::Builtin,
        kind: b.kind,
        dialect: b.dialect,
        model_count: 0,
        env: b.env.iter().map(|s| s.to_string()).collect(),
        fields: b.fields.to_vec(),
    }
}

fn custom_listing(c: &CustomProvider) -> ProviderListing {
    ProviderListing {
        id: c.id.clone(),
        name: c.name.clone(),
        origin: ProviderOrigin::Custom,
        kind: ProviderKind::Cloud,
        dialect: Some(match c.protocol {
            CustomProtocol::OpenaiCompatible => DialectKind::OpenAiChat,
            CustomProtocol::Anthropic => DialectKind::Anthropic,
        }),
        model_count: c.models.len(),
        env: vec![],
        fields: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Catalogue {
        Catalogue::new(
            data::parse(
                br#"{
          "acme": {"id": "acme", "name": "Acme", "npm": "@ai-sdk/openai-compatible", "api": "https://a/v1", "models": {
            "old": {"id": "old", "tool_call": true, "release_date": "2024-01-01"},
            "new": {"id": "new", "tool_call": true, "release_date": "2025-06-01"},
            "notools": {"id": "notools", "tool_call": false, "release_date": "2025-07-01"},
            "gone": {"id": "gone", "tool_call": true, "status": "deprecated", "release_date": "2025-08-01"},
            "shared": {"id": "shared", "limit": {"context": 1}}
          }},
          "claudeish": {"id": "claudeish", "name": "C", "npm": "@ai-sdk/anthropic", "api": "https://c/v1", "models": {
            "shared": {"id": "shared", "limit": {"context": 2}}
          }},
          "bedrock": {"id": "bedrock", "name": "B", "npm": "@ai-sdk/amazon-bedrock", "models": {}}
        }"#,
            )
            .unwrap(),
        )
    }

    fn id(s: &str) -> ProviderId {
        ProviderId::new(s.to_string()).unwrap()
    }

    #[test]
    fn the_default_model_list_is_tool_capable_current_and_newest_first() {
        let c = catalogue();
        let ids: Vec<_> = c.models(&id("acme"), false).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["new", "old", "shared"]);
        let all: Vec<_> = c.models(&id("acme"), true).into_iter().map(|m| m.id).collect();
        assert_eq!(all, vec!["gone", "notools", "new", "old", "shared"]);
    }

    #[test]
    fn providers_hide_unsupported_and_custom_wins_a_collision() {
        let c = catalogue();
        let listed = c.providers(&[]);
        assert!(listed.iter().any(|p| p.id.as_str() == "acme"));
        assert!(!listed.iter().any(|p| p.id.as_str() == "bedrock"));
        assert!(listed.iter().any(|p| p.id == ProviderId::DUMMYPLUG));

        let custom = CustomProvider {
            id: id("acme"),
            name: "My Acme".into(),
            protocol: CustomProtocol::OpenaiCompatible,
            base_url: "http://x/v1".into(),
            api_key: None,
            headers: Default::default(),
            models: vec![],
            models_fetched_at: None,
        };
        let listed = c.providers(&[custom]);
        let acme: Vec<_> = listed.iter().filter(|p| p.id.as_str() == "acme").collect();
        assert_eq!(acme.len(), 1);
        assert_eq!(acme[0].origin, ProviderOrigin::Custom);
    }

    #[test]
    fn find_model_by_id_prefers_the_matching_dialect() {
        let c = catalogue();
        assert_eq!(
            c.find_model_by_id("shared", Some(DialectKind::Anthropic)).unwrap().limit.context,
            Some(2)
        );
        assert_eq!(
            c.find_model_by_id("shared", Some(DialectKind::OpenAiChat)).unwrap().limit.context,
            Some(1)
        );
        assert!(c.find_model_by_id("missing", None).is_none());
    }

    #[test]
    fn swap_replaces_what_readers_see() {
        let c = catalogue();
        let before = c.snapshot();
        c.swap(CatalogueData::empty());
        assert!(c.snapshot().providers.is_empty());
        assert!(!before.providers.is_empty(), "an old snapshot is unchanged");
    }
}
