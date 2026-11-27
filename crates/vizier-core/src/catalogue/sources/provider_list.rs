//! A provider's own published price list as a catalogue source (spec 068): authoritative for
//! that one provider only (`Authority::Own`), with a build-time seed, a cached refresh in
//! `catalogue_source_snapshot`, and the shared refresh policy. OpenRouter and Venice each supply
//! only how to fetch their list and how to read it.

use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use parking_lot::RwLock;
use serde_json::Value;

use crate::catalogue::cache;
use crate::catalogue::data::SnapshotOrigin;
use crate::catalogue::source::{Authority, CatalogueSource, SourceData, SourceId};
use crate::catalogue::source_refresh::SourceRefresh;
use crate::config::provider::ProviderId;
use crate::dependencies::VizierDependencies;
use crate::storage::VizierStorage;
use crate::storage::catalogue_source::{CatalogueSourceRow, CatalogueSourceStorage};
use crate::time::Timestamp;

const TIMEOUT: Duration = Duration::from_secs(30);

/// Read a list in its seed shape (`{schema, fetched_at, …}`) into a snapshot.
pub type Parse = fn(&Value) -> Result<SourceData, String>;
/// Fetch the list and assemble it in the seed shape. Public lists only: no credential.
pub type Fetch = for<'a> fn(&'a VizierDependencies) -> BoxFuture<'a, Result<Value, String>>;

pub struct ProviderList {
    id: SourceId,
    name: &'static str,
    provider: ProviderId,
    schema: i64,
    parse: Parse,
    fetch: Fetch,
    data: RwLock<Arc<SourceData>>,
    refresh: SourceRefresh,
}

impl ProviderList {
    /// A list from its committed seed. A seed that does not parse (a build bug) logs and starts
    /// empty; every other source still answers (FR-014).
    pub fn new(
        id: SourceId,
        name: &'static str,
        provider: ProviderId,
        schema: i64,
        seed: &str,
        parse: Parse,
        fetch: Fetch,
    ) -> Self {
        let data = serde_json::from_str::<Value>(seed)
            .map_err(|e| e.to_string())
            .and_then(|v| parse(&v))
            .unwrap_or_else(|e| {
                tracing::error!(source = id.as_str(), "cannot read the embedded price list: {e}");
                SourceData::empty()
            });
        if data.unmapped > 0 {
            tracing::info!(source = id.as_str(), unmapped = data.unmapped, "price list entries left out");
        }
        Self {
            id,
            name,
            provider,
            schema,
            parse,
            fetch,
            data: RwLock::new(Arc::new(data)),
            refresh: SourceRefresh::default(),
        }
    }

    fn swap(&self, data: SourceData) {
        *self.data.write() = Arc::new(data);
    }

    async fn fail(&self, deps: &VizierDependencies, error: String) -> Result<(), String> {
        let now = crate::time::now();
        tracing::warn!(source = self.id.as_str(), "price list refresh failed, keeping the current one: {error}");
        self.refresh.set_failure(error.clone(), now);
        let _ = deps.storage.record_source_failure(self.id.as_str(), &error, now).await;
        Err(error)
    }
}

#[async_trait::async_trait]
impl CatalogueSource for ProviderList {
    fn id(&self) -> SourceId {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn authority(&self, provider: &ProviderId) -> Authority {
        if *provider == self.provider {
            Authority::Own
        } else {
            Authority::None
        }
    }

    fn data(&self) -> Arc<SourceData> {
        self.data.read().clone()
    }

    fn refresh_state(&self) -> Option<&SourceRefresh> {
        Some(&self.refresh)
    }

    async fn load_cached(&self, storage: &VizierStorage) {
        let row = match storage.get_source_snapshot(self.id.as_str()).await {
            Ok(Some(row)) => row,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!(source = self.id.as_str(), "cannot read the cached price list: {e}");
                return;
            }
        };
        if let (Some(error), Some(at)) = (&row.last_error, row.failed_at) {
            self.refresh.set_failure(error.clone(), at);
        }
        if row.schema != self.schema || row.fetched_at <= self.data().fetched_at {
            return;
        }
        let parsed = cache::gunzip(&row.data)
            .map_err(|e| e.to_string())
            .and_then(|json| serde_json::from_slice::<Value>(&json).map_err(|e| e.to_string()))
            .and_then(|v| (self.parse)(&v));
        match parsed {
            Ok(mut data) => {
                data.origin = SnapshotOrigin::Cached;
                data.fetched_at = row.fetched_at;
                data.checked_at = Some(row.checked_at);
                tracing::info!(source = self.id.as_str(), models = data.model_count(), "using the cached price list");
                self.swap(data);
            }
            Err(e) => tracing::warn!(source = self.id.as_str(), "the cached price list is corrupt: {e}"),
        }
    }

    async fn fetch(&self, deps: &VizierDependencies) -> Result<(), String> {
        let value = match (self.fetch)(deps).await {
            Ok(v) => v,
            Err(e) => return self.fail(deps, e).await,
        };
        let mut data = match (self.parse)(&value) {
            Ok(d) => d,
            Err(e) => return self.fail(deps, e).await,
        };
        if data.model_count() == 0 {
            return self.fail(deps, "the list has no priced models".into()).await;
        }
        let now = crate::time::now();
        data.origin = SnapshotOrigin::Cached;
        data.fetched_at = now;
        data.checked_at = Some(now);
        let row = CatalogueSourceRow {
            source: self.id.as_str().to_string(),
            schema: self.schema,
            fetched_at: now,
            checked_at: now,
            last_error: None,
            failed_at: None,
            data: cache::gzip(&serde_json::to_vec(&value).unwrap_or_default()),
        };
        if let Err(e) = deps.storage.put_source_snapshot(&row).await {
            tracing::warn!(source = self.id.as_str(), "cannot cache the refreshed price list: {e}");
        }
        tracing::info!(source = self.id.as_str(), models = data.model_count(), "the price list was refreshed");
        self.swap(data);
        self.refresh.clear_failure();
        Ok(())
    }
}

/// One public GET of a JSON document.
pub async fn get_json(deps: &VizierDependencies, url: &str) -> Result<Value, String> {
    let response = deps
        .http
        .get(url)
        .timeout(TIMEOUT)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| crate::llm::http::map_reqwest_error(e).to_string())?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(format!("{url} answered {status}"));
    }
    response
        .json::<Value>()
        .await
        .map_err(|e| format!("{url} is not valid JSON: {e}"))
}

/// The `data` array of a list response.
pub fn data_rows(value: &Value, what: &str) -> Result<Vec<Value>, String> {
    value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| format!("{what} has no data array"))
}

/// The seed's `fetched_at`, or the epoch.
pub fn fetched_at(value: &Value) -> Timestamp {
    value
        .get("fetched_at")
        .and_then(Value::as_str)
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Local))
        .unwrap_or_else(crate::time::epoch)
}

/// A number, or a number written as a string (OpenRouter's prices).
pub fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .filter(|n: &f64| n.is_finite() && *n >= 0.0)
}
