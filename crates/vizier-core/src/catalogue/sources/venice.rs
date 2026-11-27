//! Venice's own price list (spec 068, `contracts/seeds.md`): `GET /api/v1/models?type=text|
//! image|tts`, public. Authoritative for `venice` only. The list states no units; Venice's
//! pricing page gives text per 1M tokens, images per image and speech per 1M characters, and the
//! committed seed's `venice-sd35` ($0.01 an image) matches the figure spec 065 copied from it.

use std::collections::BTreeMap;

use futures::{FutureExt, future::BoxFuture};
use serde_json::{Value, json};

use super::provider_list::{ProviderList, data_rows, fetched_at, get_json, number};
use crate::catalogue::data::{Capabilities, Cost, Limits, ModelStatus, SnapshotOrigin};
use crate::catalogue::media_prices::ImagePrice;
use crate::catalogue::price::Price;
use crate::catalogue::source::{ModelFacts, SourceData, SourceId, SourceProvider};
use crate::config::provider::ProviderId;
use crate::dependencies::VizierDependencies;
use crate::schema::usage::UsageKind;

const SEED: &str = include_str!("../../../assets/catalogue/venice.json");
const URL: &str = "https://api.venice.ai/api/v1/models?type=";
const TYPES: [&str; 3] = ["text", "image", "tts"];
pub const SCHEMA: i64 = 1;

pub fn source() -> ProviderList {
    ProviderList::new(SourceId::VENICE, "Venice", ProviderId::VENICE, SCHEMA, SEED, parse, fetch)
}

fn usd(v: Option<&Value>) -> Option<f64> {
    number(v?.get("usd"))
}

fn name(row: &Value) -> Option<String> {
    row.pointer("/model_spec/name").and_then(Value::as_str).map(str::to_string)
}

fn deprecated(row: &Value) -> bool {
    row.pointer("/model_spec/deprecation/date")
        .and_then(Value::as_str)
        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok().map(|t| t.date_naive()).or_else(|| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()))
        .is_some_and(|d| d < crate::time::now().date_naive())
}

fn text(row: &Value) -> ModelFacts {
    let spec = row.get("model_spec");
    let pricing = spec.and_then(|s| s.get("pricing"));
    let caps = spec.and_then(|s| s.get("capabilities"));
    let flag = |k: &str| caps.and_then(|c| c.get(k)).and_then(Value::as_bool);
    let positive = |v: Option<&Value>| v.and_then(Value::as_u64).filter(|n| *n > 0);
    let cost = pricing.and_then(|p| {
        Some(Cost {
            input: usd(p.get("input"))?,
            output: usd(p.get("output"))?,
            cache_read: usd(p.get("cache_input")),
            cache_write: usd(p.get("cache_write")),
        })
    });
    ModelFacts {
        name: name(row),
        kind: Some(UsageKind::Text),
        caps: Capabilities {
            tool_call: flag("supportsFunctionCalling"),
            reasoning: flag("supportsReasoning"),
            image_input_override: flag("supportsVision"),
            ..Default::default()
        },
        limit: Limits {
            context: positive(spec.and_then(|s| s.get("availableContextTokens"))).or(positive(row.get("context_length"))),
            input: None,
            output: positive(spec.and_then(|s| s.get("maxCompletionTokens"))),
        },
        price: cost.map(Price::tokens),
        status: deprecated(row).then_some(ModelStatus::Deprecated),
        ..Default::default()
    }
}

/// `generation` is the price; resolution tiers (`1K`, `2K`, …) and quality levels within them
/// (`2K:high`) are variants, named as the size setting a call records. With no `generation`, the
/// lowest tier is the default.
fn image(row: &Value) -> Option<Price> {
    let pricing = row.pointer("/model_spec/pricing")?;
    let mut variants = BTreeMap::new();
    if let Some(tiers) = pricing.get("resolutions").and_then(Value::as_object) {
        for (tier, v) in tiers {
            if let Some(p) = usd(Some(v)) {
                variants.insert(tier.to_ascii_uppercase(), p);
            }
        }
    }
    if let Some(tiers) = pricing.get("quality").and_then(Value::as_object) {
        for (tier, levels) in tiers {
            for (level, v) in levels.as_object().into_iter().flatten() {
                if let Some(p) = usd(Some(v)) {
                    variants.insert(format!("{}:{level}", tier.to_ascii_uppercase()), p);
                }
            }
        }
    }
    let default = usd(pricing.get("generation"))
        .or_else(|| variants.get("1K").copied())
        .or_else(|| variants.values().copied().reduce(f64::min))?;
    Some(Price {
        per_image: Some(ImagePrice { default, variants }),
        ..Default::default()
    })
}

