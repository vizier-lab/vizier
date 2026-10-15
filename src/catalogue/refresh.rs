//! The lazy, conditional catalogue refresh (contract `catalogue-snapshot.md` § Lazy refresh).
//!
//! **Only requests from people trigger a check**: the `/catalogue/*` read endpoints,
//! `GET /providers`, `GET /providers/custom`, and `POST /catalogue/refresh`. Agent builds,
//! turns, the scheduler and the dream cycle never call [`maybe_refresh`], so a headless install
//! never contacts models.dev. A check is one background task (stale-while-revalidate); callers
//! are always served from memory at once.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde_json::{Map, Value};

use super::cache::{self, SCHEMA};
use super::data::{CatalogueData, CatalogueSource, parse};
use crate::channels::http::api::v1::catalogue::RefreshStatus;
use crate::dependencies::VizierDependencies;
use crate::storage::{
    agent::AgentStorage,
    catalogue::{CatalogueCacheStorage, CatalogueSnapshotRow},
    provider::ProviderStorage,
};

const TIMEOUT: Duration = Duration::from_secs(30);
const FAILURE_BACKOFF: chrono::Duration = chrono::Duration::hours(1);

#[derive(Default)]
pub struct RefreshState {
    in_progress: AtomicBool,
    /// The last failed check, for the 1h backoff and the status line.
    failure: Mutex<Option<(String, DateTime<Utc>)>>,
    logos_in_flight: Mutex<HashSet<String>>,
    fallback_logo: Mutex<Option<Vec<u8>>>,
}

pub fn status(deps: &VizierDependencies) -> RefreshStatus {
    let state = deps.catalogue.refresh_state();
    let failure = state.failure.lock().clone();
    RefreshStatus {
        enabled: deps.config.catalogue.refresh,
        ttl: deps.config.catalogue.ttl.to_string(),
        in_progress: state.in_progress.load(Ordering::SeqCst),
        last_error: failure.as_ref().map(|(e, _)| e.clone()),
        failed_at: failure.map(|(_, at)| at),
    }
}

/// Start a background check when refresh is on, the data is older than the TTL, no check
/// failed in the last hour, and none is running. Returns at once.
///
/// Call this only from request handlers serving a person (see the module docs): never from
/// agent, scheduler or dream code.
pub fn maybe_refresh(deps: &VizierDependencies) {
    if !deps.config.catalogue.refresh {
        return;
    }
    let data = deps.catalogue.snapshot();
    let checked = data.checked_at.unwrap_or(data.fetched_at);
    let ttl = chrono::Duration::from_std(*deps.config.catalogue.ttl).unwrap_or(chrono::Duration::hours(24));
    let now = Utc::now();
    if now - checked < ttl {
        return;
    }
    let state = deps.catalogue.refresh_state();
    if let Some((_, at)) = *state.failure.lock()
        && now - at < FAILURE_BACKOFF
    {
        return;
    }
    if state.in_progress.swap(true, Ordering::SeqCst) {
        return;
    }
    let deps = deps.clone();
    tokio::spawn(async move {
        let _ = check(&deps).await;
        deps.catalogue.refresh_state().in_progress.store(false, Ordering::SeqCst);
    });
}

#[derive(Debug, Clone, PartialEq)]
pub enum RefreshError {
    Disabled,
    Failed(String),
}

/// `POST /catalogue/refresh`: check now, ignoring the TTL and the failure backoff, and wait
/// (up to 30s). A check already running is waited for instead of started again.
pub async fn refresh_now(deps: &VizierDependencies) -> Result<(), RefreshError> {
    if !deps.config.catalogue.refresh {
        return Err(RefreshError::Disabled);
    }
    let state = deps.catalogue.refresh_state();
    if state.in_progress.swap(true, Ordering::SeqCst) {
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while state.in_progress.load(Ordering::SeqCst) && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        return match state.failure.lock().clone() {
            Some((e, _)) => Err(RefreshError::Failed(e)),
            None => Ok(()),
        };
    }
    let result = check(deps).await;
    state.in_progress.store(false, Ordering::SeqCst);
    result.map_err(RefreshError::Failed)
}

