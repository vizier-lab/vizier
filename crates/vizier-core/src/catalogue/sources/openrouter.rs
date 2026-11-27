//! OpenRouter's own price list (spec 068, `contracts/seeds.md`): its text and audio models from
//! `/api/v1/models`, its speech and transcription lists, and each image model's default
//! endpoint. Authoritative for `openrouter` only: its ids (`anthropic/claude-…`) are what vizier
//! sends to OpenRouter, never the original maker's (the reseller rule).

use std::collections::BTreeMap;

use futures::{FutureExt, StreamExt, future::BoxFuture};
use serde_json::{Value, json};

use super::provider_list::{ProviderList, data_rows, fetched_at, get_json, number};
use crate::catalogue::data::{Capabilities, Cost, Limits, ModelStatus, SnapshotOrigin, kind_for_output};
use crate::catalogue::media::MediaCapability;
use crate::catalogue::media_prices::ImagePrice;
use crate::catalogue::price::Price;
use crate::catalogue::source::{ModelFacts, SourceData, SourceId, SourceProvider};
use crate::config::provider::ProviderId;
use crate::dependencies::VizierDependencies;
use crate::schema::usage::UsageKind;

const SEED: &str = include_str!("../../../assets/catalogue/openrouter_models.json");
const MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const ENDPOINTS_URL: &str = "https://openrouter.ai/api/v1/images/models/{}/endpoints";
pub const SCHEMA: i64 = 1;

pub fn source() -> ProviderList {
    ProviderList::new(
        SourceId::OPENROUTER,
        "OpenRouter",
        ProviderId::OPENROUTER,
        SCHEMA,
        SEED,
        parse,
        fetch,
    )
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// USD per token, as a string → USD per 1M tokens.
fn per_million(v: Option<&Value>) -> Option<f64> {
    number(v).map(|n| n * 1e6)
}

fn token_cost(pricing: &Value) -> Option<Cost> {
    Some(Cost {
        input: per_million(pricing.get("prompt"))?,
        output: per_million(pricing.get("completion"))?,
        cache_read: per_million(pricing.get("input_cache_read")),
        cache_write: per_million(pricing.get("input_cache_write")),
    })
}

fn expired(date: Option<&str>) -> bool {
    date.and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .is_some_and(|d| d < crate::time::now().date_naive())
}

/// One `/api/v1/models` entry.
fn model(row: &Value) -> ModelFacts {
    let arch = row.get("architecture");
    let input = strings(arch.and_then(|a| a.get("input_modalities")));
    let output = strings(arch.and_then(|a| a.get("output_modalities")));
    let params = strings(row.get("supported_parameters"));
    let has = |p: &str| (!params.is_empty()).then(|| params.iter().any(|x| x == p));
    let positive = |v: Option<&Value>| v.and_then(Value::as_u64).filter(|n| *n > 0);
    ModelFacts {
        name: row.get("name").and_then(Value::as_str).map(str::to_string),
        kind: kind_for_output(&output),
        caps: Capabilities {
            tool_call: has("tools"),
            reasoning: has("reasoning"),
            input,
            output,
            ..Default::default()
        },
        limit: Limits {
            context: positive(row.get("context_length")),
            input: None,
            output: positive(row.pointer("/top_provider/max_completion_tokens")),
        },
        price: row.get("pricing").and_then(token_cost).map(Price::tokens),
        status: expired(row.get("expiration_date").and_then(Value::as_str)).then_some(ModelStatus::Deprecated),
        ..Default::default()
    }
}

/// A speech model: `prompt` is USD per input character.
fn speech(row: &Value) -> Option<Price> {
    let rate = number(row.pointer("/pricing/prompt"))?;
    Some(Price {
        per_million_characters: Some(rate * 1e6),
        ..Default::default()
    })
}

/// A transcription model: token-billed when it charges for output (`gpt-4o-mini-transcribe`),
/// otherwise `prompt` is USD per second of audio (ElevenLabs Scribe v2 lists 0.0000611/s, its
/// published $0.22 an hour).
fn transcription(row: &Value) -> Option<Price> {
    let pricing = row.get("pricing")?;
    if number(pricing.get("completion")).is_some_and(|c| c > 0.0) {
        return token_cost(pricing).map(Price::tokens);
    }
    let per_second = number(pricing.get("prompt"))?;
    Some(Price {
        per_audio_minute: Some(per_second * 60.0),
        ..Default::default()
    })
}

/// An image model's default endpoint pricing: per image (with variants), per request (one image),
/// or per token. Megapixel pricing is not one vizier can measure, so it is left out.
fn image(row: &Value) -> Option<Price> {
    let mut per_image: Option<ImagePrice> = None;
    let (mut input, mut output) = (None, None);
    for p in row.get("pricing")?.as_array()? {
        let Some(cost) = number(p.get("cost_usd")) else { continue };
        let billable = p.get("billable").and_then(Value::as_str).unwrap_or_default();
        let unit = p.get("unit").and_then(Value::as_str).unwrap_or_default();
        match (billable, unit) {
            ("output_image", "image" | "request") => {
                let entry = per_image.get_or_insert_with(|| ImagePrice {
                    default: cost,
                    variants: BTreeMap::new(),
                });
                match p.get("variant").and_then(Value::as_str) {
                    Some(v) => {
                        entry.variants.insert(v.to_string(), cost);
                    }
                    None => entry.default = cost,
                }
            }
            ("output_image", "token") => output = Some(cost * 1e6),
            ("input_text", "token") => input = Some(cost * 1e6),
            _ => {}
        }
    }
    let tokens = output.map(|output| Cost {
        input: input.unwrap_or(0.0),
        output,
        cache_read: None,
        cache_write: None,
    });
    let price = Price {
        tokens,
        per_image,
        ..Default::default()
    };
    (!price.is_empty()).then_some(price)
}

fn media(rows: Option<&Value>, kind: UsageKind, read: fn(&Value) -> Option<Price>) -> Vec<(String, ModelFacts)> {
    rows.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let id = row.get("id").and_then(Value::as_str)?.trim().to_string();
            let price = read(row)?;
            Some((
                id,
                ModelFacts {
                    name: row.get("name").and_then(Value::as_str).map(str::to_string),
                    kind: Some(kind),
                    price: Some(price),
                    ..Default::default()
                },
            ))
        })
        .collect()
}

