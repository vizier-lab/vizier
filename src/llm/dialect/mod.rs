//! Wire dialects: pure translation between [`PreparedRequest`]/[`CompletionResponse`] and one
//! provider wire format. No I/O, and never a provider id: anything provider-specific comes in
//! through [`ResolvedModel`] (contract `provider-layer.md` § `Dialect`).

use serde::{Deserialize, Serialize};

use super::error::LlmError;
use super::model::ListedModel;
use super::resolve::ResolvedModel;
use super::types::{AssistantPart, CompletionResponse, Message, ToolChoice, ToolDefinition};

pub mod anthropic;
pub mod bedrock;
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
    Bedrock,
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

/// A top-level string field of a response body, if present and non-empty.
pub(crate) fn str_field(body: &serde_json::Value, key: &str) -> Option<String> {
    body.get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// A provider's stop reason in the GenAI semantic conventions' terms: `stop`, `length`,
/// `tool_call` or `content_filter`, and anything unrecognised lower-cased as it came. A reply
/// that calls a tool is `tool_call` even where the provider says only that it stopped
/// (Gemini, Ollama, the Responses API's `completed`).
pub(crate) fn finish_reason(raw: Option<&str>, content: &[AssistantPart]) -> Option<String> {
    let reason = match raw?.to_ascii_lowercase().as_str() {
        "stop" | "end_turn" | "stop_sequence" | "completed" => "stop".to_string(),
        "length" | "max_tokens" | "max_output_tokens" => "length".to_string(),
        "tool_calls" | "tool_use" | "function_call" => "tool_call".to_string(),
        "content_filter" | "content_filtered" | "guardrail_intervened" | "safety"
        | "recitation" | "blocklist" | "prohibited_content" | "spii" | "refusal" => {
            "content_filter".to_string()
        }
        other => other.to_string(),
    };
    let calls_a_tool = content
        .iter()
        .any(|p| matches!(p, AssistantPart::ToolCall { .. }));
    Some(match reason.as_str() {
        "stop" if calls_a_tool => "tool_call".to_string(),
        _ => reason,
    })
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

#[cfg(test)]
mod finish_reason_tests {
    use super::*;

    fn call() -> AssistantPart {
        AssistantPart::ToolCall {
            id: "c".into(),
            name: "f".into(),
            arguments: serde_json::json!({}),
            provider_data: None,
        }
    }

    #[test]
    fn maps_each_provider_onto_the_conventions() {
        let text = [AssistantPart::Text("hi".into())];
        for (raw, expected) in [
            ("stop", "stop"),                   // OpenAI, Ollama
            ("end_turn", "stop"),               // Anthropic
            ("STOP", "stop"),                   // Gemini
            ("completed", "stop"),              // Responses
            ("length", "length"),               // OpenAI, Ollama
            ("max_tokens", "length"),           // Anthropic
            ("MAX_TOKENS", "length"),           // Gemini
            ("max_output_tokens", "length"),    // Responses
            ("content_filter", "content_filter"),
            ("content_filtered", "content_filter"),   // Bedrock Converse
            ("guardrail_intervened", "content_filter"),
            ("SAFETY", "content_filter"),
            ("refusal", "content_filter"),
            ("pause_turn", "pause_turn"),       // unrecognised: kept, lower-cased
        ] {
            assert_eq!(finish_reason(Some(raw), &text).as_deref(), Some(expected), "{raw}");
        }
        assert_eq!(finish_reason(None, &text), None);
    }

    #[test]
    fn a_reply_that_calls_a_tool_is_a_tool_call() {
        for raw in ["tool_calls", "tool_use", "STOP", "stop", "completed"] {
            assert_eq!(finish_reason(Some(raw), &[call()]).as_deref(), Some("tool_call"), "{raw}");
        }
        // Cut off while calling a tool is still cut off.
        assert_eq!(finish_reason(Some("length"), &[call()]).as_deref(), Some("length"));
    }
}
