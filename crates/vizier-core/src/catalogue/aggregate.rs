//! The aggregate (spec 068, `contracts/catalogue-source.md` §4–5): facts merged across sources
//! once per swap, and prices answered on demand by walking the sources in authority order.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::data::{CatalogueData, CatalogueModel, CatalogueProvider, ModelStatus, ProviderKind};
use super::price::{Price, PriceAnswer};
use super::source::{Authority, CatalogueSource, ModelFacts, SourceData, SourceId};
use crate::config::provider::ProviderId;
use crate::llm::dialect::DialectKind;
use crate::schema::usage::UsageKind;

/// How a provider is billed: asked before any source (research R7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Billing {
    #[default]
    Metered,
    /// Runs on the person's machine: free.
    Local,
    /// Covered by a flat subscription: free per call.
    Subscription,
}

/// Up to this many disagreeing prices are kept (FR-006).
const MAX_ALTERNATIVES: usize = 3;

fn fill<T: Clone>(into: &mut Option<T>, from: &Option<T>) {
    if into.is_none() {
        *into = from.clone();
    }
}

/// Fill every fact `into` lacks from `from`. Capabilities merge per field.
fn fill_facts(into: &mut ModelFacts, from: &ModelFacts) {
    fill(&mut into.name, &from.name);
    fill(&mut into.kind, &from.kind);
    fill(&mut into.caps.tool_call, &from.caps.tool_call);
    fill(&mut into.caps.reasoning, &from.caps.reasoning);
    fill(&mut into.caps.attachment, &from.caps.attachment);
    fill(&mut into.caps.temperature, &from.caps.temperature);
    fill(&mut into.caps.image_input_override, &from.caps.image_input_override);
    if into.caps.input.is_empty() {
        into.caps.input = from.caps.input.clone();
    }
    if into.caps.output.is_empty() {
        into.caps.output = from.caps.output.clone();
    }
    fill(&mut into.limit.context, &from.limit.context);
    fill(&mut into.limit.input, &from.limit.input);
    fill(&mut into.limit.output, &from.limit.output);
    fill(&mut into.price, &from.price);
    fill(&mut into.status, &from.status);
    fill(&mut into.release_date, &from.release_date);
    fill(&mut into.family, &from.family);
    fill(&mut into.knowledge, &from.knowledge);
    fill(&mut into.lab, &from.lab);
    fill(&mut into.route, &from.route);
}

fn to_model(id: &str, facts: ModelFacts) -> CatalogueModel {
    CatalogueModel {
        id: id.to_string(),
        name: facts.name.unwrap_or_else(|| id.to_string()),
        family: facts.family,
        caps: facts.caps,
        limit: facts.limit,
        cost: facts.price.and_then(|p| p.tokens),
        status: facts.status.unwrap_or(ModelStatus::Active),
        release_date: facts.release_date,
        knowledge: facts.knowledge,
        lab: facts.lab,
        route: facts.route,
        kind: facts.kind,
    }
}

/// The sources that speak for `provider`, strongest first, registration order breaking ties.
fn ranked<'a>(
    sources: &'a [Arc<dyn CatalogueSource>],
    provider: &ProviderId,
) -> Vec<&'a Arc<dyn CatalogueSource>> {
    let mut out: Vec<(Authority, usize, &Arc<dyn CatalogueSource>)> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| (s.authority(provider), i, s))
        .filter(|(a, _, _)| *a != Authority::None)
        .collect();
    out.sort_by_key(|(a, i, _)| (*a, *i));
    out.into_iter().map(|(_, _, s)| s).collect()
}

