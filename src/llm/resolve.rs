//! `(ProviderId, model, overrides)` → [`ResolvedModel`]: the only place that reads the
//! catalogue, built-ins, custom providers, credentials and endpoint rules (contract
//! `provider-layer.md` § Resolution).

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::auth::{CredentialSource, StaticKey};
use super::dialect::DialectKind;
use crate::catalogue::data::{Capabilities, Cost};
use crate::config::provider::ProviderId;

/// Where an agent's effective context window came from (R8), shown in agent health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    /// The agent's own `context_window`.
    Override,
    Catalogue,
    /// The provider's model listing (OpenRouter, Ollama `/api/show`, …).
    Listing,
    /// Detected from the model name.
    Name,
    Unknown,
}

/// Everything a dialect and `HttpModel` need to talk to one model (data-model §6).
#[derive(Clone)]
pub struct ResolvedModel {
    pub provider: ProviderId,
    pub model: String,
    pub dialect: DialectKind,
    pub base_url: String,
    pub credential: Arc<dyn CredentialSource>,
    /// Extra fixed headers (opencode-go's session id, a custom provider's own headers).
    pub headers: Vec<(String, String)>,
    /// Query parameters on every request (Azure's `api-version`).
    pub query: Vec<(String, String)>,
    pub context_window: Option<u64>,
    pub context_source: ContextSource,
    /// The agent's `max_tokens`: always sent when set.
    pub max_tokens: Option<u64>,
    /// The catalogue's output limit, sent only where the dialect requires a value (R8).
    pub max_output: Option<u64>,
    pub caps: Capabilities,
    pub cost: Option<Cost>,
    /// The backend only answers as a stream (ChatGPT): the body is read as SSE.
    pub stream_required: bool,
    /// Instructions merged ahead of the request's own (ChatGPT's default instructions).
    pub default_instructions: Option<String>,
}

impl std::fmt::Debug for ResolvedModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedModel")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("dialect", &self.dialect)
            .field("base_url", &self.base_url)
            .field("context_window", &self.context_window)
            .field("context_source", &self.context_source)
            .field("max_tokens", &self.max_tokens)
            .field("max_output", &self.max_output)
            .finish_non_exhaustive()
    }
}

impl ResolvedModel {
    /// A model with no credential and no limits, for tests and fixtures.
    #[cfg(test)]
    pub fn test(model: &str) -> Self {
        Self {
            provider: ProviderId::new_static("test"),
            model: model.to_string(),
            dialect: DialectKind::OpenAiChat,
            base_url: "http://127.0.0.1:1".into(),
            credential: StaticKey::none(),
            headers: vec![],
            query: vec![],
            context_window: None,
            context_source: ContextSource::Unknown,
            max_tokens: None,
            max_output: None,
            caps: Capabilities::default(),
            cost: None,
            stream_required: false,
            default_instructions: None,
        }
    }
}

// Resolution -------------------------------------------------------------------------------------

use super::auth::{ChatGptAccount, CopilotExchange, KeyStyle};
use crate::catalogue::Catalogue;
use crate::catalogue::builtin::{self, CHATGPT_BASE_URL};
use crate::catalogue::data::{self as catalogue_data, CatalogueModel, Limits, ProviderKind};
use crate::schema::provider::{CustomProtocol, CustomProvider, ProviderCredentials};
use crate::VizierError;

/// The agent's own values, which win over the catalogue's (FR-012).
#[derive(Debug, Clone, Default)]
pub struct ModelOverrides {
    pub context_window: Option<u64>,
    pub max_tokens: Option<u64>,
}

/// Everything `resolve_with` needs, already looked up: the catalogue, the custom provider with
/// this id if there is one, and the provider's credentials (saved, or from the environment).
pub struct ResolveContext<'a> {
    pub catalogue: &'a Catalogue,
    pub custom: Option<&'a CustomProvider>,
    pub credentials: Option<ProviderCredentials>,
    pub http: reqwest::Client,
}

