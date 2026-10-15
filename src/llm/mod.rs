//! vizier's provider layer, which replaced rig (spec 015): our own message types, a typed HTTP
//! error, five wire dialects behind one `HttpModel`, and media clients on the same transport.
//! Agent-agnostic: nothing here depends on `agents/`.

use std::sync::Arc;

pub mod auth;
pub mod dialect;
pub mod error;
pub mod http;
pub mod media;
pub mod model;
pub mod prepare;
pub mod resolve;
pub mod sse;
pub mod types;

#[cfg(test)]
mod transport_tests;
#[cfg(test)]
mod smoke_tests;

pub use dialect::DialectKind;
pub use error::LlmError;
pub use model::{CompletionModel, HttpModel, ListedModel, RetryingModel};
pub use resolve::{ContextSource, ResolvedModel};
pub use types::{
    AssistantPart, CompletionRequest, CompletionResponse, Message, Part, ToolChoice,
    ToolDefinition, Usage,
};

use crate::schema::agent::CompletionRetryConfig;

/// The completion model for a resolved model: the dialect is matched once, here, as a module
/// constructor registers its implementations, and wrapped in the retry decorator.
pub fn build(
    resolved: ResolvedModel,
    http: reqwest::Client,
    retry: CompletionRetryConfig,
) -> Arc<dyn CompletionModel> {
    let resolved = Arc::new(resolved);
    match resolved.dialect {
        DialectKind::OpenAiChat => wrap(dialect::openai_chat::OpenAiChat, resolved, http, retry),
        DialectKind::OpenAiResponses => {
            wrap(dialect::openai_responses::OpenAiResponses, resolved, http, retry)
        }
        DialectKind::Anthropic => wrap(dialect::anthropic::Anthropic, resolved, http, retry),
        DialectKind::Gemini => wrap(dialect::gemini::Gemini, resolved, http, retry),
        DialectKind::Ollama => wrap(dialect::ollama::Ollama, resolved, http, retry),
    }
}

fn wrap<D: dialect::Dialect>(
    dialect: D,
    resolved: Arc<ResolvedModel>,
    http: reqwest::Client,
    retry: CompletionRetryConfig,
) -> Arc<dyn CompletionModel> {
    Arc::new(RetryingModel::new(HttpModel::new(dialect, resolved, http), retry))
}
