//! The provider and model catalogue (spec 015, spec 068): an aggregate of peer sources —
//! models.dev, providers' own price lists, vizier's price table — each with a build-time seed,
//! merged into the in-memory view every query reads.

use std::sync::Arc;

use parking_lot::RwLock;
use serde::Serialize;

pub mod aggregate;
pub mod builtin;
pub mod cache;
pub mod data;
pub mod guide;
pub mod media;
pub mod media_prices;
pub mod price;
pub mod refresh;
pub mod seed;
pub mod source;
pub mod source_refresh;
pub mod sources;

pub use aggregate::Billing;
pub use price::{Price, PriceAnswer};
pub use source::{Authority, CatalogueSource, SourceId};

use builtin::{BuiltinProvider, CredentialField, builtins};
use data::{CatalogueData, CatalogueModel, CatalogueProvider, ModelStatus, ProviderKind};

use crate::config::provider::ProviderId;
use crate::llm::dialect::DialectKind;
use crate::schema::provider::{CustomProtocol, CustomProvider};

// Whether a catalogue refresh (models.dev, or a media capability's list) is on, running, or
// last failed. A plain comment: a doc comment would become the schema's description in the
// OpenAPI document.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RefreshStatus {
    pub enabled: bool,
    pub ttl: String,
    pub in_progress: bool,
    pub last_error: Option<String>,
    pub failed_at: Option<chrono::DateTime<chrono::Local>>,
}

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

/// The shared catalogue: an aggregate of peer sources (spec 068). Readers clone the merged
/// `Arc`, so a refresh's swap never makes them wait, and an agent keeps the data it was built
/// with until its next build.
#[derive(Clone)]
pub struct Catalogue(Arc<Inner>);

struct Inner {
    /// models.dev, also held typed: its refresh compares and caches its own data.
    models_dev: Arc<sources::models_dev::ModelsDev>,
    /// Every source in registration order (`contracts/catalogue-source.md` §2).
    sources: Vec<Arc<dyn CatalogueSource>>,
    /// The facts of every source, merged once per swap.
    merged: RwLock<Arc<CatalogueData>>,
    refresh: refresh::RefreshState,
    /// OpenRouter's media model lists (spec 028): the media pickers, not a price source.
    media: media::MediaCatalogue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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

/// A built-in's `model_source` when it is a second identity of a catalogue provider, else the id
/// itself. The single place the alias is applied; `models` reads through `provider` and the
/// resolver through the same two lookups (plan.md, risk "second ProviderId").
fn aliased(id: &ProviderId) -> ProviderId {
    builtin::model_source(id).unwrap_or_else(|| id.clone())
}

impl Catalogue {
    /// models.dev with `data` plus vizier's price table: what tests and tools that have only a
    /// models.dev snapshot build. The server uses [`Self::standard`].
    pub fn new(data: CatalogueData) -> Self {
        Self::with_sources(data, vec![Arc::new(sources::curated::Curated)])
    }

    /// Every shipped source, from its build-time seed. Never touches the network.
    pub fn standard(data: CatalogueData) -> Self {
        Self::with_sources(data, sources::standard())
    }

    /// models.dev with `data`, after `others` in registration order: authority decides
    /// precedence, and this order only breaks ties within one level.
    pub fn with_sources(data: CatalogueData, others: Vec<Arc<dyn CatalogueSource>>) -> Self {
        let models_dev = Arc::new(sources::models_dev::ModelsDev::new(data));
        let mut all = others;
        all.push(models_dev.clone());
        let merged = aggregate::merge(&all, &models_dev.raw());
        Self(Arc::new(Inner {
            models_dev,
            sources: all,
            merged: RwLock::new(Arc::new(merged)),
            refresh: Default::default(),
            // The media lists are seeded at build time, so a fresh or offline install lists
            // models without a request (spec 028 FR-020).
            media: media::MediaCatalogue::new(seed::load_media_seed()),
        }))
    }