/// Merge every source's listed models into one `CatalogueData` (contract §4). `meta` carries the
/// snapshot fields the status line reports (models.dev's), and the provider list is every provider
/// any source describes. Never fails: a source with nothing contributes nothing.
pub fn merge(sources: &[Arc<dyn CatalogueSource>], meta: &CatalogueData) -> CatalogueData {
    let snapshots: Vec<Arc<SourceData>> = sources.iter().map(|s| s.data()).collect();
    let index = |s: &Arc<dyn CatalogueSource>| {
        sources
            .iter()
            .position(|o| Arc::ptr_eq(o, s))
            .expect("a ranked source is one of the sources")
    };
    let ids: BTreeSet<&ProviderId> = snapshots.iter().flat_map(|d| d.providers.keys()).collect();

    let mut providers = BTreeMap::new();
    for id in ids {
        let order: Vec<&SourceData> = ranked(sources, id)
            .into_iter()
            .map(|s| snapshots[index(s)].as_ref())
            .filter(|d| d.providers.contains_key(id))
            .collect();
        if order.is_empty() {
            continue;
        }
        let entries: Vec<_> = order.iter().map(|d| &d.providers[id]).collect();
        let connection = entries.iter().find_map(|p| p.connection.clone());
        let model_ids: BTreeSet<&String> = entries.iter().flat_map(|p| p.models.keys()).collect();
        let models = model_ids
            .into_iter()
            .map(|mid| {
                let mut facts = ModelFacts::default();
                for p in &entries {
                    if let Some(f) = p.models.get(mid) {
                        fill_facts(&mut facts, f);
                    }
                }
                (mid.clone(), to_model(mid, facts))
            })
            .collect();
        let provider = match connection {
            Some(c) => CatalogueProvider {
                id: id.clone(),
                name: c.name,
                env: c.env,
                api: c.api,
                npm: c.npm,
                doc: c.doc,
                models,
                dialect: c.dialect,
                kind: c.kind,
            },
            // No source says how to connect: described and priced, but never offered (no
            // dialect), as models.dev's unsupported providers are today.
            None => CatalogueProvider {
                id: id.clone(),
                name: id.to_string(),
                env: vec![],
                api: None,
                npm: String::new(),
                doc: None,
                models,
                dialect: None,
                kind: ProviderKind::Cloud,
            },
        };
        providers.insert(id.clone(), provider);
    }
    CatalogueData {
        source: meta.source,
        fetched_at: meta.fetched_at,
        checked_at: meta.checked_at,
        etag: meta.etag.clone(),
        providers,
    }
}

/// The walk's inputs beyond the sources: billing per provider and what the merged view knows.
pub struct WalkContext<'a> {
    pub sources: &'a [Arc<dyn CatalogueSource>],
    pub merged: &'a CatalogueData,
    pub billing: &'a dyn Fn(&ProviderId) -> Billing,
}

/// The winning source and price, and the later sources whose price differs.
type Walked = (SourceId, Price, Vec<(SourceId, Price)>);

/// The first price and the differing later ones, without rules or borrowing.
fn walk_sources(ctx: &WalkContext<'_>, q: &Query<'_>, provider: &ProviderId) -> Option<Walked> {
    let mut winner: Option<(SourceId, Price)> = None;
    let mut alternatives = vec![];
    for s in ranked(ctx.sources, provider) {
        let Some(price) = s
            .price(provider, q.model, q.kind)
            .filter(|p| p.covers(q.kind, q.has_tokens))
        else {
            continue;
        };
        match &winner {
            None => winner = Some((s.id(), price)),
            Some((_, w)) => {
                if &price != w && alternatives.len() < MAX_ALTERNATIVES {
                    alternatives.push((s.id(), price));
                }
            }
        }
    }
    winner.map(|(id, p)| (id, p, alternatives))
}

/// Every provider any source lists `model` under, for borrowing.
fn holders(ctx: &WalkContext<'_>, model: &str) -> BTreeSet<ProviderId> {
    let mut out = BTreeSet::new();
    for s in ctx.sources {
        for (id, p) in &s.data().providers {
            if p.facts(model).is_some() {
                out.insert(id.clone());
            }
        }
    }
    out
}

