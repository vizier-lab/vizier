//! Usage reports (`specs/018-usage-accounting/`): price grouped usage rows with the catalogue,
//! and resolve the names and links the usage page shows. Storage groups; this prices (R7).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::Result;
use chrono::{DateTime, Duration, NaiveDate, Utc};

use crate::{
    catalogue::{Catalogue, data::Cost},
    integrations::{IntegrationKinds, kind::ConversationNames},
    config::provider::ProviderId,
    llm::DialectKind,
    schema::{
        VizierChannelId,
        metrics::{TokenCounts, add_cost, estimate_cost},
        usage::{
            BreakdownRow, CallRow, ConversationRow, ConversationUsage, DailyUsage, DayActivity,
            FailedTotals, ModelRow, NoLinkReason, PeriodTotals, UnpricedNote, UsageActivity,
            UsageLink, UsageLinkKind, UsageRange, UsageReport, UsageSummary,
        },
    },
    storage::{
        VizierStorage,
        background_job::BackgroundJobStorage,
        integration::IntegrationStorage,
        state::StateStorage,
        task::TaskStorage,
        usage::{UsageGroup, UsageGrouping, UsageQuery, UsageStorage},
    },
};

/// The most expensive conversations the report lists (FR-017).
const COSTLIEST: usize = 10;

/// Catalogue prices, looked up once per `(provider, model)` per report.
pub struct Pricer<'a> {
    catalogue: &'a Catalogue,
    prices: HashMap<(String, String), Option<Cost>>,
    outside: HashMap<String, bool>,
}

impl<'a> Pricer<'a> {
    pub fn new(catalogue: &'a Catalogue) -> Self {
        Self {
            catalogue,
            prices: HashMap::new(),
            outside: HashMap::new(),
        }
    }

    /// Whether the provider counts cached tokens outside `input` (Anthropic) rather than inside.
    fn cache_outside_input(&mut self, provider: &str) -> bool {
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

    /// Custom providers have no prices yet, and legacy rows may have no model: both unpriced.
    fn price(&mut self, provider: &str, model: &str) -> Option<Cost> {
        let key = (provider.to_string(), model.to_string());
        if let Some(c) = self.prices.get(&key) {
            return c.clone();
        }
        let c = provider
            .parse::<ProviderId>()
            .ok()
            .and_then(|id| self.catalogue.model(&id, model))
            .and_then(|m| m.cost);
        self.prices.insert(key, c.clone());
        c
    }

    pub fn cost(&mut self, provider: &str, model: &str, tokens: &TokenCounts) -> Option<f64> {
        let outside = self.cache_outside_input(provider);
        self.price(provider, model)
            .map(|c| estimate_cost(tokens, &c, outside))
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

/// Running totals for one bucket of groups.
#[derive(Debug, Default, Clone)]
struct Tally {
    tokens: TokenCounts,
    calls: u64,
    cost: Option<f64>,
    cached: u64,
    whole_input: u64,
    failed_tokens: u64,
    failed_cost: Option<f64>,
    failed_turns: u64,
    last_at: Option<DateTime<Utc>>,
}

impl Tally {
    fn add(&mut self, pricer: &mut Pricer, g: &UsageGroup) -> Option<f64> {
        let cost = pricer.cost(&g.provider, &g.model, &g.tokens);
        let (cached, whole) = pricer.cache_parts(&g.provider, &g.tokens);
        self.tokens += g.tokens;
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
        }
    }

    fn row(&self, key: String, label: String, detail: Option<String>, total: Option<f64>) -> BreakdownRow {
        BreakdownRow {
            key,
            label,
            detail,
            cost_usd: self.cost,
            share: share(self.cost, total),
            tokens: self.tokens.total(),
            calls: self.calls,
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
    let mut pricer = Pricer::new(catalogue);

    let activity_groups = storage.usage_groups(&q, UsageGrouping::Activity).await?;
    let all = tally(&mut pricer, &activity_groups);
    let mut summary = all.summary();

    if q.activity.is_some() {
        let unfiltered = UsageQuery {
            activity: None,
            ..q.clone()
        };
        let everything = storage.usage_groups(&unfiltered, UsageGrouping::Activity).await?;
        summary.share_of_all = share(all.cost, tally(&mut pricer, &everything).cost);
    }

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
            .map(|(a, t)| t.row(a.as_str().to_string(), a.label().to_string(), None, all.cost))
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
        let groups = storage.usage_groups(&before, UsageGrouping::Activity).await?;
        (!groups.is_empty()).then(|| {
            let t = tally(&mut pricer, &groups);
            PeriodTotals {
                cost_usd: t.cost,
                tokens: t.tokens.total(),
                calls: t.calls,
            }
        })
    };

    let daily = daily(&mut pricer, &q, storage.usage_groups(&q, UsageGrouping::Day).await?);

    let model_groups = storage.usage_groups(&q, UsageGrouping::Model).await?;
    let (by_model, unpriced) = by_model(&mut pricer, &model_groups, all.cost);

    let mut place_buckets: BTreeMap<String, Tally> = BTreeMap::new();
    for g in storage.usage_groups(&q, UsageGrouping::Place).await? {
        place_buckets
            .entry(g.place.clone().unwrap_or_default())
            .or_default()
            .add(&mut pricer, &g);
    }
    // Every Discord channel and Telegram chat in range is one place, so the places name every
    // conversation the costliest list can show. The costliest places are looked up first.
    let mut ranked_places: Vec<_> = place_buckets.iter().collect();
    ranked_places.sort_by(|a, b| by_cost_then_tokens((a.1.cost, a.1.tokens.total()), (b.1.cost, b.1.tokens.total())));
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
        storage.usage_groups(&q, UsageGrouping::Conversation).await?,
    )
    .await?;

    let accounting_since = storage
        .get_state(crate::dependencies::VizierDependencies::USAGE_ACCOUNTING_SINCE.to_string())
        .await?
        .and_then(|v| v.as_str().map(str::to_string))
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc));