pub fn parse(value: &Value) -> Result<SourceData, String> {
    let rows = value
        .get("models")
        .and_then(Value::as_array)
        .ok_or("the list has no models")?;
    let mut provider = SourceProvider::default();
    let mut unmapped = 0;
    for row in rows {
        let Some(id) = row.get("id").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) else {
            unmapped += 1;
            continue;
        };
        let facts = model(row);
        // A chat model is listed; anything else (an image or audio generator) is priced only.
        match facts.kind {
            None | Some(UsageKind::Text) => provider.models.insert(id.to_string(), facts),
            Some(_) => provider.priced.insert(id.to_string(), facts),
        };
    }
    for (kind, key, read) in [
        (UsageKind::Speech, "speech", speech as fn(&Value) -> Option<Price>),
        (UsageKind::Transcription, "transcription", transcription),
        (UsageKind::Image, "image", image),
    ] {
        for (id, facts) in media(value.get(key), kind, read) {
            provider.priced.entry(id).or_insert(facts);
        }
    }
    Ok(SourceData {
        origin: SnapshotOrigin::Bundled,
        fetched_at: fetched_at(value),
        checked_at: None,
        providers: BTreeMap::from([(ProviderId::OPENROUTER, provider)]),
        unmapped,
    })
}

/// The default endpoint's pricing list of one image model, or `None` (that model goes unpriced).
async fn endpoint_pricing(deps: &VizierDependencies, id: String) -> Option<Value> {
    let v = get_json(deps, &ENDPOINTS_URL.replace("{}", &id)).await.ok()?;
    let pricing = v
        .get("endpoints")?
        .as_array()?
        .iter()
        .find_map(|e| e.get("pricing").filter(|p| p.as_array().is_some_and(|a| !a.is_empty())))?
        .clone();
    Some(json!({"id": id, "pricing": pricing}))
}

