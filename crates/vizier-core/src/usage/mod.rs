//! Usage reports (`specs/018-usage-accounting/`): price grouped usage rows with the catalogue,
//! and resolve the names and links the usage page shows. Storage groups; this prices (R7).

use crate::time::Timestamp;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::Result;
use chrono::{DateTime, Duration, NaiveDate, Local};

use crate::{
    catalogue::{Catalogue, Price, PriceAnswer, SourceId, aggregate::Query, data::Cost},
    config::provider::ProviderId,
    integrations::{IntegrationKinds, kind::ConversationNames},
    llm::DialectKind,
    schema::{
        VizierChannelId,
        metrics::{TokenCounts, add_cost, estimate_cost},
        provider::ProfileEntry,
        usage::{
            BreakdownRow, CallRow, ConversationRow, ConversationUsage, DailyUsage, DayActivity,
            FailedTotals, KindRow, MediaCall, MediaCalls, MediaUnits, ModelRow, NoLinkReason,
            PeriodTotals, PriceAlternative, SourceSummary, UnpricedNote, Units, UsageActivity,
            UsageKind, UsageLink,
            UsageLinkKind, UsageRange, UsageReport, UsageSummary,
        },
    },
    service::catalogue::entry_pricing,
    storage::{
        VizierStorage,
        integration::IntegrationStorage,
        project::ProjectStorage,
        provider::ProviderStorage,
        state::StateStorage,
        task::TaskStorage,
        usage::{UsageGroup, UsageGrouping, UsageQuery, UsageStorage},
    },
};

pub mod record;

#[cfg(test)]
mod golden_tests;

/// The most expensive conversations the report lists (FR-017).
const COSTLIEST: usize = 10;

/// Prices from the catalogue's sources (spec 068), looked up once per `(provider, model, kind)`
/// per report.
pub struct Pricer<'a> {
    catalogue: &'a Catalogue,
    /// The saved inference-profile entries, by `(provider, arn)`, read once per report. An ARN is
    /// priced from its entry before the catalogue is asked (FR-034, FR-034b; spec 068 FR-020).
    entries: HashMap<(String, String), ProfileEntry>,
    answers: HashMap<(String, String, UsageKind, bool), PriceAnswer>,
    outside: HashMap<String, bool>,
}

impl<'a> Pricer<'a> {
    pub fn new(catalogue: &'a Catalogue) -> Self {
        Self::with_entries(catalogue, HashMap::new())
    }

    /// A pricer that knows the saved inference-profile entries. An ARN row is priced by its entry
    /// — the hand-entered price, then the paired model — before the catalogue is consulted for the
    /// model string, so a report prices an ARN the way the settings row describes it (FR-034,
    /// research D13). An entry with nothing to price from yields the `None` that already means
    /// unpriced, never zero (FR-034b).
    pub fn with_entries(
        catalogue: &'a Catalogue,
        entries: HashMap<(String, String), ProfileEntry>,
    ) -> Self {
        Self {
            catalogue,
            entries,
            answers: HashMap::new(),
            outside: HashMap::new(),
        }
    }

    /// Whether the provider counts cached tokens outside `input` (Anthropic) rather than inside.
    pub fn cache_outside_input(&mut self, provider: &str) -> bool {
        if let Some(v) = self.outside.get(provider) {
            return *v;
        }
        let v = provider
            .parse::<ProviderId>()
            .ok()
            .and_then(|id| self.catalogue.provider(&id))
            .and_then(|p| p.dialect)
            == Some(DialectKind::Anthropic);
        self.outside.insert(provider.to_string(), v);
        v
    }

    /// The catalogue's answer for one record shape. An ARN's own entry comes first and decides
    /// alone: explicit price, then the paired model, then unpriced — the order `entry_pricing`
    /// fixes, so a report and the settings row can never disagree (FR-034, FR-034b).
    pub fn answer(
        &mut self,
        kind: UsageKind,
        provider: &str,
        model: &str,
        has_tokens: bool,
    ) -> PriceAnswer {
        let key = (provider.to_string(), model.to_string(), kind, has_tokens);
        if let Some(a) = self.answers.get(&key) {
            return a.clone();
        }
        let answer = if let Some(entry) =
            self.entries.get(&(provider.to_string(), model.to_string()))
        {
            match entry_pricing(self.catalogue, entry).cost().cloned() {
                Some(cost) => PriceAnswer {
                    price: Some(Price::tokens(cost)),
                    source: SourceId::INFERENCE_PROFILE,
                    borrowed_from: None,
                    alternatives: vec![],
                },
                None => PriceAnswer::none(),
            }
        } else {
            match provider.parse::<ProviderId>() {
                Ok(id) => self.catalogue.price(
                    &id,
                    &Query {
                        model,
                        kind,
                        has_tokens,
                        prefer: None,
                    },
                ),
                // A legacy row with no usable provider id: unpriced.
                Err(_) => PriceAnswer::none(),
            }
        };
        self.answers.insert(key, answer.clone());
        answer
    }

    /// A group's cost, whatever its kind (spec 065 research R6).
    pub fn group_cost(&mut self, g: &UsageGroup) -> Option<f64> {
        self.priced(g.kind, &g.provider, &g.model, g.variant.as_deref(), g.local, &g.units, &g.tokens)
            .0
    }

    /// One record's cost and which source or rule priced it (spec 068). A media call measured on
    /// this machine is free whatever its provider; everything else asks the catalogue.
    #[allow(clippy::too_many_arguments)]
    pub fn priced(
        &mut self,
        kind: UsageKind,
        provider: &str,
        model: &str,
        variant: Option<&str>,
        local: bool,
        units: &MediaUnits,
        tokens: &TokenCounts,
    ) -> (Option<f64>, SourceId) {
        self.priced_with(kind, provider, model, variant, local, units, tokens)
            .map(|(cost, answer)| (Some(cost), answer.source))
            .unwrap_or((None, SourceId::NONE))
    }

    /// [`Self::priced`] with the whole answer, for rows that show where a price came from.
    #[allow(clippy::too_many_arguments)]
    pub fn priced_with(
        &mut self,
        kind: UsageKind,
        provider: &str,
        model: &str,
        variant: Option<&str>,
        local: bool,
        units: &MediaUnits,
        tokens: &TokenCounts,
    ) -> Option<(f64, PriceAnswer)> {
        if local && kind != UsageKind::Text {
            return Some((
                0.0,
                PriceAnswer {
                    price: Some(Price::zero()),
                    source: SourceId::LOCAL,
                    borrowed_from: None,
                    alternatives: vec![],
                },
            ));
        }
        let answer = self.answer(kind, provider, model, tokens.total() > 0);
        let outside = self.cache_outside_input(provider);
        let cost = answer
            .price
            .as_ref()?
            .cost(kind, tokens, units, variant, outside)?;
        Some((cost, answer))
    }

    /// A completion's cost.
    pub fn cost(&mut self, provider: &str, model: &str, tokens: &TokenCounts) -> Option<f64> {
        self.priced(UsageKind::Text, provider, model, None, false, &MediaUnits::default(), tokens)
            .0
    }

    /// `(cached, whole input)`, so cache rates can be summed across providers.
    fn cache_parts(&mut self, provider: &str, tokens: &TokenCounts) -> (u64, u64) {
        let whole = if self.cache_outside_input(provider) {
            tokens.input + tokens.cached + tokens.cache_write
        } else {
            tokens.input
        };
        (tokens.cached, whole)
    }
}

/// Every saved inference-profile entry, keyed by `(provider, arn)`, from one read of the provider
/// rows. A report builds this once, so the turn itself does no pricing work at all (FR-034a.5,
/// research D13 §3).
pub async fn saved_entries(storage: &VizierStorage) -> HashMap<(String, String), ProfileEntry> {
    let mut entries = HashMap::new();
    for provider in storage.list_providers().await.unwrap_or_default() {
        let id = provider.id.as_str().to_string();
        for entry in provider.credentials.profiles {
            entries.insert((id.clone(), entry.arn.clone()), entry);
        }
    }
    entries
}


/// Running totals for one bucket of groups.
#[derive(Debug, Default, Clone)]
struct Tally {
    tokens: TokenCounts,
    /// Media amounts, and embedding tokens, summed (spec 065).
    media: MediaUnits,
    embedding_tokens: u64,
    has_media: bool,
    calls: u64,
    cost: Option<f64>,
    cached: u64,
    whole_input: u64,
    failed_tokens: u64,
    failed_cost: Option<f64>,
    failed_turns: u64,
    last_at: Option<Timestamp>,
}

impl Tally {
    fn add(&mut self, pricer: &mut Pricer, g: &UsageGroup) -> Option<f64> {
        let cost = pricer.group_cost(g);
        // Only completions take part in the cache rate.
        let (cached, whole) = if g.kind == UsageKind::Text {
            pricer.cache_parts(&g.provider, &g.tokens)
        } else {
            (0, 0)
        };
        self.tokens += g.tokens;
        if g.kind != UsageKind::Text {
            self.has_media = true;
            self.media += g.units;
            if g.kind == UsageKind::Embedding {
                self.embedding_tokens += g.tokens.input;
            }
        }
        self.calls += g.calls;
        add_cost(&mut self.cost, cost);
        self.cached += cached;
        self.whole_input += whole;
        if g.failed {
            self.failed_tokens += g.tokens.total();
            add_cost(&mut self.failed_cost, cost);
            self.failed_turns += g.turns;
        }
        self.last_at = self.last_at.max(Some(g.last_at));
        cost
    }

    fn cache_hit_rate(&self) -> Option<f64> {
        (self.whole_input > 0).then(|| self.cached as f64 / self.whole_input as f64)
    }