    Ok(UsageReport {
        range: UsageRange {
            start: q.start,
            end: q.end,
        },
        accounting_since,
        activity: q.activity,
        summary,
        previous,
        daily,
        by_activity,
        by_model,
        by_place,
        costliest,
        unpriced,
    })
}

/// One entry per date in range, empty ones included, so the chart's axis is complete.
fn daily(pricer: &mut Pricer, q: &UsageQuery, groups: Vec<UsageGroup>) -> Vec<DailyUsage> {
    let mut days: BTreeMap<NaiveDate, BTreeMap<UsageActivity, DayActivity>> = BTreeMap::new();
    let mut d = q.start.date_naive();
    while d <= q.end.date_naive() {
        days.insert(d, BTreeMap::new());
        d = d.succ_opt().unwrap_or(d + Duration::days(1));
    }
    for g in groups {
        let (Some(day), Some(activity)) = (g.day, g.activity) else {
            continue;
        };
        let cost = pricer.cost(&g.provider, &g.model, &g.tokens);
        let cell = days.entry(day).or_default().entry(activity).or_default();
        add_cost(&mut cell.cost_usd, cost);
        cell.tokens += g.tokens.total();
        cell.calls += g.calls;
    }
    days.into_iter()
        .map(|(date, by_activity)| DailyUsage { date, by_activity })
        .collect()
}

