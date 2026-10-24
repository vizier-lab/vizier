//! The catalogue seed embedded at build time: `build.rs` gzips the committed
//! `assets/catalogue/api.json`, and `meta.json` plus the logos are embedded as they are.

use std::io::Read;

use chrono::{DateTime, Utc};
use rust_embed::RustEmbed;
use serde::Deserialize;

use super::data::{CatalogueData, CatalogueSource, parse};

const SEED_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/catalogue.json.gz"));
const SEED_META: &[u8] = include_bytes!("../../assets/catalogue/meta.json");
/// OpenRouter's media model lists (spec 028), refreshed by `just catalogue-update`.
const MEDIA_SEED: &str = include_str!("../../assets/catalogue/openrouter_media.json");

#[derive(RustEmbed)]
#[folder = "assets/catalogue/logos/"]
struct SeedLogos;

#[derive(Debug, Clone, Deserialize)]
pub struct SeedMeta {
    pub schema: u32,
    pub source_url: String,
    pub fetched_at: DateTime<Utc>,
    pub etag: Option<String>,
    #[serde(default)]
    pub no_logo: Vec<String>,
}

pub fn seed_meta() -> SeedMeta {
    serde_json::from_slice(SEED_META)
        .ok()
        .unwrap_or_else(|| SeedMeta {
            schema: 1,
            source_url: "https://models.dev/api.json".into(),
            fetched_at: DateTime::<Utc>::UNIX_EPOCH,
            etag: None,
            no_logo: vec![],
        })
}

/// The trimmed seed JSON, decompressed.
pub fn seed_json() -> Vec<u8> {
    let mut out = Vec::new();
    if let Err(e) = flate2::read::GzDecoder::new(SEED_GZ).read_to_end(&mut out) {
        tracing::error!("the embedded catalogue seed is corrupt: {e}");
        return b"{}".to_vec();
    }
    out
}

/// The seed as catalogue data. Never fails: a broken seed (a build bug) gives an empty
/// catalogue and an error in the log, and built-in and custom providers still work.
pub fn load_seed() -> CatalogueData {
    let meta = seed_meta();
    match parse(&seed_json()) {
        Ok(mut data) => {
            data.source = CatalogueSource::Bundled;
            data.fetched_at = meta.fetched_at;
            data.etag = meta.etag;
            data
        }
        Err(e) => {
            tracing::error!("cannot parse the embedded catalogue seed: {e}");
            CatalogueData::empty()
        }
    }
}

pub fn seed_logo(id: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    SeedLogos::get(&format!("{id}.svg")).map(|f| f.data)
}

pub fn seed_no_logo(id: &str) -> bool {
    static NO_LOGO: std::sync::LazyLock<std::collections::HashSet<String>> =
        std::sync::LazyLock::new(|| seed_meta().no_logo.into_iter().collect());
    NO_LOGO.contains(id)
}

/// The committed OpenRouter media seed (`specs/028-openrouter-media-provider`), small enough to
/// embed as it is. `build.rs` validates its JSON; here a logically broken seed (wrong schema, a
/// bad timestamp) logs and yields empty lists, leaving typed ids and the documented defaults
/// usable (data-model §4).
pub fn load_media_seed() -> super::media::MediaSeed {
    parse_media_seed(MEDIA_SEED)
}

/// Parse the media seed. A logically broken file logs and yields empty lists.
fn parse_media_seed(json: &str) -> super::media::MediaSeed {
    use super::media::{MediaModel, MediaSeed, SCHEMA};

    #[derive(Deserialize)]
    struct MediaSeedFile {
        #[serde(default)]
        schema: i64,
        #[serde(default)]
        fetched_at: Option<DateTime<Utc>>,
        #[serde(default)]
        speech: Vec<MediaModel>,
        #[serde(default)]
        transcription: Vec<MediaModel>,
        #[serde(default)]
        image: Vec<MediaModel>,
    }

    let fallback = MediaSeed::default();
    let file: MediaSeedFile = match serde_json::from_str(json) {
        Ok(file) => file,
        Err(e) => {
            tracing::error!("cannot parse the embedded media catalogue seed: {e}");
            return fallback;
        }
    };
    if file.schema != SCHEMA {
        tracing::error!(
            schema = file.schema,
            "the embedded media catalogue seed has an unknown schema; using typed ids only"
        );
        return fallback;
    }
    let Some(fetched_at) = file.fetched_at else {
        tracing::error!("the embedded media catalogue seed has no usable fetched_at");
        return fallback;
    };
    MediaSeed {
        fetched_at,
        speech: MediaModel::normalize(file.speech),
        transcription: MediaModel::normalize(file.transcription),
        image: MediaModel::normalize(file.image),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_parses_and_is_complete() {
        let data = load_seed();
        assert!(data.providers.len() >= 200, "{} providers", data.providers.len());
        assert!(data.model_count() >= 5000);
        assert_eq!(data.source, CatalogueSource::Bundled);
        assert!(data.fetched_at > DateTime::<Utc>::UNIX_EPOCH);
    }

    #[test]
    fn seed_logos_are_embedded() {
        assert!(seed_logo("anthropic").is_some_and(|svg| svg.starts_with(b"<svg")));
        assert!(seed_logo("no-such-provider").is_none());
        let meta = seed_meta();
        if let Some(first) = meta.no_logo.first() {
            assert!(seed_no_logo(first));
        }
    }

    /// Spec 028 (data-model §4): the committed seed lists every capability and holds the
    /// documented defaults with their published voices/sizing.
    #[test]
    fn the_media_seed_lists_the_documented_defaults() {
        let seed = load_media_seed();
        assert!(seed.fetched_at > DateTime::<Utc>::UNIX_EPOCH);

        let find = |models: &[super::super::media::MediaModel], id: &str| {
            models.iter().find(|m| m.id == id).cloned()
        };

        let kokoro = find(&seed.speech, "hexgrad/kokoro-82m").expect("the speech default");
        assert!(kokoro.voices.iter().any(|v| v == "af_alloy"), "{:?}", kokoro.voices);
        assert!(find(&seed.transcription, "openai/whisper-large-v3").is_some());
        let seedream = find(&seed.image, "bytedance-seed/seedream-4.5").expect("the image default");
        assert!(seedream.resolutions.iter().any(|r| r == "1K"), "{:?}", seedream.resolutions);
        assert!(!seedream.aspect_ratios.is_empty());
    }

    /// A broken seed is logged, not a crash: the typed fallback and the documented default
    /// still work (spec 028 edge case, FR-020).
    #[test]
    fn a_broken_media_seed_yields_empty_lists() {
        for json in ["{", "[]", r#"{"schema": 2, "fetched_at": "2026-10-24T00:00:00Z"}"#] {
            let seed = parse_media_seed(json);
            assert_eq!(seed.fetched_at, DateTime::<Utc>::UNIX_EPOCH, "{json}");
            assert!(seed.speech.is_empty() && seed.transcription.is_empty() && seed.image.is_empty());
        }
    }
}