    fn summary(&self) -> UsageSummary {
        UsageSummary {
            cost_usd: self.cost,
            tokens: self.tokens.total(),
            input_tokens: self.tokens.input,
            cached_input_tokens: self.tokens.cached,
            cache_creation_input_tokens: self.tokens.cache_write,
            output_tokens: self.tokens.output,
            calls: self.calls,
            cache_hit_rate: self.cache_hit_rate(),
            failed: FailedTotals {
                cost_usd: self.failed_cost,
                tokens: self.failed_tokens,
                turns: self.failed_turns,
            },
            share_of_all: None,
            kind_share_of_all: None,
        }
    }

    /// The media amounts of a mixed row, when it has any.
    fn units(&self) -> Option<Units> {
        if !self.has_media {
            return None;
        }
        let u = Units {
            images: (self.media.images > 0).then_some(self.media.images),
            characters: (self.media.characters > 0).then_some(self.media.characters),
            audio_seconds: (self.media.audio_ms > 0).then(|| self.media.audio_ms as f64 / 1000.0),
            tokens: (self.embedding_tokens > 0).then_some(self.embedding_tokens),
        };
        (!u.is_empty()).then_some(u)
    }

    fn row(
        &self,
        key: String,
        label: String,
        detail: Option<String>,
        total: Option<f64>,
    ) -> BreakdownRow {
        BreakdownRow {
            key,
            label,
            detail,
            cost_usd: self.cost,
            share: share(self.cost, total),
            tokens: self.tokens.total(),
            calls: self.calls,
            units: self.units(),
        }
    }
}

fn share(part: Option<f64>, total: Option<f64>) -> Option<f64> {
    match (part, total) {
        (Some(p), Some(t)) if t > 0.0 => Some(p / t),
        _ => None,
    }
}

/// Cost descending; unpriced last, by tokens.
fn by_cost_then_tokens(a: (Option<f64>, u64), b: (Option<f64>, u64)) -> std::cmp::Ordering {
    match (a.0, b.0) {
        (Some(x), Some(y)) => y.total_cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.1.cmp(&a.1),
    }
}

fn model_key(provider: &str, model: &str) -> String {
    if provider.is_empty() && model.is_empty() {
        "unknown".to_string()
    } else {
        format!("{provider}/{model}")
    }
}

fn tally<'g>(pricer: &mut Pricer, groups: impl IntoIterator<Item = &'g UsageGroup>) -> Tally {
    let mut t = Tally::default();
    for g in groups {
        t.add(pricer, g);
    }
    t
}

/// The usage page's report for one agent, range and optional activity.
#[tracing::instrument(name = "usage_report", skip_all, fields(vizier.agent_id = %q.agent_id))]
pub async fn build_report(
    storage: &VizierStorage,
    catalogue: &Catalogue,
    kinds: &IntegrationKinds,
    q: UsageQuery,
) -> Result<UsageReport> {
    let mut pricer = Pricer::with_entries(catalogue, saved_entries(storage).await);

    // One pass for every view (spec 065 SC-005): each sums the groups by its own key and
    // ignores the rest.
    let activity_groups = storage.usage_groups(&q, UsageGrouping::Detail).await?;
    let all = tally(&mut pricer, &activity_groups);
    let mut summary = all.summary();

    if q.activity.is_some() {
        let unfiltered = UsageQuery {
            activity: None,
            ..q.clone()
        };
        let everything = storage
            .usage_groups(&unfiltered, UsageGrouping::Activity)
            .await?;
        summary.share_of_all = share(all.cost, tally(&mut pricer, &everything).cost);
    }
    if q.kind.is_some() {
        let every_kind = UsageQuery {
            kind: None,
            ..q.clone()
        };
        let everything = storage
            .usage_groups(&every_kind, UsageGrouping::Kind)
            .await?;
        summary.kind_share_of_all = share(all.cost, tally(&mut pricer, &everything).cost);
    }

    let by_kind = if q.kind.is_some() {
        vec![]
    } else {
        let mut buckets: BTreeMap<UsageKind, Tally> = BTreeMap::new();
        for g in &activity_groups {
            buckets.entry(g.kind).or_default().add(&mut pricer, g);
        }
        let mut rows: Vec<KindRow> = buckets
            .into_iter()
            .map(|(k, t)| {
                let mut row = t.row(k.as_str().to_string(), k.label().to_string(), None, all.cost);
                row.units = Units::of(k, &t.media, t.embedding_tokens);
                KindRow { row, kind: k }
            })
            .collect();
        rows.sort_by(|a, b| {
            by_cost_then_tokens((a.row.cost_usd, a.row.tokens), (b.row.cost_usd, b.row.tokens))
        });
        rows
    };

    let by_activity = if q.activity.is_some() {
        vec![]
    } else {
        let mut buckets: BTreeMap<UsageActivity, Tally> = BTreeMap::new();
        for g in &activity_groups {
            buckets
                .entry(g.activity.unwrap_or(UsageActivity::Other))
                .or_default()
                .add(&mut pricer, g);
        }
        let mut rows: Vec<BreakdownRow> = buckets
            .into_iter()
            .map(|(a, t)| {
                t.row(
                    a.as_str().to_string(),
                    a.label().to_string(),
                    None,
                    all.cost,
                )
            })
            .collect();
        rows.sort_by(|a, b| by_cost_then_tokens((a.cost_usd, a.tokens), (b.cost_usd, b.tokens)));
        rows
    };

    let previous = {
        let span = q.end - q.start;
        let before = UsageQuery {
            start: q.start - span,
            end: q.start - Duration::milliseconds(1),
            ..q.clone()
        };
        let groups = storage
            .usage_groups(&before, UsageGrouping::Activity)
            .await?;
        (!groups.is_empty()).then(|| {
            let t = tally(&mut pricer, &groups);
            PeriodTotals {
                cost_usd: t.cost,
                tokens: t.tokens.total(),
                calls: t.calls,
            }
        })
    };

    let daily = daily(&mut pricer, &q, activity_groups.clone());

    let (by_model, unpriced) = by_model(&mut pricer, &activity_groups, all.cost);

    let mut place_buckets: BTreeMap<String, Tally> = BTreeMap::new();
    for g in &activity_groups {
        place_buckets
            .entry(g.place.clone().unwrap_or_default())
            .or_default()
            .add(&mut pricer, g);
    }
    // Every Discord channel and Telegram chat in range is one place, so the places name every
    // conversation the costliest list can show. The costliest places are looked up first.
    let mut ranked_places: Vec<_> = place_buckets.iter().collect();
    ranked_places.sort_by(|a, b| {
        by_cost_then_tokens(
            (a.1.cost, a.1.tokens.total()),
            (b.1.cost, b.1.tokens.total()),
        )
    });
    let names = NameBook::resolve(
        storage,
        kinds,
        &q.agent_id,
        ranked_places.iter().map(|(place, _)| place.as_str()),
    )
    .await;

    let mut by_place = Vec::with_capacity(place_buckets.len());
    for (place, t) in place_buckets {
        let (label, detail) = place_label(storage, &names, &q.agent_id, &place).await;
        by_place.push(t.row(place, label, detail, all.cost));
    }
    by_place.sort_by(|a, b| by_cost_then_tokens((a.cost_usd, a.tokens), (b.cost_usd, b.tokens)));

    let costliest = costliest(
        storage,
        &names,
        &mut pricer,
        &q.agent_id,
        activity_groups,
    )
    .await?;

    let accounting_since = storage
        .get_state(crate::dependencies::VizierDependencies::USAGE_ACCOUNTING_SINCE.to_string())
        .await?
        .and_then(|v| v.as_str().map(str::to_string))
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Local));
    let media_accounting_since = storage
        .get_state(crate::dependencies::VizierDependencies::USAGE_MEDIA_ACCOUNTING_SINCE.to_string())
        .await?
        .and_then(|v| v.as_str().map(str::to_string))
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Local));

    let sources = report_sources(catalogue, &by_model);
    Ok(UsageReport {
        range: UsageRange {
            start: q.start,
            end: q.end,
        },
        accounting_since,
        activity: q.activity,
        kind: q.kind,
        media_accounting_since,
        summary,
        previous,
        daily,
        by_activity,
        by_kind,
        by_model,
        by_place,
        sources,
        costliest,
        unpriced,
    })
}

/// The catalogue sources the model rows name (as their price or a disagreeing one), with their
/// display names; rule labels (`local`, `borrowed`, …) are not sources and are left out.
fn report_sources(catalogue: &Catalogue, rows: &[ModelRow]) -> Vec<SourceSummary> {
    let named: BTreeSet<&str> = rows
        .iter()
        .flat_map(|r| {
            std::iter::once(r.price_source.as_str())
                .chain(r.price_alternatives.iter().map(|a| a.source.as_str()))
        })
        .collect();
    catalogue
        .sources()
        .iter()
        .filter(|s| named.contains(s.id().as_str()))
        .map(|s| SourceSummary {
            id: s.id().to_string(),
            name: s.name().to_string(),
            fetched_at: s.data().fetched_at,
        })
        .collect()
}

