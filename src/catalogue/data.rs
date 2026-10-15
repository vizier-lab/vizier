//! The catalogue's data model (data-model §2): models.dev providers and models, trimmed to the
//! fields vizier uses, plus what is derived from them (dialect, kind).

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::config::provider::ProviderId;
use crate::llm::dialect::DialectKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogueSource {
    /// The build-time seed embedded in the binary.
    Bundled,
    /// A refreshed copy from the `catalogue_snapshot` table.
    Cached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Cloud,
    /// Runs on the operator's machine or network (the `api` host is localhost/127.0.0.1).
    Local,
    /// Credentials only, used by media and embedding tools; never offered for chat.
    Media,
}

#[derive(Debug, Clone)]
pub struct CatalogueData {
    pub source: CatalogueSource,
    pub fetched_at: DateTime<Utc>,
    pub checked_at: Option<DateTime<Utc>>,
    pub etag: Option<String>,
    pub providers: BTreeMap<ProviderId, CatalogueProvider>,
}

impl CatalogueData {
    pub fn empty() -> Self {
        Self {
            source: CatalogueSource::Bundled,
            fetched_at: DateTime::<Utc>::UNIX_EPOCH,
            checked_at: None,
            etag: None,
            providers: BTreeMap::new(),
        }
    }

    pub fn model_count(&self) -> usize {
        self.providers.values().map(|p| p.models.len()).sum()
    }
}

#[derive(Debug, Clone)]
pub struct CatalogueProvider {
    pub id: ProviderId,
    pub name: String,
    /// Credential env vars, in lookup order.
    pub env: Vec<String>,
    /// Base URL; may contain `${VAR}`.
    pub api: Option<String>,
    /// The AI SDK package, used as the protocol hint (research R2).
    pub npm: String,
    pub doc: Option<String>,
    pub models: BTreeMap<String, CatalogueModel>,
    /// `None`: no dialect vizier speaks, so the provider is hidden from pickers (FR-008).
    pub dialect: Option<DialectKind>,
    pub kind: ProviderKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogueModel {
    pub id: String,
    pub name: String,
    pub family: Option<String>,
    pub caps: Capabilities,
    pub limit: Limits,
    pub cost: Option<Cost>,
    pub status: ModelStatus,
    pub release_date: Option<NaiveDate>,
    pub knowledge: Option<String>,
    /// The `canonical_model_id` prefix, for the model's logo.
    pub lab: Option<String>,
    pub route: Option<RouteOverride>,
}

/// What a model can do. Absence is "unknown", never "false", and unknown counts as supported
/// (FR-013).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Capabilities {
    pub tool_call: Option<bool>,
    pub reasoning: Option<bool>,
    pub attachment: Option<bool>,
    pub temperature: Option<bool>,
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default)]
    pub output: Vec<String>,
    /// Set directly for custom models, which state it instead of listing modalities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_input_override: Option<bool>,
}

impl Capabilities {
    /// `Some(true)` when the input modalities contain `image`; unknown when they are empty.
    pub fn image_input(&self) -> Option<bool> {
        if let Some(v) = self.image_input_override {
            return Some(v);
        }
        if self.input.is_empty() {
            None
        } else {
            Some(self.input.iter().any(|m| m == "image"))
        }
    }

    pub fn pdf_input(&self) -> Option<bool> {
        if self.input.is_empty() {
            None
        } else {
            Some(self.input.iter().any(|m| m == "pdf"))
        }
    }
}

/// Token limits; a `0` in the catalogue is unknown.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Limits {
    pub context: Option<u64>,
    pub input: Option<u64>,
    pub output: Option<u64>,
}

/// USD per 1M tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Cost {
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    #[default]
    Active,
    Beta,
    Alpha,
    Deprecated,
}

/// A model's own `provider.{npm, api, shape}`, overriding the provider's.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RouteOverride {
    pub npm: Option<String>,
    pub api: Option<String>,
    pub shape: Option<String>,
}