/// The model's maker, from any source's `lab` for this id (models.dev's `canonical_model_id`).
fn maker(ctx: &WalkContext<'_>, model: &str) -> Option<ProviderId> {
    ctx.merged
        .providers
        .values()
        .filter_map(|p| p.models.get(model))
        .find_map(|m| m.lab.as_deref())
        .and_then(|lab| ProviderId::new(lab.to_string()).ok())
}

/// What is being priced: one model's record of one kind.
#[derive(Debug, Clone, Copy)]
pub struct Query<'a> {
    pub model: &'a str,
    pub kind: UsageKind,
    /// Whether the record reported tokens: a media call that did is priced by tokens first.
    pub has_tokens: bool,
    /// The asking provider's dialect when the merged view cannot say (a custom provider), for
    /// borrowing.
    pub prefer: Option<DialectKind>,
}

/// The price of `(provider, q)` (contract §5): the billing rule, then the sources by authority,
/// then a price borrowed from the same model id at another provider, else `none`. `provider`
/// must already be de-aliased.
pub fn price(ctx: &WalkContext<'_>, provider: &ProviderId, q: &Query<'_>) -> PriceAnswer {
    let rule = |label| PriceAnswer {
        price: Some(Price::zero()),
        source: label,
        borrowed_from: None,
        alternatives: vec![],
    };
    match (ctx.billing)(provider) {
        Billing::Local => return rule(SourceId::LOCAL),
        Billing::Subscription => return rule(SourceId::SUBSCRIPTION),
        Billing::Metered => {}
    }
    if let Some((source, price, alternatives)) = walk_sources(ctx, q, provider) {
        return PriceAnswer {
            price: Some(price),
            source,
            borrowed_from: None,
            alternatives,
        };
    }
    borrow(ctx, provider, q).unwrap_or_else(PriceAnswer::none)
}