/// One entry per date in range, empty ones included, so the chart's axis is complete.
/// Day groups carry their kind too, so one read fills both the activity stacks and the kind lines.
fn daily(pricer: &mut Pricer, q: &UsageQuery, groups: Vec<UsageGroup>) -> Vec<DailyUsage> {
    type Cells<K> = BTreeMap<NaiveDate, BTreeMap<K, DayActivity>>;
    let mut days: Cells<UsageActivity> = BTreeMap::new();
    let mut kinds: Cells<UsageKind> = BTreeMap::new();
    let mut d = q.start.date_naive();
    while d <= q.end.date_naive() {
        days.insert(d, BTreeMap::new());
        d = d.succ_opt().unwrap_or(d + Duration::days(1));
    }
    let mut put = |cell: &mut DayActivity, cost: Option<f64>, g: &UsageGroup| {
        add_cost(&mut cell.cost_usd, cost);
        cell.tokens += g.tokens.total();
        cell.calls += g.calls;
    };
    for g in groups {
        let (Some(day), Some(activity)) = (g.day, g.activity) else {
            continue;
        };
        let cost = pricer.group_cost(&g);
        put(days.entry(day).or_default().entry(activity).or_default(), cost, &g);
        put(kinds.entry(day).or_default().entry(g.kind).or_default(), cost, &g);
    }
    days.into_iter()
        .map(|(date, by_activity)| DailyUsage {
            date,
            by_kind: kinds.remove(&date).unwrap_or_default(),
            by_activity,
        })
        .collect()
}

