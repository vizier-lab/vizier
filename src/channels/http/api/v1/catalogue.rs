//! The read-only catalogue API (contract `http-api.md` § Catalogue): providers and models for
//! the pickers, and logos. Any authenticated user can read it; logos need no auth, since
//! `<img>` tags send no token. No secret is ever returned.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderValue, header},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    catalogue::{
        POPULAR, ProviderListing, ProviderOrigin,
        builtin::CredentialField,
        data::{CatalogueModel, Cost, Limits, ModelStatus, ProviderKind},
        seed,
    },
    channels::http::{
        auth::middleware::{PermissionState, require_auth, require_permission},
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    config::provider::ProviderId,
    llm::DialectKind,
    schema::CustomProvider,
    storage::provider::{CustomProviderStorage, ProviderStorage},
};

pub fn catalogue(state: HTTPState) -> Router<HTTPState> {
    let authed = Router::new()
        .route("/", get(catalogue_status))
        .route("/providers", get(list_providers))
        .route("/providers/{id}/models", get(list_models))
        .route("/providers/{id}/models/{*model}", get(get_model))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));
    let admin = Router::new()
        .route("/refresh", axum::routing::post(refresh))
        .route_layer(middleware::from_fn_with_state(
            PermissionState {
                permission: "settings:providers".to_string(),
            },
            require_permission,
        ))
        .route_layer(middleware::from_fn_with_state(state, require_auth));
    let public = Router::new().route("/logos/{file}", get(logo));
    authed.merge(admin).merge(public)
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct RefreshStatus {
    pub enabled: bool,
    pub ttl: String,
    pub in_progress: bool,
    pub last_error: Option<String>,
    pub failed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CatalogueStatus {
    /// `bundled` (the build-time seed) or `cached` (a refreshed copy).
    pub source: String,
    pub fetched_at: DateTime<Utc>,
    pub checked_at: Option<DateTime<Utc>>,
    pub refresh: RefreshStatus,
    pub provider_count: usize,
    pub model_count: usize,
}

pub(crate) fn status(state: &HTTPState) -> CatalogueStatus {
    let data = state.catalogue.snapshot();
    let refresh = crate::catalogue::refresh::status(&state.deps);
    CatalogueStatus {
        source: match data.source {
            crate::catalogue::data::CatalogueSource::Bundled => "bundled".into(),
            crate::catalogue::data::CatalogueSource::Cached => "cached".into(),
        },
        fetched_at: data.fetched_at,
        checked_at: data.checked_at,
        refresh,
        provider_count: data.providers.len(),
        model_count: data.model_count(),
    }
}

#[utoipa::path(
    get,
    path = "/catalogue",
    responses((status = 200, description = "Catalogue status", body = APIResponse<CatalogueStatus>))
)]
pub async fn catalogue_status(State(state): State<HTTPState>) -> models::response::Response<CatalogueStatus> {
    crate::catalogue::refresh::maybe_refresh(&state.deps);
    api_response(StatusCode::OK, status(&state))
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Connection {
    /// `connected`, `missing` or `unreachable`.
    pub status: String,
    /// `saved`, `env:NAME`, or `null` (a local provider, or none found).
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ProviderSummary {
    pub id: ProviderId,
    pub name: String,
    pub origin: ProviderOrigin,
    pub kind: ProviderKind,
    pub dialect: Option<DialectKind>,
    pub logo: Option<String>,
    /// The logo has its own colours; don't invert it on a dark theme.
    pub logo_full_color: bool,
    pub model_count: usize,
    pub connection: Connection,
    pub env: Vec<String>,
    pub fields: Vec<CredentialField>,
    /// One of the providers vizier supported before the catalogue.
    pub popular: bool,
    /// Where requests go by default (may hold `${VAR}`); custom providers: their base URL.
    pub endpoint: Option<String>,
    /// The provider's documentation page.
    pub doc: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ProvidersQuery {
    pub q: Option<String>,
    /// `chat`, `media`, `local` or `custom`.
    pub kind: Option<String>,
    pub connected: Option<bool>,
}

/// Providers whose credentials a media or embedding feature uses (data-model §8).
const MEDIA_PROVIDERS: &[&str] = &[
    "openai", "google", "togetherai", "github-copilot", "groq", "mistral", "cohere",
    "openrouter", "huggingface", "xai", "elevenlabs", "voyageai", "hyperbolic", "ollama",
];

pub(crate) fn logo_url(id: &str) -> Option<String> {
    seed::seed_logo(id).map(|_| format!("/api/v1/catalogue/logos/{id}.svg"))
}

/// A logo drawn in its own colours. The others are drawn in `currentColor`, which an `<img>`
/// renders black, so the WebUI inverts them on a dark theme.
pub(crate) fn logo_full_color(id: &str) -> bool {
    seed::seed_logo(id).is_some_and(|svg| {
        !svg.windows(b"currentColor".len()).any(|w| w == b"currentColor")
    })
}

/// Connection state for every listed provider, from one read of the saved rows plus the
/// environment.
pub(crate) async fn connections(
    state: &HTTPState,
    listings: &[ProviderListing],
) -> HashMap<ProviderId, Connection> {
    let saved: HashSet<ProviderId> = state
        .storage
        .list_providers()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|e| !e.credentials.is_empty())
        .map(|e| e.id)
        .collect();
    listings
        .iter()
        .map(|p| {
            let connection = if p.origin == ProviderOrigin::Custom
                || p.id == ProviderId::DUMMYPLUG
                || p.kind == ProviderKind::Local
            {
                Connection {
                    status: "connected".into(),
                    source: saved.contains(&p.id).then(|| "saved".into()),
                }
            } else if saved.contains(&p.id) {
                Connection {
                    status: "connected".into(),
                    source: Some("saved".into()),
                }
            } else {
                match crate::provider_keys::credentials_from_env(&p.id, &p.env) {
                    Some((_, origin)) => Connection {
                        status: "connected".into(),
                        source: Some(origin.label()),
                    },
                    None => Connection {
                        status: "missing".into(),
                        source: None,
                    },
                }
            };
            (p.id.clone(), connection)
        })
        .collect()
}

pub(crate) async fn provider_summaries(state: &HTTPState) -> Vec<ProviderSummary> {
    let custom = state.storage.list_custom_providers().await.unwrap_or_default();
    let listings = state.catalogue.providers(&custom);
    let mut connections = connections(state, &listings).await;
    let snapshot = state.catalogue.snapshot();
    let mut out: Vec<ProviderSummary> = listings
        .into_iter()
        .map(|p| {
            let entry = snapshot.providers.get(&p.id);
            let endpoint = custom
                .iter()
                .find(|c| c.id == p.id)
                .map(|c| c.base_url.clone())
                .or_else(|| entry.and_then(|e| e.api.clone()))
                .or_else(|| crate::catalogue::data::default_base_url(&p.id).map(String::from))
                .or_else(|| {
                    crate::catalogue::builtin::builtin(&p.id)
                        .and_then(|b| b.default_base_url.map(String::from))
                });
            let doc = entry.and_then(|e| e.doc.clone());
            let connection = connections.remove(&p.id).unwrap_or(Connection {
                status: "missing".into(),
                source: None,
            });
            ProviderSummary {
                logo: logo_url(p.id.as_str()),
                logo_full_color: logo_full_color(p.id.as_str()),
                popular: POPULAR.contains(&p.id.as_str()),
                name: if p.id == ProviderId::DUMMYPLUG {
                    "dummyplug (testing)".into()
                } else {
                    p.name
                },
                id: p.id,
                origin: p.origin,
                kind: p.kind,
                dialect: p.dialect,
                model_count: p.model_count,
                connection,
                env: p.env,
                fields: p.fields,
                endpoint,
                doc,
            }
        })
        .collect();
    // Connected first, then custom, then the providers vizier supported before, then A–Z.
    out.sort_by_key(|p| {
        (
            p.connection.status != "connected",
            p.origin != ProviderOrigin::Custom,
            POPULAR
                .iter()
                .position(|id| *id == p.id.as_str())
                .unwrap_or(usize::MAX),
            p.name.to_lowercase(),
        )
    });
    out
}

#[utoipa::path(
    get,
    path = "/catalogue/providers",
    params(ProvidersQuery),
    responses((status = 200, description = "Selectable providers", body = APIResponse<Vec<ProviderSummary>>))
)]
pub async fn list_providers(
    State(state): State<HTTPState>,
    Query(query): Query<ProvidersQuery>,
) -> models::response::Response<Vec<ProviderSummary>> {
    crate::catalogue::refresh::maybe_refresh(&state.deps);
    let q = query.q.as_deref().map(str::to_lowercase).filter(|q| !q.is_empty());
    let providers = provider_summaries(&state)
        .await
        .into_iter()
        .filter(|p| {
            q.as_deref().is_none_or(|q| {
                p.name.to_lowercase().contains(q) || p.id.as_str().contains(q)
            })
        })
        .filter(|p| match query.kind.as_deref() {
            Some("chat") => p.dialect.is_some(),
            Some("media") => p.kind == ProviderKind::Media || MEDIA_PROVIDERS.contains(&p.id.as_str()),
            Some("local") => p.kind == ProviderKind::Local,
            Some("custom") => p.origin == ProviderOrigin::Custom,
            _ => true,
        })
        .filter(|p| {
            query
                .connected
                .is_none_or(|want| (p.connection.status == "connected") == want)
        })
        .collect();
    api_response(StatusCode::OK, providers)
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ModelCaps {
    pub tool_call: Option<bool>,
    pub reasoning: Option<bool>,
    pub image_input: Option<bool>,
    pub attachment: Option<bool>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ModelSummary {
    pub id: String,
    pub name: String,
    pub default: bool,
    pub logo: Option<String>,
    pub logo_full_color: bool,
    pub limit: Limits,
    pub caps: ModelCaps,
    pub cost: Option<Cost>,
    pub status: ModelStatus,
    pub release_date: Option<NaiveDate>,
    pub knowledge: Option<String>,
    /// `server` for a local provider's live model the catalogue doesn't have.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Custom providers: the fields filled in from the catalogue (FR-024).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filled_from_catalogue: Option<Vec<String>>,
}

fn summarize(provider: &str, m: &CatalogueModel) -> ModelSummary {
    let logo_id = m
        .lab
        .as_deref()
        .filter(|lab| logo_url(lab).is_some())
        .unwrap_or(provider);
    ModelSummary {
        id: m.id.clone(),
        name: m.name.clone(),
        default: false,
        logo: logo_url(logo_id),
        logo_full_color: logo_full_color(logo_id),
        limit: m.limit.clone(),
        caps: ModelCaps {
            tool_call: m.caps.tool_call,
            reasoning: m.caps.reasoning,
            image_input: m.caps.image_input(),
            attachment: m.caps.attachment,
        },
        cost: m.cost.clone(),
        status: m.status,
        release_date: m.release_date,
        knowledge: m.knowledge.clone(),
        origin: None,
        filled_from_catalogue: None,
    }
}

fn bare(provider: &str, id: &str, context: Option<u64>, origin: &str) -> ModelSummary {
    ModelSummary {
        id: id.into(),
        name: id.into(),
        default: false,
        logo: logo_url(provider),
        logo_full_color: logo_full_color(provider),
        limit: Limits {
            context,
            ..Default::default()
        },
        caps: ModelCaps {
            tool_call: None,
            reasoning: None,
            image_input: None,
            attachment: None,
        },
        cost: None,
        status: ModelStatus::Active,
        release_date: None,
        knowledge: None,
        origin: Some(origin.into()),
        filled_from_catalogue: None,
    }
}

/// A custom provider's own rows, with what the catalogue fills in marked per field (FR-024).
pub(crate) fn custom_models(state: &HTTPState, custom: &CustomProvider) -> Vec<ModelSummary> {
    let prefer = match custom.protocol {
        crate::schema::CustomProtocol::OpenaiCompatible => DialectKind::OpenAiChat,
        crate::schema::CustomProtocol::Anthropic => DialectKind::Anthropic,
    };
    custom
        .models
        .iter()
        .map(|row| {
            let borrowed = state.catalogue.find_model_by_id(&row.id, Some(prefer));
            let mut filled = vec![];
            let mut pick = |own: Option<u64>, theirs: Option<u64>, name: &str| {
                own.or_else(|| {
                    theirs.inspect(|_| filled.push(name.to_string()))
                })
            };
            let b = borrowed.as_ref();
            let context = pick(row.context, b.and_then(|m| m.limit.input.or(m.limit.context)), "limit.context");
            let output = pick(row.output, b.and_then(|m| m.limit.output), "limit.output");
            let mut flag = |own: Option<bool>, theirs: Option<bool>, name: &str| {
                own.or_else(|| theirs.inspect(|_| filled.push(name.to_string())))
            };
            let tool_call = flag(row.tool_call, b.and_then(|m| m.caps.tool_call), "caps.tool_call");
            let image_input = flag(row.image_input, b.and_then(|m| m.caps.image_input()), "caps.image_input");
            let reasoning = flag(row.reasoning, b.and_then(|m| m.caps.reasoning), "caps.reasoning");
            ModelSummary {
                id: row.id.clone(),
                name: b.map(|m| m.name.clone()).unwrap_or_else(|| row.id.clone()),
                default: false,
                logo: b.and_then(|m| m.lab.as_deref()).and_then(logo_url),
                logo_full_color: b
                    .and_then(|m| m.lab.as_deref())
                    .is_some_and(logo_full_color),
                limit: Limits {
                    context,
                    input: None,
                    output,
                },
                caps: ModelCaps {
                    tool_call,
                    reasoning,
                    image_input,
                    attachment: b.and_then(|m| m.caps.attachment),
                },
                cost: b.and_then(|m| m.cost.clone()),
                status: ModelStatus::Active,
                release_date: b.and_then(|m| m.release_date),
                knowledge: b.and_then(|m| m.knowledge.clone()),
                origin: None,
                filled_from_catalogue: Some(filled),
            }
        })
        .collect()
}

/// A local provider's live models, within 2 seconds.
async fn live_models(state: &HTTPState, provider: &ProviderId) -> Vec<crate::llm::ListedModel> {
    let Ok(resolved) =
        crate::llm::resolve::resolve(&state.deps, provider, "", &Default::default()).await
    else {
        return vec![];
    };
    let model = crate::llm::build(resolved, state.deps.http.clone(), Default::default());
    tokio::time::timeout(Duration::from_secs(2), model.list_models())
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default()
}

pub(crate) async fn models_for(state: &HTTPState, id: &ProviderId, all: bool) -> Option<Vec<ModelSummary>> {
    if let Ok(Some(custom)) = state.storage.get_custom_provider(id).await {
        return Some(custom_models(state, &custom));
    }
    let builtin = crate::catalogue::builtin::builtin(id);
    let catalogue = state.catalogue.provider(id);
    if builtin.is_none() && catalogue.is_none() {
        return None;
    }
    let mut models: Vec<ModelSummary> = state
        .catalogue
        .models(id, all)
        .iter()
        .map(|m| summarize(id.as_str(), m))
        .collect();
    let local = catalogue
        .as_ref()
        .map(|p| p.kind == ProviderKind::Local)
        .unwrap_or_else(|| builtin.is_some_and(|b| b.kind == ProviderKind::Local));
    if local && *id != ProviderId::DUMMYPLUG {
        for live in live_models(state, id).await {
            if !models.iter().any(|m| m.id == live.id) {
                models.push(bare(id.as_str(), &live.id, live.context, "server"));
            }
        }
    }
    if *id == ProviderId::DUMMYPLUG {
        models.push(bare("dummyplug", "dummyplug", None, "builtin"));
    }
    if let Some(first) = models.first_mut() {
        first.default = true;
    }
    Some(models)
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ModelsQuery {
    /// Every model, deprecated and not tool-capable ones included.
    #[serde(default)]
    pub all: bool,
}

#[utoipa::path(
    get,
    path = "/catalogue/providers/{id}/models",
    params(("id" = String, Path, description = "Provider id"), ModelsQuery),
    responses(
        (status = 200, description = "The provider's models, newest first", body = APIResponse<Vec<ModelSummary>>),
        (status = 404, description = "Unknown provider", body = APIResponse<String>)
    )
)]
pub async fn list_models(
    State(state): State<HTTPState>,
    Path(id): Path<String>,
    Query(query): Query<ModelsQuery>,
) -> models::response::Response<Vec<ModelSummary>> {
    crate::catalogue::refresh::maybe_refresh(&state.deps);
    let Ok(id) = ProviderId::parse(&id) else {
        return err_response(StatusCode::BAD_REQUEST, "invalid provider id".into());
    };
    match models_for(&state, &id, query.all).await {
        Some(models) => api_response(StatusCode::OK, models),
        None => err_response(StatusCode::NOT_FOUND, "provider not found".into()),
    }
}

#[utoipa::path(
    get,
    path = "/catalogue/providers/{id}/models/{model}",
    params(("id" = String, Path, description = "Provider id"), ("model" = String, Path, description = "Model id")),
    responses(
        (status = 200, description = "One model", body = APIResponse<ModelSummary>),
        (status = 404, description = "Not in catalogue", body = APIResponse<String>)
    )
)]
pub async fn get_model(
    State(state): State<HTTPState>,
    Path((id, model)): Path<(String, String)>,
) -> models::response::Response<ModelSummary> {
    crate::catalogue::refresh::maybe_refresh(&state.deps);
    let Ok(id) = ProviderId::parse(&id) else {
        return err_response(StatusCode::BAD_REQUEST, "invalid provider id".into());
    };
    let found = match models_for(&state, &id, true).await {
        Some(models) => models.into_iter().find(|m| m.id == model),
        None => None,
    };
    match found {
        Some(mut m) => {
            m.default = false;
            api_response(StatusCode::OK, m)
        }
        None => err_response(StatusCode::NOT_FOUND, "not in catalogue".into()),
    }
}

