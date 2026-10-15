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
