//! FR-021 (spec 068): every figure the pre-068 pricer gave from the committed seeds is unchanged,
//! unless a more authoritative source now answers. The deliberate changes are listed, with the
//! source that now wins, in `fr021_exceptions.txt`; this test fails on any change not listed and
//! on any listed line that no longer happens.

use std::collections::BTreeSet;

use super::Pricer;
use crate::catalogue::media_prices::media_prices;
use crate::catalogue::{Catalogue, seed::load_seed};
use crate::config::provider::ProviderId;
use crate::schema::metrics::{TokenCounts, estimate_cost};
use crate::schema::usage::{MediaUnits, UsageKind};

const EXCEPTIONS: &str = include_str!("fr021_exceptions.txt");

/// The pricing rules as they were before spec 068 (`Pricer::priced` on `master`): text from
/// models.dev only; media local → models.dev token price → table token rates → table units.
fn before(
    catalogue: &Catalogue,
    kind: UsageKind,
    provider: &str,
    model: &str,
    units: &MediaUnits,
    tokens: &TokenCounts,
) -> Option<f64> {
    let models_dev = catalogue.models_dev();
    let outside = provider
        .parse::<ProviderId>()
        .ok()
        .and_then(|id| models_dev.providers.get(&id).map(|p| p.dialect))
        .flatten()
        == Some(crate::llm::DialectKind::Anthropic);
    let catalogue_cost = || {
        let id = provider.parse::<ProviderId>().ok()?;
        let id = crate::catalogue::builtin::model_source(&id).unwrap_or(id);
        let cost = models_dev.providers.get(&id)?.models.get(model)?.cost.clone()?;
        Some(estimate_cost(tokens, &cost, outside))
    };
    if kind == UsageKind::Text {
        return catalogue_cost();
    }
    let has_tokens = tokens.total() > 0;
    if has_tokens && let Some(c) = catalogue_cost() {
        return Some(c);
    }
    let entry = media_prices().lookup(provider, model, kind)?;
    if has_tokens && let Some(rates) = &entry.tokens {
        return Some(estimate_cost(tokens, rates, false));
    }
    entry.unit_cost(units, None)
}

#[test]
fn fr021_every_old_figure_is_unchanged_or_explained() {
    let catalogue = Catalogue::standard(load_seed());
    let mut pricer = Pricer::new(&catalogue);
    let text = TokenCounts {
        input: 1_000_000,
        cached: 200_000,
        cache_write: 0,
        output: 300_000,
    };
    let embed = TokenCounts {
        input: 1_000_000,
        ..Default::default()
    };
    let units = MediaUnits {
        images: 2,
        characters: 1_000_000,
        audio_ms: 60_000,
    };
    let none = TokenCounts::default();

    let mut cases: Vec<(UsageKind, String, String, TokenCounts)> = vec![];
    for (pid, p) in &catalogue.models_dev().providers {
        for mid in p.models.keys() {
            cases.push((UsageKind::Text, pid.to_string(), mid.clone(), text));
            cases.push((UsageKind::Embedding, pid.to_string(), mid.clone(), embed));
        }
    }
    for e in &media_prices().entries {
        let model = e.model.trim_end_matches('*').to_string();
        let tokens = if e.kind == UsageKind::Embedding { embed } else { none };
        cases.push((e.kind, e.provider.clone(), model, tokens));
    }

    let mut changed = BTreeSet::new();
    let mut by_own_list = 0;
    for (kind, provider, model, tokens) in &cases {
        let old = before(&catalogue, *kind, provider, model, &units, tokens);
        let (new, source) =
            pricer.priced(*kind, provider, model, None, false, &units, tokens);
        let same = match (old, new) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            (None, None) => true,
            _ => false,
        };
        // Newly priced where nothing priced before, by a rule or by borrowing, is the point of
        // spec 068 (US2, US4); only a figure that existed and moved must be explained.
        let newly_covered = old.is_none()
            && matches!(source.as_str(), "borrowed" | "local" | "subscription");
        // A provider's own list now answering for that provider is the more authoritative
        // source FR-021 allows: count it, but it needs no line of its own.
        let own_list = provider.parse::<ProviderId>().is_ok_and(|id| {
            catalogue
                .sources()
                .iter()
                .any(|s| s.id() == source && s.authority(&id) == crate::catalogue::Authority::Own)
        });
        if !same && own_list {
            by_own_list += 1;
        }
        if !same && !newly_covered && !own_list {
            changed.insert(format!("{} {provider} {model} -> {source}", kind.as_str()));
        }
    }
    let listed: BTreeSet<String> = EXCEPTIONS
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();
    println!("FR-021: {by_own_list} figures now come from the provider's own list");
    let unexplained: Vec<_> = changed.difference(&listed).collect();
    let stale: Vec<_> = listed.difference(&changed).collect();
    assert!(
        unexplained.is_empty() && stale.is_empty(),
        "figures changed without an entry in fr021_exceptions.txt: {unexplained:#?}\n\
         entries that no longer change: {stale:#?}"
    );
}