fn tts(row: &Value) -> Option<Price> {
    Some(Price {
        per_million_characters: Some(usd(row.pointer("/model_spec/pricing/input"))?),
        ..Default::default()
    })
}

pub fn parse(value: &Value) -> Result<SourceData, String> {
    let mut provider = SourceProvider::default();
    let mut unmapped = 0;
    let rows = |key: &str| value.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    if rows("text").is_empty() && rows("image").is_empty() {
        return Err("the list has no models".into());
    }
    let id_of = |row: &Value| row.get("id").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    for row in rows("text") {
        match id_of(&row) {
            Some(id) => {
                provider.models.insert(id, text(&row));
            }
            None => unmapped += 1,
        }
    }
    for (key, kind, read) in [
        ("image", UsageKind::Image, image as fn(&Value) -> Option<Price>),
        ("tts", UsageKind::Speech, tts),
    ] {
        for row in rows(key) {
            let (Some(id), Some(price)) = (id_of(&row), read(&row)) else {
                unmapped += 1;
                continue;
            };
            provider.priced.insert(
                id,
                ModelFacts {
                    name: name(&row),
                    kind: Some(kind),
                    price: Some(price),
                    ..Default::default()
                },
            );
        }
    }
    Ok(SourceData {
        origin: SnapshotOrigin::Bundled,
        fetched_at: fetched_at(value),
        checked_at: None,
        providers: BTreeMap::from([(ProviderId::VENICE, provider)]),
        unmapped,
    })
}

fn fetch(deps: &VizierDependencies) -> BoxFuture<'_, Result<Value, String>> {
    async move {
        let mut out = json!({"schema": SCHEMA});
        for kind in TYPES {
            let rows = data_rows(&get_json(deps, &format!("{URL}{kind}")).await?, &format!("Venice's {kind} list"))?;
            out[kind] = Value::Array(rows);
        }
        Ok(out)
    }
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::source::CatalogueSource;

    #[test]
    fn the_committed_seed_prices_text_images_and_speech() {
        let s = source();
        let v = ProviderId::VENICE;
        let sd = s.price(&v, "venice-sd35", UsageKind::Image).expect("venice-sd35");
        assert_eq!(sd.per_image.unwrap().default, 0.01, "Venice's published $0.01 an image");
        let data = s.data();
        let p = &data.providers[&v];
        assert!(p.models.len() > 50, "{}", p.models.len());
        assert!(p.models.values().all(|m| m.kind == Some(UsageKind::Text)));
        assert!(p.priced.values().any(|m| m.kind == Some(UsageKind::Speech)));
        let tiered = p
            .priced
            .values()
            .filter_map(|m| m.price.as_ref()?.per_image.as_ref())
            .find(|i| i.variants.contains_key("2K"))
            .expect("a model priced by resolution");
        assert!(tiered.variants["2K"] >= tiered.default);
    }

    #[test]
    fn quality_levels_become_variants() {
        let p = image(&json!({"model_spec": {"pricing": {
            "quality": {"1K": {"low": {"usd": 0.05}, "medium": {"usd": 0.07}}},
            "resolutions": {"1K": {"usd": 0.07}, "2K": {"usd": 0.1}}
        }}}))
        .unwrap()
        .per_image
        .unwrap();
        assert_eq!(p.default, 0.07, "1K is the default with no generation price");
        assert_eq!(p.variants["1K:low"], 0.05);
        assert_eq!(p.variants["2K"], 0.1);
    }
}