/// The last resort (FR-016): the same id's price at another metered provider, its maker first,
/// then a provider of the preferred dialect, then provider id order. Never recursive.
fn borrow(ctx: &WalkContext<'_>, provider: &ProviderId, q: &Query<'_>) -> Option<PriceAnswer> {
    let prefer = q
        .prefer
        .or_else(|| ctx.merged.providers.get(provider).and_then(|p| p.dialect));
    let maker = maker(ctx, q.model);
    let mut candidates: Vec<ProviderId> = holders(ctx, q.model)
        .into_iter()
        .filter(|p| p != provider && (ctx.billing)(p) == Billing::Metered)
        .collect();
    let dialect_of = |p: &ProviderId| ctx.merged.providers.get(p).and_then(|e| e.dialect);
    candidates.sort_by_key(|p| {
        (
            Some(p) != maker.as_ref(),
            prefer.is_none() || dialect_of(p) != prefer,
            p.clone(),
        )
    });
    candidates.into_iter().find_map(|other| {
        walk_sources(ctx, q, &other).map(|(_, price, _)| PriceAnswer {
            price: Some(price),
            source: SourceId::BORROWED,
            borrowed_from: Some(other),
            alternatives: vec![],
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::data::{Cost, SnapshotOrigin};
    use crate::catalogue::source::{ProviderConnection, SourceProvider};

    struct Fake {
        id: SourceId,
        own: Option<&'static str>,
        data: Arc<SourceData>,
    }

    impl CatalogueSource for Fake {
        fn id(&self) -> SourceId {
            self.id
        }
        fn name(&self) -> &'static str {
            self.id.0
        }
        fn authority(&self, provider: &ProviderId) -> Authority {
            match self.own {
                Some(own) if provider.as_str() == own => Authority::Own,
                Some(_) => Authority::None,
                None => Authority::General,
            }
        }
        fn data(&self) -> Arc<SourceData> {
            self.data.clone()
        }
    }

    fn pid(s: &str) -> ProviderId {
        ProviderId::new(s.to_string()).unwrap()
    }

    fn cost(input: f64) -> Price {
        Price::tokens(Cost {
            input,
            output: input * 2.0,
            cache_read: None,
            cache_write: None,
        })
    }

    fn facts(context: Option<u64>, price: Option<Price>, lab: Option<&str>) -> ModelFacts {
        let mut f = ModelFacts {
            price,
            lab: lab.map(str::to_string),
            ..Default::default()
        };
        f.limit.context = context;
        f
    }

    fn source(
        id: SourceId,
        own: Option<&'static str>,
        rows: Vec<(&str, Option<DialectKind>, &str, ModelFacts)>,
    ) -> Arc<dyn CatalogueSource> {
        let mut providers: BTreeMap<ProviderId, SourceProvider> = BTreeMap::new();
        for (p, dialect, m, f) in rows {
            let e = providers.entry(pid(p)).or_default();
            if let Some(d) = dialect {
                e.connection = Some(ProviderConnection {
                    name: p.to_string(),
                    env: vec![],
                    api: None,
                    npm: String::new(),
                    doc: None,
                    dialect: Some(d),
                    kind: ProviderKind::Cloud,
                });
            }
            e.models.insert(m.to_string(), f);
        }
        Arc::new(Fake {
            id,
            own,
            data: Arc::new(SourceData {
                origin: SnapshotOrigin::Bundled,
                providers,
                ..SourceData::empty()
            }),
        })
    }

    fn q(model: &str, kind: UsageKind, prefer: Option<DialectKind>) -> Query<'_> {
        Query {
            model,
            kind,
            has_tokens: true,
            prefer,
        }
    }

    fn metered(_: &ProviderId) -> Billing {
        Billing::Metered
    }

    fn walk<'a>(
        sources: &'a [Arc<dyn CatalogueSource>],
        merged: &'a CatalogueData,
        billing: &'a dyn Fn(&ProviderId) -> Billing,
    ) -> WalkContext<'a> {
        WalkContext { sources, merged, billing }
    }

    #[test]
    fn facts_fall_through_and_authority_beats_registration_order() {
        let general = source(
            SourceId::MODELS_DEV,
            None,
            vec![("venice", Some(DialectKind::OpenAiChat), "m", facts(Some(1), Some(cost(1.0)), None))],
        );
        let own = source(SourceId::VENICE, Some("venice"), vec![("venice", None, "m", facts(None, Some(cost(2.0)), None))]);
        // models.dev registered first: the provider's own list still wins what it has.
        let sources = vec![general, own];
        let merged = merge(&sources, &CatalogueData::empty());
        let m = &merged.providers[&pid("venice")].models["m"];
        assert_eq!(m.cost.as_ref().map(|c| c.input), Some(2.0), "the own list's price");
        assert_eq!(m.limit.context, Some(1), "a fact the own list lacks comes from models.dev");
        assert_eq!(merged.providers[&pid("venice")].dialect, Some(DialectKind::OpenAiChat));
    }

    #[test]
    fn a_source_without_a_connection_describes_but_never_offers() {
        let own = source(SourceId::VENICE, Some("venice"), vec![("venice", None, "m", facts(Some(5), None, None))]);
        let merged = merge(&[own], &CatalogueData::empty());
        let p = &merged.providers[&pid("venice")];
        assert_eq!(p.dialect, None);
        assert_eq!(p.models["m"].limit.context, Some(5));
    }

    #[test]
    fn the_first_price_wins_and_a_different_one_is_kept() {
        let own = source(SourceId::OPENROUTER, Some("openrouter"), vec![("openrouter", None, "m", facts(None, Some(cost(1.0)), None))]);
        let same = source(SourceId::CURATED, None, vec![("openrouter", None, "m", facts(None, Some(cost(1.0)), None))]);
        let other = source(SourceId::MODELS_DEV, None, vec![("openrouter", None, "m", facts(None, Some(cost(3.0)), None))]);
        let sources = vec![other, same, own];
        let merged = merge(&sources, &CatalogueData::empty());
        let a = price(&walk(&sources, &merged, &metered), &pid("openrouter"), &q("m", UsageKind::Text, None));
        assert_eq!(a.source, SourceId::OPENROUTER);
        assert_eq!(a.alternatives.len(), 1, "an equal price is no disagreement");
        assert_eq!(a.alternatives[0].0, SourceId::MODELS_DEV);
        let miss = price(&walk(&sources, &merged, &metered), &pid("openrouter"), &q("nope", UsageKind::Text, None));
        assert_eq!(miss, PriceAnswer::none());
    }

    #[test]
    fn a_reseller_list_never_prices_another_provider() {
        let own = source(SourceId::OPENROUTER, Some("openrouter"), vec![("anthropic", None, "m", facts(None, Some(cost(1.0)), None))]);
        let sources = vec![own];
        let merged = merge(&sources, &CatalogueData::empty());
        assert!(!merged.providers.contains_key(&pid("anthropic")));
        let a = price(&walk(&sources, &merged, &metered), &pid("anthropic"), &q("m", UsageKind::Text, None));
        assert_eq!(a.source, SourceId::NONE);
    }

    #[test]
    fn rules_come_first_and_are_never_borrowed_for() {
        let general = source(SourceId::MODELS_DEV, None, vec![("ollama", None, "llama", facts(None, Some(cost(9.0)), None))]);
        let sources = vec![general];
        let merged = merge(&sources, &CatalogueData::empty());
        let billing = |p: &ProviderId| match p.as_str() {
            "ollama" => Billing::Local,
            "chatgpt" => Billing::Subscription,
            _ => Billing::Metered,
        };
        let w = walk(&sources, &merged, &billing);
        let local = price(&w, &pid("ollama"), &q("llama", UsageKind::Text, None));
        assert_eq!(local.source, SourceId::LOCAL);
        assert_eq!(local.price, Some(Price::zero()));
        let sub = price(&w, &pid("chatgpt"), &q("llama", UsageKind::Image, None));
        assert_eq!(sub.source, SourceId::SUBSCRIPTION);
        // A metered provider never borrows from a local one.
        let proxy = price(&w, &pid("my-proxy"), &q("llama", UsageKind::Text, None));
        assert_eq!(proxy.source, SourceId::NONE);
    }

    #[test]
    fn borrowing_prefers_the_maker_then_the_dialect_then_the_id() {
        let general = source(
            SourceId::MODELS_DEV,
            None,
            vec![
                ("abacus", Some(DialectKind::OpenAiChat), "gpt", facts(None, Some(cost(0.2)), Some("openai"))),
                ("openai", Some(DialectKind::OpenAiResponses), "gpt", facts(None, Some(cost(0.15)), None)),
                ("zeta", Some(DialectKind::Anthropic), "claude", facts(None, Some(cost(3.0)), None)),
                ("alpha", Some(DialectKind::OpenAiChat), "claude", facts(None, Some(cost(4.0)), None)),
            ],
        );
        let sources = vec![general];
        let merged = merge(&sources, &CatalogueData::empty());
        let w = walk(&sources, &merged, &metered);
        let a = price(&w, &pid("my-proxy"), &q("gpt", UsageKind::Text, Some(DialectKind::OpenAiChat)));
        assert_eq!(a.source, SourceId::BORROWED);
        assert_eq!(a.borrowed_from, Some(pid("openai")), "the maker, not the first reseller");
        let b = price(&w, &pid("my-proxy"), &q("claude", UsageKind::Text, Some(DialectKind::Anthropic)));
        assert_eq!(b.borrowed_from, Some(pid("zeta")), "the same dialect before id order");
        let c = price(&w, &pid("my-proxy"), &q("claude", UsageKind::Text, None));
        assert_eq!(c.borrowed_from, Some(pid("alpha")), "id order last");
        for _ in 0..5 {
            assert_eq!(price(&w, &pid("my-proxy"), &q("claude", UsageKind::Text, None)), c, "deterministic");
        }
    }
}
