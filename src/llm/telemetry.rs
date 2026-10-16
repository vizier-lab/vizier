//! OpenTelemetry GenAI semantic conventions for a model call: the `chat {model}` client span
//! and the `gen_ai.client.*` metrics. Both are no-ops unless `crate::telemetry` installed an
//! exporter at startup. Message content is opt-in, as the conventions require: set
//! `OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true` to put prompts and replies on the
//! span (`just run-otel` does).

use std::sync::LazyLock;
use std::time::Duration;

use opentelemetry::KeyValue;
use opentelemetry::metrics::Histogram;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::error::LlmError;
use super::resolve::ResolvedModel;
use super::types::{AssistantPart, CompletionRequest, CompletionResponse, Message, Part};

/// Whether prompts, replies and tool definitions go on the `chat` span.
pub fn capture_content() -> bool {
    static CAPTURE: LazyLock<bool> = LazyLock::new(|| {
        std::env::var("OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT")
            .is_ok_and(|v| v.eq_ignore_ascii_case("true"))
    });
    *CAPTURE
}

/// The client span for one request to a provider. Response fields are filled by [`record`].
pub fn chat_span(resolved: &ResolvedModel, max_tokens: Option<u64>) -> Span {
    let (address, port) = server(&resolved.base_url);
    tracing::info_span!(
        "chat",
        otel.name = %format!("chat {}", resolved.model),
        otel.kind = "client",
        otel.status_code = tracing::field::Empty,
        gen_ai.operation.name = "chat",
        gen_ai.provider.name = %resolved.provider,
        gen_ai.request.model = %resolved.model,
        gen_ai.request.max_tokens = max_tokens,
        server.address = address.as_deref(),
        server.port = port,
        gen_ai.response.id = tracing::field::Empty,
        gen_ai.response.model = tracing::field::Empty,
        gen_ai.system_instructions = tracing::field::Empty,
        gen_ai.input.messages = tracing::field::Empty,
        gen_ai.output.messages = tracing::field::Empty,
        gen_ai.tool.definitions = tracing::field::Empty,
        gen_ai.usage.input_tokens = tracing::field::Empty,
        gen_ai.usage.output_tokens = tracing::field::Empty,
        gen_ai.usage.cache_read.input_tokens = tracing::field::Empty,
        gen_ai.usage.cache_creation.input_tokens = tracing::field::Empty,
        error.type = tracing::field::Empty,
    )
}

/// Fill the span's response fields and record the `gen_ai.client.*` metrics.
pub fn record(
    span: &Span,
    resolved: &ResolvedModel,
    elapsed: Duration,
    result: &Result<CompletionResponse, LlmError>,
) {
    let mut attrs = vec![
        KeyValue::new("gen_ai.operation.name", "chat"),
        KeyValue::new("gen_ai.provider.name", resolved.provider.to_string()),
        KeyValue::new("gen_ai.request.model", resolved.model.clone()),
    ];
    if let (Some(address), _) = server(&resolved.base_url) {
        attrs.push(KeyValue::new("server.address", address));
    }

    match result {
        Ok(response) => {
            let usage = &response.usage;
            if capture_content() {
                let output = serde_json::json!([{
                    "role": "assistant",
                    "parts": response.content.iter().map(assistant_part).collect::<Vec<_>>(),
                    // The conventions require one; a provider that gave none is read as `stop`.
                    "finish_reason": response.finish_reason.as_deref().unwrap_or("stop"),
                }]);
                span.record("gen_ai.output.messages", output.to_string());
            }
            if let Some(id) = &response.id {
                span.record("gen_ai.response.id", id.as_str());
            }
            if let Some(model) = &response.model {
                span.record("gen_ai.response.model", model.as_str());
                attrs.push(KeyValue::new("gen_ai.response.model", model.clone()));
            }
            if let Some(reason) = &response.finish_reason {
                // A string array in the conventions, which a `tracing` field can't hold.
                span.set_attribute(
                    "gen_ai.response.finish_reasons",
                    opentelemetry::Value::Array(vec![opentelemetry::StringValue::from(reason.clone())].into()),
                );
            }
            span.record("gen_ai.usage.input_tokens", usage.input_tokens);
            span.record("gen_ai.usage.output_tokens", usage.output_tokens);
            span.record("gen_ai.usage.cache_read.input_tokens", usage.cached_input_tokens);
            span.record(
                "gen_ai.usage.cache_creation.input_tokens",
                usage.cache_creation_input_tokens,
            );

            let token_attrs = |kind: &'static str| {
                let mut a = attrs.clone();
                a.push(KeyValue::new("gen_ai.token.type", kind));
                a
            };
            TOKEN_USAGE.record(usage.input_tokens, &token_attrs("input"));
            TOKEN_USAGE.record(usage.output_tokens, &token_attrs("output"));
        }
        Err(err) => {
            let kind = error_type(err);
            span.record("otel.status_code", "ERROR");
            span.record("error.type", kind.as_str());
            attrs.push(KeyValue::new("error.type", kind));
        }
    }
    DURATION.record(elapsed.as_secs_f64(), &attrs);
}