/// Azure OpenAI's API version when `AZURE_API_VERSION` is unset (rig 0.39's default).
const AZURE_API_VERSION: &str = "2024-10-21";
const CHATGPT_DEFAULT_INSTRUCTIONS: &str = "You are ChatGPT, a helpful AI assistant.";

pub fn not_available(provider: &ProviderId) -> VizierError {
    VizierError(format!(
        "provider \"{provider}\" is not available; pick another provider"
    ))
}

/// Expand `${VAR}` from the environment, or from `extra` (e.g. Azure's resource name taken
/// from a saved endpoint).
fn expand(template: &str, extra: &[(&str, String)]) -> crate::Result<String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find('}')
            .ok_or_else(|| VizierError(format!("bad URL template {template}")))?;
        let name = &after[..end];
        let value = extra
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.clone())
            .or_else(|| std::env::var(name).ok().filter(|v| !v.is_empty()))
            .ok_or_else(|| VizierError(format!("set {name} to use this provider")))?;
        out.push_str(&value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Context window and output limit from the catalogue, in the research R8 order: `limit.input`
/// is the real prompt budget where it differs from the context.
fn limits(overrides: &ModelOverrides, limit: Option<&Limits>) -> (Option<u64>, ContextSource, Option<u64>) {
    let catalogue = limit.and_then(|l| l.input.or(l.context));
    let (window, source) = match (overrides.context_window, catalogue) {
        (Some(w), _) => (Some(w), ContextSource::Override),
        (None, Some(w)) => (Some(w), ContextSource::Catalogue),
        (None, None) => (None, ContextSource::Unknown),
    };
    (window, source, limit.and_then(|l| l.output))
}

fn key_style(dialect: DialectKind) -> KeyStyle {
    match dialect {
        DialectKind::Anthropic => KeyStyle::Header("x-api-key"),
        DialectKind::Gemini => KeyStyle::Header("x-goog-api-key"),
        _ => KeyStyle::Bearer,
    }
}

fn base(provider: &ProviderId, model: &str, dialect: DialectKind) -> ResolvedModel {
    ResolvedModel {
        provider: provider.clone(),
        model: model.to_string(),
        dialect,
        base_url: String::new(),
        credential: StaticKey::none(),
        headers: vec![],
        query: vec![],
        context_window: None,
        context_source: ContextSource::Unknown,
        max_tokens: None,
        max_output: None,
        caps: Capabilities::default(),
        cost: None,
        stream_required: false,
        default_instructions: None,
    }
}

fn missing_key(provider: &ProviderId, env: &[String]) -> VizierError {
    let hint = env
        .iter()
        .find(|e| e.contains("KEY") || e.contains("TOKEN"))
        .map(|e| format!(" or set {e}"))
        .unwrap_or_default();
    VizierError(format!(
        "no API key found for provider \"{provider}\": add one in Settings › Providers{hint}"
    ))
}

/// Resolve one `(provider, model)` (data-model §6). Custom providers come first, then the
/// catalogue, then the built-ins; anything else is "not available".
pub fn resolve_with(
    ctx: &ResolveContext,
    provider: &ProviderId,
    model: &str,
    overrides: &ModelOverrides,
) -> crate::Result<ResolvedModel> {
    if let Some(custom) = ctx.custom.filter(|c| &c.id == provider) {
        return Ok(resolve_custom(ctx, custom, model, overrides));
    }
    if let Some(b) = builtin::builtin(provider) {
        return resolve_builtin(ctx, b, model, overrides);
    }
    let Some(p) = ctx.catalogue.provider(provider) else {
        return Err(not_available(provider));
    };
    let entry = p.models.get(model);
    let (dialect, api) = catalogue_data::dialect_for(&p, entry).ok_or_else(|| not_available(provider))?;
    let creds = ctx.credentials.clone().unwrap_or_default();

    let mut resolved = base(provider, model, dialect);
    let (window, source, output) = limits(overrides, entry.map(|m| &m.limit));
    resolved.context_window = window;
    resolved.context_source = source;
    resolved.max_output = output;
    resolved.max_tokens = overrides.max_tokens;
    if let Some(m) = entry {
        resolved.caps = m.caps.clone();
        resolved.cost = m.cost.clone();
    }

    let key = creds.api_key.clone().filter(|k| !k.is_empty());
    if key.is_none() && p.kind != ProviderKind::Local {
        return Err(missing_key(provider, &p.env));
    }

    // Endpoint rules that are data on the provider, not branches in a dialect (research R5).
    let is_azure_openai = p.npm == "@ai-sdk/azure" && entry.and_then(|m| m.route.as_ref()).and_then(|r| r.api.as_ref()).is_none();
    let resource = creds
        .endpoint
        .as_deref()
        .and_then(|e| reqwest::Url::parse(e).ok())
        .and_then(|u| u.host_str().and_then(|h| h.split('.').next()).map(str::to_string));
    let extra: Vec<(&str, String)> = resource
        .iter()
        .map(|r| ("AZURE_RESOURCE_NAME", r.clone()))
        .collect();

    resolved.base_url = if is_azure_openai {
        let root = match creds.endpoint.as_deref().filter(|e| !e.is_empty()) {
            Some(endpoint) => endpoint.trim_end_matches('/').to_string(),
            None => expand("https://${AZURE_RESOURCE_NAME}.openai.azure.com", &extra)?,
        };
        resolved.query = vec![(
            "api-version".into(),
            std::env::var("AZURE_API_VERSION")
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| AZURE_API_VERSION.into()),
        )];
        format!("{root}/openai/deployments/{model}")
    } else {
        let api = api.ok_or_else(|| not_available(provider))?;
        expand(&api, &extra)?
    };

    resolved.credential = if *provider == ProviderId::GITHUB_COPILOT {
        CopilotExchange::new(ctx.http.clone(), key.unwrap_or_default(), resolved.base_url.clone())
    } else if p.npm == "@ai-sdk/azure" && dialect == DialectKind::OpenAiChat {
        StaticKey::shared(KeyStyle::Header("api-key"), key)
    } else {
        StaticKey::shared(key_style(dialect), key)
    };

    // OpenCode Go routes and caches by a session id that stays stable across requests from
    // one client (https://opencode.ai/docs/go/).
    if *provider == ProviderId::OPENCODE_GO {
        resolved.headers.push((
            "x-opencode-session".into(),
            format!("vizier-{}", uuid::Uuid::new_v4()),
        ));
    }
    Ok(resolved)
}

