//! One price, from any catalogue source (spec 068, data-model §6), and the answer the aggregate
//! gives for one `(provider, model, kind)` (data-model §8).

use super::data::Cost;
use super::media_prices::ImagePrice;
use super::source::SourceId;
use crate::config::provider::ProviderId;
use crate::schema::metrics::{TokenCounts, estimate_cost};
use crate::schema::usage::{MediaUnits, UsageKind};

/// A model's price: token rates, per-unit media rates, or both. USD; token rates are per 1M.
#[derive(Debug, Clone, Default)]
pub struct Price {
    pub tokens: Option<Cost>,
    pub per_image: Option<ImagePrice>,
    pub per_million_characters: Option<f64>,
    pub per_audio_minute: Option<f64>,
}

impl Price {
    /// A token price alone.
    pub fn tokens(cost: Cost) -> Self {
        Self {
            tokens: Some(cost),
            ..Default::default()
        }
    }

    /// The price of a free call (a local model, a subscription): every kind costs 0.
    pub fn zero() -> Self {
        Self {
            tokens: Some(Cost {
                input: 0.0,
                output: 0.0,
                cache_read: None,
                cache_write: None,
            }),
            per_image: Some(ImagePrice {
                default: 0.0,
                variants: Default::default(),
            }),
            per_million_characters: Some(0.0),
            per_audio_minute: Some(0.0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_none()
            && self.per_image.is_none()
            && self.per_million_characters.is_none()
            && self.per_audio_minute.is_none()
    }

    /// Whether this price can price a record of `kind`: by tokens when the record has tokens (a
    /// completion and an embedding always count by tokens), else by the kind's unit.
    pub fn covers(&self, kind: UsageKind, has_tokens: bool) -> bool {
        let by_tokens = matches!(kind, UsageKind::Text | UsageKind::Embedding) || has_tokens;
        (by_tokens && self.tokens.is_some())
            || match kind {
                UsageKind::Image => self.per_image.is_some(),
                UsageKind::Speech => self.per_million_characters.is_some(),
                UsageKind::Transcription => self.per_audio_minute.is_some(),
                UsageKind::Text | UsageKind::Embedding => false,
            }
    }

    /// The one pricing function (spec 065 research R6, kept): token rates when the record has
    /// tokens and the price has them, else the unit rate for `kind`. Text is tokens only.
    pub fn cost(
        &self,
        kind: UsageKind,
        tokens: &TokenCounts,
        units: &MediaUnits,
        variant: Option<&str>,
        cache_outside_input: bool,
    ) -> Option<f64> {
        if kind == UsageKind::Text {
            return self
                .tokens
                .as_ref()
                .map(|c| estimate_cost(tokens, c, cache_outside_input));
        }
        if tokens.total() > 0
            && let Some(c) = &self.tokens
        {
            return Some(estimate_cost(tokens, c, false));
        }
        match kind {
            UsageKind::Image => self.per_image.as_ref().map(|p| {
                // A size is recorded as typed (`2k`); a tier is named upper case (`2K`).
                let each = variant
                    .and_then(|v| {
                        p.variants
                            .get(v)
                            .or_else(|| p.variants.get(&v.to_ascii_uppercase()))
                    })
                    .copied()
                    .unwrap_or(p.default);
                each * units.images as f64
            }),
            UsageKind::Speech => self
                .per_million_characters
                .map(|p| p * units.characters as f64 / 1e6),
            UsageKind::Transcription => self
                .per_audio_minute
                .map(|p| p * units.audio_ms as f64 / 60_000.0),
            UsageKind::Embedding | UsageKind::Text => None,
        }
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn close_opt(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => close(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn same_cost(a: &Cost, b: &Cost) -> bool {
    close(a.input, b.input)
        && close(a.output, b.output)
        && close_opt(a.cache_read, b.cache_read)
        && close_opt(a.cache_write, b.cache_write)
}

/// Equal when every rate both have matches within 1e-9 (research R8): a source that lists
/// fewer rates does not disagree with one that lists more.
impl PartialEq for Price {
    fn eq(&self, other: &Self) -> bool {
        let tokens = match (&self.tokens, &other.tokens) {
            (Some(a), Some(b)) => same_cost(a, b),
            _ => true,
        };
        let image = match (&self.per_image, &other.per_image) {
            (Some(a), Some(b)) => close(a.default, b.default),
            _ => true,
        };
        let both = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(a), Some(b)) => close(a, b),
            _ => true,
        };
        tokens
            && image
            && both(self.per_million_characters, other.per_million_characters)
            && both(self.per_audio_minute, other.per_audio_minute)
    }
}

/// What the aggregate says about one `(provider, model, kind)`.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceAnswer {
    /// `None`: unpriced, never 0 (FR-017).
    pub price: Option<Price>,
    /// A source id or a rule label (`local`, `subscription`, `borrowed`, `none`).
    pub source: SourceId,
    /// The provider a `borrowed` price came from.
    pub borrowed_from: Option<ProviderId>,
    /// Up to three later sources whose price differs (FR-006).
    pub alternatives: Vec<(SourceId, Price)>,
}

impl PriceAnswer {
    pub fn none() -> Self {
        Self {
            price: None,
            source: SourceId::NONE,
            borrowed_from: None,
            alternatives: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cost(input: f64, output: f64) -> Cost {
        Cost {
            input,
            output,
            cache_read: None,
            cache_write: None,
        }
    }

    #[test]
    fn text_is_priced_by_tokens_only() {
        let p = Price::tokens(cost(1.0, 2.0));
        let t = TokenCounts {
            input: 1_000_000,
            output: 500_000,
            ..Default::default()
        };
        assert_eq!(p.cost(UsageKind::Text, &t, &MediaUnits::default(), None, false), Some(2.0));
        let units_only = Price {
            per_million_characters: Some(5.0),
            ..Default::default()
        };
        assert_eq!(units_only.cost(UsageKind::Text, &t, &MediaUnits::default(), None, false), None);
    }

    #[test]
    fn media_takes_tokens_when_there_are_tokens_else_units() {
        let p = Price {
            tokens: Some(cost(10.0, 0.0)),
            per_image: Some(ImagePrice {
                default: 0.04,
                variants: [("2K".to_string(), 0.08)].into(),
            }),
            ..Default::default()
        };
        let none = TokenCounts::default();
        let two = MediaUnits {
            images: 2,
            ..Default::default()
        };
        assert_eq!(p.cost(UsageKind::Image, &none, &two, Some("2K"), false), Some(0.16));
        assert_eq!(p.cost(UsageKind::Image, &none, &two, Some("9K"), false), Some(0.08));
        let some = TokenCounts {
            input: 100_000,
            ..Default::default()
        };
        assert_eq!(p.cost(UsageKind::Image, &some, &two, None, false), Some(1.0));
    }

    #[test]
    fn speech_and_transcription_units() {
        let p = Price {
            per_million_characters: Some(15.0),
            per_audio_minute: Some(0.006),
            ..Default::default()
        };
        let none = TokenCounts::default();
        let u = MediaUnits {
            characters: 1_000_000,
            audio_ms: 120_000,
            ..Default::default()
        };
        assert_eq!(p.cost(UsageKind::Speech, &none, &u, None, false), Some(15.0));
        assert!((p.cost(UsageKind::Transcription, &none, &u, None, false).unwrap() - 0.012).abs() < 1e-12);
        assert_eq!(p.cost(UsageKind::Embedding, &none, &u, None, false), None);
    }

    #[test]
    fn zero_prices_every_kind_at_zero() {
        let z = Price::zero();
        let t = TokenCounts {
            input: 9,
            output: 9,
            ..Default::default()
        };
        let u = MediaUnits {
            images: 3,
            characters: 9,
            audio_ms: 9,
        };
        for kind in UsageKind::ALL {
            assert_eq!(z.cost(kind, &t, &u, None, false), Some(0.0), "{kind:?}");
        }
    }

    #[test]
    fn equality_compares_only_rates_both_have() {
        assert_eq!(Price::tokens(cost(1.0, 2.0)), Price::tokens(cost(1.0 + 1e-12, 2.0)));
        assert_ne!(Price::tokens(cost(1.0, 2.0)), Price::tokens(cost(1.1, 2.0)));
        let units = Price {
            per_million_characters: Some(3.0),
            ..Default::default()
        };
        assert_eq!(Price::tokens(cost(1.0, 2.0)), units, "nothing in common to disagree on");
    }
}
