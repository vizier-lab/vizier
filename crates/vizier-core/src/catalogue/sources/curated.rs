//! vizier's own price table (`assets/media-prices.json`, spec 065) as a source: hand-checked
//! figures, each naming the page it was copied from. `Curated` for every provider, so it comes
//! after a provider's own list and before a general catalogue. It prices by lookup (prefix ids,
//! per kind) and lists no models.

use std::sync::{Arc, LazyLock};

use crate::catalogue::media_prices::{MediaPrice, media_prices};
use crate::catalogue::price::Price;
use crate::catalogue::source::{Authority, CatalogueSource, SourceData, SourceId};
use crate::config::provider::ProviderId;
use crate::schema::usage::UsageKind;

pub struct Curated;

impl From<&MediaPrice> for Price {
    fn from(e: &MediaPrice) -> Self {
        Price {
            tokens: e.tokens.clone(),
            per_image: e.per_image.clone(),
            per_million_characters: e.per_million_characters,
            per_audio_minute: e.per_audio_minute.or(e.per_audio_hour.map(|h| h / 60.0)),
        }
    }
}

impl CatalogueSource for Curated {
    fn id(&self) -> SourceId {
        SourceId::CURATED
    }

    fn name(&self) -> &'static str {
        "vizier's price table"
    }

    fn authority(&self, _provider: &ProviderId) -> Authority {
        Authority::Curated
    }

    fn data(&self) -> Arc<SourceData> {
        // No listed models; only the table's date, for the status line and report sources.
        static DATA: LazyLock<Arc<SourceData>> = LazyLock::new(|| {
            let fetched_at = chrono::NaiveDate::parse_from_str(&media_prices().as_of, "%Y-%m-%d")
                .map(crate::time::start_of_local_day)
                .unwrap_or_else(|_| crate::time::epoch());
            Arc::new(SourceData {
                fetched_at,
                ..SourceData::empty()
            })
        });
        DATA.clone()
    }

    fn price(&self, provider: &ProviderId, model: &str, kind: UsageKind) -> Option<Price> {
        media_prices()
            .lookup(provider.as_str(), model, kind)
            .map(Price::from)
    }
}