fn by_model(
    pricer: &mut Pricer,
    groups: &[UsageGroup],
    total: Option<f64>,
) -> (Vec<ModelRow>, UnpricedNote) {
    let mut buckets: BTreeMap<(String, String), (Tally, BTreeSet<String>)> = BTreeMap::new();
    for g in groups {
        let (t, served) = buckets.entry((g.provider.clone(), g.model.clone())).or_default();
        t.add(pricer, g);
        if let Some(s) = &g.served_model
            && s != &g.model
        {
            served.insert(s.clone());
        }
    }

    let mut unpriced = UnpricedNote::default();
    let mut rows: Vec<ModelRow> = buckets
        .into_iter()
        .map(|((provider, model), (t, served))| {
            let priced = pricer.price(&provider, &model).is_some();
            let key = model_key(&provider, &model);
            if !priced {
                unpriced.tokens += t.tokens.total();
                unpriced.models.push(key.clone());
            }
            let label = if provider.is_empty() && model.is_empty() {
                "Unknown model".to_string()
            } else {
                format!("{provider} / {model}")
            };
            ModelRow {
                row: t.row(key, label, None, total),
                provider,
                model,
                served_models: served.into_iter().collect(),
                cache_hit_rate: t.cache_hit_rate(),
                failed_tokens: t.failed_tokens,
                priced,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        by_cost_then_tokens((a.row.cost_usd, a.row.tokens), (b.row.cost_usd, b.row.tokens))
    });
    (rows, unpriced)
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
        let m = conv.by_model.entry(model_key(&g.provider, &g.model)).or_default();
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
        let (label, detail) = conversation_label(storage, names, agent_id, &channel, topic.as_deref()).await;
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
    let mut pricer = Pricer::new(catalogue);

    let all = tally(&mut pricer, &storage.usage_groups(&q, UsageGrouping::Activity).await?);
    let rows = storage.conversation_usage(&q, limit.saturating_mul(4)).await?;

    let mut calls: Vec<CallRow> = rows
        .rows
        .into_iter()
        .map(|r| {
            let e = r.event;
            CallRow {
                cost_usd: pricer.cost(&e.provider, &e.model, &e.tokens),
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

    let names = NameBook::resolve(storage, kinds, &q.agent_id, slug_place(&channel).as_deref()).await;
    let (label, detail) = conversation_label(storage, &names, &q.agent_id, &channel, topic.as_deref()).await;
    let exists = conversation_exists(storage, &q.agent_id, &channel, topic.as_deref()).await;
    let (link, no_link_reason) = conversation_link(storage, &q.agent_id, &channel, topic.as_deref(), exists).await;

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
    Discord(&'a str),
    Telegram(&'a str),
    Task(&'a str),
    Dream(&'a str),
    Background,
    System,
}

fn slug_kind(channel: &str) -> SlugKind<'_> {
    if channel.starts_with("http__") {
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
struct NameBook(HashMap<String, ConversationNames>);

impl NameBook {
    async fn resolve<'p>(
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
        let integrations = storage.list_agent_integrations(agent_id).await.unwrap_or_default();
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
                let ttl = if cached.is_some() { NAME_TTL } else { NAME_FAILED_TTL };
                if now.duration_since(at) < ttl {
                    if let Some(names) = cached {
                        book.0.insert(place.to_string(), names);
                    }
                    continue;
                }
            }
            if let Some(lookup) = kind.conversation_names(integration, chat_id) {
                lookups.push(async move {
                    let found = tokio::time::timeout(NAME_TIMEOUT, lookup).await.ok().flatten();
                    (key, found)
                });
            }
        }

        for (key, found) in futures::future::join_all(lookups).await {
            name_cache().lock().insert(key.clone(), (std::time::Instant::now(), found.clone()));
            if let Some(names) = found {
                book.0.insert(key.1, names);
            }
        }
        book
    }

    /// `Vizier HQ / #general`, `Family chat`, or `fallback()` when nothing was found.
    fn label(&self, place: &str, fallback: impl FnOnce() -> String) -> String {
        let parts: Vec<&str> = self
            .0
            .get(place)
            .map(|n| [n.space.as_deref(), n.conversation.as_deref()].into_iter().flatten().collect())
            .unwrap_or_default();
        if parts.is_empty() { fallback() } else { parts.join(" / ") }
    }
}

/// The place a Discord or Telegram channel slug is, for a name lookup.
fn slug_place(channel: &str) -> Option<String> {
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
        SlugKind::Http => storage.session_detail_exists(agent_id, channel, topic).await,
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
            (title.unwrap_or_else(|| "WebUI topic".into()), Some("WebUI topic".into()))
        }
        "discord" => (names.label(place, || format!("Discord channel {rest}")), Some("Discord".into())),
        "telegram" => (names.label(place, || format!("Telegram chat {rest}")), Some("Telegram".into())),
        "task" => (task_title(storage, agent_id, rest).await, Some("Scheduled task".into())),
        "dreams" => ("Dream cycle".into(), Some("Unattended reflection".into())),
        "background" => ("Background jobs".into(), Some("Subtasks, delegations and consults".into())),
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
    let title = storage.session_title(agent_id, channel, topic).await.ok().flatten();
    match slug_kind(channel) {
        SlugKind::Http => (title.unwrap_or_else(|| "WebUI topic".into()), Some("WebUI topic".into())),
        // `Vizier HQ / #general / <topic>`: the live names, then the topic's title once it has one.
        SlugKind::Discord(id) => (
            with_topic(names.label(&format!("discord:{id}"), || format!("Discord channel {id}")), title, topic),
            Some("Discord".into()),
        ),
        SlugKind::Telegram(id) => (
            with_topic(names.label(&format!("telegram:{id}"), || format!("Telegram chat {id}")), title, topic),
            Some("Telegram".into()),
        ),
        SlugKind::Task(slug) => (task_title(storage, agent_id, slug).await, Some("Scheduled run".into())),
        SlugKind::Dream(stage) => (
            format!("Dream {}", if stage.is_empty() { "cycle" } else { stage }),
            Some("Unattended reflection".into()),
        ),
        // A piece's session title is its fresh topic id: say what it is instead.
        SlugKind::Background if channel == "SUBAGENT" => ("Subtask".into(), Some("Background job".into())),
        SlugKind::Background => ("Agent conversation".into(), Some("Delegation or consult".into())),
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
        SlugKind::Task(slug) => link(UsageLinkKind::TaskRun, format!("/{agent_id}/tasks?task={slug}")),
        SlugKind::Dream(_) => link(UsageLinkKind::Dream, format!("/{agent_id}/dream")),
        SlugKind::Background => {
            let job = match topic {
                Some(t) => storage.background_job_by_piece(agent_id, t).await.ok().flatten(),
                None => None,
            };
            match job {
                Some(job) => match (&job.origin.1, &job.origin.2) {
                    (VizierChannelId::HTTP(..), Some(origin_topic)) => link(
                        UsageLinkKind::Topic,
                        format!("/{}/chat/{origin_topic}", job.origin.0),
                    ),
                    _ => none(NoLinkReason::NoWebPage),
                },
                None => none(NoLinkReason::NoWebPage),
            }
        }
        SlugKind::Discord(_) | SlugKind::Telegram(_) | SlugKind::System => none(NoLinkReason::NoWebPage),
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
        assert!(insight(&tally(1.0, 0.6, 2), None).unwrap().starts_with("2 turns failed"));
        assert!(insight(&tally(1.0, 0.4, 2), Some(3)).is_none());
        assert!(insight(&tally(1.0, 0.9, 0), None).is_none(), "no failed turn, no insight");
    }

    #[test]
    fn slugs_read_back_into_their_kind() {
        assert!(matches!(slug_kind("http__alice__webui"), SlugKind::Http));
        assert!(matches!(slug_kind("discord__42"), SlugKind::Discord("42")));
        assert!(matches!(slug_kind("task__morning__brief__1700000000000"), SlugKind::Task("morning__brief")));
        assert!(matches!(
            slug_kind("DREAM__a__discord__1__DEFAULT__extraction"),
            SlugKind::Dream("extraction")
        ));
        assert!(matches!(slug_kind("SUBAGENT"), SlugKind::Background));
        assert!(matches!(slug_kind("inter_agent__[a__b]"), SlugKind::Background));
        assert!(matches!(slug_kind("SYSTEM"), SlugKind::System));
    }

    #[test]
    fn discord_and_telegram_labels_read_space_conversation_topic() {
        let mut book = NameBook::default();
        book.0.insert(
            "discord:1".into(),
            ConversationNames { space: Some("Vizier HQ".into()), conversation: Some("#general".into()) },
        );
        book.0.insert(
            "telegram:-5".into(),
            ConversationNames { space: None, conversation: Some("Family chat".into()) },
        );
        let fallback = || "Discord channel 2".to_string();
        assert_eq!(book.label("discord:1", fallback), "Vizier HQ / #general");
        assert_eq!(book.label("telegram:-5", fallback), "Family chat");
        assert_eq!(book.label("discord:2", fallback), "Discord channel 2", "not looked up");

        let base = || book.label("discord:1", fallback);
        assert_eq!(
            with_topic(base(), Some("Trip planning".into()), Some("9f2c")),
            "Vizier HQ / #general / Trip planning"
        );
        assert_eq!(with_topic(base(), Some("9f2c".into()), Some("9f2c")), "Vizier HQ / #general", "untitled topic");
        assert_eq!(with_topic(base(), Some("DEFAULT".into()), None), "Vizier HQ / #general");
        assert_eq!(with_topic(base(), None, None), "Vizier HQ / #general");
        assert_eq!(slug_place("discord__42").as_deref(), Some("discord:42"));
        assert_eq!(slug_place("http__u__webui"), None);
    }

    #[test]
    fn unpriced_sorts_last_and_cost_sorts_descending() {
        let mut v = vec![(None, 900u64), (Some(1.0), 10), (Some(3.0), 5), (None, 1000)];
        v.sort_by(|a, b| by_cost_then_tokens(*a, *b));
        assert_eq!(v, vec![(Some(3.0), 5), (Some(1.0), 10), (None, 1000), (None, 900)]);
    }

    mod report {
        use std::sync::Arc;

        use parking_lot::Mutex;
        use rusqlite::Connection;

        use super::super::*;
        use crate::{
            schema::usage::UsageEvent,
            storage::{document::LocalDocumentStore, sqlite::SqliteStorage},
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
                 CREATE TABLE task (agent_id TEXT, slug TEXT, data TEXT);",
            )
            .unwrap();
            let storage = VizierStorage::new(SqliteStorage::new(
                Arc::new(Mutex::new(conn)),
                Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
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

        fn at(ms: i64) -> DateTime<Utc> {
            DateTime::from_timestamp_millis(ms).unwrap()
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
            }
        }

        fn q() -> UsageQuery {
            UsageQuery {
                agent_id: "a".into(),
                start: at(0),
                end: at(7 * DAY - 1),
                activity: None,
                conversation: None,
            }
        }

        #[tokio::test]
        async fn a_model_switch_splits_by_model_and_an_unpriced_model_is_reported() {
            let (s, _d) = storage();
            let catalogue = Catalogue::new(crate::catalogue::seed::load_seed());
            let (provider, model) = priced(&catalogue);
            s.record_usage(event(DAY, "discord__1", &provider, &model, 1_000_000)).await.unwrap();
            s.record_usage(event(5 * DAY, "discord__1", "custom-local", "qwen", 50_000)).await.unwrap();

            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q()).await.unwrap();
            assert_eq!(r.daily.len(), 7, "every date, empty ones too");
            assert_eq!(r.by_model.len(), 2);
            assert!(r.by_model[0].priced, "priced first");
            assert_eq!(r.by_model[0].served_models, vec![format!("{model}-dated")]);
            assert!(!r.by_model[1].priced);
            assert_eq!(r.unpriced.tokens, 55_000);
            assert_eq!(r.unpriced.models, vec!["custom-local/qwen".to_string()]);
            let priced_cost = r.by_model[0].row.cost_usd.unwrap();
            assert!((r.summary.cost_usd.unwrap() - priced_cost).abs() < 1e-12, "unpriced excluded");
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
                s.record_usage(event(DAY, &channel, &provider, &model, 1000 * (i + 1))).await.unwrap();
            }
            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q()).await.unwrap();
            assert_eq!(r.costliest.len(), 10);
            assert_eq!(r.costliest[0].channel, "discord__11");
            assert!(!r.costliest[0].exists, "no history saved for it");
            assert_eq!(r.costliest[0].label, "Discord channel 11");

            let mut conv = q();
            conv.conversation = Some(("discord__11".into(), None));
            let d = conversation_usage(&s, &catalogue, &IntegrationKinds::builtin(), conv, 20).await.unwrap();
            assert_eq!(d.calls.len(), 1);
            assert_eq!(d.no_link_reason, Some(NoLinkReason::Deleted));
            assert!(d.insight.is_none());

            let (link, reason) = conversation_link(&s, "a", "http__u__webui", Some("t1"), true).await;
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
            s.record_usage(event(DAY, "discord__1", &provider, &model, 3000)).await.unwrap();
            let mut dream = event(DAY, "DREAM__x__extraction", &provider, &model, 1000);
            dream.activity = UsageActivity::Dreams;
            dream.place = "dreams".into();
            s.record_usage(dream).await.unwrap();

            let mut filtered = q();
            filtered.activity = Some(UsageActivity::Dreams);
            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), filtered).await.unwrap();
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
                let mut e = event((i * 90 * DAY) / 100_000, &format!("discord__{}", i % 400), &provider, &model, 1000 + (i as u64 % 5000));
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
                conversation: None,
            };
            let started = std::time::Instant::now();
            let r = build_report(&s, &catalogue, &IntegrationKinds::builtin(), q).await.unwrap();
            let took = started.elapsed();
            assert_eq!(r.summary.calls, 100_000);
            println!("90-day report over 100k events: {took:?}");
            assert!(took < std::time::Duration::from_secs(2), "{took:?}");
        }
    }
}
