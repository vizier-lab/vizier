//! The SQLite copy of the last refreshed catalogue (contract `catalogue-snapshot.md` § Cache):
//! at startup the newer of it and the seed is used, so a refresh survives restarts offline.

use std::io::{Read, Write};

use super::data::{CatalogueData, CatalogueSource, parse};
use crate::storage::{VizierStorage, catalogue::CatalogueCacheStorage};

pub const SCHEMA: i64 = 1;

pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    // Writing into a Vec can't fail.
    let _ = encoder.write_all(bytes);
    encoder.finish().unwrap_or_default()
}

pub fn gunzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes).read_to_end(&mut out)?;
    Ok(out)
}

/// The cached catalogue when it parses, has this schema, and is newer than `seed`; otherwise
/// `seed`. A corrupt cache is logged and ignored. Never touches the network.
pub async fn newest(storage: &VizierStorage, seed: CatalogueData) -> CatalogueData {
    let row = match storage.get_snapshot().await {
        Ok(Some(row)) => row,
        Ok(None) => return seed,
        Err(e) => {
            tracing::warn!("cannot read the cached catalogue, using the bundled one: {e}");
            return seed;
        }
    };
    if row.schema != SCHEMA || row.fetched_at <= seed.fetched_at {
        return seed;
    }
    match gunzip(&row.data)
        .map_err(|e| e.to_string())
        .and_then(|json| parse(&json).map_err(|e| e.0))
    {
        Ok(mut data) => {
            data.source = CatalogueSource::Cached;
            data.fetched_at = row.fetched_at;
            data.checked_at = Some(row.checked_at);
            data.etag = row.etag;
            tracing::info!(
                providers = data.providers.len(),
                fetched_at = %data.fetched_at,
                "using the cached model catalogue"
            );
            data
        }
        Err(e) => {
            tracing::warn!("the cached catalogue is corrupt, using the bundled one: {e}");
            seed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_round_trips() {
        let data = br#"{"a": 1}"#;
        assert_eq!(gunzip(&gzip(data)).unwrap(), data);
        assert!(gunzip(b"not gzip").is_err());
    }
}
