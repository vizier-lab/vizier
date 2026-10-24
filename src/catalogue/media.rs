//! OpenRouter's media model lists (spec 028, research R6–R9): one per capability, seeded at
//! build time, cached in SQLite and refreshed lazily only when a person reads or refreshes one.
//!
//! The chat/embedding catalogue (`super::data`) and this one share the `Catalogue` handle and the
//! `catalogue.refresh`/`catalogue.ttl` policy, but nothing else: the sources, the payloads and
//! the storage table are different, so they are versioned and refreshed separately.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::cache;
use super::data::CatalogueSource;
use super::refresh::RefreshError;
use crate::channels::http::api::v1::catalogue::RefreshStatus;
use crate::config::provider::ProviderId;
use crate::dependencies::VizierDependencies;
use crate::schema::agent::{ImageGenProvider, SttProvider, TtsProvider};
use crate::storage::VizierStorage;
use crate::storage::media_catalogue::{MediaCatalogueSnapshotRow, MediaCatalogueStorage};

/// The snapshot payload version. A row with another value is ignored.
pub const SCHEMA: i64 = 1;

/// A defensive cap; the live lists are ~60 entries.
const MAX_MODELS: usize = 2_000;
const TIMEOUT: Duration = Duration::from_secs(30);
const FAILURE_BACKOFF: chrono::Duration = chrono::Duration::hours(1);

/// The one axis discovery varies on (data-model §1). It is also the storage key and the API
/// query value.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MediaCapability {
    Speech,
    Transcription,
    Image,
}

impl MediaCapability {
    pub const ALL: [Self; 3] = [Self::Speech, Self::Transcription, Self::Image];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Speech => "speech",
            Self::Transcription => "transcription",
            Self::Image => "image",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "speech" => Some(Self::Speech),
            "transcription" => Some(Self::Transcription),
            "image" => Some(Self::Image),
            _ => None,
        }
    }

    /// Where the live list comes from (research R6). All three are public.
    pub fn source_url(&self) -> &'static str {
        match self {
            Self::Speech => "https://openrouter.ai/api/v1/models?output_modalities=speech",
            Self::Transcription => "https://openrouter.ai/api/v1/models?output_modalities=transcription",
            Self::Image => "https://openrouter.ai/api/v1/images/models",
        }
    }

    /// The documented default, delegated to the provider enum so the factory and the catalogue
    /// cannot drift (data-model §1).
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Speech => TtsProvider::Openrouter.default_model(),
            Self::Transcription => SttProvider::Openrouter.default_model(),
            Self::Image => ImageGenProvider::Openrouter.default_model(),
        }
    }

    pub fn default_voice(&self) -> Option<&'static str> {
        match self {
            Self::Speech => Some(TtsProvider::Openrouter.default_voice()),
            Self::Transcription | Self::Image => None,
        }
    }
}

/// One model in a capability's list, as the seed, the cache and the domain hold it.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(default)]
pub struct MediaModel {
    pub id: String,
    /// The readable name; falls back to the id when the source omits it.
    pub name: String,
    /// Speech only; published voices.
    pub voices: Vec<String>,
    /// Image only; `supported_parameters.aspect_ratio` enum values.
    pub aspect_ratios: Vec<String>,
    /// Image only; `supported_parameters.resolution` enum values.
    pub resolutions: Vec<String>,
}

impl MediaModel {
    /// Drop blank/duplicate arrays, trim the id and fall back to it for the name.
    fn clean(mut self) -> Self {
        self.id = self.id.trim().to_string();
        if self.name.trim().is_empty() {
            self.name = self.id.clone();
        }
        self.voices = clean_strings(self.voices);
        self.aspect_ratios = clean_strings(self.aspect_ratios);
        self.resolutions = clean_strings(self.resolutions);
        self
    }

    /// Trim, drop blanks and duplicates (first wins) and cap the list. Used by the seed and the
    /// cache so stored input can never break a listing.
    pub(crate) fn normalize(models: Vec<MediaModel>) -> Vec<MediaModel> {
        let mut out: Vec<MediaModel> = Vec::with_capacity(models.len());
        for model in models.into_iter().take(MAX_MODELS) {
            let model = model.clean();
            if !model.id.is_empty() && !out.iter().any(|m| m.id == model.id) {
                out.push(model);
            }
        }
        out
    }
}

