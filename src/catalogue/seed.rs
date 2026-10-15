//! The catalogue seed embedded at build time: `build.rs` gzips the committed
//! `assets/catalogue/api.json`, and `meta.json` plus the logos are embedded as they are.

use std::io::Read;

use chrono::{DateTime, Utc};
use rust_embed::RustEmbed;
use serde::Deserialize;

use super::data::{CatalogueData, CatalogueSource, parse};

const SEED_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/catalogue.json.gz"));
const SEED_META: &[u8] = include_bytes!("../../assets/catalogue/meta.json");

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
}