fn resolve_builtin(
    ctx: &ResolveContext,
    b: &builtin::BuiltinProvider,
    model: &str,
    overrides: &ModelOverrides,
) -> crate::Result<ResolvedModel> {
    let dialect = b.dialect.ok_or_else(|| {
        if b.id == ProviderId::DUMMYPLUG {
            VizierError("dummyplug has its own model and is not resolved over HTTP".into())
        } else {
            VizierError(format!(
                "\"{}\" is a media provider and can't be used for chat; pick another provider",
                b.id
            ))
        }
    })?;
    let creds = ctx.credentials.clone().unwrap_or_default();
    let mut resolved = base(&b.id, model, dialect);
    resolved.context_window = overrides.context_window;
    if overrides.context_window.is_some() {
        resolved.context_source = ContextSource::Override;
    }
    resolved.max_tokens = overrides.max_tokens;
    let configured = creds
        .base_url
        .clone()
        .filter(|u| !u.is_empty())
        .or_else(|| b.default_base_url.map(str::to_string))
        .unwrap_or_default();

    if b.id == ProviderId::CHATGPT {
        let token = creds
            .access_token
            .clone()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| {
                VizierError(
                    "no credentials found for chatgpt: add them in Settings › Providers or set \
                     CHATGPT_ACCESS_TOKEN and CHATGPT_ACCOUNT_ID"
                        .into(),
                )
            })?;
        resolved.credential = ChatGptAccount::new(token, creds.account_id.clone());
        resolved.base_url = if configured.is_empty() {
            CHATGPT_BASE_URL.into()
        } else {
            configured
        };
        resolved.stream_required = true;
        resolved.default_instructions = Some(
            std::env::var("CHATGPT_DEFAULT_INSTRUCTIONS")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| CHATGPT_DEFAULT_INSTRUCTIONS.into()),
        );
        return Ok(resolved);
    }

    resolved.base_url = if b.id == ProviderId::LLAMA_CPP && !configured.trim_end_matches('/').ends_with("/v1") {
        // llama.cpp's server serves the OpenAI API under `/v1`.
        format!("{}/v1", configured.trim_end_matches('/'))
    } else {
        configured
    };
    resolved.credential = StaticKey::shared(key_style(dialect), creds.api_key.clone());
    Ok(resolved)
}