/// One capability's current list (data-model §2).
#[derive(Debug, Clone)]
pub struct MediaList {
    pub capability: MediaCapability,
    pub models: Vec<MediaModel>,
    pub source: CatalogueSource,
    /// When the current bytes were successfully fetched (the seed's timestamp or a refresh).
    pub fetched_at: DateTime<Utc>,
    /// The last successful check; `None` for the seed until a refresh.
    pub checked_at: Option<DateTime<Utc>>,
}

/// The embedded seed: one timestamp for all three lists, because `just catalogue-update` writes
/// them together (data-model §4).
#[derive(Debug, Clone, Default)]
pub struct MediaSeed {
    pub fetched_at: DateTime<Utc>,
    pub speech: Vec<MediaModel>,
    pub transcription: Vec<MediaModel>,
    pub image: Vec<MediaModel>,
}

impl MediaSeed {
    fn models(&self, capability: MediaCapability) -> &[MediaModel] {
        match capability {
            MediaCapability::Speech => &self.speech,
            MediaCapability::Transcription => &self.transcription,
            MediaCapability::Image => &self.image,
        }
    }
}

/// The listing the API serves: `MediaModel` plus whether it is the documented default and
/// whether it came from a fetched list (data-model §2).
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MediaModelSummary {
    pub id: String,
    pub name: String,
    pub default: bool,
    pub listed: bool,
    pub voices: Vec<String>,
    pub aspect_ratios: Vec<String>,
    pub resolutions: Vec<String>,
}

/// Build the served listing: the documented default marked, and prepended with `listed: false`
/// when the current list no longer contains it. Never substitutes a different model.
pub fn listing(capability: MediaCapability, models: &[MediaModel]) -> Vec<MediaModelSummary> {
    let default = capability.default_model();
    let mut out = Vec::with_capacity(models.len() + 1);
    let mut present = false;
    for m in models {
        let is_default = m.id == default;
        present |= is_default;
        out.push(MediaModelSummary {
            id: m.id.clone(),
            name: m.name.clone(),
            default: is_default,
            listed: true,
            voices: m.voices.clone(),
            aspect_ratios: m.aspect_ratios.clone(),
            resolutions: m.resolutions.clone(),
        });
    }
    if !present {
        out.insert(
            0,
            MediaModelSummary {
                id: default.to_string(),
                name: default.to_string(),
                default: true,
                listed: false,
                voices: vec![],
                aspect_ratios: vec![],
                resolutions: vec![],
            },
        );
    }
    out
}

/// Parse a fetched list (contract `openrouter-media.md` §4). Speech/transcription answer with
/// `{data: [{id, name, supported_voices}]}`; images with `{data: [{id, name,
/// supported_parameters}]}`.
pub fn parse_models(capability: MediaCapability, body: &[u8]) -> Result<Vec<MediaModel>, String> {
    let value: Value =
        serde_json::from_slice(body).map_err(|e| format!("the list is not valid JSON: {e}"))?;
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "the response has no data array".to_string())?;

    let mut out: Vec<MediaModel> = Vec::new();
    for item in data {
        let Value::Object(row) = item else { continue };
        let id = row
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        if id.is_empty() || out.iter().any(|m| m.id == id) {
            continue;
        }
        let name = row
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .unwrap_or(&id)
            .to_string();
        let (voices, aspect_ratios, resolutions) = match capability {
            MediaCapability::Speech => (strings(row.get("supported_voices")), vec![], vec![]),
            MediaCapability::Transcription => (vec![], vec![], vec![]),
            MediaCapability::Image => (
                vec![],
                enum_values(row, "aspect_ratio"),
                enum_values(row, "resolution"),
            ),
        };
        out.push(MediaModel {
            id,
            name,
            voices,
            aspect_ratios,
            resolutions,
        });
        if out.len() >= MAX_MODELS {
            break;
        }
    }
    if out.is_empty() {
        return Err("the list has no usable models".into());
    }
    Ok(out)
}