/// Put the request's messages and tool definitions on the span, in the conventions' JSON
/// shapes. The leading system messages are the system instructions; one later in the
/// conversation stays in it, in place. Attachments are described, never inlined.
pub fn record_input(span: &Span, req: &CompletionRequest) {
    let leading = req
        .messages
        .iter()
        .take_while(|m| matches!(m, Message::System(_)))
        .count();
    let instructions: Vec<_> = req.messages[..leading]
        .iter()
        .filter_map(|m| match m {
            Message::System(text) => Some(serde_json::json!({"type": "text", "content": text})),
            _ => None,
        })
        .collect();
    if !instructions.is_empty() {
        span.record(
            "gen_ai.system_instructions",
            serde_json::Value::from(instructions).to_string(),
        );
    }

    let messages: Vec<_> = req.messages[leading..]
        .iter()
        .map(|m| match m {
            Message::System(text) => serde_json::json!({
                "role": "system",
                "parts": [{"type": "text", "content": text}],
            }),
            Message::User(parts) => serde_json::json!({
                "role": "user",
                "parts": parts.iter().map(part).collect::<Vec<_>>(),
            }),
            Message::Assistant(parts) => serde_json::json!({
                "role": "assistant",
                "parts": parts.iter().map(assistant_part).collect::<Vec<_>>(),
            }),
            Message::ToolResult { call_id, content } => serde_json::json!({
                "role": "tool",
                "parts": [{
                    "type": "tool_call_response",
                    "id": call_id,
                    "response": content.iter().filter_map(Part::text).collect::<Vec<_>>().join("\n"),
                }],
            }),
        })
        .collect();
    span.record("gen_ai.input.messages", serde_json::Value::from(messages).to_string());

    let tools: Vec<_> = req
        .tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            })
        })
        .collect();
    span.record("gen_ai.tool.definitions", serde_json::Value::from(tools).to_string());
}

fn part(p: &Part) -> serde_json::Value {
    match p {
        Part::Text(text) => serde_json::json!({"type": "text", "content": text}),
        Part::Attachment { mime, data, name } => serde_json::json!({
            "type": "attachment",
            "mime_type": mime,
            "name": name,
            "size": data.len(),
        }),
    }
}

fn assistant_part(p: &AssistantPart) -> serde_json::Value {
    match p {
        AssistantPart::Text(text) => serde_json::json!({"type": "text", "content": text}),
        AssistantPart::ToolCall { id, name, arguments, .. } => serde_json::json!({
            "type": "tool_call",
            "id": id,
            "name": name,
            "arguments": arguments,
        }),
        AssistantPart::Reasoning { text, .. } => {
            serde_json::json!({"type": "reasoning", "content": text})
        }
    }
}

/// Low-cardinality `error.type`: the HTTP status, or the failure's kind.
fn error_type(err: &LlmError) -> String {
    match err {
        LlmError::Http { status, .. } => status.to_string(),
        LlmError::Transport(_) => "transport".into(),
        LlmError::Timeout => "timeout".into(),
        LlmError::Decode(_) => "decode".into(),
        LlmError::Config(_) => "config".into(),
        LlmError::Unsupported => "unsupported".into(),
    }
}

fn server(base_url: &str) -> (Option<String>, Option<u16>) {
    match reqwest::Url::parse(base_url) {
        Ok(url) => (url.host_str().map(str::to_owned), url.port_or_known_default()),
        Err(_) => (None, None),
    }
}

// Bucket boundaries are the ones the semantic conventions recommend.

static TOKEN_USAGE: LazyLock<Histogram<u64>> = LazyLock::new(|| {
    opentelemetry::global::meter("vizier")
        .u64_histogram("gen_ai.client.token.usage")
        .with_unit("{token}")
        .with_description("Number of input and output tokens used")
        .with_boundaries(vec![
            1.0, 4.0, 16.0, 64.0, 256.0, 1024.0, 4096.0, 16384.0, 65536.0, 262144.0, 1048576.0,
            4194304.0, 16777216.0, 67108864.0,
        ])
        .build()
});

static DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    opentelemetry::global::meter("vizier")
        .f64_histogram("gen_ai.client.operation.duration")
        .with_unit("s")
        .with_description("GenAI operation duration")
        .with_boundaries(vec![
            0.01, 0.02, 0.04, 0.08, 0.16, 0.32, 0.64, 1.28, 2.56, 5.12, 10.24, 20.48, 40.96, 81.92,
        ])
        .build()
});
