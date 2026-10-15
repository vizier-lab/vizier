//! Wire dialects: pure translation between [`PreparedRequest`]/[`CompletionResponse`] and one
//! provider wire format. No I/O, and never a provider id: anything provider-specific comes in
//! through [`ResolvedModel`] (contract `provider-layer.md` § `Dialect`).

use serde::{Deserialize, Serialize};

use super::error::LlmError;
use super::model::ListedModel;
use super::resolve::ResolvedModel;
use super::types::{CompletionResponse, Message, ToolChoice, ToolDefinition};

pub mod anthropic;
pub mod gemini;
pub mod ollama;
pub mod openai_chat;
pub mod openai_responses;

#[cfg(test)]
mod golden_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DialectKind {
    OpenAiChat,
    OpenAiResponses,
    Anthropic,
    Gemini,
    Ollama,
}

/// How a dialect wants system messages (contract S8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemStyle {
    /// One system field: leading system messages are joined into it, later ones become user
    /// text (Anthropic, Gemini).
    Field,
    /// System messages stay inline as `role: system` (OpenAI chat, Ollama).
    Inline,
    /// Leading system messages become `instructions`; later ones stay for the dialect to send
    /// as developer messages (Responses).
    Instructions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Json,
    /// A Server-Sent Events body read whole (the ChatGPT backend).
    Sse,
}

/// A [`super::types::CompletionRequest`] after `prepare`: attachments handled by capability
/// and system messages normalised for the dialect's [`SystemStyle`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreparedRequest {
    /// The system field (`Field`) or instructions (`Instructions`); always `None` for
    /// `Inline`.
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub tool_choice: Option<ToolChoice>,
    pub max_tokens: Option<u64>,
}

pub trait Dialect: Send + Sync + 'static {
    fn system_style(&self) -> SystemStyle;

    /// The request path, appended to the base URL.
    fn path(&self, m: &ResolvedModel) -> String;

    fn encode(&self, req: &PreparedRequest, m: &ResolvedModel) -> serde_json::Value;

    fn decode(&self, body: serde_json::Value) -> Result<CompletionResponse, LlmError>;

    /// Fixed headers the wire format needs (e.g. `anthropic-version`).
    fn headers(&self) -> Vec<(&'static str, String)> {
        vec![]
    }

    fn read_body(&self, _m: &ResolvedModel) -> BodyKind {
        BodyKind::Json
    }

    /// Reduce a whole SSE body to the JSON [`Dialect::decode`] reads.
    fn aggregate_sse(&self, _body: &str) -> Result<serde_json::Value, LlmError> {
        Err(LlmError::Unsupported)
    }

    fn list_models_path(&self, _m: &ResolvedModel) -> Option<String> {
        None
    }

    fn decode_models(&self, _body: serde_json::Value) -> Vec<ListedModel> {
        vec![]
    }
}

/// An error object inside a 2xx body (`{"error": {...}}` or `{"type": "error", ...}`), which
/// some providers send instead of a status code.
pub(crate) fn error_in_body(body: &serde_json::Value) -> Option<String> {
    let err = body.get("error")?;
    if err.is_null() {
        return None;
    }
    Some(
        err.get("message")
            .and_then(|m| m.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| err.to_string()),
    )
}

/// The list models shape every OpenAI-compatible server returns: `{"data": [{"id": …}]}`, with
/// `context_length` where the provider reports it (OpenRouter and others).
pub(crate) fn decode_openai_models(body: serde_json::Value) -> Vec<ListedModel> {
    body.get("data")
        .and_then(|d| d.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|m| {
                    Some(ListedModel {
                        id: m.get("id")?.as_str()?.to_string(),
                        context: ["context_length", "context_window", "max_model_len"]
                            .iter()
                            .find_map(|k| m.get(*k).and_then(|v| v.as_u64()))
                            .or_else(|| {
                                m.get("top_provider")
                                    .and_then(|t| t.get("context_length"))
                                    .and_then(|v| v.as_u64())
                            }),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Base URL plus path, with exactly one `/` between them.
pub(crate) fn join_url(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}