/// A JSON array of strings, trimmed, without blanks or duplicates, order preserved.
fn strings(value: Option<&Value>) -> Vec<String> {
    let Some(items) = value.and_then(Value::as_array) else {
        return vec![];
    };
    clean_strings(
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn clean_strings(values: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(values.len());
    for value in values {
        let value = value.trim().to_string();
        if !value.is_empty() && !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

/// One `supported_parameters` entry's `values`, only when it is an enum.
fn enum_values(row: &Map<String, Value>, key: &str) -> Vec<String> {
    let Some(param) = row.get("supported_parameters").and_then(|p| p.get(key)) else {
        return vec![];
    };
    if param.get("type").and_then(Value::as_str) != Some("enum") {
        return vec![];
    }
    strings(param.get("values"))
}

/// One capability's in-memory list and refresh state.
#[derive(Default)]
struct CapabilityRefresh {
    in_progress: AtomicBool,
    /// The last failed check, for the 1 h backoff and the status line; restored at startup.
    failure: Mutex<Option<(String, DateTime<Utc>)>>,
}

#[derive(Default)]
struct MediaLists {
    speech: Option<MediaList>,
    transcription: Option<MediaList>,
    image: Option<MediaList>,
}

impl MediaLists {
    fn get(&self, capability: MediaCapability) -> Option<&MediaList> {
        match capability {
            MediaCapability::Speech => self.speech.as_ref(),
            MediaCapability::Transcription => self.transcription.as_ref(),
            MediaCapability::Image => self.image.as_ref(),
        }
    }

    fn set(&mut self, list: MediaList) {
        match list.capability {
            MediaCapability::Speech => self.speech = Some(list),
            MediaCapability::Transcription => self.transcription = Some(list),
            MediaCapability::Image => self.image = Some(list),
        }
    }
}

/// The three per-capability lists plus their refresh state. Held by `Catalogue`, so every
/// reader of that handle sees the same lists.
#[derive(Default)]
pub struct MediaCatalogue {
    lists: RwLock<MediaLists>,
    refresh: [CapabilityRefresh; 3],
}

impl MediaCatalogue {
    pub fn new(seed: MediaSeed) -> Self {
        let mut lists = MediaLists::default();
        for capability in MediaCapability::ALL {
            lists.set(MediaList {
                capability,
                models: seed.models(capability).to_vec(),
                source: CatalogueSource::Bundled,
                fetched_at: seed.fetched_at,
                checked_at: None,
            });
        }
        Self {
            lists: RwLock::new(lists),
            refresh: Default::default(),
        }
    }

    fn state(&self, capability: MediaCapability) -> &CapabilityRefresh {
        let index = match capability {
            MediaCapability::Speech => 0,
            MediaCapability::Transcription => 1,
            MediaCapability::Image => 2,
        };
        &self.refresh[index]
    }

    /// The current list, always available: the seed until a refresh replaces it.
    pub fn list(&self, capability: MediaCapability) -> MediaList {
        self.lists
            .read()
            .get(capability)
            .cloned()
            .unwrap_or_else(|| MediaList {
                capability,
                models: vec![],
                source: CatalogueSource::Bundled,
                fetched_at: DateTime::<Utc>::UNIX_EPOCH,
                checked_at: None,
            })
    }

    fn swap(&self, list: MediaList) {
        self.lists.write().set(list);
    }

    /// Startup: the stored list per capability when it is usable and newer than the seed, plus
    /// the stored last failure so the notice survives a restart. Never touches the network.
    pub async fn load_cache(&self, storage: &VizierStorage) {
        for capability in MediaCapability::ALL {
            let row = match storage.get_media_snapshot(capability.as_str()).await {
                Ok(Some(row)) => row,
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!(
                        capability = capability.as_str(),
                        "cannot read the cached media model list, using the bundled one: {e}"
                    );
                    continue;
                }
            };
            if let Some(error) = &row.last_error
                && let Some(at) = row.failed_at
            {
                *self.state(capability).failure.lock() = Some((error.clone(), at));
            }
            let current = self.list(capability);
            if row.schema != SCHEMA || row.fetched_at <= current.fetched_at {
                continue;
            }
            match decode_cached(&row.data) {
                Ok(models) => {
                    tracing::info!(
                        capability = capability.as_str(),
                        models = models.len(),
                        fetched_at = %row.fetched_at,
                        "using the cached media model list"
                    );
                    self.swap(MediaList {
                        capability,
                        models,
                        source: CatalogueSource::Cached,
                        fetched_at: row.fetched_at,
                        checked_at: Some(row.checked_at),
                    });
                }
                Err(e) => tracing::warn!(
                    capability = capability.as_str(),
                    "the cached media model list is corrupt, using the bundled one: {e}"
                ),
            }
        }
    }

    /// Start a background check when refresh is on, the list is older than the TTL, no check
    /// failed in the last hour, and none is running. Returns at once; only ever called from a
    /// person's read of the list (spec FR-018/SC-011).
    pub fn maybe_refresh(&self, deps: &VizierDependencies, capability: MediaCapability) {
        if !deps.config.catalogue.refresh {
            return;
        }
        let list = self.list(capability);
        let checked = list.checked_at.unwrap_or(list.fetched_at);
        let ttl = chrono::Duration::from_std(*deps.config.catalogue.ttl)
            .unwrap_or(chrono::Duration::hours(24));
        let now = Utc::now();
        if now - checked < ttl {
            return;
        }
        let state = self.state(capability);
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
            let _ = check(&deps, capability).await;
            deps.catalogue
                .media()
                .state(capability)
                .in_progress
                .store(false, Ordering::SeqCst);
        });
    }

    /// A manual refresh: ignore the TTL and the failure backoff, wait (up to 30 s) for a check
    /// already running instead of starting a second one.
    pub async fn refresh_now(
        &self,
        deps: &VizierDependencies,
        capability: MediaCapability,
    ) -> Result<(), RefreshError> {
        if !deps.config.catalogue.refresh {
            return Err(RefreshError::Disabled);
        }
        let state = self.state(capability);
        if state.in_progress.swap(true, Ordering::SeqCst) {
            let deadline = tokio::time::Instant::now() + TIMEOUT;
            while state.in_progress.load(Ordering::SeqCst) && tokio::time::Instant::now() < deadline
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            return match state.failure.lock().clone() {
                Some((e, _)) => Err(RefreshError::Failed(e)),
                None => Ok(()),
            };
        }
        let result = check(deps, capability).await;
        state.in_progress.store(false, Ordering::SeqCst);
        result.map_err(RefreshError::Failed)
    }
}

fn decode_cached(data: &[u8]) -> Result<Vec<MediaModel>, String> {
    let json = cache::gunzip(data).map_err(|e| e.to_string())?;
    let models: Vec<MediaModel> =
        serde_json::from_slice(&json).map_err(|e| e.to_string())?;
    Ok(MediaModel::normalize(models))
}

/// The status block the API returns, reusing the chat catalogue's shape and vocabulary.
pub fn status(deps: &VizierDependencies, capability: MediaCapability) -> RefreshStatus {
    let state = deps.catalogue.media().state(capability);
    let failure = state.failure.lock().clone();
    RefreshStatus {
        enabled: deps.config.catalogue.refresh,
        ttl: deps.config.catalogue.ttl.to_string(),
        in_progress: state.in_progress.load(Ordering::SeqCst),
        last_error: failure.as_ref().map(|(e, _)| e.clone()),
        failed_at: failure.map(|(_, at)| at),
    }
}

async fn get(
    http: &reqwest::Client,
    url: &str,
    key: Option<&str>,
) -> Result<(u16, Vec<u8>), String> {
    let mut req = http
        .get(url)
        .timeout(TIMEOUT)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(key) = key {
        req = req.bearer_auth(key);
    }
    let response = req
        .send()
        .await
        .map_err(|e| crate::llm::http::map_reqwest_error(e).to_string())?;
    let status = response.status().as_u16();
    let body = response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| e.to_string())?;
    Ok((status, body))
}