    pub fn refresh_state(&self) -> &refresh::RefreshState {
        &self.0.refresh
    }

    /// OpenRouter's media model lists (spec 028).
    pub fn media(&self) -> &media::MediaCatalogue {
        &self.0.media
    }

    /// Startup: use each cached media list when it is newer than the seed, and restore the
    /// stored last failure. Never touches the network.
    pub async fn load_media_cache(&self, storage: &crate::storage::VizierStorage) {
        self.0.media.load_cache(storage).await;
    }

    /// Startup: every source adopts its stored snapshot when newer than its seed, then one
    /// merge. Never touches the network.
    pub async fn load_cached(&self, storage: &crate::storage::VizierStorage) {
        for s in &self.0.sources {
            s.load_cached(storage).await;
        }
        self.remerge();
    }

    /// One capability's current list; always available (the seed until a refresh).
    pub fn media_list(&self, capability: media::MediaCapability) -> media::MediaList {
        self.0.media.list(capability)
    }

    /// The merged view every reader uses.
    pub fn snapshot(&self) -> Arc<CatalogueData> {
        self.0.merged.read().clone()
    }

    /// models.dev's own data, not the merged view: what its refresh compares and caches.
    pub fn models_dev(&self) -> Arc<CatalogueData> {
        self.0.models_dev.raw()
    }

    /// Replace models.dev's data, then merge again.
    pub fn swap(&self, data: CatalogueData) {
        self.0.models_dev.set(data);
        self.remerge();
    }

    /// Merge every source's current snapshot; run after any source swaps its data.
    pub fn remerge(&self) {
        let merged = aggregate::merge(&self.0.sources, &self.0.models_dev.raw());
        *self.0.merged.write() = Arc::new(merged);
    }

    pub fn sources(&self) -> &[Arc<dyn CatalogueSource>] {
        &self.0.sources
    }

    /// How `id` is billed (research R7): a built-in says so. A catalogue provider that runs on
    /// localhost is local only while no source prices any of its models: a local proxy in front
    /// of a paid service (models.dev's `privatemode-ai`) publishes prices and stays metered.
    /// Anything else is metered.
    pub fn billing(&self, id: &ProviderId) -> Billing {
        let id = aliased(id);
        if let Some(b) = builtin::builtin(&id) {
            return b.billing;
        }
        match self.snapshot().providers.get(&id) {
            Some(p) if p.kind == ProviderKind::Local && p.models.values().all(|m| m.cost.is_none()) => {
                Billing::Local
            }
            _ => Billing::Metered,
        }
    }

    /// The price of one model's record and where it came from (contract §5).
    pub fn price(&self, provider: &ProviderId, q: &aggregate::Query<'_>) -> PriceAnswer {
        let provider = aliased(provider);
        let merged = self.snapshot();
        let billing = |p: &ProviderId| self.billing(p);
        let ctx = aggregate::WalkContext {
            sources: &self.0.sources,
            merged: &merged,
            billing: &billing,
        };
        aggregate::price(&ctx, &provider, q)
    }