fn fetch(deps: &VizierDependencies) -> BoxFuture<'_, Result<Value, String>> {
    async move {
        let models = data_rows(&get_json(deps, MODELS_URL).await?, "the OpenRouter list")?;
        let speech = data_rows(&get_json(deps, MediaCapability::Speech.source_url()).await?, "the speech list")?;
        let transcription = data_rows(
            &get_json(deps, MediaCapability::Transcription.source_url()).await?,
            "the transcription list",
        )?;
        let images = data_rows(&get_json(deps, MediaCapability::Image.source_url()).await?, "the image list")?;
        let ids: Vec<String> = images
            .iter()
            .filter_map(|r| r.get("id").and_then(Value::as_str).map(str::to_string))
            .collect();
        // Four at a time, as the seed script does; a model whose endpoint fails goes unpriced.
        let image: Vec<Value> = futures::stream::iter(ids)
            .map(|id| endpoint_pricing(deps, id))
            .buffered(4)
            .filter_map(|v| async move { v })
            .collect()
            .await;
        Ok(json!({
            "schema": SCHEMA,
            "models": models,
            "speech": speech,
            "transcription": transcription,
            "image": image,
        }))
    }
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::source::CatalogueSource;

    #[test]
    fn the_committed_seed_prices_text_speech_transcription_and_images() {
        let s = source();
        let data = s.data();
        let p = &data.providers[&ProviderId::OPENROUTER];
        assert!(p.models.len() > 100, "{}", p.models.len());
        let (id, m) = p
            .models
            .iter()
            .find(|(_, m)| m.price.as_ref().and_then(|p| p.tokens.as_ref()).is_some_and(|c| c.input > 0.0))
            .expect("a priced text model");
        let raw: Value = serde_json::from_str(SEED).unwrap();
        let row = raw["models"].as_array().unwrap().iter().find(|r| r["id"] == id.as_str()).unwrap();
        let prompt: f64 = row["pricing"]["prompt"].as_str().unwrap().parse().unwrap();
        let cost = m.price.as_ref().unwrap().tokens.as_ref().unwrap();
        assert!((cost.input - prompt * 1e6).abs() < 1e-9, "per 1M tokens");

        // Spec 065's KNOWN_UNPRICED default image model: priced now, by OpenRouter itself.
        let seedream = s
            .price(&ProviderId::OPENROUTER, "bytedance-seed/seedream-4.5", UsageKind::Image)
            .expect("priced by its endpoint");
        assert_eq!(seedream.per_image.unwrap().default, 0.04);
        assert!(p.priced.values().any(|m| m.kind == Some(UsageKind::Speech) && m.price.as_ref().is_some_and(|p| p.per_million_characters.is_some())));
        assert!(p.priced.values().any(|m| m.kind == Some(UsageKind::Transcription)));
        assert!(
            !p.models.values().any(|m| m.kind == Some(UsageKind::Image)),
            "an image model is never listed for chat"
        );
    }

    #[test]
    fn a_reseller_list_speaks_for_openrouter_only() {
        let s = source();
        assert_eq!(s.authority(&ProviderId::OPENROUTER), crate::catalogue::Authority::Own);
        assert_eq!(s.authority(&ProviderId::ANTHROPIC), crate::catalogue::Authority::None);
        assert_eq!(s.data().providers.len(), 1);
    }

    #[test]
    fn transcription_is_per_second_unless_it_bills_output_tokens() {
        let seconds = transcription(&json!({"pricing": {"prompt": "0.0000611111111111", "completion": "0"}})).unwrap();
        assert!((seconds.per_audio_minute.unwrap() - 0.00366666666).abs() < 1e-9);
        let tokens = transcription(&json!({"pricing": {"prompt": "0.00000125", "completion": "0.000005"}})).unwrap();
        assert_eq!(tokens.tokens.unwrap().output, 5.0);
    }

    #[test]
    fn image_prices_by_image_variant_request_or_token() {
        let p = image(&json!({"pricing": [
            {"billable": "output_image", "cost_usd": 0.02, "unit": "image"},
            {"billable": "output_image", "cost_usd": 0.035, "unit": "image", "variant": "high_resolution"},
            {"billable": "input_image", "cost_usd": 0.008, "unit": "image"}
        ]})).unwrap();
        let per = p.per_image.unwrap();
        assert_eq!((per.default, per.variants["high_resolution"]), (0.02, 0.035));
        let t = image(&json!({"pricing": [
            {"billable": "input_text", "cost_usd": 0.00000175, "unit": "token"},
            {"billable": "output_image", "cost_usd": 0.00001, "unit": "token"}
        ]})).unwrap();
        let c = t.tokens.unwrap();
        assert!((c.input - 1.75).abs() < 1e-9 && (c.output - 10.0).abs() < 1e-9);
        assert!(image(&json!({"pricing": [{"billable": "output_image", "cost_usd": 0.1, "unit": "megapixel"}]})).is_none());
    }
}