/// One check, then: parse/validate, cache and swap on success; keep the list and record the
/// failure otherwise (contract § Refresh write order).
async fn check(deps: &VizierDependencies, capability: MediaCapability) -> Result<(), String> {
    let url = capability.source_url();
    let mut fetched = match get(&deps.http, url, None).await {
        Ok(fetched) => fetched,
        Err(e) => return fail(deps, capability, e).await,
    };
    // Public on 2026-10-24; if that ever changes, one retry with the saved/env key.
    if matches!(fetched.0, 401 | 403) {
        let key = match crate::llm::media::api_key(deps, &ProviderId::OPENROUTER).await {
            Ok(key) => key,
            Err(e) => return fail(deps, capability, e.to_string()).await,
        };
        fetched = match get(&deps.http, url, Some(&key)).await {
            Ok(fetched) => fetched,
            Err(e) => return fail(deps, capability, e).await,
        };
    }
    let (status, body) = fetched;
    if status != 200 {
        return fail(deps, capability, format!("{url} answered {status}")).await;
    }
    let models = match parse_models(capability, &body) {
        Ok(models) => models,
        Err(e) => return fail(deps, capability, e).await,
    };
    let now = DateTime::from_timestamp_millis(Utc::now().timestamp_millis()).unwrap_or_else(Utc::now);
    let data = cache::gzip(&serde_json::to_vec(&models).unwrap_or_default());
    let row = MediaCatalogueSnapshotRow {
        capability: capability.as_str().to_string(),
        schema: SCHEMA,
        fetched_at: now,
        checked_at: now,
        last_error: None,
        failed_at: None,
        data,
    };
    if let Err(e) = deps.storage.put_media_snapshot(&row).await {
        tracing::warn!(
            capability = capability.as_str(),
            "cannot cache the refreshed media model list: {e}"
        );
    }
    tracing::info!(
        capability = capability.as_str(),
        models = models.len(),
        "the media model list was refreshed"
    );
    let media = deps.catalogue.media();
    media.swap(MediaList {
        capability,
        models,
        source: CatalogueSource::Cached,
        fetched_at: now,
        checked_at: Some(now),
    });
    *media.state(capability).failure.lock() = None;
    Ok(())
}