/// A custom provider's model (FR-024): the operator's row where set, then the first catalogue
/// model with the same id (preferring a provider of the same protocol), then unknown.
fn resolve_custom(
    ctx: &ResolveContext,
    custom: &CustomProvider,
    model: &str,
    overrides: &ModelOverrides,
) -> ResolvedModel {
    let dialect = match custom.protocol {
        CustomProtocol::OpenaiCompatible => DialectKind::OpenAiChat,
        CustomProtocol::Anthropic => DialectKind::Anthropic,
    };
    let row = custom.models.iter().find(|m| m.id == model);
    let borrowed: Option<CatalogueModel> = ctx.catalogue.find_model_by_id(model, Some(dialect));

    let mut limit = borrowed.as_ref().map(|m| m.limit.clone()).unwrap_or_default();
    let mut caps = borrowed.as_ref().map(|m| m.caps.clone()).unwrap_or_default();
    if let Some(row) = row {
        if row.context.is_some() {
            limit.input = None;
            limit.context = row.context;
        }
        if row.output.is_some() {
            limit.output = row.output;
        }
        caps.tool_call = row.tool_call.or(caps.tool_call);
        caps.reasoning = row.reasoning.or(caps.reasoning);
        if row.image_input.is_some() {
            caps.image_input_override = row.image_input;
        }
    }

    let mut resolved = base(&custom.id, model, dialect);
    let (window, source, output) = limits(overrides, Some(&limit));
    resolved.context_window = window;
    resolved.context_source = source;
    resolved.max_output = output;
    resolved.max_tokens = overrides.max_tokens;
    resolved.caps = caps;
    resolved.cost = borrowed.and_then(|m| m.cost);
    resolved.base_url = custom.base_url.clone();
    resolved.headers = custom
        .headers
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    resolved.credential = StaticKey::shared(key_style(dialect), custom.api_key.clone());
    resolved
}

/// The env vars a provider's credentials may come from: the catalogue's, or a built-in's.
pub fn provider_env(catalogue: &Catalogue, provider: &ProviderId) -> Vec<String> {
    catalogue
        .provider(provider)
        .map(|p| p.env)
        .or_else(|| builtin::builtin(provider).map(|b| b.env.iter().map(|e| e.to_string()).collect()))
        .unwrap_or_default()
}

/// [`resolve_with`] after looking up this provider's custom definition and credentials.
pub async fn resolve(
    deps: &crate::dependencies::VizierDependencies,
    provider: &ProviderId,
    model: &str,
    overrides: &ModelOverrides,
) -> crate::Result<ResolvedModel> {
    use crate::storage::provider::CustomProviderStorage;

    let custom = deps
        .storage
        .get_custom_provider(provider)
        .await
        .map_err(|e| VizierError(e.to_string()))?;
    let env = provider_env(&deps.catalogue, provider);
    let credentials = crate::provider_keys::resolve_credentials(&deps.storage, provider, &env)
        .await
        .map(|(c, _)| c);
    resolve_with(
        &ResolveContext {
            catalogue: &deps.catalogue,
            custom: custom.as_ref(),
            credentials,
            http: deps.http.clone(),
        },
        provider,
        model,
        overrides,
    )
}

#[cfg(test)]
mod resolve_tests {
    use super::*;
    use crate::catalogue::data::parse;
    use crate::schema::provider::CustomModel;

