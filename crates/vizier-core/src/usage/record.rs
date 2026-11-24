//! Writing usage records off the call's path, and the meter the media handles record through
//! (`specs/065-media-usage-accounting/` research R1, R4).

use std::sync::Arc;

use crate::{
    agents::agent::model::usage::UsageScope,
    schema::{
        metrics::TokenCounts,
        usage::{MediaUnits, UsageEvent, UsageKind},
    },
    storage::{VizierStorage, usage::UsageStorage},
};

/// Write `event` in a spawned task: a failure is logged and never reaches the call (FR-007).
pub fn spawn_record(storage: Arc<VizierStorage>, event: UsageEvent) {
    // A record made while the runtime shuts down has nowhere to go; losing it is fine.
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    handle.spawn(async move {
        let agent_id = event.agent_id.clone();
        if let Err(e) = storage.record_usage(event).await {
            tracing::warn!(agent_id = %agent_id, "failed to record model usage: {e}");
        }
    });
}

/// What a provider's response said it used (research R3). Every field is optional: a client that
/// finds nothing leaves it unset and the meter measures instead.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reported {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub images: Option<u64>,
    pub characters: Option<u64>,
    pub audio_ms: Option<u64>,
    pub served_model: Option<String>,
}

impl Reported {
    /// What an OpenAI-shaped response's `usage` object says (research R3): `prompt_tokens` or
    /// `input_tokens` (embeddings, images, transcription), `output_tokens`, and a transcription's
    /// `seconds` (`type: duration`). Missing fields stay unset.
    pub fn from_openai_usage(body: &serde_json::Value) -> Self {
        let usage = body.get("usage");
        let field = |name: &str| usage.and_then(|u| u.get(name)).and_then(serde_json::Value::as_u64);
        Self {
            input_tokens: field("prompt_tokens").or_else(|| field("input_tokens")),
            output_tokens: field("output_tokens"),
            audio_ms: usage
                .and_then(|u| u.get("seconds"))
                .and_then(serde_json::Value::as_f64)
                .map(|s| (s * 1000.0).round() as u64),
            served_model: body
                .get("model")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            ..Default::default()
        }
    }
}

/// One media handle's identity, for the records it writes.
#[derive(Clone)]
pub struct MediaMeter {
    agent_id: String,
    provider: String,
    model: String,
    local: bool,
    storage: Arc<VizierStorage>,
}

/// What the handle measured itself, used for any figure the provider did not report.
#[derive(Debug, Clone, Default)]
pub struct Measured {
    pub units: MediaUnits,
    pub input_tokens: Option<u64>,
    pub variant: Option<String>,
}

impl MediaMeter {
    pub fn new(
        agent_id: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        local: bool,
        storage: Arc<VizierStorage>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            provider: provider.into(),
            model: model.into(),
            local,
            storage,
        }
    }

    /// Record one answered request.
    pub fn record(
        &self,
        scope: &UsageScope,
        kind: UsageKind,
        reported: Reported,
        measured: Measured,
        took: std::time::Duration,
    ) {
        let event = media_event(self, scope, kind, reported, measured, took);
        spawn_record(self.storage.clone(), event);
    }
}

/// The record for one answered media request: reported figures win, measured ones fill the gaps,
/// and `measured` says whether any gap was filled (research R4).
fn media_event(
    meter: &MediaMeter,
    scope: &UsageScope,
    kind: UsageKind,
    reported: Reported,
    m: Measured,
    took: std::time::Duration,
) -> UsageEvent {
    let mut was_measured = false;
    let mut pick = |reported: Option<u64>, measured: u64| match reported {
        Some(v) => v,
        None => {
            if measured > 0 {
                was_measured = true;
            }
            measured
        }
    };
    let units = match kind {
        UsageKind::Image => MediaUnits {
            images: pick(reported.images, m.units.images),
            ..Default::default()
        },
        UsageKind::Speech => MediaUnits {
            characters: pick(reported.characters, m.units.characters),
            audio_ms: pick(reported.audio_ms, m.units.audio_ms),
            ..Default::default()
        },
        UsageKind::Transcription => MediaUnits {
            audio_ms: pick(reported.audio_ms, m.units.audio_ms),
            ..Default::default()
        },
        UsageKind::Embedding | UsageKind::Text => MediaUnits::default(),
    };
    let input = match (reported.input_tokens, m.input_tokens) {
        (Some(v), _) => v,
        (None, Some(v)) => {
            was_measured = true;
            v
        }
        (None, None) => 0,
    };
    let session = scope.session.as_ref();
    UsageEvent {
        agent_id: meter.agent_id.clone(),
        at: crate::time::now(),
        activity: scope.activity(),
        channel: session.map(|s| s.1.to_slug()),
        topic: session.and_then(|s| s.2.clone()),
        place: session.map_or_else(|| "system".to_string(), |s| s.usage_place()),
        turn_id: scope.turn.as_ref().map(|t| t.as_str().to_string()),
        round: scope.round,
        provider: meter.provider.clone(),
        model: meter.model.clone(),
        served_model: reported.served_model,
        tokens: TokenCounts {
            input,
            cached: 0,
            cache_write: 0,
            output: reported.output_tokens.unwrap_or(0),
        },
        duration_ms: took.as_millis() as u64,
        legacy: false,
        kind,
        variant: m.variant,
        units,
        measured: was_measured,
        local: meter.local,
        call_id: scope.call_id.clone(),
    }
}

