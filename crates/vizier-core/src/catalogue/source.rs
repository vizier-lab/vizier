//! The catalogue-source seam (spec 068, `contracts/catalogue-source.md`). The catalogue is an
//! aggregate of peer sources: models.dev is one of them, with no special place. A new source
//! is one implementation of [`CatalogueSource`] plus one line in [`super::Catalogue::standard`].

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::NaiveDate;

use super::data::{Capabilities, Limits, ModelStatus, ProviderKind, RouteOverride, SnapshotOrigin};
use super::price::Price;
use super::source_refresh::SourceRefresh;
use crate::config::provider::ProviderId;
use crate::dependencies::VizierDependencies;
use crate::llm::dialect::DialectKind;
use crate::schema::usage::UsageKind;
use crate::storage::VizierStorage;
use crate::time::Timestamp;

/// A source's stable id, which is also its wire label (`price_source`). Rule and fallback
/// labels share the namespace but are not sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(pub &'static str);

impl SourceId {
    pub const MODELS_DEV: Self = Self("models_dev");
    pub const OPENROUTER: Self = Self("openrouter");
    pub const VENICE: Self = Self("venice");
    pub const CURATED: Self = Self("curated");
    pub const LOCAL: Self = Self("local");
    pub const SUBSCRIPTION: Self = Self("subscription");
    pub const INFERENCE_PROFILE: Self = Self("inference_profile");
    pub const BORROWED: Self = Self("borrowed");
    pub const NONE: Self = Self("none");

    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// What a source is for one provider, strongest first. Precedence follows this order; the
/// registration order only breaks ties within one level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Authority {
    /// The provider's own published list.
    Own,
    /// vizier's hand-checked figures.
    Curated,
    /// A catalogue of many providers (models.dev).
    General,
    /// The source says nothing about this provider.
    None,
}

/// How vizier connects to a provider (FR-002): kept apart from what its models cost.
#[derive(Debug, Clone)]
pub struct ProviderConnection {
    pub name: String,
    pub env: Vec<String>,
    pub api: Option<String>,
    pub npm: String,
    pub doc: Option<String>,
    pub dialect: Option<DialectKind>,
    pub kind: ProviderKind,
}

/// What one source says about one model. Every field is optional: absent is "this source does
/// not say", never "false" or "zero".
#[derive(Debug, Clone, Default)]
pub struct ModelFacts {
    pub name: Option<String>,
    pub kind: Option<UsageKind>,
    pub caps: Capabilities,
    pub limit: Limits,
    pub price: Option<Price>,
    pub status: Option<ModelStatus>,
    pub release_date: Option<NaiveDate>,
    pub family: Option<String>,
    pub knowledge: Option<String>,
    /// The maker, from models.dev's `canonical_model_id` prefix (borrowing prefers it).
    pub lab: Option<String>,
    pub route: Option<RouteOverride>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceProvider {
    pub connection: Option<ProviderConnection>,
    /// Models the catalogue lists (pickers, the resolver, agent warnings): merged across sources.
    pub models: BTreeMap<String, ModelFacts>,
    /// Models this source only prices (a provider list's image, speech and transcription
    /// models): asked for prices, never listed, so a media model never lands in a chat picker.
    pub priced: BTreeMap<String, ModelFacts>,
}

impl SourceProvider {
    /// A model's facts, listed or priced-only.
    pub fn facts(&self, model: &str) -> Option<&ModelFacts> {
        self.models.get(model).or_else(|| self.priced.get(model))
    }
}

/// One source's snapshot, already mapped onto vizier's ids (FR-003).
#[derive(Debug, Clone)]
pub struct SourceData {
    pub origin: SnapshotOrigin,
    pub fetched_at: Timestamp,
    pub checked_at: Option<Timestamp>,
    pub providers: BTreeMap<ProviderId, SourceProvider>,
    /// Upstream entries dropped because their names did not map.
    pub unmapped: u32,
}

impl SourceData {
    pub fn empty() -> Self {
        Self {
            origin: SnapshotOrigin::Bundled,
            fetched_at: crate::time::epoch(),
            checked_at: None,
            providers: BTreeMap::new(),
            unmapped: 0,
        }
    }

    pub fn model_count(&self) -> usize {
        self.providers
            .values()
            .map(|p| p.models.len() + p.priced.len())
            .sum()
    }
}

/// One peer source of provider and model facts.
#[async_trait::async_trait]
pub trait CatalogueSource: Send + Sync {
    /// Stable id, also the wire label. Never changes once shipped.
    fn id(&self) -> SourceId;
    /// Display name.
    fn name(&self) -> &'static str;
    /// What this source is for `provider`. Pure.
    fn authority(&self, provider: &ProviderId) -> Authority;
    /// The current snapshot (the seed until a refresh). Cheap: clones an `Arc`.
    fn data(&self) -> Arc<SourceData>;

    /// The price this source gives `(provider, model)` for a record of `kind`. The default
    /// reads the model's own price; the walk then keeps it only when it can price the record
    /// ([`Price::covers`]), so a token price still prices a media call that reported tokens, as
    /// before spec 068.
    fn price(&self, provider: &ProviderId, model: &str, _kind: UsageKind) -> Option<Price> {
        let data = self.data();
        data.providers.get(provider)?.facts(model)?.price.clone()
    }

    /// Refresh state; `None` for a source that never refreshes (the shipped table).
    fn refresh_state(&self) -> Option<&SourceRefresh> {
        None
    }

    /// Startup: adopt a stored snapshot newer than the seed. Never touches the network.
    async fn load_cached(&self, _storage: &VizierStorage) {}

    /// Fetch upstream, normalize, store and swap the snapshot. Called only by the catalogue's
    /// refresh, which re-merges afterwards. `Err` keeps the current snapshot.
    async fn fetch(&self, _deps: &VizierDependencies) -> Result<(), String> {
        Ok(())
    }
}
