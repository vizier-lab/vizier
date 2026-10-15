//! The provider and model catalogue, from models.dev (spec 015): a seed embedded at build
//! time, a SQLite cache of lazy refreshes, and the in-memory copy every query reads.

use std::sync::Arc;

use parking_lot::RwLock;
use serde::Serialize;

pub mod builtin;
pub mod cache;
pub mod data;
pub mod refresh;
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
pub struct Catalogue(Arc<RwLock<Arc<CatalogueData>>>, Arc<refresh::RefreshState>);

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
        Self(Arc::new(RwLock::new(Arc::new(data))), Arc::default())
    }

    pub fn refresh_state(&self) -> &refresh::RefreshState {
        &self.1
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

    fn agent_config(provider: &str, model: &str, read_image: bool) -> crate::schema::AgentConfig {
        serde_json::from_value(serde_json::json!({
            "name": "a", "system_prompt": null, "description": null,
            "provider": provider, "model": model, "thinking_depth": 5,
            "tools": {"timeout": "30s", "read_image": {"enabled": read_image, "settings": {}}},
            "silent_read_initiative_chance": 0.0, "prompt_timeout": "60s", "heartbeat_interval": "60s"
        }))
        .unwrap()
    }

    #[test]
    fn agent_warnings_flag_tools_vision_and_unknown_models() {
        let c = catalogue();
        let codes = |cfg| -> Vec<&'static str> {
            agent_warnings(&c, None, &cfg).into_iter().map(|w| w.code).collect()
        };
        assert!(codes(agent_config("acme", "new", true)).is_empty(), "image input unknown counts as supported");
        assert_eq!(codes(agent_config("acme", "notools", false)), vec!["no_tool_calling"]);
        assert_eq!(codes(agent_config("acme", "my-unlisted", false)), vec!["unknown_model"]);
        assert!(codes(agent_config("dummyplug", "dummyplug", false)).is_empty(), "built-ins list live");

        let text_only = Catalogue::new(
            data::parse(br#"{"acme": {"id": "acme", "name": "A", "npm": "@ai-sdk/openai-compatible", "api": "https://a/v1",
                "models": {"t": {"id": "t", "tool_call": true, "modalities": {"input": ["text"]}}}}}"#)
            .unwrap(),
        );
        let w = agent_warnings(&text_only, None, &agent_config("acme", "t", true));
        assert_eq!(w.iter().map(|w| w.code).collect::<Vec<_>>(), vec!["no_image_input"]);
        assert!(agent_warnings(&text_only, None, &agent_config("acme", "t", false)).is_empty());
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

/// A setting that saves but probably isn't what was meant (FR-013, FR-035). Never blocks a
/// save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct AgentWarning {
    pub code: &'static str,
    pub message: String,
}

/// The warnings for an agent's chosen model, from what the catalogue (or the custom
/// provider's own row) says about it.
pub fn agent_warnings(
    catalogue: &Catalogue,
    custom: Option<&CustomProvider>,
    config: &crate::schema::AgentConfig,
) -> Vec<AgentWarning> {
    let mut out = vec![];
    let (tool_call, image_input, known) = match custom.filter(|c| c.id == config.provider) {
        Some(c) => {
            let row = c.models.iter().find(|m| m.id == config.model);
            let borrowed = catalogue.find_model_by_id(&config.model, None);
            (
                row.and_then(|r| r.tool_call)
                    .or_else(|| borrowed.as_ref().and_then(|m| m.caps.tool_call)),
                row.and_then(|r| r.image_input)
                    .or_else(|| borrowed.as_ref().and_then(|m| m.caps.image_input())),
                true,
            )
        }
        None => match catalogue.model(&config.provider, &config.model) {
            Some(m) => (m.caps.tool_call, m.caps.image_input(), true),
            // Built-ins list their models live; there is nothing to check them against.
            None => (None, None, builtin::builtin(&config.provider).is_some()),
        },
    };
    if tool_call == Some(false) {
        out.push(AgentWarning {
            code: "no_tool_calling",
            message: format!(
                "{} can't call tools; the agent will only be able to answer in text",
                config.model
            ),
        });
    }
    let read_image = &config.tools.read_image;
    let explicit_vision = read_image.settings.provider.is_some()
        && read_image.settings.model.as_deref().is_some_and(|m| !m.trim().is_empty());
    if read_image.enabled && !explicit_vision && image_input == Some(false) {
        out.push(AgentWarning {
            code: "no_image_input",
            message: format!(
                "{} can't view images; configure a vision model for read-image",
                config.model
            ),
        });
    }
    if !known {
        out.push(AgentWarning {
            code: "unknown_model",
            message: format!(
                "{} isn't in the catalogue for {}; it will be used as typed",
                config.model, config.provider
            ),
        });
    }
    out
}