// Parsing ----------------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawProvider {
    id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    env: Vec<String>,
    npm: Option<String>,
    api: Option<String>,
    doc: Option<String>,
    #[serde(default)]
    models: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct RawModel {
    id: Option<String>,
    name: Option<String>,
    family: Option<String>,
    attachment: Option<bool>,
    reasoning: Option<bool>,
    tool_call: Option<bool>,
    temperature: Option<bool>,
    knowledge: Option<String>,
    release_date: Option<String>,
    modalities: Option<RawModalities>,
    limit: Option<RawLimits>,
    cost: Option<RawCost>,
    status: Option<String>,
    canonical_model_id: Option<String>,
    provider: Option<RouteOverride>,
}

#[derive(Deserialize)]
struct RawModalities {
    #[serde(default)]
    input: Vec<String>,
    #[serde(default)]
    output: Vec<String>,
}

#[derive(Deserialize)]
struct RawLimits {
    context: Option<f64>,
    input: Option<f64>,
    output: Option<f64>,
}

#[derive(Deserialize)]
struct RawCost {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
}

fn limit(v: Option<f64>) -> Option<u64> {
    v.filter(|n| n.is_finite() && *n >= 1.0).map(|n| n as u64)
}

/// Parse a (trimmed or raw) models.dev `api.json`. Unknown fields are ignored; a provider or
/// model that does not parse is skipped and logged at `debug`, never failing the whole
/// catalogue.
pub fn parse(json: &[u8]) -> crate::Result<CatalogueData> {
    let raw: BTreeMap<String, serde_json::Value> = serde_json::from_slice(json)
        .map_err(|e| crate::VizierError(format!("catalogue is not valid JSON: {e}")))?;

    let mut providers = BTreeMap::new();
    for (key, value) in raw {
        let provider: RawProvider = match serde_json::from_value(value) {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!("catalogue: skipping provider {key}: {e}");
                continue;
            }
        };
        let Ok(id) = ProviderId::new(provider.id.clone().unwrap_or_else(|| key.clone())) else {
            tracing::debug!("catalogue: skipping provider with invalid id {key:?}");
            continue;
        };

        let mut models = BTreeMap::new();
        for (model_key, value) in provider.models {
            match serde_json::from_value::<RawModel>(value) {
                Ok(m) => {
                    let model = to_model(model_key.clone(), m);
                    models.insert(model.id.clone(), model);
                }
                Err(e) => tracing::debug!("catalogue: skipping model {key}/{model_key}: {e}"),
            }
        }

        let npm = provider.npm.unwrap_or_default();
        let api = provider.api.filter(|a| !a.trim().is_empty());
        let dialect = dialect_for_npm(&npm, None);
        let kind = kind_for_api(api.as_deref());
        providers.insert(
            id.clone(),
            CatalogueProvider {
                name: provider.name.unwrap_or_else(|| id.to_string()),
                id,
                env: provider.env,
                api,
                npm,
                doc: provider.doc,
                models,
                dialect,
                kind,
            },
        );
    }

    Ok(CatalogueData {
        providers,
        ..CatalogueData::empty()
    })
}

fn to_model(key: String, m: RawModel) -> CatalogueModel {
    let (input, output) = m
        .modalities
        .map(|md| (md.input, md.output))
        .unwrap_or_default();
    CatalogueModel {
        name: m.name.unwrap_or_else(|| m.id.clone().unwrap_or_else(|| key.clone())),
        id: m.id.unwrap_or(key),
        family: m.family,
        caps: Capabilities {
            tool_call: m.tool_call,
            reasoning: m.reasoning,
            attachment: m.attachment,
            temperature: m.temperature,
            input,
            output,
            image_input_override: None,
        },
        limit: m
            .limit
            .map(|l| Limits {
                context: limit(l.context),
                input: limit(l.input),
                output: limit(l.output),
            })
            .unwrap_or_default(),
        cost: m.cost.and_then(|c| {
            Some(Cost {
                input: c.input?,
                output: c.output?,
                cache_read: c.cache_read,
                cache_write: c.cache_write,
            })
        }),
        status: match m.status.as_deref() {
            Some("deprecated") => ModelStatus::Deprecated,
            Some("beta") => ModelStatus::Beta,
            Some("alpha") => ModelStatus::Alpha,
            _ => ModelStatus::Active,
        },
        release_date: m
            .release_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
        knowledge: m.knowledge,
        lab: m
            .canonical_model_id
            .as_deref()
            .and_then(|c| c.split_once('/'))
            .map(|(lab, _)| lab.to_string()),
        route: m.provider.filter(|r| r.npm.is_some() || r.api.is_some() || r.shape.is_some()),
    }
}

