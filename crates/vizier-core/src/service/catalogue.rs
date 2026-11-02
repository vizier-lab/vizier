//! The catalogue (contract `http-api.md` § Catalogue): providers and models for the pickers.
//! Any signed-in person can read it; the refresh operations need `settings:providers`. No
//! secret is ever returned.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

use crate::{
    auth::Principal,
    catalogue::{
        Catalogue, POPULAR, ProviderListing, ProviderOrigin,
        builtin::CredentialField,
        data::{CatalogueModel, Cost, Limits, ModelStatus, ProviderKind},
        seed,
    },
    config::provider::ProviderId,
    dependencies::VizierDependencies,
    llm::DialectKind,
    schema::{CustomProvider, ProfileEntry},
    service::ServiceError,
    storage::provider::{CustomProviderStorage, ProviderStorage},
};

pub use crate::catalogue::RefreshStatus;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CatalogueStatus {
    /// `bundled` (the build-time seed) or `cached` (a refreshed copy).
    pub source: String,
    pub fetched_at: DateTime<Utc>,
    pub checked_at: Option<DateTime<Utc>>,
    pub refresh: RefreshStatus,
    pub provider_count: usize,
    pub model_count: usize,
}

pub fn status(deps: &VizierDependencies) -> CatalogueStatus {
    let data = deps.catalogue.snapshot();
    let refresh = crate::catalogue::refresh::status(deps);
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

/// One OpenRouter media capability's listing (spec 028, contract `media-catalogue-api.md`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MediaModels {
    pub capability: crate::catalogue::media::MediaCapability,
    /// `bundled` (the embedded seed) or `cached` (a refreshed copy).
    pub source: String,
    pub fetched_at: DateTime<Utc>,
    pub checked_at: Option<DateTime<Utc>>,
    /// The documented default voice; `Some` for speech, `None` otherwise.
    pub default_voice: Option<String>,
    pub refresh: RefreshStatus,
    pub models: Vec<crate::catalogue::media::MediaModelSummary>,
}

impl MediaModels {
    pub fn build(deps: &VizierDependencies, capability: crate::catalogue::media::MediaCapability) -> Self {
        use crate::catalogue::data::CatalogueSource;
        use crate::catalogue::media;
        let list = deps.catalogue.media_list(capability);
        MediaModels {
            capability,
            source: match list.source {
                CatalogueSource::Bundled => "bundled".into(),
                CatalogueSource::Cached => "cached".into(),
            },
            fetched_at: list.fetched_at,
            checked_at: list.checked_at,
            default_voice: capability.default_voice().map(str::to_string),
            refresh: media::status(deps, capability),
            models: media::listing(capability, &list.models),
        }
    }
}

pub const UNKNOWN_CAPABILITY: &str = "capability must be one of: speech, transcription, image";

