//! The media price table shipped with vizier (`specs/065-media-usage-accounting/` research R5,
//! `contracts/media-prices.md`): prices the provider catalogue cannot express — per image, per
//! character, per audio minute — and token rates for media models the catalogue does not list.
//!
//! Every figure is copied from the provider's own pricing page (`source`, `as_of`); a model with
//! no published price is left out rather than guessed.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;

use super::data::Cost;
use crate::config::provider::ProviderId;
use crate::schema::usage::{MediaUnits, UsageKind};

const TABLE: &str = include_str!("../../assets/media-prices.json");

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ImagePrice {
    pub default: f64,
    #[serde(default)]
    pub variants: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaPrice {
    pub provider: String,
    /// An exact id, or a prefix ending in `*`.
    pub model: String,
    pub kind: UsageKind,
    #[serde(default)]
    pub tokens: Option<Cost>,
    #[serde(default)]
    pub per_image: Option<ImagePrice>,
    #[serde(default)]
    pub per_million_characters: Option<f64>,
    #[serde(default)]
    pub per_audio_minute: Option<f64>,
    #[serde(default)]
    pub per_audio_hour: Option<f64>,
    pub source: String,
    pub as_of: String,
}

impl MediaPrice {
    /// The price of `units` at this entry's unit rates, when it has one for the kind.
    pub fn unit_cost(&self, units: &MediaUnits, variant: Option<&str>) -> Option<f64> {
        match self.kind {
            UsageKind::Image => self.per_image.as_ref().map(|p| {
                let each = variant
                    .and_then(|v| p.variants.get(v))
                    .copied()
                    .unwrap_or(p.default);
                each * units.images as f64
            }),
            UsageKind::Speech => self
                .per_million_characters
                .map(|p| p * units.characters as f64 / 1e6),
            UsageKind::Transcription => {
                let minutes = units.audio_ms as f64 / 60_000.0;
                self.per_audio_minute
                    .or(self.per_audio_hour.map(|h| h / 60.0))
                    .map(|p| p * minutes)
            }
            UsageKind::Embedding | UsageKind::Text => None,
        }
    }

    fn matches(&self, model: &str) -> Option<usize> {
        match self.model.strip_suffix('*') {
            Some(prefix) => model.starts_with(prefix).then_some(prefix.len()),
            None => (self.model == model).then_some(usize::MAX),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MediaPrices {
    pub as_of: String,
    pub entries: Vec<MediaPrice>,
}

impl MediaPrices {
    fn parse(text: &str) -> Self {
        #[derive(Deserialize)]
        struct Raw {
            as_of: String,
            entries: Vec<serde_json::Value>,
        }
        let raw: Raw = match serde_json::from_str(text) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("the media price table does not parse: {e}");
                return Self {
                    as_of: String::new(),
                    entries: vec![],
                };
            }
        };
        let entries = raw
            .entries
            .into_iter()
            .filter_map(|v| match serde_json::from_value::<MediaPrice>(v) {
                Ok(e) => Some(e),
                Err(e) => {
                    tracing::warn!("a media price entry was skipped: {e}");
                    None
                }
            })
            .collect();
        Self {
            as_of: raw.as_of,
            entries,
        }
    }

    /// The entry that prices `model` of `provider` for `kind`: an exact id before a prefix, the
    /// longest prefix first. Provider aliases resolve as everywhere else.
    pub fn lookup(&self, provider: &str, model: &str, kind: UsageKind) -> Option<&MediaPrice> {
        let provider = canonical(provider);
        self.entries
            .iter()
            .filter(|e| e.kind == kind && canonical(&e.provider) == provider)
            .filter_map(|e| e.matches(model).map(|rank| (rank, e)))
            .max_by_key(|(rank, _)| *rank)
            .map(|(_, e)| e)
    }
}

fn canonical(provider: &str) -> String {
    provider
        .parse::<ProviderId>()
        .map(|id| id.to_string())
        .unwrap_or_else(|_| provider.to_string())
}

/// The embedded table, parsed once.
pub fn media_prices() -> &'static MediaPrices {
    static PRICES: LazyLock<MediaPrices> = LazyLock::new(|| MediaPrices::parse(TABLE));
    &PRICES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::agent::{ImageGenProvider, SttProvider, TtsProvider};

    #[test]
    fn media_prices_parse() {
        let raw: serde_json::Value = serde_json::from_str(TABLE).unwrap();
        let listed = raw["entries"].as_array().unwrap().len();
        let table = media_prices();
        assert_eq!(table.entries.len(), listed, "no entry may be skipped");
        assert!(!table.as_of.is_empty());
        for e in &table.entries {
            assert!(e.source.starts_with("https://"), "{}: {}", e.model, e.source);
            assert!(!e.as_of.is_empty(), "{}", e.model);
            assert!(
                e.tokens.is_some()
                    || e.per_image.is_some()
                    || e.per_million_characters.is_some()
                    || e.per_audio_minute.is_some()
                    || e.per_audio_hour.is_some(),
                "{} has no price",
                e.model
            );
        }
    }

    #[test]
    fn prefix_and_exact_lookup() {
        let t = MediaPrices::parse(
            r#"{"as_of":"x","entries":[
              {"provider":"openai","model":"tts-*","kind":"speech","per_million_characters":1.0,"source":"https://a","as_of":"x"},
              {"provider":"openai","model":"tts-1-*","kind":"speech","per_million_characters":2.0,"source":"https://a","as_of":"x"},
              {"provider":"openai","model":"tts-1-hd","kind":"speech","per_million_characters":3.0,"source":"https://a","as_of":"x"},
              {"provider":"openai","model":"broken","kind":"speech","source":"https://a"}
            ]}"#,
        );
        assert_eq!(t.entries.len(), 3, "a bad entry is skipped");
        let rate = |m: &str| t.lookup("openai", m, UsageKind::Speech).and_then(|e| e.per_million_characters);
        assert_eq!(rate("tts-1-hd"), Some(3.0));
        assert_eq!(rate("tts-1-x"), Some(2.0));
        assert_eq!(rate("tts-9"), Some(1.0));
        assert_eq!(rate("whisper"), None);
        assert!(t.lookup("openai", "tts-1-hd", UsageKind::Image).is_none());
    }

    #[test]
    fn image_variant_falls_back_to_default() {
        let e = MediaPrice {
            provider: "openai".into(),
            model: "m".into(),
            kind: UsageKind::Image,
            tokens: None,
            per_image: Some(ImagePrice {
                default: 0.04,
                variants: [("1024x1024/high".to_string(), 0.17)].into(),
            }),
            per_million_characters: None,
            per_audio_minute: None,
            per_audio_hour: None,
            source: "https://a".into(),
            as_of: "x".into(),
        };
        let two = MediaUnits { images: 2, ..Default::default() };
        assert_eq!(e.unit_cost(&two, Some("1024x1024/high")), Some(0.34));
        assert_eq!(e.unit_cost(&two, Some("512x512")), Some(0.08));
        assert_eq!(e.unit_cost(&two, None), Some(0.08));
    }

    /// Default models no provider page prices (or that a page lists only under another name),
    /// each with why. Everything else a media provider uses by default must be local or priced.
    const KNOWN_UNPRICED: &[(&str, &str, &str)] = &[
        ("openai", "dall-e-3", "not on OpenAI's API pricing page (2026-11-24)"),
        ("openrouter", "bytedance-seed/seedream-4.5", "listed only as 'from $0.04'"),
        ("xai", "grok-2-image-1212", "not on xAI's models page; grok-imagine-image is"),
        ("huggingface", "stabilityai/stable-diffusion-xl-base-1.0", "billed by compute, not per image"),
        ("hyperbolic", "SDXL1.0-base", "Hyperbolic's pricing page is gone (404)"),
        ("amazon-bedrock", "amazon.nova-canvas-v1:0", "Bedrock's page did not render its tables"),
        ("togetherai", "black-forest-labs/FLUX.1-schnell", "not on Together's pricing page"),
        ("openrouter", "hexgrad/kokoro-82m", "no published per-character price"),
        ("hyperbolic", "Melo-TTS", "Hyperbolic's pricing page is gone (404)"),
        ("openrouter", "openai/whisper-large-v3", "no published per-minute price"),
        ("elevenlabs", "scribe_v1", "only scribe_v2/v3 are listed"),
        ("groq", "whisper-large-v3", "Groq's pricing page shows no table"),
        ("mistral", "voxtral-mini-2507", "Mistral lists no figure for speech"),
        ("huggingface", "openai/whisper-large-v3", "billed by compute, not per minute"),
        ("google", "gemini-1.5-flash", "not on Google's pricing page"),
    ];

    /// research R5: a media provider cannot ship a default model that is silently unpriced.
    #[test]
    fn every_default_media_model_is_priced_or_local() {
        let table = media_prices();
        let mut missing = vec![];
        let mut check = |provider: Option<ProviderId>, model: &str, kind: UsageKind| {
            let Some(provider) = provider else {
                return; // local
            };
            let id = provider.to_string();
            let listed = table.lookup(&id, model, kind).is_some();
            let excused = KNOWN_UNPRICED.iter().any(|(p, m, _)| *p == id && *m == model);
            if !listed && !excused {
                missing.push(format!("{id} / {model} ({})", kind.as_str()));
            }
            assert!(!(listed && excused), "{id}/{model} is priced: drop it from KNOWN_UNPRICED");
        };
        let images = [
            ImageGenProvider::Openai,
            ImageGenProvider::Openrouter,
            ImageGenProvider::Xai,
            ImageGenProvider::Huggingface,
            ImageGenProvider::Hyperbolic,
            ImageGenProvider::Bedrock,
            ImageGenProvider::Venice,
            ImageGenProvider::Together,
            ImageGenProvider::Dummyplug,
        ];
        for p in images {
            check(p.credentials(), p.default_model(), UsageKind::Image);
        }
        let speech = [
            TtsProvider::Openai,
            TtsProvider::Openrouter,
            TtsProvider::Elevenlabs,
            TtsProvider::Xai,
            TtsProvider::Hyperbolic,
            TtsProvider::Kokoro,
            TtsProvider::Dummyplug,
        ];
        for p in speech {
            check(p.credentials(), p.default_model(), UsageKind::Speech);
        }
        let heard = [
            SttProvider::Openai,
            SttProvider::Openrouter,
            SttProvider::Elevenlabs,
            SttProvider::Groq,
            SttProvider::Mistral,
            SttProvider::Huggingface,
            SttProvider::Gemini,
            SttProvider::Whisper,
            SttProvider::Dummyplug,
        ];
        for p in heard {
            check(p.credentials(), p.default_model(), UsageKind::Transcription);
        }
        assert!(missing.is_empty(), "unpriced default media models: {missing:?}");
    }
}