fn by_model(
    pricer: &mut Pricer,
    groups: &[UsageGroup],
    total: Option<f64>,
) -> (Vec<ModelRow>, UnpricedNote) {
    /// A row's tally, served models, the first answer that priced it, and what each disagreeing
    /// source would have charged for the same groups.
    type Bucket = (Tally, BTreeSet<String>, Option<PriceAnswer>, BTreeMap<SourceId, f64>);
    let mut buckets: BTreeMap<(String, String, UsageKind), Bucket> = BTreeMap::new();
    for g in groups {
        let (t, served, answer, alternatives) = buckets
            .entry((g.provider.clone(), g.model.clone(), g.kind))
            .or_default();
        t.add(pricer, g);
        if let Some((_, this)) = pricer.priced_with(
            g.kind,
            &g.provider,
            &g.model,
            g.variant.as_deref(),
            g.local,
            &g.units,
            &g.tokens,
        ) {
            let outside = pricer.cache_outside_input(&g.provider);
            for (source, price) in &this.alternatives {
                if let Some(c) = price.cost(g.kind, &g.tokens, &g.units, g.variant.as_deref(), outside) {
                    *alternatives.entry(*source).or_default() += c;
                }
            }
            answer.get_or_insert(this);
        }
        if let Some(s) = &g.served_model
            && s != &g.model
        {
            served.insert(s.clone());
        }
    }

    let mut unpriced = UnpricedNote::default();
    let mut rows: Vec<ModelRow> = buckets
        .into_iter()
        .map(|((provider, model, kind), (t, served, answer, alternatives))| {
            let priced = answer.is_some();
            let price_source = answer.as_ref().map_or(SourceId::NONE, |a| a.source);
            let borrowed_from = answer
                .as_ref()
                .and_then(|a| a.borrowed_from.as_ref())
                .map(|p| p.to_string());
            let key = model_key(&provider, &model);
            if !priced {
                match Units::of(kind, &t.media, t.embedding_tokens) {
                    Some(units) => {
                        unpriced.units.insert(kind, merge_units(unpriced.units.get(&kind), &units));
                    }
                    None => unpriced.tokens += t.tokens.total(),
                }
                if !unpriced.models.contains(&key) {
                    unpriced.models.push(key.clone());
                }
            }
            let label = if provider.is_empty() && model.is_empty() {
                "Unknown model".to_string()
            } else {
                format!("{provider} / {model}")
            };
            let mut row = t.row(key, label, None, total);
            row.units = Units::of(kind, &t.media, t.embedding_tokens);
            ModelRow {
                row,
                provider,
                model,
                served_models: served.into_iter().collect(),
                cache_hit_rate: t.cache_hit_rate(),
                failed_tokens: t.failed_tokens,
                priced,
                kind,
                price_source: price_source.to_string(),
                borrowed_from,
                price_alternatives: alternatives
                    .into_iter()
                    .map(|(source, cost_usd)| PriceAlternative {
                        source: source.to_string(),
                        cost_usd,
                    })
                    .collect(),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        by_cost_then_tokens(
            (a.row.cost_usd, a.row.tokens),
            (b.row.cost_usd, b.row.tokens),
        )
    });
    (rows, unpriced)
}

/// Two amounts of one kind, added.
fn merge_units(a: Option<&Units>, b: &Units) -> Units {
    let add = |x: Option<u64>, y: Option<u64>| match (x, y) {
        (None, None) => None,
        (x, y) => Some(x.unwrap_or(0) + y.unwrap_or(0)),
    };
    let Some(a) = a else {
        return b.clone();
    };
    Units {
        images: add(a.images, b.images),
        characters: add(a.characters, b.characters),
        audio_seconds: match (a.audio_seconds, b.audio_seconds) {
            (None, None) => None,
            (x, y) => Some(x.unwrap_or(0.0) + y.unwrap_or(0.0)),
        },
        tokens: add(a.tokens, b.tokens),
    }
}

/// The media records of `call_ids`, priced (spec 065 R10).
pub async fn media_calls(
    storage: &VizierStorage,
    catalogue: &Catalogue,
    agent_id: &str,
    call_ids: &[String],
) -> Result<MediaCalls> {
    let mut pricer = Pricer::with_entries(catalogue, saved_entries(storage).await);
    let mut out = MediaCalls::default();
    for e in storage.media_calls(agent_id, call_ids).await? {
        let Some(call_id) = e.call_id.clone() else {
            continue;
        };
        let (cost_usd, price_source) = pricer.priced(
            e.kind,
            &e.provider,
            &e.model,
            e.variant.as_deref(),
            e.local,
            &e.units,
            &e.tokens,
        );
        out.calls.entry(call_id).or_default().push(MediaCall {
            units: Units::of(e.kind, &e.units, e.tokens.input),
            kind: e.kind,
            provider: e.provider,
            model: e.model,
            variant: e.variant,
            measured: e.measured,
            local: e.local,
            cost_usd,
            price_source: price_source.to_string(),
            at: e.at,
        });
    }
    Ok(out)
}

async fn costliest(
    storage: &VizierStorage,
    names: &NameBook,
    pricer: &mut Pricer<'_>,
    agent_id: &str,
    groups: Vec<UsageGroup>,
) -> Result<Vec<ConversationRow>> {
    #[derive(Default)]
    struct Conv {
        tally: Tally,
        by_activity: HashMap<UsageActivity, (Option<f64>, u64)>,
        by_model: HashMap<String, (Option<f64>, u64)>,
    }
    let mut convs: HashMap<(String, Option<String>), Conv> = HashMap::new();
    for g in &groups {
        let Some(channel) = g.channel.clone() else {
            continue;
        };
        let conv = convs.entry((channel, g.topic.clone())).or_default();
        let cost = conv.tally.add(pricer, g);
        let a = conv
            .by_activity
            .entry(g.activity.unwrap_or(UsageActivity::Other))
            .or_default();
        add_cost(&mut a.0, cost);
        a.1 += g.tokens.total();
        let m = conv
            .by_model
            .entry(model_key(&g.provider, &g.model))
            .or_default();
        add_cost(&mut m.0, cost);
        m.1 += g.tokens.total();
    }

    let mut ranked: Vec<_> = convs.into_iter().collect();
    ranked.sort_by(|a, b| {
        by_cost_then_tokens(
            (a.1.tally.cost, a.1.tally.tokens.total()),
            (b.1.tally.cost, b.1.tally.tokens.total()),
        )
    });
    ranked.truncate(COSTLIEST);

    let mut rows = Vec::with_capacity(ranked.len());
    for ((channel, topic), conv) in ranked {
        let exists = conversation_exists(storage, agent_id, &channel, topic.as_deref()).await;
        let (label, detail) =
            conversation_label(storage, names, agent_id, &channel, topic.as_deref()).await;
        rows.push(ConversationRow {
            activity: top(&conv.by_activity).unwrap_or(UsageActivity::Other),
            model: top(&conv.by_model).unwrap_or_default(),
            tokens: conv.tally.tokens.total(),
            cost_usd: conv.tally.cost,
            failed_share: failed_share(&conv.tally),
            last_at: conv.tally.last_at.unwrap_or_default(),
            exists,
            label,
            detail,
            channel,
            topic,
        });
    }
    Ok(rows)
}

/// The key that cost the most (by tokens when unpriced).
fn top<K: Clone>(m: &HashMap<K, (Option<f64>, u64)>) -> Option<K> {
    m.iter()
        .min_by(|a, b| by_cost_then_tokens(*a.1, *b.1))
        .map(|(k, _)| k.clone())
}

/// Failed cost over cost, or failed tokens over tokens when nothing is priced.
fn failed_share(t: &Tally) -> Option<f64> {
    match (t.failed_cost, t.cost) {
        (_, Some(c)) if c > 0.0 => Some(t.failed_cost.unwrap_or(0.0) / c),
        _ if t.tokens.total() > 0 => Some(t.failed_tokens as f64 / t.tokens.total() as f64),
        _ => None,
    }
}

/// One conversation's details: its calls, most expensive first, and where to open it.
#[tracing::instrument(name = "usage_conversation", skip_all, fields(vizier.agent_id = %q.agent_id))]
pub async fn conversation_usage(
    storage: &VizierStorage,
    catalogue: &Catalogue,
    kinds: &IntegrationKinds,
    q: UsageQuery,
    limit: usize,
) -> Result<ConversationUsage> {
    let (channel, topic) = q.conversation.clone().unwrap_or_default();
    let mut pricer = Pricer::with_entries(catalogue, saved_entries(storage).await);

    let all = tally(
        &mut pricer,
        &storage.usage_groups(&q, UsageGrouping::Activity).await?,
    );
    let rows = storage
        .conversation_usage(&q, limit.saturating_mul(4))
        .await?;

    let mut calls: Vec<CallRow> = rows
        .rows
        .into_iter()
        .map(|r| {
            let e = r.event;
            let (cost_usd, _) = pricer.priced(
                e.kind,
                &e.provider,
                &e.model,
                e.variant.as_deref(),
                e.local,
                &e.units,
                &e.tokens,
            );
            CallRow {
                cost_usd,
                kind: e.kind,
                variant: e.variant.clone(),
                units: Units::of(e.kind, &e.units, e.tokens.input),
                measured: e.measured,
                at: e.at,
                activity: e.activity,
                round: e.round,
                input_tokens: e.tokens.input,
                cached_input_tokens: e.tokens.cached,
                output_tokens: e.tokens.output,
                provider: e.provider,
                model: e.model,
                served_model: e.served_model,
                failed: r.failed,
            }
        })
        .collect();
    calls.sort_by(|a, b| {
        by_cost_then_tokens(
            (a.cost_usd, a.input_tokens + a.output_tokens),
            (b.cost_usd, b.input_tokens + b.output_tokens),
        )
    });
    calls.truncate(limit);
    let more = rows.total.saturating_sub(calls.len() as u64);

    let names =
        NameBook::resolve(storage, kinds, &q.agent_id, slug_place(&channel).as_deref()).await;
    let (label, detail) =
        conversation_label(storage, &names, &q.agent_id, &channel, topic.as_deref()).await;
    let exists = conversation_exists(storage, &q.agent_id, &channel, topic.as_deref()).await;
    let (link, no_link_reason) =
        conversation_link(storage, &q.agent_id, &channel, topic.as_deref(), exists).await;

    Ok(ConversationUsage {
        label,
        detail,
        insight: insight(&all, rows.longest_failed_turn),
        summary: all.summary(),
        calls,
        more,
        link,
        no_link_reason,
    })
}

fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn money(x: f64) -> String {
    format!("${x:.2}")
}

/// A plain-language note when failed turns are most of a conversation's spend (R10). A fixed
/// template over the accounting, never a model call.
fn insight(t: &Tally, longest_failed_turn: Option<u32>) -> Option<String> {
    if t.failed_turns == 0 || failed_share(t)? < 0.5 {
        return None;
    }
    let wasted = match (t.failed_cost, t.cost) {
        (Some(f), Some(c)) => format!("{} of {} bought no answer.", money(f), money(c)),
        _ => format!(
            "{} of {} tokens bought no answer.",
            t.failed_tokens,
            t.tokens.total()
        ),
    };
    let mut text = format!(
        "{} failed (errored, timed out or {} aborted); {wasted}",
        plural(t.failed_turns, "turn", "turns"),
        if t.failed_turns == 1 { "was" } else { "were" },
    );
    if let Some(k) = longest_failed_turn.filter(|k| *k > 1) {
        text.push_str(&format!(" The longest ran {k} rounds."));
    }
    Some(text)
}

/// What a channel slug is, for labels and links. The slug formats are
/// `VizierChannelId::to_slug()`'s; this is the one place that reads them back.
enum SlugKind<'a> {
    Http,
    /// A project chat; the id is the project the conversation belongs to.
    Project(&'a str),
    /// A Workspace topic: the Workspace id (`specs/053-workspace-channel`).
    /// A removed Workspace (spec 064).
    Workspace,
    /// A consultation, by the consulting agent (spec 064).
    Consultation(&'a str),
    Discord(&'a str),
    Telegram(&'a str),
    Task(&'a str),
    Dream(&'a str),
    Background,
    System,
}

fn slug_kind(channel: &str) -> SlugKind<'_> {
    if channel.starts_with(crate::projects::channel::PREFIX) {
        SlugKind::Project(channel.rsplit_once("__").map_or("", |(_, id)| id))
    } else if channel.starts_with("workspace__") {
        SlugKind::Workspace
    } else if let Some(from) = channel.strip_prefix("consultation__") {
        SlugKind::Consultation(from)
    } else if channel.starts_with("http__") {
        SlugKind::Http
    } else if let Some(id) = channel.strip_prefix("discord__") {
        SlugKind::Discord(id)
    } else if let Some(id) = channel.strip_prefix("telegram__") {
        SlugKind::Telegram(id)
    } else if let Some(rest) = channel.strip_prefix("task__") {
        SlugKind::Task(rest.rsplit_once("__").map_or(rest, |(slug, _)| slug))
    } else if let Some(rest) = channel.strip_prefix("DREAM__") {
        SlugKind::Dream(rest.rsplit_once("__").map_or("", |(_, stage)| stage))
    } else if channel == "SUBAGENT" || channel.starts_with("inter_agent__") {
        SlugKind::Background
    } else {
        SlugKind::System
    }
}

/// How many places one report looks up names for; the rest show their ids.
const NAME_LOOKUPS: usize = 25;
/// How long a platform may take to answer one lookup.
const NAME_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
/// How long a looked-up name is reused; a failed lookup is retried sooner.
const NAME_TTL: std::time::Duration = std::time::Duration::from_secs(600);
const NAME_FAILED_TTL: std::time::Duration = std::time::Duration::from_secs(60);

type NameCache = HashMap<(String, String), (std::time::Instant, Option<ConversationNames>)>;

/// Names looked up on the platforms, shared across reports so reloading the page doesn't ask
/// Discord or Telegram again. Keyed by integration and place.
fn name_cache() -> &'static parking_lot::Mutex<NameCache> {
    static CACHE: std::sync::OnceLock<parking_lot::Mutex<NameCache>> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The Discord and Telegram names one report shows, looked up live through the agent's own
/// channel connection (`IntegrationKind::conversation_names`), in parallel, each bounded by
/// [`NAME_TIMEOUT`] and cached. A conversation whose bot is gone, disabled or unreachable keeps
/// its id.
#[derive(Default)]
pub struct NameBook(HashMap<String, ConversationNames>);

impl NameBook {
    pub async fn resolve<'p>(
        storage: &VizierStorage,
        kinds: &IntegrationKinds,
        agent_id: &str,
        places: impl IntoIterator<Item = &'p str>,
    ) -> Self {
        let wanted: Vec<&str> = places
            .into_iter()
            .filter(|p| p.starts_with("discord:") || p.starts_with("telegram:"))
            .take(NAME_LOOKUPS)
            .collect();
        if wanted.is_empty() {
            return Self::default();
        }
        let integrations = storage
            .list_agent_integrations(agent_id)
            .await
            .unwrap_or_default();
        // The agent's enabled connection for each platform, by the place prefix it names.
        let connection = |prefix: &str| {
            integrations.iter().filter(|i| i.enabled).find_map(|i| {
                let kind = kinds.get(&i.kind)?;
                (kind.platform()?.to_string() == prefix).then_some((i, kind))
            })
        };

        let now = std::time::Instant::now();
        let mut book = Self::default();
        let mut lookups = Vec::new();
        for place in wanted {
            let Some((prefix, chat_id)) = place.split_once(':') else {
                continue;
            };
            let Some((integration, kind)) = connection(prefix) else {
                continue;
            };
            let key = (integration.id.clone(), place.to_string());
            if let Some((at, cached)) = name_cache().lock().get(&key).cloned() {
                let ttl = if cached.is_some() {
                    NAME_TTL
                } else {
                    NAME_FAILED_TTL
                };
                if now.duration_since(at) < ttl {
                    if let Some(names) = cached {
                        book.0.insert(place.to_string(), names);
                    }
                    continue;
                }
            }
            if let Some(lookup) = kind.conversation_names(integration, chat_id) {
                lookups.push(async move {
                    let found = tokio::time::timeout(NAME_TIMEOUT, lookup)
                        .await
                        .ok()
                        .flatten();
                    (key, found)
                });
            }
        }

        for (key, found) in futures::future::join_all(lookups).await {
            name_cache()
                .lock()
                .insert(key.clone(), (std::time::Instant::now(), found.clone()));
            if let Some(names) = found {
                book.0.insert(key.1, names);
            }
        }
        book
    }

    /// The server and conversation names found for a place like `discord:123`.
    pub fn names(&self, place: &str) -> Option<&ConversationNames> {
        self.0.get(place)
    }

    /// `Vizier HQ / #general`, `Family chat`, or `fallback()` when nothing was found.
    fn label(&self, place: &str, fallback: impl FnOnce() -> String) -> String {
        let parts: Vec<&str> = self
            .0
            .get(place)
            .map(|n| {
                [n.space.as_deref(), n.conversation.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect()
            })
            .unwrap_or_default();
        if parts.is_empty() {
            fallback()
        } else {
            parts.join(" / ")
        }
    }
}

/// The place a Discord or Telegram channel slug is, for a name lookup.
pub fn slug_place(channel: &str) -> Option<String> {
    match slug_kind(channel) {
        SlugKind::Discord(id) => Some(format!("discord:{id}")),
        SlugKind::Telegram(id) => Some(format!("telegram:{id}")),
        _ => None,
    }
}

/// Appends ` / <topic title>` once the topic has a real title: a new topic is titled with its
/// own id (or `DEFAULT`) until one is generated.
fn with_topic(base: String, title: Option<String>, topic: Option<&str>) -> String {
    match title {
        Some(t) if Some(t.as_str()) != topic && t != "DEFAULT" => format!("{base} / {t}"),
        _ => base,
    }
}

/// Whether a conversation is still there to open. Dream turns keep no session history (the
/// dream journal is their record), so a dream conversation always exists; a WebUI topic exists
/// while its `session_detail` row does, which is what deleting the topic removes.
async fn conversation_exists(
    storage: &VizierStorage,
    agent_id: &str,
    channel: &str,
    topic: Option<&str>,
) -> bool {
    let found = match slug_kind(channel) {
        SlugKind::Dream(_) => return true,
        SlugKind::Http | SlugKind::Project(_) => {
            storage
                .session_detail_exists(agent_id, channel, topic)
                .await
        }
        _ => storage.conversation_exists(agent_id, channel, topic).await,
    };
    found.unwrap_or(false)
}

async fn task_title(storage: &VizierStorage, agent_id: &str, slug: &str) -> String {
    storage
        .get_task(agent_id.to_string(), slug.to_string())
        .await
        .ok()
        .flatten()
        .map(|t| t.title)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| slug.to_string())
}

/// Names come from storage only — no platform lookups on a person's read (R9).
/// What a Workspace's usage reads as: Workspaces were removed by spec 064, with their names, and
/// usage recorded in one stays (#82).
const REMOVED_WORKSPACE: &str = "Workspace (removed)";

async fn place_label(
    storage: &VizierStorage,
    names: &NameBook,
    agent_id: &str,
    place: &str,
) -> (String, Option<String>) {
    let (kind, rest) = place.split_once(':').unwrap_or((place, ""));
    match kind {
        "http" => {
            let (channel, topic) = rest.rsplit_once(':').unwrap_or((rest, ""));
            let title = storage
                .session_title(agent_id, channel, Some(topic).filter(|t| !t.is_empty()))
                .await
                .ok()
                .flatten();
            (
                title.unwrap_or_else(|| "WebUI topic".into()),
                Some("WebUI topic".into()),
            )
        }
        "workspace" => (REMOVED_WORKSPACE.into(), Some("Workspace".into())),
        "consultation" => {
            let (from, _) = rest.split_once(':').unwrap_or((rest, ""));
            (format!("Consultation from {from}"), Some("Consultation".into()))
        }
        "project" => {
            let (channel, topic) = rest.rsplit_once(':').unwrap_or((rest, ""));
            let project = match channel.rsplit_once("__").map(|(_, id)| id) {
                Some(id) => storage.get_project(agent_id, id).await.ok().flatten(),
                None => None,
            };
            let title = storage
                .session_title(agent_id, channel, Some(topic).filter(|t| !t.is_empty()))
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "Project chat".into());
            match project {
                Some(project) => (
                    format!("{} / {title}", project.name),
                    Some("Project".into()),
                ),
                None => (title, Some("Project".into())),
            }
        }
        "discord" => (
            names.label(place, || format!("Discord channel {rest}")),
            Some("Discord".into()),
        ),
        "telegram" => (
            names.label(place, || format!("Telegram chat {rest}")),
            Some("Telegram".into()),
        ),
        "task" => (
            task_title(storage, agent_id, rest).await,
            Some("Scheduled task".into()),
        ),
        "dreams" => ("Dream cycle".into(), Some("Unattended reflection".into())),
        "background" => (
            "Subagents".into(),
            Some("Sub-agent work, inline or in the background".into()),
        ),
        _ => ("System".into(), None),
    }
}

async fn conversation_label(
    storage: &VizierStorage,
    names: &NameBook,
    agent_id: &str,
    channel: &str,
    topic: Option<&str>,
) -> (String, Option<String>) {
    let title = storage
        .session_title(agent_id, channel, topic)
        .await
        .ok()
        .flatten();
    match slug_kind(channel) {
        SlugKind::Http => (
            title.unwrap_or_else(|| "WebUI topic".into()),
            Some("WebUI topic".into()),
        ),
        SlugKind::Project(id) => {
            let project = storage.get_project(agent_id, id).await.ok().flatten();
            let title = title.unwrap_or_else(|| "Project chat".into());
            match project {
                Some(project) => (
                    format!("{} / {title}", project.name),
                    Some("Project".into()),
                ),
                None => (title, Some("Project".into())),
            }
        }
        SlugKind::Workspace => (REMOVED_WORKSPACE.into(), Some("Workspace".into())),
        SlugKind::Consultation(from) => {
            (format!("Consultation from {from}"), Some("Consultation".into()))
        }
        // `Vizier HQ / #general / <topic>`: the live names, then the topic's title once it has one.
        SlugKind::Discord(id) => (
            with_topic(
                names.label(&format!("discord:{id}"), || format!("Discord channel {id}")),
                title,
                topic,
            ),
            Some("Discord".into()),
        ),
        SlugKind::Telegram(id) => (
            with_topic(
                names.label(&format!("telegram:{id}"), || format!("Telegram chat {id}")),
                title,
                topic,
            ),
            Some("Telegram".into()),
        ),
        SlugKind::Task(slug) => (
            task_title(storage, agent_id, slug).await,
            Some("Scheduled run".into()),
        ),
        SlugKind::Dream(stage) => (
            format!("Dream {}", if stage.is_empty() { "cycle" } else { stage }),
            Some("Unattended reflection".into()),
        ),
        // A piece's session title is its fresh topic id: say what it is instead.
        SlugKind::Background if channel == "SUBAGENT" => {
            ("Subtask".into(), Some("Subagent".into()))
        }
        SlugKind::Background => (
            "Agent conversation".into(),
            Some("Delegation or consult".into()),
        ),
        SlugKind::System => ("System".into(), None),
    }
}

/// Where the WebUI can open a conversation, or why it can't (R10).
async fn conversation_link(
    storage: &VizierStorage,
    agent_id: &str,
    channel: &str,
    topic: Option<&str>,
    exists: bool,
) -> (Option<UsageLink>, Option<NoLinkReason>) {
    let link = |kind, path: String| (Some(UsageLink { kind, path }), None);
    let none = |reason| (None, Some(reason));
    match slug_kind(channel) {
        _ if !exists => none(NoLinkReason::Deleted),
        SlugKind::Http => match topic {
            Some(t) => link(UsageLinkKind::Chat, format!("/{agent_id}/chat/{t}")),
            None => link(UsageLinkKind::Chat, format!("/{agent_id}/chat")),
        },
        SlugKind::Project(id) => match topic {
            Some(t) => link(
                UsageLinkKind::Topic,
                format!("/{agent_id}/projects/{id}/chat/{t}"),
            ),
            None => none(NoLinkReason::NoWebPage),
        },
        SlugKind::Workspace => none(NoLinkReason::Deleted),
        // Opened from the consulted agent's conversation list, which needs edit rights on it;
        // the usage report links nowhere yet (spec 064 US7).
        SlugKind::Consultation(_) => none(NoLinkReason::NoWebPage),
        SlugKind::Task(slug) => link(
            UsageLinkKind::TaskRun,
            format!("/{agent_id}/tasks?task={slug}"),
        ),
        SlugKind::Dream(_) => link(UsageLinkKind::Dream, format!("/{agent_id}/dream")),
        // A subagent's conversation has no page of its own, and since spec 058 no row names
        // the conversation that started it.
        SlugKind::Background => none(NoLinkReason::NoWebPage),
        SlugKind::Discord(_) | SlugKind::Telegram(_) | SlugKind::System => {
            none(NoLinkReason::NoWebPage)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tally(cost: f64, failed_cost: f64, failed_turns: u64) -> Tally {
        Tally {
            tokens: TokenCounts {
                input: 1000,
                output: 100,
                ..Default::default()
            },
            calls: 10,
            cost: Some(cost),
            failed_cost: Some(failed_cost),
            failed_tokens: 600,
            failed_turns,
            ..Default::default()
        }
    }

    #[test]
    fn the_insight_appears_only_when_failed_turns_are_most_of_the_spend() {
        let text = insight(&tally(4.12, 3.94, 1), Some(46)).unwrap();
        assert_eq!(
            text,
            "1 turn failed (errored, timed out or was aborted); $3.94 of $4.12 bought no answer. \
             The longest ran 46 rounds."
        );
        assert!(
            insight(&tally(1.0, 0.6, 2), None)
                .unwrap()
                .starts_with("2 turns failed")
        );
        assert!(insight(&tally(1.0, 0.4, 2), Some(3)).is_none());
        assert!(
            insight(&tally(1.0, 0.9, 0), None).is_none(),
            "no failed turn, no insight"
        );
    }

    #[test]
    fn slugs_read_back_into_their_kind() {
        assert!(matches!(slug_kind("http__alice__webui"), SlugKind::Http));
        assert!(matches!(
            slug_kind("project__alice__p3xY9aQ2mZ"),
            SlugKind::Project("p3xY9aQ2mZ")
        ));
        assert!(matches!(slug_kind("discord__42"), SlugKind::Discord("42")));
        assert!(matches!(
            slug_kind("task__morning__brief__1700000000000"),
            SlugKind::Task("morning__brief")
        ));
        assert!(matches!(
            slug_kind("DREAM__a__discord__1__DEFAULT__extraction"),
            SlugKind::Dream("extraction")
        ));
        assert!(matches!(slug_kind("SUBAGENT"), SlugKind::Background));
        assert!(matches!(
            slug_kind("inter_agent__[a__b]"),
            SlugKind::Background
        ));
        assert!(matches!(slug_kind("SYSTEM"), SlugKind::System));
    }

    #[test]
    fn discord_and_telegram_labels_read_space_conversation_topic() {
        let mut book = NameBook::default();
        book.0.insert(
            "discord:1".into(),
            ConversationNames {
                space: Some("Vizier HQ".into()),
                conversation: Some("#general".into()),
            },
        );
        book.0.insert(
            "telegram:-5".into(),
            ConversationNames {
                space: None,
                conversation: Some("Family chat".into()),
            },
        );
        let fallback = || "Discord channel 2".to_string();
        assert_eq!(book.label("discord:1", fallback), "Vizier HQ / #general");
        assert_eq!(book.label("telegram:-5", fallback), "Family chat");
        assert_eq!(
            book.label("discord:2", fallback),
            "Discord channel 2",
            "not looked up"
        );

        let base = || book.label("discord:1", fallback);
        assert_eq!(
            with_topic(base(), Some("Trip planning".into()), Some("9f2c")),
            "Vizier HQ / #general / Trip planning"
        );
        assert_eq!(
            with_topic(base(), Some("9f2c".into()), Some("9f2c")),
            "Vizier HQ / #general",
            "untitled topic"
        );
        assert_eq!(
            with_topic(base(), Some("DEFAULT".into()), None),
            "Vizier HQ / #general"
        );
        assert_eq!(with_topic(base(), None, None), "Vizier HQ / #general");
        assert_eq!(slug_place("discord__42").as_deref(), Some("discord:42"));
        assert_eq!(slug_place("http__u__webui"), None);
    }

    #[test]
    fn unpriced_sorts_last_and_cost_sorts_descending() {
        let mut v = vec![
            (None, 900u64),
            (Some(1.0), 10),
            (Some(3.0), 5),
            (None, 1000),
        ];
        v.sort_by(|a, b| by_cost_then_tokens(*a, *b));
        assert_eq!(
            v,
            vec![(Some(3.0), 5), (Some(1.0), 10), (None, 1000), (None, 900)]
        );
    }

    fn bedrock_catalogue() -> Catalogue {
        Catalogue::new(
            crate::catalogue::data::parse(
                br#"{"amazon-bedrock": {"id": "amazon-bedrock", "name": "Amazon Bedrock",
                    "npm": "@ai-sdk/amazon-bedrock", "models": {
                      "amazon.nova-lite-v1:0": {"id": "amazon.nova-lite-v1:0", "tool_call": true,
                        "cost": {"input": 0.06, "output": 0.24}},
                      "anthropic.claude-haiku-4-5-20251001-v1:0": {"id": "anthropic.claude-haiku-4-5-20251001-v1:0",
                        "tool_call": true, "cost": {"input": 1.0, "output": 5.0, "cache_read": 0.1, "cache_write": 1.25}},
                      "amazon.nova-pro-v1:0": {"id": "amazon.nova-pro-v1:0", "tool_call": true}
                    }}}"#,
            )
            .unwrap(),
        )
    }

    /// T037: a Converse reply's usage is priced from the catalogue; a model with no price shows
    /// no invented cost, and Bedrock's embedding/image models are not chat-priced at all.
    /// spec 065 research R6: local is free; then the catalogue's token price, then the shipped
    /// table's token rates, then its unit price; anything else is unpriced, never zero.
    #[test]
    fn pricer_media_order() {
        let c = bedrock_catalogue();
        let mut p = Pricer::new(&c);
        let none = MediaUnits::default();
        let no_tokens = TokenCounts::default();
        let tokens = |input| TokenCounts { input, ..Default::default() };

        let local = p.priced(UsageKind::Speech, "kokoro", "Kokoro-82M", None, true, &none, &no_tokens);
        assert_eq!(local, (Some(0.0), SourceId::LOCAL));

        let catalogue = p.priced(
            UsageKind::Embedding, "amazon-bedrock", "amazon.nova-lite-v1:0", None, false, &none, &tokens(1_000_000),
        );
        assert_eq!(catalogue.1, SourceId::MODELS_DEV);
        assert!((catalogue.0.unwrap() - 0.06).abs() < 1e-9);

        let chars = MediaUnits { characters: 1_000_000, ..Default::default() };
        let speech = p.priced(UsageKind::Speech, "openai", "tts-1", None, false, &chars, &no_tokens);
        assert_eq!(speech.1, SourceId::CURATED);
        assert!((speech.0.unwrap() - 15.0).abs() < 1e-9);

        let minute = MediaUnits { audio_ms: 60_000, ..Default::default() };
        let heard = p.priced(UsageKind::Transcription, "openai", "whisper-1", None, false, &minute, &no_tokens);
        assert!((heard.0.unwrap() - 0.006).abs() < 1e-9);

        // Reported tokens beat the per-minute estimate.
        let by_tokens = p.priced(
            UsageKind::Transcription, "openai", "gpt-4o-transcribe", None, false, &minute, &tokens(1_000_000),
        );
        assert_eq!(by_tokens.1, SourceId::CURATED);
        assert!((by_tokens.0.unwrap() - 2.5).abs() < 1e-9);

        let image = MediaUnits { images: 3, ..Default::default() };
        let unknown = p.priced(UsageKind::Image, "openai", "dall-e-3", Some("1024x1024"), false, &image, &no_tokens);
        assert_eq!(unknown, (None, SourceId::NONE));
    }

    #[test]
    fn a_converse_reply_is_priced_from_the_catalogue() {
        let c = bedrock_catalogue();
        let mut pricer = Pricer::new(&c);
        let tokens = TokenCounts {
            input: 1_000_000,
            output: 500_000,
            cached: 0,
            cache_write: 0,
        };
        let cost = pricer
            .cost("amazon-bedrock", "amazon.nova-lite-v1:0", &tokens)
            .unwrap();
        assert!((cost - (0.06 + 0.12)).abs() < 1e-9, "{cost}");

        assert_eq!(
            pricer.cost("amazon-bedrock", "amazon.nova-pro-v1:0", &tokens),
            None,
            "a catalogue model without a price gets no invented cost"
        );
        for media in [
            "amazon.titan-embed-text-v2:0",
            "cohere.embed-english-v3",
            "amazon.nova-canvas-v1:0",
            "amazon.titan-image-generator-v2:0",
        ] {
            assert!(
                pricer.cost("amazon-bedrock", media, &tokens).is_none(),
                "{media} must not be chat-priced"
            );
        }
        assert!(
            c.model(&ProviderId::AMAZON_BEDROCK, "amazon.nova-canvas-v1:0")
                .is_none()
        );
    }

    /// FR-034, FR-034b: an ARN's saved entry decides its price before the catalogue is asked —
    /// the hand-entered figure, then the paired model, then the existing unpriced `None`.
    #[test]
    fn an_arn_is_priced_from_its_entry_before_the_catalogue() {
        let c = bedrock_catalogue();
        let arn = "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc";
        let key = ("amazon-bedrock-sso".to_string(), arn.to_string());
        let tokens = TokenCounts {
            input: 1_000_000,
            output: 500_000,
            ..Default::default()
        };
        let entry = |price: Option<Cost>, model: Option<&str>| ProfileEntry {
            arn: arn.into(),
            price,
            model: model.map(str::to_string),
            ..Default::default()
        };
        let hand = Cost { input: 2.0, output: 8.0, cache_read: None, cache_write: None };

        // The hand-entered price wins over the paired model, which the catalogue would price.
        let mut priced = Pricer::with_entries(
            &c,
            HashMap::from([(key.clone(), entry(Some(hand), Some("amazon.nova-lite-v1:0")))]),
        );
        let cost = priced.cost("amazon-bedrock-sso", arn, &tokens).unwrap();
        assert!((cost - (2.0 + 4.0)).abs() < 1e-9, "{cost}");

        // No hand-entered price: the paired model's catalogue cost, through the provider alias.
        let mut paired = Pricer::with_entries(
            &c,
            HashMap::from([(key.clone(), entry(None, Some("amazon.nova-lite-v1:0")))]),
        );
        let cost = paired.cost("amazon-bedrock-sso", arn, &tokens).unwrap();
        assert!((cost - (0.06 + 0.12)).abs() < 1e-9, "{cost}");

        // Nothing to price from — no pairing, and a pair the catalogue carries without a price —
        // is unpriced, never zero.
        for model in [None, Some("amazon.nova-pro-v1:0")] {
            let mut unpriced = Pricer::with_entries(
                &c,
                HashMap::from([(key.clone(), entry(None, model))]),
            );
            assert_eq!(unpriced.cost("amazon-bedrock-sso", arn, &tokens), None, "{model:?}");
        }

        // A catalogue model id is untouched by the entry map.
        let mut plain = Pricer::with_entries(&c, HashMap::from([(key, entry(None, None))]));
        assert!(plain.cost("amazon-bedrock", "amazon.nova-lite-v1:0", &tokens).is_some());
    }

    mod report {
        use std::sync::Arc;

        use rusqlite::Connection;

        use super::super::*;
        use crate::{
            schema::usage::UsageEvent,
            storage::{document::BlobDocumentStore, sqlite::SqlStorage},
        };

        const DAY: i64 = 86_400_000;

        fn storage() -> (VizierStorage, tempfile::TempDir) {
            let dir = tempfile::tempdir().unwrap();
            let conn = Connection::open_in_memory().unwrap();
            crate::storage::sqlite::init_history_schema(&conn).unwrap();
            crate::storage::sqlite::init_usage_schema(&conn).unwrap();
            conn.execute_batch(
                "CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE session_detail (id TEXT PRIMARY KEY, agent_id TEXT NOT NULL,
                     channel TEXT NOT NULL, topic TEXT, data TEXT NOT NULL);
                 CREATE TABLE task (agent_id TEXT, slug TEXT, data TEXT);
                 CREATE TABLE provider_config (variant TEXT PRIMARY KEY, data TEXT NOT NULL);",
            )
            .unwrap();
            let storage = VizierStorage::new(SqlStorage::new(
                crate::storage::sql::SqlStore::sqlite(conn),
                Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
            ));
            (storage, dir)
        }

        /// A catalogue model with a price, so the tests don't hard-code one.
        fn priced(catalogue: &Catalogue) -> (String, String) {
            let data = catalogue.snapshot();
            data.providers
                .iter()
                .find_map(|(id, p)| {
                    p.models
                        .iter()
                        .find(|(_, m)| m.cost.as_ref().is_some_and(|c| c.input > 0.0))
                        .map(|(m, _)| (id.to_string(), m.clone()))
                })
                .expect("the seed has a priced model")
        }

        fn at(ms: i64) -> Timestamp {
            crate::time::from_millis_opt(ms).unwrap()
        }

        fn event(ms: i64, channel: &str, provider: &str, model: &str, input: u64) -> UsageEvent {
            UsageEvent {
                agent_id: "a".into(),
                at: at(ms),
                activity: UsageActivity::Chat,
                channel: Some(channel.into()),
                topic: None,
                place: format!("discord:{channel}"),
                turn_id: None,
                round: Some(1),
                provider: provider.into(),
                model: model.into(),
                served_model: Some(format!("{model}-dated")),
                tokens: TokenCounts {
                    input,
                    output: input / 10,
                    ..Default::default()
                },
                duration_ms: 1,
                legacy: false,
                kind: crate::schema::usage::UsageKind::Text,
                variant: None,
                units: Default::default(),
                measured: false,
                local: false,
                call_id: None,
            }
        }

        fn q() -> UsageQuery {
            // Seven whole days on the instance's clock, starting at a local midnight.
            let start = crate::time::start_of_local_day(chrono::NaiveDate::from_ymd_opt(1970, 1, 2).unwrap());
            UsageQuery {
                agent_id: "a".into(),
                start,
                end: start + chrono::Duration::days(7) - chrono::Duration::milliseconds(1),
                activity: None,
                kind: None,
                conversation: None,
            }
        }

        /// T038: a report prices a saved profile by its ARN — the report's own pricer, not the
        /// catalogue alone (FR-034, FR-034a.1).
        #[tokio::test]
        async fn a_report_prices_a_saved_arns_entry() {
            use crate::schema::{ProviderCredentials, ProviderEntry};

            let (s, _d) = storage();
            let catalogue = super::bedrock_catalogue();
            let arn = "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc";
            s.upsert_provider(&ProviderEntry {
                id: ProviderId::AMAZON_BEDROCK_SSO,
                credentials: ProviderCredentials {
                    profiles: vec![ProfileEntry {
                        arn: arn.into(),
                        price: Some(Cost {
                            input: 2.0,
                            output: 8.0,
                            cache_read: None,
                            cache_write: None,
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            })
            .await
            .unwrap();
            s.record_usage(event(DAY, "discord__1", "amazon-bedrock-sso", arn, 1_000_000))
                .await
                .unwrap();

            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q())
                .await
                .unwrap();
            let cost = r.summary.cost_usd.expect("priced from the entry");
            // 1M input at 2.0/M and 100k output at 8.0/M — the hand-entered figure, not 0.
            assert!((cost - (2.0 + 0.8)).abs() < 1e-9, "{cost}");
            assert!(r.unpriced.models.is_empty(), "the ARN is priced: {:?}", r.unpriced.models);
        }

        /// T044 (FR-034a.5, FR-034b, SC-021): a missing or stale price still records the turn.
        /// The usage row keeps the provider, the ARN as the model and the tokens, with an absent
        /// cost — never 0, never a fabricated figure — and the report never fails over it.
        #[tokio::test]
        async fn an_unpriced_or_stale_arn_still_records_its_row() {
            use crate::schema::{ProviderCredentials, ProviderEntry};

            let (s, _d) = storage();
            let catalogue = super::bedrock_catalogue();
            let unpriced =
                "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/unpriced";
            let stale =
                "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/stale";
            s.upsert_provider(&ProviderEntry {
                id: ProviderId::AMAZON_BEDROCK_SSO,
                credentials: ProviderCredentials {
                    profiles: vec![
                        // Nothing paired and no hand-entered price.
                        ProfileEntry { arn: unpriced.into(), ..Default::default() },
                        // A pairing the catalogue has since dropped: the price is stale, the
                        // selection and the turn are not.
                        ProfileEntry {
                            arn: stale.into(),
                            model: Some("anthropic.claude-3-5-sonnet-20240620-v1:0".into()),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
            })
            .await
            .unwrap();
            s.record_usage(event(DAY, "discord__1", "amazon-bedrock-sso", unpriced, 1_000_000))
                .await
                .unwrap();
            s.record_usage(event(DAY + 1, "discord__1", "amazon-bedrock-sso", stale, 500_000))
                .await
                .unwrap();

            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q())
                .await
                .unwrap();

            // Both calls are recorded: provider and ARN survive, and the tokens are the row's own.
            assert_eq!(r.by_model.len(), 2, "{:?}", r.by_model);
            let row = |arn: &str| r.by_model.iter().find(|row| row.model == arn).unwrap();
            for (arn, tokens) in [(unpriced, 1_100_000), (stale, 550_000)] {
                let row = row(arn);
                assert_eq!(row.provider, "amazon-bedrock-sso");
                assert_eq!(row.row.tokens, tokens, "input plus output, recorded as sent");
                assert_eq!(row.row.cost_usd, None, "unpriced is absent, never 0 (FR-034b)");
                assert!(!row.priced);
            }

            // The report states the absence rather than costing it: no summary figure, and both
            // models named as unpriced with their token counts.
            assert_eq!(r.summary.cost_usd, None, "nothing was costed at zero");
            assert_eq!(r.unpriced.tokens, 1_650_000);
            for arn in [unpriced, stale] {
                assert!(
                    r.unpriced.models.contains(&format!("amazon-bedrock-sso/{arn}")),
                    "{} missing from {:?}",
                    arn,
                    r.unpriced.models
                );
            }
        }
        /// T064 (FR-034, US5 acceptance 1): a completion answered over the SSO connection is
        /// attributed to `amazon-bedrock-sso` — its own provider, its own tokens — and priced
        /// through the entry map, while the same model answered by static Bedrock is a second row.
        #[tokio::test]
        async fn an_sso_completion_is_attributed_and_priced_apart_from_static_bedrock() {
            use crate::schema::{ProviderCredentials, ProviderEntry};

            let (s, _d) = storage();
            let catalogue = super::bedrock_catalogue();
            let arn = "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc";
            let model = "amazon.nova-lite-v1:0";
            s.upsert_provider(&ProviderEntry {
                id: ProviderId::AMAZON_BEDROCK_SSO,
                credentials: ProviderCredentials {
                    profiles: vec![ProfileEntry {
                        arn: arn.into(),
                        price: Some(Cost {
                            input: 2.0,
                            output: 8.0,
                            cache_read: None,
                            cache_write: None,
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            })
            .await
            .unwrap();

            // The event `VizierModel::record` writes for an answered call: the provider that
            // answered, the selected ARN as the model, the tokens the reply reported.
            let mut answered = event(DAY, "discord__1", "amazon-bedrock-sso", arn, 1_000_000);
            answered.served_model = Some(model.into());
            s.record_usage(answered).await.unwrap();
            // The other Bedrock provider answers the very same catalogue model.
            s.record_usage(event(DAY, "discord__1", "amazon-bedrock", model, 1_000_000))
                .await
                .unwrap();

            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q())
                .await
                .unwrap();

            // Two rows, never merged: the ARN under the SSO provider and the model under the
            // static one, each carrying only its own tokens.
            assert_eq!(r.by_model.len(), 2, "{:?}", r.by_model);
            let row = |provider: &str| {
                r.by_model
                    .iter()
                    .find(|row| row.provider == provider)
                    .unwrap_or_else(|| panic!("no row for {provider}: {:?}", r.by_model))
            };
            let sso = row("amazon-bedrock-sso");
            assert_eq!(sso.model, arn);
            assert_eq!(sso.row.tokens, 1_100_000, "input plus output, recorded as sent");
            assert!(sso.served_models.contains(&model.to_string()), "{:?}", sso.served_models);
            // Priced from the entry map: 1M input at 2.0/M, 100k output at 8.0/M.
            let cost = sso.row.cost_usd.expect("priced from the entry");
            assert!((cost - (2.0 + 0.8)).abs() < 1e-9, "{cost}");
            assert!(sso.priced);

            let plain = row("amazon-bedrock");
            assert_eq!(plain.model, model);
            assert_eq!(plain.row.tokens, 1_100_000, "the SSO row's tokens did not merge in");
            assert!(plain.priced, "the catalogue price still applies to static Bedrock");

            // And the summary counts both, each at its own price.
            let total = r.summary.cost_usd.expect("both priced");
            assert!((total - (2.8 + 0.084)).abs() < 1e-9, "{total}");
            assert!(r.unpriced.models.is_empty(), "{:?}", r.unpriced.models);
        }



        #[tokio::test]
        async fn a_model_switch_splits_by_model_and_an_unpriced_model_is_reported() {
            let (s, _d) = storage();
            let catalogue = Catalogue::new(crate::catalogue::seed::load_seed());
            let (provider, model) = priced(&catalogue);
            s.record_usage(event(DAY, "discord__1", &provider, &model, 1_000_000))
                .await
                .unwrap();
            s.record_usage(event(5 * DAY, "discord__1", "custom-local", "qwen", 50_000))
                .await
                .unwrap();

            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q())
                .await
                .unwrap();
            assert_eq!(r.daily.len(), 7, "every date, empty ones too");
            assert_eq!(r.by_model.len(), 2);
            assert!(r.by_model[0].priced, "priced first");
            assert_eq!(r.by_model[0].served_models, vec![format!("{model}-dated")]);
            assert!(!r.by_model[1].priced);
            assert_eq!(r.unpriced.tokens, 55_000);
            assert_eq!(r.unpriced.models, vec!["custom-local/qwen".to_string()]);
            let priced_cost = r.by_model[0].row.cost_usd.unwrap();
            assert!(
                (r.summary.cost_usd.unwrap() - priced_cost).abs() < 1e-12,
                "unpriced excluded"
            );
            assert!(r.previous.is_none(), "nothing before the range");
            assert!(r.summary.share_of_all.is_none());
        }

        #[tokio::test]
        async fn costliest_ranks_by_cost_caps_at_ten_and_links_by_kind() {
            let (s, _d) = storage();
            let catalogue = Catalogue::new(crate::catalogue::seed::load_seed());
            let (provider, model) = priced(&catalogue);
            for i in 0..12u64 {
                let channel = format!("discord__{i}");
                s.record_usage(event(DAY, &channel, &provider, &model, 1000 * (i + 1)))
                    .await
                    .unwrap();
            }
            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q())
                .await
                .unwrap();
            assert_eq!(r.costliest.len(), 10);
            assert_eq!(r.costliest[0].channel, "discord__11");
            assert!(!r.costliest[0].exists, "no history saved for it");
            assert_eq!(r.costliest[0].label, "Discord channel 11");

            let mut conv = q();
            conv.conversation = Some(("discord__11".into(), None));
            let d = conversation_usage(&s, &catalogue, &IntegrationKinds::builtin(), conv, 20)
                .await
                .unwrap();
            assert_eq!(d.calls.len(), 1);
            assert_eq!(d.no_link_reason, Some(NoLinkReason::Deleted));
            assert!(d.insight.is_none());

            let (link, reason) =
                conversation_link(&s, "a", "http__u__webui", Some("t1"), true).await;
            assert_eq!(link.unwrap().path, "/a/chat/t1");
            assert!(reason.is_none());
            let (link, _) = conversation_link(&s, "a", "task__brief__17", None, true).await;
            assert_eq!(link.unwrap().kind, UsageLinkKind::TaskRun);
            let (link, _) = conversation_link(&s, "a", "DREAM__x__extraction", None, true).await;
            assert_eq!(link.unwrap().kind, UsageLinkKind::Dream);
            let (link, reason) = conversation_link(&s, "a", "discord__1", None, true).await;
            assert!(link.is_none());
            assert_eq!(reason, Some(NoLinkReason::NoWebPage));
        }

        #[tokio::test]
        async fn a_filter_narrows_the_report_and_reports_its_share() {
            let (s, _d) = storage();
            let catalogue = Catalogue::new(crate::catalogue::seed::load_seed());
            let (provider, model) = priced(&catalogue);
            s.record_usage(event(DAY, "discord__1", &provider, &model, 3000))
                .await
                .unwrap();
            let mut dream = event(DAY, "DREAM__x__extraction", &provider, &model, 1000);
            dream.activity = UsageActivity::Dreams;
            dream.place = "dreams".into();
            s.record_usage(dream).await.unwrap();

            let mut filtered = q();
            filtered.activity = Some(UsageActivity::Dreams);
            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), filtered)
                .await
                .unwrap();
            assert!(r.by_activity.is_empty());
            assert_eq!(r.summary.calls, 1);
            assert!((r.summary.share_of_all.unwrap() - 0.25).abs() < 1e-9);
            assert_eq!(r.by_place.len(), 1);
            assert_eq!(r.by_place[0].label, "Dream cycle");
        }

        /// SC-006: a 90-day report over 100k events in under 2 s. Run with `--ignored`.
        #[tokio::test]
        #[ignore]
        async fn a_ninety_day_report_over_100k_events_is_fast() {
            let (s, _d) = storage();
            let catalogue = Catalogue::new(crate::catalogue::seed::load_seed());
            let (provider, model) = priced(&catalogue);
            let activities = UsageActivity::ALL;
            let mut batch = Vec::with_capacity(100_000);
            for i in 0..100_000i64 {
                let mut e = event(
                    (i * 90 * DAY) / 100_000,
                    &format!("discord__{}", i % 400),
                    &provider,
                    &model,
                    1000 + (i as u64 % 5000),
                );
                e.activity = activities[(i as usize) % activities.len()];
                e.place = format!("discord:{}", i % 400);
                e.turn_id = Some(format!("t{}", i / 4));
                batch.push(e);
            }
            s.insert_legacy_usage(batch).await.unwrap();
            let q = UsageQuery {
                agent_id: "a".into(),
                start: at(0),
                end: at(90 * DAY),
                activity: None,
                kind: None,
                conversation: None,
            };
            let started = std::time::Instant::now();
            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q)
                .await
                .unwrap();
            let took = started.elapsed();
            assert_eq!(r.summary.calls, 100_000);
            println!("90-day report over 100k events: {took:?}");
            assert!(took < std::time::Duration::from_secs(2), "{took:?}");
        }

        /// Spec 065 SC-005: the same report with 500k embedding records beside 100k completions,
        /// in under 2 s. Run with `--ignored`.
        #[tokio::test]
        #[ignore]
        async fn a_ninety_day_report_with_500k_embeddings_is_fast() {
            let (s, _d) = storage();
            let catalogue = Catalogue::new(crate::catalogue::seed::load_seed());
            let (provider, model) = priced(&catalogue);
            let mut batch = Vec::with_capacity(100_000);
            for i in 0..100_000i64 {
                let mut e = event((i * 90 * DAY) / 100_000, &format!("discord__{}", i % 400), &provider, &model, 1000);
                e.turn_id = Some(format!("t{}", i / 4));
                batch.push(e);
            }
            s.insert_legacy_usage(batch).await.unwrap();
            for chunk in 0..5i64 {
                let mut batch = Vec::with_capacity(100_000);
                for j in 0..100_000i64 {
                    let i = chunk * 100_000 + j;
                    let mut e = event((i * 90 * DAY) / 500_000, &format!("discord__{}", i % 400), "openai", "text-embedding-3-small", 200);
                    e.kind = UsageKind::Embedding;
                    e.tokens.output = 0;
                    e.activity = if i % 3 == 0 { UsageActivity::Indexing } else { UsageActivity::Chat };
                    batch.push(e);
                }
                s.insert_legacy_usage(batch).await.unwrap();
            }
            let q = UsageQuery {
                agent_id: "a".into(),
                start: at(0),
                end: at(90 * DAY),
                activity: None,
                kind: None,
                conversation: None,
            };
            let started = std::time::Instant::now();
            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q)
                .await
                .unwrap();
            let took = started.elapsed();
            assert_eq!(r.summary.calls, 600_000);
            assert!(r.by_kind.iter().any(|k| k.kind == UsageKind::Embedding && k.row.cost_usd.is_some()));
            println!("90-day report over 100k completions and 500k embeddings: {took:?}");
            assert!(took < std::time::Duration::from_secs(2), "{took:?}");
        }

        /// Spec 068 US2, US3, US4 (quickstart steps 1, 3–5): every row names where its price
        /// came from; local and subscription usage is free and never unpriced; a proxy borrows
        /// its maker's price; a model nobody prices stays unpriced; the report names its sources.
        #[tokio::test]
        async fn rows_name_their_price_source() {
            let (s, _dir) = storage();
            let catalogue = Catalogue::standard(crate::catalogue::seed::load_seed());
            for (provider, model) in [
                ("dummyplug", "dummyplug"),
                ("ollama", "llama3.2"),
                ("chatgpt", "gpt-5.1-codex"),
                ("my-proxy", "gpt-4o-mini"),
                ("my-proxy", "mystery-1"),
                ("openai", "gpt-4o-mini"),
            ] {
                s.record_usage(event(DAY, "discord__1", provider, model, 1_000_000)).await.unwrap();
            }
            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q())
                .await
                .unwrap();
            let row = |p: &str, m: &str| {
                r.by_model
                    .iter()
                    .find(|row| row.provider == p && row.model == m)
                    .unwrap_or_else(|| panic!("{p}/{m} in {:?}", r.by_model))
            };
            for (p, m, label) in [
                ("dummyplug", "dummyplug", "local"),
                ("ollama", "llama3.2", "local"),
                ("chatgpt", "gpt-5.1-codex", "subscription"),
            ] {
                let row = row(p, m);
                assert_eq!(row.price_source, label, "{p}/{m}");
                assert_eq!(row.row.cost_usd, Some(0.0), "{p}/{m} is free");
                assert!(row.priced);
            }
            let borrowed = row("my-proxy", "gpt-4o-mini");
            assert_eq!(borrowed.price_source, "borrowed");
            assert_eq!(borrowed.borrowed_from.as_deref(), Some("openai"), "the maker, not a reseller");
            let own = row("openai", "gpt-4o-mini");
            assert_eq!(own.price_source, "models_dev");
            assert_eq!(borrowed.row.cost_usd, own.row.cost_usd, "the maker's price");
            let unknown = row("my-proxy", "mystery-1");
            assert_eq!(unknown.price_source, "none");
            assert_eq!(unknown.row.cost_usd, None, "never shown as $0");
            assert_eq!(r.unpriced.models, vec!["my-proxy/mystery-1".to_string()]);
            assert!(r.sources.iter().any(|s| s.id == "models_dev" && s.name == "models.dev"));
            assert!(
                r.sources.iter().all(|s| !["local", "subscription", "borrowed", "none"].contains(&s.id.as_str())),
                "rules are not sources: {:?}",
                r.sources
            );
        }
    }
}