    fn catalogue() -> Catalogue {
        Catalogue::new(
            parse(
                br#"{
          "anthropic": {"id": "anthropic", "name": "Anthropic", "npm": "@ai-sdk/anthropic", "env": ["ANTHROPIC_API_KEY"], "models": {
            "claude-x": {"id": "claude-x", "tool_call": true, "modalities": {"input": ["text", "image"]},
                         "limit": {"context": 200000, "output": 64000}, "cost": {"input": 3, "output": 15}}
          }},
          "openai": {"id": "openai", "name": "OpenAI", "npm": "@ai-sdk/openai", "models": {
            "gpt-5": {"id": "gpt-5", "limit": {"context": 400000, "input": 272000, "output": 128000}}
          }},
          "routed": {"id": "routed", "name": "Routed", "npm": "@ai-sdk/openai-compatible", "api": "https://r.test/v1", "models": {
            "claude-x": {"id": "claude-x", "provider": {"npm": "@ai-sdk/anthropic", "api": "https://r.test/anthropic/v1"}}
          }},
          "azure": {"id": "azure", "name": "Azure", "npm": "@ai-sdk/azure", "env": ["AZURE_RESOURCE_NAME", "AZURE_API_KEY"], "models": {
            "gpt-4o": {"id": "gpt-4o", "limit": {"context": 128000}}
          }},
          "lmstudio": {"id": "lmstudio", "name": "LM Studio", "npm": "@ai-sdk/openai-compatible", "api": "http://127.0.0.1:1234/v1", "models": {}},
          "bedrock": {"id": "bedrock", "name": "B", "npm": "@ai-sdk/amazon-bedrock", "models": {}}
        }"#,
            )
            .unwrap(),
        )
    }

    fn ctx<'a>(c: &'a Catalogue, creds: Option<ProviderCredentials>) -> ResolveContext<'a> {
        ResolveContext {
            catalogue: c,
            custom: None,
            credentials: creds,
            http: reqwest::Client::new(),
        }
    }

    fn key(k: &str) -> Option<ProviderCredentials> {
        Some(ProviderCredentials {
            api_key: Some(k.into()),
            ..Default::default()
        })
    }

    fn id(s: &str) -> ProviderId {
        ProviderId::new(s.to_string()).unwrap()
    }

    #[test]
    fn context_window_order_is_override_then_input_then_context() {
        let c = catalogue();
        let none = ModelOverrides::default();
        let r = resolve_with(&ctx(&c, key("k")), &id("openai"), "gpt-5", &none).unwrap();
        assert_eq!((r.context_window, r.context_source), (Some(272000), ContextSource::Catalogue));
        let r = resolve_with(&ctx(&c, key("k")), &id("anthropic"), "claude-x", &none).unwrap();
        assert_eq!(r.context_window, Some(200000));
        let r = resolve_with(
            &ctx(&c, key("k")),
            &id("anthropic"),
            "claude-x",
            &ModelOverrides {
                context_window: Some(50000),
                max_tokens: None,
            },
        )
        .unwrap();
        assert_eq!((r.context_window, r.context_source), (Some(50000), ContextSource::Override));
        let r = resolve_with(&ctx(&c, key("k")), &id("anthropic"), "unlisted", &none).unwrap();
        assert_eq!((r.context_window, r.context_source), (None, ContextSource::Unknown));
    }

    #[test]
    fn max_output_comes_from_the_catalogue_and_max_tokens_from_the_agent() {
        let c = catalogue();
        let r = resolve_with(
            &ctx(&c, key("k")),
            &id("anthropic"),
            "claude-x",
            &ModelOverrides {
                context_window: None,
                max_tokens: Some(1000),
            },
        )
        .unwrap();
        assert_eq!(r.max_output, Some(64000));
        assert_eq!(r.max_tokens, Some(1000));
        assert_eq!(r.base_url, "https://api.anthropic.com/v1");
        assert_eq!(r.caps.image_input(), Some(true));
        assert!(r.cost.is_some());
    }

    #[test]
    fn a_route_override_picks_the_dialect_and_url() {
        let c = catalogue();
        let r = resolve_with(&ctx(&c, key("k")), &id("routed"), "claude-x", &ModelOverrides::default()).unwrap();
        assert_eq!(r.dialect, DialectKind::Anthropic);
        assert_eq!(r.base_url, "https://r.test/anthropic/v1");
    }

    #[test]
    fn azure_builds_the_deployment_url() {
        let c = catalogue();
        let creds = Some(ProviderCredentials {
            api_key: Some("k".into()),
            endpoint: Some("https://myres.openai.azure.com/".into()),
            ..Default::default()
        });
        let r = resolve_with(&ctx(&c, creds), &id("azure"), "gpt-4o", &ModelOverrides::default()).unwrap();
        assert_eq!(r.base_url, "https://myres.openai.azure.com/openai/deployments/gpt-4o");
        assert_eq!(r.query[0].0, "api-version");
        assert_eq!(expand("https://${X}.test", &[("X", "abc".into())]).unwrap(), "https://abc.test");
        assert!(expand("https://${VZT_SURELY_UNSET}.test", &[]).is_err());
    }

    #[test]
    fn unknown_unsupported_and_keyless_providers_are_refused() {
        let c = catalogue();
        let err = resolve_with(&ctx(&c, None), &id("galadriel"), "m", &ModelOverrides::default()).unwrap_err();
        assert_eq!(err.0, "provider \"galadriel\" is not available; pick another provider");
        assert!(resolve_with(&ctx(&c, key("k")), &id("bedrock"), "m", &ModelOverrides::default()).is_err());
        assert!(resolve_with(&ctx(&c, None), &id("anthropic"), "claude-x", &ModelOverrides::default()).is_err());
        // Local catalogue providers need no key.
        assert!(resolve_with(&ctx(&c, None), &id("lmstudio"), "x", &ModelOverrides::default()).is_ok());
    }

    #[test]
    fn builtins_resolve_with_their_defaults() {
        let c = catalogue();
        let r = resolve_with(&ctx(&c, None), &ProviderId::OLLAMA, "llama3.2", &ModelOverrides::default()).unwrap();
        assert_eq!((r.dialect, r.base_url.as_str()), (DialectKind::Ollama, "http://localhost:11434"));
        let r = resolve_with(&ctx(&c, None), &ProviderId::LLAMA_CPP, "m", &ModelOverrides::default()).unwrap();
        assert_eq!(r.base_url, "http://localhost:8080/v1");
        let creds = Some(ProviderCredentials {
            access_token: Some("t".into()),
            ..Default::default()
        });
        let r = resolve_with(&ctx(&c, creds), &ProviderId::CHATGPT, "gpt-5.4", &ModelOverrides::default()).unwrap();
        assert!(r.stream_required);
        assert_eq!(r.base_url, CHATGPT_BASE_URL);
        assert!(resolve_with(&ctx(&c, None), &ProviderId::ELEVENLABS, "x", &ModelOverrides::default()).is_err());
    }

    #[test]
    fn a_custom_model_fills_in_from_the_catalogue() {
        let c = catalogue();
        let custom = CustomProvider {
            id: id("home"),
            name: "Home".into(),
            protocol: CustomProtocol::Anthropic,
            base_url: "http://10.0.0.5/v1".into(),
            api_key: Some("k".into()),
            headers: [("x-extra".to_string(), "1".to_string())].into(),
            models: vec![CustomModel {
                id: "claude-x".into(),
                output: Some(8192),
                image_input: Some(false),
                ..Default::default()
            }],
            models_fetched_at: None,
        };
        let context = ResolveContext {
            custom: Some(&custom),
            ..ctx(&c, None)
        };
        let r = resolve_with(&context, &id("home"), "claude-x", &ModelOverrides::default()).unwrap();
        assert_eq!(r.dialect, DialectKind::Anthropic);
        assert_eq!(r.context_window, Some(200000), "from the catalogue");
        assert_eq!(r.max_output, Some(8192), "from the row");
        assert_eq!(r.caps.image_input(), Some(false), "the row wins");
        assert_eq!(r.caps.tool_call, Some(true), "filled in");
        assert_eq!(r.headers, vec![("x-extra".into(), "1".into())]);

        let r = resolve_with(&context, &id("home"), "own-model", &ModelOverrides::default()).unwrap();
        assert_eq!(r.context_window, None);
    }
}