pub fn parse_capability(value: &str) -> Option<crate::catalogue::media::MediaCapability> {
    crate::catalogue::media::MediaCapability::parse(value)
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Connection {
    /// `connected`, `missing` or `unreachable`.
    pub status: String,
    /// `saved`, `env:NAME`, or `null` (a local provider, or none found).
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
    /// The in-form setup guide, when this provider has one (Amazon Bedrock today).
    pub guide: Option<crate::catalogue::guide::ProviderGuide>,
}

/// Providers whose credentials a media or embedding feature uses (data-model §8).
const MEDIA_PROVIDERS: &[&str] = &[
    "openai", "google", "togetherai", "github-copilot", "groq", "mistral", "cohere",
    "openrouter", "huggingface", "xai", "elevenlabs", "voyageai", "hyperbolic", "ollama",
    "amazon-bedrock", "amazon-bedrock-sso",
];

pub fn logo_url(id: &str) -> Option<String> {
    seed::seed_logo(id).map(|_| format!("/api/v1/catalogue/logos/{id}.svg"))
}

/// A logo drawn in its own colours. The others are drawn in `currentColor`, which an `<img>`
/// renders black, so the WebUI inverts them on a dark theme.
pub fn logo_full_color(id: &str) -> bool {
    seed::seed_logo(id).is_some_and(|svg| {
        !svg.windows(b"currentColor".len()).any(|w| w == b"currentColor")
    })
}

/// Connection state for every listed provider, from one read of the saved rows plus the
/// environment.
pub async fn connections(
    deps: &VizierDependencies,
    listings: &[ProviderListing],
) -> HashMap<ProviderId, Connection> {
    let saved: HashSet<ProviderId> = deps
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
            let connection = if p.origin == ProviderOrigin::Custom {
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

pub async fn provider_summaries(deps: &VizierDependencies) -> Vec<ProviderSummary> {
    let custom = deps.storage.list_custom_providers().await.unwrap_or_default();
    let listings = deps.catalogue.providers(&custom);
    let mut connections = connections(deps, &listings).await;
    let snapshot = deps.catalogue.snapshot();
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
            // Static provider metadata, never for a custom provider that shadows the id.
            let guide = if p.origin == ProviderOrigin::Custom {
                None
            } else {
                crate::catalogue::guide::for_provider(&p.id)
            };
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
                guide,
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

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ModelCaps {
    pub tool_call: Option<bool>,
    pub reasoning: Option<bool>,
    pub image_input: Option<bool>,
    pub attachment: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
pub fn custom_models(deps: &VizierDependencies, custom: &CustomProvider) -> Vec<ModelSummary> {
    let prefer = match custom.protocol {
        crate::schema::CustomProtocol::OpenaiCompatible => DialectKind::OpenAiChat,
        crate::schema::CustomProtocol::Anthropic => DialectKind::Anthropic,
    };
    custom
        .models
        .iter()
        .map(|row| {
            let borrowed = deps.catalogue.find_model_by_id(&row.id, Some(prefer));
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
async fn live_models(deps: &VizierDependencies, provider: &ProviderId) -> Vec<crate::llm::ListedModel> {
    let Ok(resolved) =
        crate::llm::resolve::resolve(deps, provider, "", &Default::default()).await
    else {
        return vec![];
    };
    let model = crate::llm::build(resolved, deps.http.clone(), Default::default());
    tokio::time::timeout(Duration::from_secs(2), model.list_models())
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default()
}

pub async fn models_for(deps: &VizierDependencies, id: &ProviderId, all: bool) -> Option<Vec<ModelSummary>> {
    if let Ok(Some(custom)) = deps.storage.get_custom_provider(id).await {
        return Some(custom_models(deps, &custom));
    }
    let builtin = crate::catalogue::builtin::builtin(id);
    let catalogue = deps.catalogue.provider(id);
    if builtin.is_none() && catalogue.is_none() {
        return None;
    }
    let mut models: Vec<ModelSummary> = deps
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
        for live in live_models(deps, id).await {
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

/// What an entry's usage will cost right now: the five outcomes of data-model "profile entry",
/// in their fixed precedence (research D13, FR-034a.1-a.5, FR-034b).
#[derive(Debug, Clone, PartialEq)]
pub enum EntryPricing {
    /// A hand-entered price, used as given: no catalogue lookup happens (FR-034a.4).
    Explicit(Cost),
    /// The paired model's cost, re-resolved from the catalogue as it stands now (FR-034a.1).
    FromCatalogue(Cost),
    /// The paired id is not in the catalogue: withdrawn, or the catalogue has not caught up
    /// (FR-034a.3).
    NotResolved { model: String },
    /// No model was paired, so there is nothing to price from (FR-034a.2).
    NotSet,
    /// The catalogue carries no Bedrock list at all, so `model` could not be checked (FR-034b).
    NoCatalogue,
}

impl EntryPricing {
    /// The cost a report adds, or `None` when the entry is unpriced — never zero (FR-034b).
    pub fn cost(&self) -> Option<&Cost> {
        match self {
            Self::Explicit(cost) | Self::FromCatalogue(cost) => Some(cost),
            Self::NotResolved { .. } | Self::NotSet | Self::NoCatalogue => None,
        }
    }

    /// The `state` `contracts/provider-credentials-api.md` §4a reports.
    pub fn state(&self) -> &'static str {
        match self {
            Self::Explicit(_) => "explicit",
            Self::FromCatalogue(_) => "from_catalogue",
            Self::NotResolved { .. } => "not_resolved",
            Self::NotSet => "not_set",
            Self::NoCatalogue => "no_catalogue",
        }
    }

    /// The label the settings row shows; `None` once the entry is priced.
    pub fn flag(&self) -> Option<&'static str> {
        match self {
            Self::Explicit(_) | Self::FromCatalogue(_) => None,
            Self::NotResolved { .. } => Some("pricing not resolved"),
            Self::NotSet | Self::NoCatalogue => Some("pricing not set"),
        }
    }

    /// Why the entry is unpriced and the way out. Every absence carries its own, so no caller can
    /// render one without a cause, and no two causes share wording (FR-034a.2, FR-034a.3,
    /// FR-034b, SC-021). `None` once the entry is priced.
    pub fn reason(&self) -> Option<String> {
        match self {
            Self::Explicit(_) | Self::FromCatalogue(_) => None,
            Self::NotResolved { model } => Some(format!(
                "no price for {model}: it is no longer in the catalogue — pick another model \
                 or enter a price by hand"
            )),
            Self::NotSet => Some(
                "no model is paired with this ARN, so there is nothing to price it from — pick \
                 one from the list or enter a price by hand"
                    .to_string(),
            ),
            Self::NoCatalogue => Some(
                "the model catalogue carries no Bedrock model list, so there is nothing to \
                 price this against yet — this resolves itself once the list is present; or \
                 enter a price by hand"
                    .to_string(),
            ),
        }
    }
}

/// An entry's price, in the precedence data-model "profile entry" fixes — the order is
/// load-bearing, not incidental (research D13):
///
/// 1. `price` is checked **first and short-circuits**, so a hand-entered figure wins over every
///    absence below and no catalogue lookup happens at all (FR-034a.4).
/// 2. Catalogue readiness is checked **before `model` is looked at**: with no Bedrock list there
///    is nothing to resolve against, so `NoCatalogue` wins over both `NotSet` and `NotResolved`
///    — neither of their reasons could be acted on, or even be true, there (FR-034b).
/// 3. Only then is `model` resolved through the provider alias, and only then is an empty
///    `model` a `NotSet` (FR-034a.1-a.3).
///
/// Nothing is ever derived from the ARN (FR-034a, SC-020).
pub fn entry_pricing(catalogue: &Catalogue, entry: &ProfileEntry) -> EntryPricing {
    if let Some(price) = entry.price.clone() {
        return EntryPricing::Explicit(price);
    }
    // The D2 alias is applied by `Catalogue::provider`, so this asks the same question the model
    // list and the pickers ask.
    if catalogue.provider(&ProviderId::AMAZON_BEDROCK_SSO).is_none() {
        return EntryPricing::NoCatalogue;
    }
    let Some(model) = entry.model.as_deref().map(str::trim).filter(|m| !m.is_empty()) else {
        return EntryPricing::NotSet;
    };
    match catalogue
        .model(&ProviderId::AMAZON_BEDROCK_SSO, model)
        .and_then(|m| m.cost)
    {
        Some(cost) => EntryPricing::FromCatalogue(cost),
        None => EntryPricing::NotResolved { model: model.to_string() },
    }
}

fn invalid(message: impl Into<String>) -> ServiceError {
    ServiceError::invalid(message)
}

/// Status of the catalogue; a read of it may start the lazy refresh.
pub fn catalogue_status(deps: &VizierDependencies) -> CatalogueStatus {
    crate::catalogue::refresh::maybe_refresh(deps);
    status(deps)
}

/// `capability` is `speech`, `transcription` or `image`.
pub fn media_models(
    deps: &VizierDependencies,
    capability: Option<&str>,
) -> Result<MediaModels, ServiceError> {
    let Some(capability) = capability.and_then(parse_capability) else {
        return Err(invalid(UNKNOWN_CAPABILITY));
    };
    deps.catalogue.media().maybe_refresh(deps, capability);
    Ok(MediaModels::build(deps, capability))
}

pub async fn refresh_media(
    deps: &VizierDependencies,
    user: &Principal,
    capability: &str,
) -> Result<MediaModels, ServiceError> {
    use crate::catalogue::refresh::RefreshError;
    crate::access::require(user, "settings:providers")?;
    let Some(capability) = parse_capability(capability) else {
        return Err(invalid(UNKNOWN_CAPABILITY));
    };
    match deps.catalogue.media().refresh_now(deps, capability).await {
        Ok(()) => Ok(MediaModels::build(deps, capability)),
        Err(RefreshError::Disabled) => Err(ServiceError::Conflict {
            message: "catalogue refresh is disabled (catalogue.refresh: false)".into(),
            code: None,
            data: None,
        }),
        Err(RefreshError::Failed(e)) => Err(ServiceError::Invalid { status: 502, message: e, code: None }),
    }
}

pub async fn refresh(
    deps: &VizierDependencies,
    user: &Principal,
) -> Result<CatalogueStatus, ServiceError> {
    use crate::catalogue::refresh::{RefreshError, refresh_now};
    crate::access::require(user, "settings:providers")?;
    match refresh_now(deps).await {
        Ok(()) => Ok(status(deps)),
        Err(RefreshError::Disabled) => Err(ServiceError::Conflict {
            message: "catalogue refresh is disabled (catalogue.refresh: false)".into(),
            code: None,
            data: None,
        }),
        Err(RefreshError::Failed(e)) => Err(ServiceError::Invalid { status: 502, message: e, code: None }),
    }
}

/// The selectable providers, filtered by name, kind and connection.
pub async fn list_providers(
    deps: &VizierDependencies,
    q: Option<&str>,
    kind: Option<&str>,
    connected: Option<bool>,
) -> Vec<ProviderSummary> {
    crate::catalogue::refresh::maybe_refresh(deps);
    let q = q.map(str::to_lowercase).filter(|q| !q.is_empty());
    provider_summaries(deps)
        .await
        .into_iter()
        .filter(|p| {
            q.as_deref().is_none_or(|q| {
                p.name.to_lowercase().contains(q) || p.id.as_str().contains(q)
            })
        })
        .filter(|p| match kind {
            Some("chat") => p.dialect.is_some(),
            Some("media") => p.kind == ProviderKind::Media || MEDIA_PROVIDERS.contains(&p.id.as_str()),
            Some("local") => p.kind == ProviderKind::Local,
            Some("custom") => p.origin == ProviderOrigin::Custom,
            _ => true,
        })
        .filter(|p| connected.is_none_or(|want| (p.connection.status == "connected") == want))
        .collect()
}

pub async fn list_models(
    deps: &VizierDependencies,
    id: &str,
    all: bool,
) -> Result<Vec<ModelSummary>, ServiceError> {
    crate::catalogue::refresh::maybe_refresh(deps);
    let Ok(id) = ProviderId::parse(id) else {
        return Err(invalid("invalid provider id"));
    };
    models_for(deps, &id, all)
        .await
        .ok_or_else(|| ServiceError::NotFound("provider not found".into()))
}

pub async fn get_model(
    deps: &VizierDependencies,
    id: &str,
    model: &str,
) -> Result<ModelSummary, ServiceError> {
    crate::catalogue::refresh::maybe_refresh(deps);
    let Ok(id) = ProviderId::parse(id) else {
        return Err(invalid("invalid provider id"));
    };
    let found = match models_for(deps, &id, true).await {
        Some(models) => models.into_iter().find(|m| m.id == model),
        None => None,
    };
    match found {
        Some(mut m) => {
            m.default = false;
            Ok(m)
        }
        None => Err(ServiceError::NotFound("not in catalogue".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_capabilities_parse_and_anything_else_is_refused() {
        for (value, expected) in [
            ("speech", "speech"),
            ("transcription", "transcription"),
            ("image", "image"),
        ] {
            assert_eq!(parse_capability(value).map(|c| c.as_str()), Some(expected));
        }
        for bad in ["", "video", "Speech", " speech", "images"] {
            assert!(parse_capability(bad).is_none(), "{bad} parsed");
        }
        assert_eq!(UNKNOWN_CAPABILITY, "capability must be one of: speech, transcription, image");
    }

    /// FR-008/FR-023: the documented default is always visible, even when a refresh retired it.
    #[test]
    fn the_listing_injects_a_default_the_list_lost() {
        use crate::catalogue::media::{MediaCapability, MediaModel, listing};

        let models = vec![MediaModel {
            id: "someone/other".into(),
            name: "Other".into(),
            ..Default::default()
        }];
        let listed = listing(MediaCapability::Speech, &models);
        assert_eq!(listed[0].id, MediaCapability::Speech.default_model());
        assert!(listed[0].default && !listed[0].listed);
        assert_eq!(listed[1].id, "someone/other");
        assert!(!listed[1].default && listed[1].listed);

        // An empty list still shows the default, so the effective value is never invisible.
        let empty = listing(MediaCapability::Image, &[]);
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].id, "bytedance-seed/seedream-4.5");
        assert!(empty[0].default && !empty[0].listed);
    }
    /// FR-034a.1-a.5, FR-034b: the five outcomes and the precedence that decides between them.
    #[test]
    fn entry_pricing_follows_the_precedence_the_data_model_fixes() {
        use crate::catalogue::data::CatalogueData;

        let seeded = Catalogue::new(crate::catalogue::seed::load_seed());
        let entry = |price: Option<Cost>, model: Option<&str>| ProfileEntry {
            arn: "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc".into(),
            model: model.map(str::to_string),
            price,
            ..Default::default()
        };
        let hand = Cost { input: 1.5, output: 6.0, cache_read: None, cache_write: None };

        // 1. The hand-entered figure wins and stops there, whatever `model` holds.
        for model in [None, Some("amazon.nova-lite-v1:0"), Some("gone-model")] {
            let priced = entry_pricing(&seeded, &entry(Some(hand.clone()), model));
            assert_eq!(priced, EntryPricing::Explicit(hand.clone()));
            assert_eq!(priced.state(), "explicit");
            assert_eq!(priced.cost(), Some(&hand));
            assert_eq!(priced.flag(), None);
            assert_eq!(priced.reason(), None);
        }

        // 3. A paired id the catalogue carries is priced from it, as it stands now.
        let from_catalogue = entry_pricing(&seeded, &entry(None, Some("amazon.nova-lite-v1:0")));
        assert!(matches!(from_catalogue, EntryPricing::FromCatalogue(_)));
        assert_eq!(from_catalogue.state(), "from_catalogue");
        assert!(from_catalogue.cost().is_some());
        assert_eq!(from_catalogue.flag(), None);
        assert_eq!(from_catalogue.reason(), None);

        // 4. A gone id is reported with the id itself, never refused or re-pointed.
        let gone = entry_pricing(&seeded, &entry(None, Some("anthropic.retired-model-v1")));
        assert_eq!(
            gone,
            EntryPricing::NotResolved { model: "anthropic.retired-model-v1".into() }
        );
        assert_eq!(gone.state(), "not_resolved");
        assert_eq!(gone.flag(), Some("pricing not resolved"));
        assert!(gone.reason().unwrap().contains("anthropic.retired-model-v1"));
        assert_eq!(gone.cost(), None);

        // 5. Nothing paired — an id that is only whitespace is nothing paired too.
        for model in [None, Some("   ")] {
            let unset = entry_pricing(&seeded, &entry(None, model));
            assert_eq!(unset, EntryPricing::NotSet);
            assert_eq!(unset.state(), "not_set");
            assert_eq!(unset.flag(), Some("pricing not set"));
            assert!(unset.reason().is_some_and(|r| !r.is_empty()));
            assert_eq!(unset.cost(), None);
        }

        // 2. No Bedrock list wins over both `NotSet` and `NotResolved` — and the explicit price
        //    still wins over it, because row 1 comes first.
        let empty = Catalogue::new(CatalogueData::empty());
        for model in [None, Some("amazon.nova-lite-v1:0"), Some("gone-model")] {
            let no_catalogue = entry_pricing(&empty, &entry(None, model));
            assert_eq!(no_catalogue, EntryPricing::NoCatalogue);
            assert_eq!(no_catalogue.state(), "no_catalogue");
            assert_eq!(no_catalogue.flag(), Some("pricing not set"));
            assert!(no_catalogue.reason().is_some_and(|r| !r.is_empty()));
            assert_eq!(no_catalogue.cost(), None);
        }
        assert_eq!(
            entry_pricing(&empty, &entry(Some(hand.clone()), None)),
            EntryPricing::Explicit(hand)
        );

        // SC-021: the three absences are told apart by their reason alone.
        let reasons = [
            entry_pricing(&seeded, &entry(None, None)).reason().unwrap(),
            gone.reason().unwrap(),
            entry_pricing(&empty, &entry(None, Some("amazon.nova-lite-v1:0")))
                .reason()
                .unwrap(),
        ];
        for a in 0..reasons.len() {
            for b in a + 1..reasons.len() {
                assert_ne!(reasons[a], reasons[b], "two absences share wording");
            }
        }
    }
}