#[utoipa::path(
    post,
    path = "/catalogue/refresh",
    responses(
        (status = 200, description = "Refreshed (or already current)", body = APIResponse<CatalogueStatus>),
        (status = 409, description = "Refresh is disabled in the config", body = APIResponse<String>),
        (status = 502, description = "The check failed; the previous catalogue stays in use", body = APIResponse<String>)
    )
)]
pub async fn refresh(State(state): State<HTTPState>) -> models::response::Response<CatalogueStatus> {
    use crate::catalogue::refresh::{RefreshError, refresh_now};
    match refresh_now(&state.deps).await {
        Ok(()) => api_response(StatusCode::OK, status(&state)),
        Err(RefreshError::Disabled) => err_response(
            StatusCode::CONFLICT,
            "catalogue refresh is disabled (catalogue.refresh: false)".into(),
        ),
        Err(RefreshError::Failed(e)) => err_response(StatusCode::BAD_GATEWAY, e),
    }
}

fn svg_response(svg: Vec<u8>) -> Response {
    let mut response = (StatusCode::OK, svg).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'"),
    );
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    response
}

#[utoipa::path(
    get,
    path = "/catalogue/logos/{file}",
    params(("file" = String, Path, description = "`<provider or lab id>.svg`")),
    responses(
        (status = 200, description = "The logo", content_type = "image/svg+xml"),
        (status = 404, description = "No logo")
    )
)]
pub async fn logo(State(state): State<HTTPState>, Path(file): Path<String>) -> Response {
    let Some(id) = file.strip_suffix(".svg").filter(|id| ProviderId::is_valid(id)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match crate::catalogue::refresh::logo(&state.deps, id).await {
        Some(svg) => svg_response(svg),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