    /// Every selectable provider: catalogue providers with a dialect vizier speaks (FR-008),
    /// the built-ins, and `custom`. A custom provider whose id equals a catalogue provider's
    /// hides it (spec edge case).
    pub fn providers(&self, custom: &[CustomProvider]) -> Vec<ProviderListing> {
        let data = self.snapshot();
        let mut out: Vec<ProviderListing> = custom.iter().map(custom_listing).collect();
        for b in builtins() {
            if !custom.iter().any(|c| c.id == b.id) {
                out.push(builtin_listing(b, &data));
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

    /// One catalogue entry. A built-in that is a second identity of a catalogue provider (the
    /// SSO Bedrock one, FR-030) answers with that provider's entry, so `models`, the pickers and
    /// the pricer all see one list rather than a copy.
    pub fn provider(&self, id: &ProviderId) -> Option<CatalogueProvider> {
        let id = aliased(id);
        self.snapshot().providers.get(&id).cloned()
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
        let provider = aliased(provider);
        self.snapshot()
            .providers
            .get(&provider)
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

/// Start a background check of every source that is due (spec 068 FR-011): models.dev through
/// its own conditional refresh, every other refreshing source through the shared policy, each
/// followed by one merge. Returns at once.
///
/// Call this only from request handlers serving a person: never from agent, scheduler or dream
/// code.
pub fn maybe_refresh(deps: &crate::dependencies::VizierDependencies) {
    refresh::maybe_refresh(deps);
    for source in deps.catalogue.sources() {
        let Some(state) = source.refresh_state() else {
            continue;
        };
        let data = source.data();
        if !state.is_due(&deps.config.catalogue, data.checked_at.unwrap_or(data.fetched_at))
            || !state.try_begin()
        {
            continue;
        }
        let deps = deps.clone();
        let source = source.clone();
        tokio::spawn(async move {
            if source.fetch(&deps).await.is_ok() {
                deps.catalogue.remerge();
            }
            if let Some(state) = source.refresh_state() {
                state.end();
            }
        });
    }
}

/// `POST /catalogue/refresh`: check every source now, ignoring TTLs and backoffs, waiting (up
/// to 30 s each) for a check already running. Answers with models.dev's result, as before spec
/// 068; another source's failure is in its own status (`contracts/http-api.md`).
pub async fn refresh_now(
    deps: &crate::dependencies::VizierDependencies,
) -> Result<(), refresh::RefreshError> {
    let others = async {
        if !deps.config.catalogue.refresh {
            return;
        }
        for source in deps.catalogue.sources() {
            let Some(state) = source.refresh_state() else {
                continue;
            };
            if !state.try_begin() {
                let _ = state.wait().await;
                continue;
            }
            if source.fetch(deps).await.is_ok() {
                deps.catalogue.remerge();
            }
            state.end();
        }
    };
    let (models_dev, ()) = tokio::join!(refresh::refresh_now(deps), others);
    models_dev
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
    // Bedrock signs requests with an access key pair and a region: no `api_key` at all.
    let fields = if p.dialect == Some(DialectKind::Bedrock) {
        vec![AccessKeyId, SecretAccessKey, SessionToken, Region]
    } else {
        let mut fields = vec![ApiKey];
        if p.dialect == Some(DialectKind::OpenAiChat) && p.npm == "@ai-sdk/azure" {
            fields.push(Endpoint);
        }
        if p.kind == ProviderKind::Local {
            fields.push(BaseUrl);
        }
        fields
    };
    // The catalogue lists only some of a provider's variables; the extra ones matter for the
    // connection state, the form and the resolver's fallback order.
    let mut env = p.env.clone();
    env.extend(
        crate::provider_keys::extra_env(&p.id)
            .iter()
            .map(|name| name.to_string()),
    );
    ProviderListing {
        id: p.id.clone(),
        name: p.name.clone(),
        origin: ProviderOrigin::Catalogue,
        kind: p.kind,
        dialect: p.dialect,
        model_count: p.models.len(),
        env,
        fields,
    }
}

fn builtin_listing(b: &BuiltinProvider, data: &CatalogueData) -> ProviderListing {
    ProviderListing {
        id: b.id.clone(),
        name: b.name.to_string(),
        origin: ProviderOrigin::Builtin,
        kind: b.kind,
        dialect: b.dialect,
        // An aliased built-in lists the models of the provider it serves (FR-030); every other
        // built-in has none of its own.
        model_count: b
            .model_source
            .as_ref()
            .and_then(|source| data.providers.get(source))
            .map_or(0, |p| p.models.len()),
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
          "bedrock": {"id": "amazon-bedrock", "name": "Amazon Bedrock", "npm": "@ai-sdk/amazon-bedrock", "models": {
            "anthropic.claude-x-v1:0": {"id": "anthropic.claude-x-v1:0", "tool_call": true, "limit": {"context": 200000}},
            "amazon.nova-lite-v1:0": {"id": "amazon.nova-lite-v1:0", "tool_call": true, "limit": {"context": 300000}}
          }},
          "galadriel": {"id": "galadriel", "name": "G", "npm": "some-unsupported-provider", "models": {}}
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
        let bedrock = listed
            .iter()
            .find(|p| p.id == ProviderId::AMAZON_BEDROCK)
            .expect("amazon-bedrock is listed");
        assert_eq!(bedrock.dialect, Some(DialectKind::Bedrock));
        assert_eq!(bedrock.model_count, 2);
        assert!(!listed.iter().any(|p| p.id.as_str() == "galadriel"));
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
    fn the_sso_id_is_listed_with_the_bedrock_models_and_resolves_through_them() {
        let c = catalogue();
        let sso = c
            .providers(&[])
            .into_iter()
            .find(|p| p.id == ProviderId::AMAZON_BEDROCK_SSO)
            .expect("the SSO Bedrock provider is listed");
        assert_eq!(sso.name, "Amazon Bedrock (SSO)");
        assert_eq!(sso.dialect, Some(DialectKind::Bedrock));
        assert_eq!(sso.kind, ProviderKind::Cloud);
        assert_eq!(sso.model_count, 2, "the same count as amazon-bedrock, not a copy");

        // FR-030: one catalogue entry answers under both ids, through every lookup.
        assert!(c.provider(&ProviderId::AMAZON_BEDROCK_SSO).is_some());
        let ids: Vec<_> = c
            .models(&ProviderId::AMAZON_BEDROCK_SSO, false)
            .into_iter()
            .map(|m| m.id)
            .collect();
        let shared_ids: Vec<_> = c
            .models(&ProviderId::AMAZON_BEDROCK, false)
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, shared_ids);
        let model = c
            .model(&ProviderId::AMAZON_BEDROCK_SSO, "amazon.nova-lite-v1:0")
            .expect("the SSO id finds the Bedrock model");
        assert_eq!(model.limit.context, Some(300000));
        assert!(c.model(&ProviderId::AMAZON_BEDROCK_SSO, "no-such-model").is_none());

        // A built-in with no alias is untouched.
        assert!(c.provider(&ProviderId::OLLAMA).is_none());
        assert_eq!(super::builtin::model_source(&ProviderId::OLLAMA), None);
        assert_eq!(
            super::builtin::model_source(&ProviderId::AMAZON_BEDROCK_SSO),
            Some(ProviderId::AMAZON_BEDROCK)
        );
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

    fn text(model: &str) -> aggregate::Query<'_> {
        aggregate::Query {
            model,
            kind: crate::schema::usage::UsageKind::Text,
            has_tokens: true,
            prefer: None,
        }
    }

    /// SC-005 (spec 068): models.dev is a peer, not the base. Without it, every model a
    /// provider's own list carries is still priced and described, by that list.
    #[test]
    fn without_models_dev_the_provider_lists_still_answer() {
        use crate::schema::usage::UsageKind;
        let c = Catalogue::with_sources(CatalogueData::empty(), sources::standard());
        let mut checked = 0;
        for source in c.sources().iter().filter(|s| s.id() != SourceId::MODELS_DEV) {
            for (pid, p) in &source.data().providers {
                for (id, facts) in p.models.iter().chain(p.priced.iter()) {
                    let kind = facts.kind.unwrap_or(UsageKind::Text);
                    let q = aggregate::Query {
                        model: id,
                        kind,
                        has_tokens: matches!(kind, UsageKind::Text | UsageKind::Embedding),
                        prefer: None,
                    };
                    let a = c.price(pid, &q);
                    if facts.price.as_ref().is_some_and(|pr| pr.covers(kind, q.has_tokens)) {
                        assert_eq!(a.source, source.id(), "{pid}/{id} is priced by its own list");
                        checked += 1;
                    }
                }
                for id in p.models.keys() {
                    assert!(c.model(pid, id).is_some(), "{pid}/{id} is described");
                }
            }
        }
        assert!(checked > 500, "{checked}");
    }

    /// US5 (FR-022, FR-023): a chat model only a provider's own list carries is in that
    /// provider's model list, with that list's limits, raises no "unknown model" warning, and
    /// resolves through models.dev's connection for the provider. A media model never lists.
    #[test]
    fn a_model_only_a_provider_list_knows_is_listed_and_resolves() {
        let c = Catalogue::standard(seed::load_seed());
        let venice = ProviderId::VENICE;
        let in_models_dev = c.models_dev().providers.get(&venice).map(|p| p.models.clone()).unwrap_or_default();
        let only = c
            .sources()
            .iter()
            .find(|s| s.id() == SourceId::VENICE)
            .unwrap()
            .data()
            .providers[&venice]
            .models
            .iter()
            .find(|(id, f)| !in_models_dev.contains_key(*id) && f.limit.context.is_some())
            .map(|(id, f)| (id.clone(), f.limit.context))
            .expect("the seed has a Venice-only chat model");
        let (id, context) = only;
        let listed = c.models(&venice, true);
        assert!(listed.iter().any(|m| m.id == id), "{id} is listed");
        assert_eq!(c.model(&venice, &id).unwrap().limit.context, context);
        assert!(
            !agent_warnings(&c, None, &agent_config("venice", &id, false))
                .iter()
                .any(|w| w.code == "unknown_model"),
            "{id} is known"
        );
        let p = c.provider(&venice).unwrap();
        let (dialect, api) = data::dialect_for(&p, p.models.get(&id)).expect("venice is speakable");
        assert_eq!(dialect, DialectKind::OpenAiChat);
        assert_eq!(api.as_deref(), Some("https://api.venice.ai/api/v1"));
        assert!(!listed.iter().any(|m| m.id == "venice-sd35"), "an image model is never a chat choice");
    }

    /// A size recorded as typed (`2k`) finds the tier priced as `2K`.
    #[test]
    fn a_lower_case_tier_finds_its_price() {
        use crate::schema::{metrics::TokenCounts, usage::{MediaUnits, UsageKind}};
        let price = Price {
            per_image: Some(media_prices::ImagePrice {
                default: 0.01,
                variants: [("2K".to_string(), 0.05)].into(),
            }),
            ..Default::default()
        };
        let one = MediaUnits { images: 1, ..Default::default() };
        let none = TokenCounts::default();
        assert_eq!(price.cost(UsageKind::Image, &none, &one, Some("2k"), false), Some(0.05));
    }

    /// US1 scenario 2: a model both its provider's own list and models.dev price takes the own
    /// list's price, and a different models.dev figure is kept as a disagreement (FR-006).
    #[test]
    fn a_providers_own_list_wins_over_models_dev() {
        let c = Catalogue::standard(seed::load_seed());
        let models_dev = c.models_dev();
        let Some(or) = models_dev.providers.get(&ProviderId::OPENROUTER) else {
            return;
        };
        let own = c.sources().iter().find(|s| s.id() == SourceId::OPENROUTER).unwrap().data();
        let own = &own.providers[&ProviderId::OPENROUTER];
        let mut both = 0;
        for (id, m) in &or.models {
            let (Some(md), Some(listed)) = (&m.cost, own.models.get(id).and_then(|f| f.price.as_ref())) else {
                continue;
            };
            let a = c.price(&ProviderId::OPENROUTER, &text(id));
            assert_eq!(a.source, SourceId::OPENROUTER, "{id}");
            let differs = listed.tokens.as_ref().is_some_and(|t| (t.input - md.input).abs() > 1e-9 || (t.output - md.output).abs() > 1e-9);
            if differs {
                assert!(
                    a.alternatives.iter().any(|(s, _)| *s == SourceId::MODELS_DEV),
                    "{id}: models.dev's different figure is kept"
                );
            }
            both += 1;
        }
        assert!(both > 50, "{both}");
    }
}

/// A setting that saves but probably isn't what was meant (FR-013, FR-035). Never blocks a
/// save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