/// The `npm` hint → dialect map (research R2). A `shape` of `responses` or `completions`
/// overrides it.
pub fn dialect_for_npm(npm: &str, shape: Option<&str>) -> Option<DialectKind> {
    let by_npm = match npm {
        "@ai-sdk/openai-compatible"
        | "@openrouter/ai-sdk-provider"
        | "@ai-sdk/groq"
        | "@ai-sdk/mistral"
        | "@ai-sdk/xai"
        | "@ai-sdk/togetherai"
        | "@ai-sdk/deepinfra"
        | "@ai-sdk/cerebras"
        | "@ai-sdk/perplexity"
        | "@ai-sdk/cohere"
        | "@ai-sdk/azure" => Some(DialectKind::OpenAiChat),
        "@ai-sdk/openai" => Some(DialectKind::OpenAiResponses),
        "@ai-sdk/anthropic" => Some(DialectKind::Anthropic),
        "@ai-sdk/google" => Some(DialectKind::Gemini),
        _ => None,
    }?;
    Some(match shape {
        Some("responses") => DialectKind::OpenAiResponses,
        Some("completions") => DialectKind::OpenAiChat,
        _ => by_npm,
    })
}

/// The dialect and base URL for one model of a provider: a model's own route override wins
/// over the provider's `npm`/`api`. `None` when vizier can't speak to it.
pub fn dialect_for(
    provider: &CatalogueProvider,
    model: Option<&CatalogueModel>,
) -> Option<(DialectKind, Option<String>)> {
    let route = model.and_then(|m| m.route.as_ref());
    let npm = route
        .and_then(|r| r.npm.as_deref())
        .unwrap_or(&provider.npm);
    let shape = route.and_then(|r| r.shape.as_deref());
    let dialect = dialect_for_npm(npm, shape)?;
    let api = route
        .and_then(|r| r.api.clone())
        .or_else(|| provider.api.clone())
        .or_else(|| default_base_url(&provider.id).map(str::to_string));
    Some((dialect, api))
}

/// Default base URLs for supported providers the catalogue gives no `api` (research R3).
/// These follow the AI SDK `baseURL` convention the catalogue's own `api` values use.
pub fn default_base_url(id: &ProviderId) -> Option<&'static str> {
    Some(match id.as_str() {
        "openai" => "https://api.openai.com/v1",
        "anthropic" => "https://api.anthropic.com/v1",
        "google" => "https://generativelanguage.googleapis.com/v1beta",
        "groq" => "https://api.groq.com/openai/v1",
        "mistral" => "https://api.mistral.ai/v1",
        "xai" => "https://api.x.ai/v1",
        "togetherai" => "https://api.together.xyz/v1",
        "perplexity" => "https://api.perplexity.ai",
        "cerebras" => "https://api.cerebras.ai/v1",
        "deepinfra" => "https://api.deepinfra.com/v1/openai",
        "cohere" => "https://api.cohere.ai/compatibility/v1",
        _ => return None,
    })
}