/// The fields the catalogue keeps (contract § Kept fields); the rest of models.dev's JSON is
/// dropped before it is cached.
fn trim(raw: Value) -> Value {
    const PROVIDER: &[&str] = &["id", "name", "env", "npm", "api", "doc"];
    const MODEL: &[&str] = &[
        "id", "name", "family", "attachment", "reasoning", "tool_call", "temperature", "knowledge",
        "release_date", "last_updated", "modalities", "limit", "cost", "status",
        "canonical_model_id", "provider",
    ];
    let Value::Object(providers) = raw else {
        return Value::Object(Map::new());
    };
    let trimmed = providers
        .into_iter()
        .filter_map(|(id, p)| {
            let Value::Object(p) = p else { return None };
            let mut out: Map<String, Value> = p
                .iter()
                .filter(|(k, _)| PROVIDER.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let models: Map<String, Value> = p
                .get("models")
                .and_then(Value::as_object)
                .map(|models| {
                    models
                        .iter()
                        .filter_map(|(mid, m)| {
                            let m = m.as_object()?;
                            Some((
                                mid.clone(),
                                Value::Object(
                                    m.iter()
                                        .filter(|(k, _)| MODEL.contains(&k.as_str()))
                                        .map(|(k, v)| (k.clone(), v.clone()))
                                        .collect(),
                                ),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.insert("models".into(), Value::Object(models));
            Some((id, Value::Object(out)))
        })
        .collect();
    Value::Object(trimmed)
}

/// Every catalogue provider id something stored uses: agents, saved credentials, media
/// settings. A refresh that would drop one is refused.
async fn ids_in_use(deps: &VizierDependencies) -> HashSet<String> {
    let mut ids = HashSet::new();
    for (_, agent) in deps.storage.list_agents().await.unwrap_or_default() {
        ids.insert(agent.provider.to_string());
        ids.extend(agent.dream_provider.map(|p| p.to_string()));
        ids.extend(agent.tools.read_image.settings.provider.map(|p| p.to_string()));
        if agent.tools.tts.enabled {
            ids.extend(agent.tools.tts.settings.provider.credentials().map(|p| p.to_string()));
        }
        if agent.tools.stt.enabled {
            ids.extend(agent.tools.stt.settings.provider.credentials().map(|p| p.to_string()));
        }
        if agent.tools.image_gen.enabled {
            ids.extend(agent.tools.image_gen.settings.provider.credentials().map(|p| p.to_string()));
        }
        ids.extend(agent.embedding.and_then(|e| e.provider.credentials()).map(|p| p.to_string()));
    }
    for entry in deps.storage.list_providers().await.unwrap_or_default() {
        ids.insert(entry.id.to_string());
    }
    ids
}

/// Validation (contract § Lazy refresh): it parses, a supported provider exists, it keeps at
/// least half the providers, and it keeps every catalogue provider something uses.
fn validate(new: &CatalogueData, current: &CatalogueData, in_use: &HashSet<String>) -> Result<(), String> {
    if !new.providers.values().any(|p| p.dialect.is_some()) {
        return Err("the catalogue has no provider vizier can talk to".into());
    }
    if new.providers.len() * 2 < current.providers.len() {
        return Err(format!(
            "the catalogue shrank from {} to {} providers",
            current.providers.len(),
            new.providers.len()
        ));
    }
    let missing: Vec<&str> = in_use
        .iter()
        .filter(|id| current.providers.keys().any(|k| k.as_str() == id.as_str()))
        .filter(|id| !new.providers.keys().any(|k| k.as_str() == id.as_str()))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        return Err(format!("the catalogue dropped providers in use: {}", missing.join(", ")));
    }
    Ok(())
}

async fn fail(deps: &VizierDependencies, error: String) -> Result<(), String> {
    let now = Utc::now();
    tracing::warn!("catalogue refresh failed, keeping the current catalogue: {error}");
    *deps.catalogue.refresh_state().failure.lock() = Some((error.clone(), now));
    let _ = deps.storage.record_failure(&error, now).await;
    Err(error)
}

/// One conditional GET, then: 304 → note the check; 200 → trim, validate, cache and swap in.
async fn check(deps: &VizierDependencies) -> Result<(), String> {
    let current = deps.catalogue.snapshot();
    let url = deps.config.catalogue.url.clone();
    let mut req = deps
        .http
        .get(&url)
        .timeout(TIMEOUT)
        .header(reqwest::header::ACCEPT_ENCODING, "gzip");
    if let Some(etag) = &current.etag {
        req = req.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    tracing::info!(url = %url, "checking the model catalogue");
    let response = match req.send().await {
        Ok(r) => r,
        Err(e) => return fail(deps, crate::llm::http::map_reqwest_error(e).to_string()).await,
    };
    // Milliseconds, as SQLite stores them, so memory and the cache always agree.
    let now = DateTime::from_timestamp_millis(Utc::now().timestamp_millis()).unwrap_or_else(Utc::now);
    let status = response.status().as_u16();

    if status == 304 {
        tracing::info!("the model catalogue is current (304)");
        let mut data = (*current).clone();
        data.checked_at = Some(now);
        deps.catalogue.swap(data);
        if deps.storage.get_snapshot().await.ok().flatten().is_some() {
            let _ = deps.storage.touch_checked(now).await;
        }
        *deps.catalogue.refresh_state().failure.lock() = None;
        return Ok(());
    }
    if status != 200 {
        return fail(deps, format!("{url} answered {status}")).await;
    }

    let gzipped = response
        .headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("gzip"));
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = match response.bytes().await {
        Ok(b) => b.to_vec(),
        Err(e) => return fail(deps, e.to_string()).await,
    };
    let body = if gzipped {
        match cache::gunzip(&body) {
            Ok(b) => b,
            Err(e) => return fail(deps, format!("cannot decompress the catalogue: {e}")).await,
        }
    } else {
        body
    };
    let raw: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return fail(deps, format!("the catalogue is not valid JSON: {e}")).await,
    };
    let trimmed = serde_json::to_vec(&trim(raw)).unwrap_or_default();
    let mut data = match parse(&trimmed) {
        Ok(d) => d,
        Err(e) => return fail(deps, e.0).await,
    };
    if let Err(e) = validate(&data, &current, &ids_in_use(deps).await) {
        return fail(deps, e).await;
    }

    data.source = CatalogueSource::Cached;
    data.fetched_at = now;
    data.checked_at = Some(now);
    data.etag = etag.clone();
    let row = CatalogueSnapshotRow {
        schema: SCHEMA,
        source_url: url,
        etag,
        fetched_at: now,
        checked_at: now,
        last_error: None,
        failed_at: None,
        data: cache::gzip(&trimmed),
    };
    if let Err(e) = deps.storage.put_snapshot(&row).await {
        tracing::warn!("cannot cache the refreshed catalogue: {e}");
    }
    tracing::info!(
        providers = data.providers.len(),
        models = data.model_count(),
        "the model catalogue was refreshed"
    );
    deps.catalogue.swap(data);
    *deps.catalogue.refresh_state().failure.lock() = None;
    Ok(())
}

/// The logos' origin: the catalogue URL's scheme and host.
fn logo_url(deps: &VizierDependencies, id: &str) -> Option<String> {
    let url = reqwest::Url::parse(&deps.config.catalogue.url).ok()?;
    Some(format!("{}://{}/logos/{id}.svg", url.scheme(), url.host_str()?))
}

/// A logo: the cached row, then the seed; otherwise, with refresh on, fetch it in the
/// background for next time (contract § Logos). `None` means 404 for now.
pub async fn logo(deps: &VizierDependencies, id: &str) -> Option<Vec<u8>> {
    match deps.storage.get_logo(id).await {
        Ok(Some(cached)) => return cached,
        Ok(None) => {}
        Err(e) => tracing::debug!("cannot read the cached logo {id}: {e}"),
    }
    if let Some(svg) = super::seed::seed_logo(id) {
        return Some(svg.into_owned());
    }
    if super::seed::seed_no_logo(id) || !deps.config.catalogue.refresh {
        return None;
    }
    let state = deps.catalogue.refresh_state();
    if !state.logos_in_flight.lock().insert(id.to_string()) {
        return None;
    }
    let (deps, id) = (deps.clone(), id.to_string());
    tokio::spawn(async move {
        fetch_logo(&deps, &id).await;
        deps.catalogue.refresh_state().logos_in_flight.lock().remove(&id);
    });
    None
}

async fn fetch_logo(deps: &VizierDependencies, id: &str) {
    let get = |url: String| async move {
        let r = deps.http.get(url).timeout(Duration::from_secs(10)).send().await.ok()?;
        r.status().is_success().then_some(())?;
        r.bytes().await.ok().map(|b| b.to_vec())
    };
    let fallback = {
        let known = deps.catalogue.refresh_state().fallback_logo.lock().clone();
        match known {
            Some(f) => Some(f),
            None => {
                let fetched = match logo_url(deps, "__none__") {
                    Some(url) => get(url).await,
                    None => None,
                };
                if let Some(f) = &fetched {
                    *deps.catalogue.refresh_state().fallback_logo.lock() = Some(f.clone());
                }
                fetched
            }
        }
    };
    let Some(url) = logo_url(deps, id) else { return };
    match get(url).await {
        Some(svg) if Some(&svg) != fallback.as_ref() && svg.starts_with(b"<svg") => {
            let _ = deps.storage.put_logo(id, Some(&svg)).await;
        }
        Some(_) => {
            let _ = deps.storage.put_logo(id, None).await;
        }
        None => tracing::debug!("cannot fetch the logo for {id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn trim_keeps_only_the_listed_fields() {
        let out = trim(json!({"p": {"id": "p", "name": "P", "npm": "x", "secret": 1,
            "models": {"m": {"id": "m", "description": "long", "tool_call": true, "open_weights": false}}}}));
        assert_eq!(out, json!({"p": {"id": "p", "name": "P", "npm": "x", "models": {"m": {"id": "m", "tool_call": true}}}}));
    }

    fn data(json: &str) -> CatalogueData {
        parse(json.as_bytes()).unwrap()
    }

    #[test]
    fn validation_refuses_shrinking_unsupported_and_dropped_providers() {
        let current = data(r#"{"a": {"id": "a", "npm": "@ai-sdk/openai-compatible", "api": "https://a"}, "b": {"id": "b", "npm": "@ai-sdk/openai-compatible", "api": "https://b"}, "c": {"id": "c", "npm": "@ai-sdk/openai-compatible", "api": "https://c"}}"#);
        let in_use: HashSet<String> = ["b".to_string()].into();
        assert!(validate(&current, &current, &in_use).is_ok());
        let unsupported = data(r#"{"a": {"id": "a", "npm": "bedrock"}, "b": {"id": "b", "npm": "bedrock"}, "c": {"id": "c", "npm": "bedrock"}}"#);
        assert!(validate(&unsupported, &current, &in_use).is_err());
        let shrunk = data(r#"{"b": {"id": "b", "npm": "@ai-sdk/openai-compatible", "api": "https://b"}}"#);
        assert!(validate(&shrunk, &current, &in_use).unwrap_err().contains("shrank"));
        let dropped = data(r#"{"a": {"id": "a", "npm": "@ai-sdk/openai-compatible", "api": "https://a"}, "c": {"id": "c", "npm": "@ai-sdk/openai-compatible", "api": "https://c"}}"#);
        assert!(validate(&dropped, &current, &in_use).unwrap_err().contains("in use: b"));
    }
}