async fn fail(
    deps: &VizierDependencies,
    capability: MediaCapability,
    error: String,
) -> Result<(), String> {
    let now = DateTime::from_timestamp_millis(Utc::now().timestamp_millis()).unwrap_or_else(Utc::now);
    tracing::warn!(
        capability = capability.as_str(),
        "media model list refresh failed, keeping the current list: {error}"
    );
    *deps.catalogue.media().state(capability).failure.lock() = Some((error.clone(), now));
    let _ = deps
        .storage
        .record_media_failure(capability.as_str(), &error, now)
        .await;
    Err(error)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn ids(models: &[MediaModel]) -> Vec<&str> {
        models.iter().map(|m| m.id.as_str()).collect()
    }

    #[test]
    fn the_three_capabilities_delegate_to_the_provider_enums() {
        assert_eq!(MediaCapability::parse("speech"), Some(MediaCapability::Speech));
        assert_eq!(MediaCapability::parse("transcription"), Some(MediaCapability::Transcription));
        assert_eq!(MediaCapability::parse("image"), Some(MediaCapability::Image));
        assert_eq!(MediaCapability::parse("video"), None);

        assert_eq!(MediaCapability::Speech.default_model(), TtsProvider::Openrouter.default_model());
        assert_eq!(MediaCapability::Transcription.default_model(), "openai/whisper-large-v3");
        assert_eq!(MediaCapability::Image.default_model(), "bytedance-seed/seedream-4.5");
        assert_eq!(MediaCapability::Speech.default_voice(), Some("af_alloy"));
        assert_eq!(MediaCapability::Transcription.default_voice(), None);
        assert_eq!(MediaCapability::Image.default_voice(), None);
    }

    #[test]
    fn a_speech_list_keeps_ids_names_and_published_voices() {
        let body = json!({"data": [
            {"id": "hexgrad/kokoro-82m", "name": "hexgrad: Kokoro 82M",
             "supported_voices": ["af_alloy", "af_aoede", "af_alloy", "", 3]},
            {"id": "  spaced/model  ", "name": "  "},
            {"id": "hexgrad/kokoro-82m", "name": "duplicate"},
            {"name": "no id"},
            "not an object"
        ]})
        .to_string();
        let models = parse_models(MediaCapability::Speech, body.as_bytes()).unwrap();
        assert_eq!(ids(&models), vec!["hexgrad/kokoro-82m", "spaced/model"]);
        assert_eq!(models[0].name, "hexgrad: Kokoro 82M");
        assert_eq!(models[0].voices, vec!["af_alloy", "af_aoede"]);
        assert_eq!(models[0].aspect_ratios, Vec::<String>::new());
        // A missing name falls back to the id, and the id is trimmed.
        assert_eq!(models[1].name, "spaced/model");
        assert_eq!(models[1].voices, Vec::<String>::new());
    }

    #[test]
    fn a_transcription_list_has_no_capability_metadata() {
        let body = json!({"data": [
            {"id": "openai/whisper-large-v3", "name": "OpenAI: Whisper Large V3", "supported_voices": ["x"]}
        ]})
        .to_string();
        let models = parse_models(MediaCapability::Transcription, body.as_bytes()).unwrap();
        assert_eq!(models[0].name, "OpenAI: Whisper Large V3");
        assert_eq!(models[0].voices, Vec::<String>::new());
    }

    #[test]
    fn an_image_list_keeps_only_enum_aspect_ratios_and_resolutions() {
        let body = json!({"data": [{
            "id": "bytedance-seed/seedream-4.5",
            "name": "ByteDance Seed: Seedream 4.5",
            "supported_parameters": {
                "resolution": {"type": "enum", "values": ["1K", "2K", "4K", 1]},
                "aspect_ratio": {"type": "enum", "values": ["1:1", "16:9", "1:1"]},
                "size": {"type": "range", "min": 1, "max": 4},
                "seed": {"type": "boolean"}
            }
        }]})
        .to_string();
        let models = parse_models(MediaCapability::Image, body.as_bytes()).unwrap();
        assert_eq!(models[0].aspect_ratios, vec!["1:1", "16:9"]);
        assert_eq!(models[0].resolutions, vec!["1K", "2K", "4K"]);
        assert_eq!(models[0].voices, Vec::<String>::new());

        // A non-enum parameter contributes nothing.
        let body = json!({"data": [{"id": "m", "supported_parameters": {"aspect_ratio": {"type": "range"}}}]})
            .to_string();
        let models = parse_models(MediaCapability::Image, body.as_bytes()).unwrap();
        assert!(models[0].aspect_ratios.is_empty());
        assert!(models[0].resolutions.is_empty());
    }

    #[test]
    fn an_empty_or_malformed_list_is_refused() {
        assert!(parse_models(MediaCapability::Speech, b"not json").is_err());
        assert!(parse_models(MediaCapability::Speech, br#"{"data": []}"#).is_err());
        assert!(parse_models(MediaCapability::Speech, br#"{"data": [{"id": "  "}]}"#).is_err());
        assert!(parse_models(MediaCapability::Image, br#"{"models": []}"#).is_err());
        // Non-objects alone leave nothing usable.
        assert!(parse_models(MediaCapability::Image, br#"{"data": [1, 2]}"#).is_err());
    }

    #[test]
    fn the_listing_marks_the_default_and_injects_it_when_absent() {
        let models = vec![
            MediaModel { id: "a".into(), name: "A".into(), ..Default::default() },
            MediaModel { id: "hexgrad/kokoro-82m".into(), name: "Kokoro".into(), ..Default::default() },
        ];
        let listed = listing(MediaCapability::Speech, &models);
        assert_eq!(listed.len(), 2);
        assert!(listed[1].default && listed[1].listed);
        assert!(!listed[0].default && listed[0].listed);

        let listed = listing(MediaCapability::Speech, &models[..1]);
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, "hexgrad/kokoro-82m");
        assert!(listed[0].default);
        assert!(!listed[0].listed);
        assert_eq!(listed[0].name, "hexgrad/kokoro-82m");
        assert!(!listed[1].default);
    }

    #[test]
    fn a_fresh_catalogue_serves_the_seed_for_every_capability() {
        let seed = MediaSeed {
            fetched_at: DateTime::from_timestamp_millis(1_700_000_000_000).unwrap(),
            speech: vec![MediaModel { id: "s".into(), name: "S".into(), ..Default::default() }],
            transcription: vec![],
            image: vec![MediaModel { id: "i".into(), name: "I".into(), ..Default::default() }],
        };
        let catalogue = MediaCatalogue::new(seed);
        let speech = catalogue.list(MediaCapability::Speech);
        assert_eq!(speech.source, CatalogueSource::Bundled);
        assert_eq!(speech.checked_at, None);
        assert_eq!(speech.fetched_at.timestamp_millis(), 1_700_000_000_000);
        assert_eq!(ids(&speech.models), vec!["s"]);
        assert!(catalogue.list(MediaCapability::Transcription).models.is_empty());
    }

    #[test]
    fn a_corrupt_cache_payload_is_refused() {
        assert!(decode_cached(b"not gzip").is_err());
        assert!(decode_cached(&cache::gzip(b"not json")).is_err());
        let good = cache::gzip(br#"[{"id": " a ", "name": "", "voices": ["x", "x", ""]}]"#);
        let models = decode_cached(&good).unwrap();
        assert_eq!(models[0].id, "a");
        assert_eq!(models[0].name, "a");
        assert_eq!(models[0].voices, vec!["x"]);
    }
}