fn kind_for_api(api: Option<&str>) -> ProviderKind {
    let Some(api) = api else {
        return ProviderKind::Cloud;
    };
    let host = reqwest::Url::parse(api)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase));
    match host.as_deref() {
        Some("localhost") | Some("127.0.0.1") | Some("[::1]") | Some("::1") => {
            ProviderKind::Local
        }
        _ => ProviderKind::Cloud,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "acme": {"id": "acme", "name": "Acme", "env": ["ACME_API_KEY"], "npm": "@ai-sdk/openai-compatible",
               "api": "https://api.acme.test/v1", "unknown_field": 1,
               "models": {
                 "chat-1": {"id": "chat-1", "name": "Chat 1", "tool_call": true,
                            "modalities": {"input": ["text", "image"], "output": ["text"]},
                            "limit": {"context": 200000, "input": 150000, "output": 0},
                            "cost": {"input": 1, "output": 2}, "release_date": "2025-01-02",
                            "canonical_model_id": "acmelab/chat-1"},
                 "claude-ish": {"id": "claude-ish", "provider": {"npm": "@ai-sdk/anthropic", "api": "https://api.acme.test/anthropic/v1"}},
                 "bad": {"id": 5}
               }},
      "openai": {"id": "openai", "name": "OpenAI", "env": ["OPENAI_API_KEY"], "npm": "@ai-sdk/openai", "models": {}},
      "bedrock": {"id": "bedrock", "name": "Bedrock", "npm": "@ai-sdk/amazon-bedrock", "models": {}},
      "local": {"id": "local", "name": "Local", "npm": "@ai-sdk/openai-compatible", "api": "http://127.0.0.1:1234/v1", "models": {}}
    }"#;

    fn data() -> CatalogueData {
        parse(FIXTURE.as_bytes()).unwrap()
    }

    fn id(s: &str) -> ProviderId {
        ProviderId::new(s.to_string()).unwrap()
    }

    #[test]
    fn parses_and_skips_bad_models() {
        let d = data();
        let acme = &d.providers[&id("acme")];
        assert_eq!(acme.models.len(), 2, "the bad model is skipped");
        let m = &acme.models["chat-1"];
        assert_eq!(m.caps.image_input(), Some(true));
        assert_eq!(m.limit.output, None, "0 is unknown");
        assert_eq!(m.limit.input, Some(150000));
        assert_eq!(m.lab.as_deref(), Some("acmelab"));
        assert_eq!(m.release_date, NaiveDate::from_ymd_opt(2025, 1, 2));
        assert_eq!(acme.models["claude-ish"].caps.image_input(), None);
    }

    #[test]
    fn a_model_override_wins() {
        let d = data();
        let acme = &d.providers[&id("acme")];
        assert_eq!(
            dialect_for(acme, acme.models.get("claude-ish")),
            Some((
                DialectKind::Anthropic,
                Some("https://api.acme.test/anthropic/v1".into())
            ))
        );
        assert_eq!(
            dialect_for(acme, acme.models.get("chat-1")),
            Some((DialectKind::OpenAiChat, Some("https://api.acme.test/v1".into())))
        );
    }

    #[test]
    fn an_unsupported_provider_has_no_dialect() {
        let d = data();
        let bedrock = &d.providers[&id("bedrock")];
        assert_eq!(bedrock.dialect, None);
        assert_eq!(dialect_for(bedrock, None), None);
    }

    #[test]
    fn a_missing_api_uses_the_default_table() {
        let d = data();
        let openai = &d.providers[&id("openai")];
        assert_eq!(
            dialect_for(openai, None),
            Some((
                DialectKind::OpenAiResponses,
                Some("https://api.openai.com/v1".into())
            ))
        );
    }

    #[test]
    fn localhost_providers_are_local() {
        let d = data();
        assert_eq!(d.providers[&id("local")].kind, ProviderKind::Local);
        assert_eq!(d.providers[&id("acme")].kind, ProviderKind::Cloud);
    }

    #[test]
    fn shape_overrides_npm() {
        assert_eq!(
            dialect_for_npm("@ai-sdk/openai-compatible", Some("responses")),
            Some(DialectKind::OpenAiResponses)
        );
        assert_eq!(
            dialect_for_npm("@ai-sdk/azure", Some("completions")),
            Some(DialectKind::OpenAiChat)
        );
    }
}