/// The length of a RIFF/WAVE file's audio, from its `fmt ` and `data` chunks.
pub fn wav_duration_ms(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut byte_rate: Option<u64> = None;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let body = at + 8;
        if id == b"fmt " && body + 12 <= bytes.len() {
            byte_rate = Some(u32::from_le_bytes(bytes[body + 8..body + 12].try_into().ok()?) as u64);
        } else if id == b"data" {
            let rate = byte_rate.filter(|r| *r > 0)?;
            // A streamed WAV may declare a data length past the end; count what is there.
            let len = len.min(bytes.len().saturating_sub(body)) as u64;
            return Some(len * 1000 / rate);
        }
        at = body + len + (len & 1);
    }
    None
}

/// The length of raw 16-bit mono PCM at `rate` Hz.
pub fn pcm_duration_ms(len: usize, rate: u32) -> u64 {
    if rate == 0 {
        return 0;
    }
    len as u64 / 2 * 1000 / rate as u64
}

/// About four bytes a token, the estimate the prune rule uses too (spec 060).
pub fn estimate_tokens<S: AsRef<str>>(texts: &[S]) -> u64 {
    texts
        .iter()
        .map(|t| (t.as_ref().len() as u64).div_ceil(4))
        .sum()
}

/// A provider enum's own name (`whisper`, `kokoro`, `local`, …), from its serde form.
pub fn provider_name<T: serde::Serialize>(provider: &T) -> String {
    serde_json::to_value(provider)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(samples: usize, rate: u32) -> Vec<u8> {
        let data = samples * 2;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&((36 + data) as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data as u32).to_le_bytes());
        b.resize(b.len() + data, 0);
        b
    }

    #[test]
    fn a_wav_reads_its_own_length() {
        assert_eq!(wav_duration_ms(&wav(16_000, 16_000)), Some(1000));
        assert_eq!(wav_duration_ms(&wav(1_600, 16_000)), Some(100));
        assert_eq!(wav_duration_ms(b"ID3 not a wav"), None);
    }

    #[test]
    fn openai_usage_is_read_from_every_shape() {
        let embed = serde_json::json!({"model": "text-embedding-3-small", "usage": {"prompt_tokens": 8, "total_tokens": 8}});
        let r = Reported::from_openai_usage(&embed);
        assert_eq!(r.input_tokens, Some(8));
        assert_eq!(r.served_model.as_deref(), Some("text-embedding-3-small"));
        let image = serde_json::json!({"usage": {"input_tokens": 50, "output_tokens": 4160}});
        let r = Reported::from_openai_usage(&image);
        assert_eq!((r.input_tokens, r.output_tokens), (Some(50), Some(4160)));
        let whisper = serde_json::json!({"text": "hi", "usage": {"type": "duration", "seconds": 3.2}});
        assert_eq!(Reported::from_openai_usage(&whisper).audio_ms, Some(3200));
        assert_eq!(Reported::from_openai_usage(&serde_json::json!({})), Reported::default());
    }

    #[test]
    fn pcm_and_tokens_are_estimated() {
        assert_eq!(pcm_duration_ms(48_000, 24_000), 1000);
        assert_eq!(pcm_duration_ms(10, 0), 0);
        assert_eq!(estimate_tokens(&["abcd", "abcde"]), 1 + 2);
    }
}
