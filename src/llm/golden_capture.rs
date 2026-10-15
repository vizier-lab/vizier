//! Records golden fixtures from rig 0.39, once, before rig was removed (spec 015, T031).
//!
//! An axum capture server on `127.0.0.1:0` records the request body each rig client sends and
//! answers with a canned provider response. For every dialect × scenario this writes, under
//! `src/llm/dialect/fixtures/golden/<target>/`:
//!
//! - `<scenario>.input.json`: the scenario as `llm::types`, plus the model settings;
//! - `<scenario>.request.json`: the path and body rig sent;
//! - `<scenario>.response.json`: the canned response;
//! - `<scenario>.parsed.json`: rig's parsed result, normalised to `llm::types` JSON.
//!
//! Run with `cargo test -- --ignored golden_capture`. `golden_tests.rs` checks our dialects
//! against these files with no rig involved.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use base64::Engine;
use rig_core::OneOrMany;
use rig_core::client::CompletionClient;
use rig_core::completion::CompletionModel as RigCompletionModel;
use rig_core::message as rm;
use serde_json::{Value, json};

use super::types::{
    AssistantPart, CompletionRequest, CompletionResponse, Message, Part, ToolChoice,
    ToolDefinition, Usage,
};

#[derive(Default)]
struct Capture {
    response: (String, String),
    seen: Vec<(String, Value, Vec<(String, String)>)>,
}

type Shared = Arc<Mutex<Capture>>;

async fn handler(State(state): State<Shared>, uri: Uri, headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    let mut s = state.lock().unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let headers = headers
        .iter()
        .filter(|(k, _)| {
            matches!(
                k.as_str(),
                "anthropic-version" | "x-api-key" | "x-goog-api-key" | "authorization"
            )
        })
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
        .collect();
    s.seen.push((uri.to_string(), body, headers));
    let (content_type, text) = s.response.clone();
    (StatusCode::OK, [("content-type", content_type)], text)
}

async fn server() -> (String, Shared) {
    let state: Shared = Arc::default();
    let app = Router::new().fallback(handler).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, state)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    OpenAiChat,
    DeepSeek,
    Anthropic,
    Gemini,
    Ollama,
    OpenAiResponses,
    ChatGpt,
}

impl Target {
    fn dir(self) -> &'static str {
        match self {
            Target::OpenAiChat => "openai_chat",
            Target::DeepSeek => "deepseek",
            Target::Anthropic => "anthropic",
            Target::Gemini => "gemini",
            Target::Ollama => "ollama",
            Target::OpenAiResponses => "openai_responses",
            Target::ChatGpt => "chatgpt",
        }
    }

    fn model(self) -> &'static str {
        match self {
            Target::OpenAiChat => "gpt-4o",
            Target::DeepSeek => "deepseek-reasoner",
            Target::Anthropic => "claude-sonnet-4-5",
            Target::Gemini => "gemini-2.5-flash",
            Target::Ollama => "llama3.2",
            Target::OpenAiResponses => "gpt-5",
            Target::ChatGpt => "gpt-5.4",
        }
    }

    /// Ids name the function where the wire format answers by name.
    fn call_id(self, n: usize, name: &str) -> String {
        match self {
            Target::Gemini | Target::Ollama => name.to_string(),
            Target::Anthropic => format!("toolu_{n}"),
            _ => format!("call_{n}"),
        }
    }

    fn inline_system(self) -> bool {
        matches!(
            self,
            Target::OpenAiChat | Target::DeepSeek | Target::Ollama | Target::OpenAiResponses
        )
    }
}

fn tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "memory_search".into(),
            description: "Search your memory for passages.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "What to look for"},
                    "limit": {"type": ["integer", "null"], "description": "At most this many"}
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "execute_python".into(),
            description: "Run a Python script in the sandbox.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "intent": {"type": "string"},
                    "code": {"type": "string"}
                },
                "required": ["intent", "code"]
            }),
        },
    ]
}

fn systems() -> Vec<Message> {
    vec![
        Message::system("You are Vizier, a steward."),
        Message::system("# CORE\nBe brief."),
    ]
}

fn png() -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==")
        .unwrap()
}

fn reasoning_part(t: Target) -> AssistantPart {
    let provider_data = match t {
        Target::Anthropic => Some(json!({"signature": "sig-history"})),
        Target::Gemini => Some(json!({"thought_signature": "ts-history"})),
        Target::OpenAiResponses | Target::ChatGpt => Some(json!({
            "id": "rs_history",
            "summary": [{"type": "summary_text", "text": "Plan: search first."}],
            "encrypted_content": "enc-history"
        })),
        _ => None,
    };
    AssistantPart::Reasoning {
        text: "Plan: search first.".into(),
        provider_data,
    }
}

fn call(t: Target, n: usize, name: &str, args: Value) -> AssistantPart {
    AssistantPart::ToolCall {
        id: t.call_id(n, name),
        name: name.into(),
        arguments: args,
        provider_data: if t == Target::Gemini && n == 1 {
            Some(json!({"thought_signature": "ts-call"}))
        } else {
            None
        },
    }
}

/// A turn that called two tools and got both results back.
fn tool_round(t: Target, with_reasoning: bool) -> Vec<Message> {
    let mut m = systems();
    m.push(Message::user_text("What did I say about the garden?").unwrap());
    let mut parts = vec![];
    if with_reasoning {
        parts.push(reasoning_part(t));
    }
    parts.push(AssistantPart::Text("Let me check.".into()));
    parts.push(call(t, 1, "memory_search", json!({"query": "garden"})));
    parts.push(call(t, 2, "execute_python", json!({"intent": "count", "code": "1+1"})));
    m.push(Message::Assistant(parts));
    m.push(Message::tool_result(t.call_id(1, "memory_search"), "You planted tomatoes."));
    m.push(Message::tool_result(
        t.call_id(2, "execute_python"),
        r#"{"ok": true, "value": 2}"#,
    ));
    m
}

/// What the canned response should say.
enum Reply {
    Text(&'static str),
    Calls(usize),
    Reasoning,
    Cached,
}

struct Scenario {
    name: &'static str,
    request: CompletionRequest,
    reply: Reply,
}

fn scenarios(t: Target) -> Vec<Scenario> {
    let text = |extra: Vec<Message>| {
        let mut m = systems();
        m.extend(extra);
        m
    };
    let mut out = vec![
        Scenario {
            name: "text",
            request: CompletionRequest {
                messages: text(vec![Message::user_text("Hello").unwrap()]),
                ..Default::default()
            },
            reply: Reply::Text("Hello there."),
        },
        Scenario {
            name: "tool_call",
            request: CompletionRequest {
                messages: text(vec![Message::user_text("Search for garden").unwrap()]),
                tools: tools(),
                ..Default::default()
            },
            reply: Reply::Calls(1),
        },
        Scenario {
            name: "parallel_tool_calls",
            request: CompletionRequest {
                messages: text(vec![Message::user_text("Do both").unwrap()]),
                tools: tools(),
                ..Default::default()
            },
            reply: Reply::Calls(2),
        },
        Scenario {
            name: "tool_result",
            request: CompletionRequest {
                messages: tool_round(t, false),
                tools: tools(),
                ..Default::default()
            },
            reply: Reply::Text("You planted tomatoes."),
        },
        Scenario {
            name: "tool_choice_none",
            request: CompletionRequest {
                messages: tool_round(t, false),
                tools: tools(),
                tool_choice: Some(ToolChoice::None),
                ..Default::default()
            },
            reply: Reply::Text("Here is what I found."),
        },
        Scenario {
            name: "reasoning",
            request: CompletionRequest {
                messages: tool_round(t, true),
                tools: tools(),
                ..Default::default()
            },
            reply: Reply::Reasoning,
        },
        Scenario {
            name: "cached_usage",
            request: CompletionRequest {
                messages: text(vec![Message::user_text("Hello again").unwrap()]),
                ..Default::default()
            },
            reply: Reply::Cached,
        },
    ];
    // DeepSeek has no vision, and its rig client drops images and `max_tokens`.
    if t != Target::DeepSeek {
        out.push(Scenario {
            name: "image",
            request: CompletionRequest {
                messages: text(vec![Message::User(vec![
                    Part::Text("What is in this picture?".into()),
                    Part::Attachment {
                        mime: "image/png".into(),
                        data: png(),
                        name: Some("dot.png".into()),
                    },
                ])]),
                ..Default::default()
            },
            reply: Reply::Text("A single pixel."),
        });
        out.push(Scenario {
            name: "max_tokens",
            request: CompletionRequest {
                messages: text(vec![Message::user_text("Be short").unwrap()]),
                max_tokens: Some(512),
                ..Default::default()
            },
            reply: Reply::Text("Short."),
        });
    }
    // rig moves a later system message to the top for Anthropic and Gemini, which we don't
    // (contract S8), so those request bodies differ by design and are unit-tested instead.
    if t.inline_system() {
        out.push(Scenario {
            name: "mid_system",
            request: CompletionRequest {
                messages: text(vec![
                    Message::user_text("Hi").unwrap(),
                    Message::assistant_text("Hello."),
                    Message::system("Checkpoint: the user greeted you."),
                    Message::user_text("Continue").unwrap(),
                ]),
                ..Default::default()
            },
            reply: Reply::Text("Continuing."),
        });
    }
    out
}

fn calls_for(t: Target, n: usize) -> Vec<(String, &'static str, Value)> {
    let all = [
        ("memory_search", json!({"query": "garden"})),
        ("execute_python", json!({"intent": "count", "code": "1+1"})),
    ];
    all.into_iter()
        .take(n)
        .enumerate()
        .map(|(i, (name, args))| (t.call_id(i + 1, name), name, args))
        .collect()
}

/// The canned provider response: (content type, body).
fn canned(t: Target, reply: &Reply) -> (String, String) {
    let json_body = |v: Value| ("application/json".to_string(), v.to_string());
    match t {
        Target::OpenAiChat | Target::DeepSeek => {
            let (message, finish) = match reply {
                Reply::Text(s) => (json!({"role": "assistant", "content": s}), "stop"),
                Reply::Cached => (json!({"role": "assistant", "content": "Cached hello."}), "stop"),
                Reply::Calls(n) => (
                    json!({"role": "assistant", "content": if t == Target::DeepSeek { json!("") } else { Value::Null }, "tool_calls": calls_for(t, *n).iter().enumerate().map(|(i, (id, name, args))| json!({
                        "index": i, "id": id, "type": "function", "function": {"name": name, "arguments": args.to_string()}
                    })).collect::<Vec<_>>()}),
                    "tool_calls",
                ),
                Reply::Reasoning => (
                    json!({"role": "assistant", "content": "Tomatoes.", "reasoning_content": "The memory says tomatoes."}),
                    "stop",
                ),
            };
            let mut usage = json!({"prompt_tokens": 120, "completion_tokens": 9, "total_tokens": 129});
            let cached = if matches!(reply, Reply::Cached) { 96 } else { 0 };
            if cached > 0 {
                usage["prompt_tokens_details"] = json!({"cached_tokens": cached});
            }
            if t == Target::DeepSeek {
                usage["prompt_cache_hit_tokens"] = json!(cached);
                usage["prompt_cache_miss_tokens"] = json!(120 - cached);
            }
            json_body(json!({
                "id": "chatcmpl-1", "object": "chat.completion", "created": 1760000000, "model": t.model(),
                "choices": [{"index": 0, "message": message, "logprobs": null, "finish_reason": finish}],
                "usage": usage
            }))
        }
        Target::Anthropic => {
            let (content, stop) = match reply {
                Reply::Text(s) => (json!([{"type": "text", "text": s}]), "end_turn"),
                Reply::Cached => (json!([{"type": "text", "text": "Cached hello."}]), "end_turn"),
                Reply::Calls(n) => {
                    let mut c = vec![json!({"type": "text", "text": "On it."})];
                    c.extend(calls_for(t, *n).into_iter().map(|(id, name, args)| {
                        json!({"type": "tool_use", "id": id, "name": name, "input": args})
                    }));
                    (Value::Array(c), "tool_use")
                }
                Reply::Reasoning => (
                    json!([
                        {"type": "thinking", "thinking": "The memory says tomatoes.", "signature": "sig-reply"},
                        {"type": "redacted_thinking", "data": "redacted-blob"},
                        {"type": "text", "text": "Tomatoes."}
                    ]),
                    "end_turn",
                ),
            };
            let mut usage = json!({"input_tokens": 120, "output_tokens": 9});
            if matches!(reply, Reply::Cached) {
                usage["cache_read_input_tokens"] = json!(1800);
                usage["cache_creation_input_tokens"] = json!(40);
            }
            json_body(json!({
                "id": "msg_1", "type": "message", "role": "assistant", "model": t.model(),
                "content": content, "stop_reason": stop, "stop_sequence": null, "usage": usage
            }))
        }
        Target::Gemini => {
            let parts = match reply {
                Reply::Text(s) => json!([{"text": s}]),
                Reply::Cached => json!([{"text": "Cached hello."}]),
                Reply::Calls(n) => Value::Array(
                    calls_for(t, *n)
                        .into_iter()
                        .enumerate()
                        .map(|(i, (_, name, args))| {
                            let mut p = json!({"functionCall": {"name": name, "args": args}});
                            if i == 0 {
                                p["thoughtSignature"] = json!("ts-reply");
                            }
                            p
                        })
                        .collect(),
                ),
                Reply::Reasoning => json!([
                    {"text": "The memory says tomatoes.", "thought": true, "thoughtSignature": "ts-thought"},
                    {"text": "Tomatoes."}
                ]),
            };
            let mut usage = json!({"promptTokenCount": 120, "candidatesTokenCount": 9, "totalTokenCount": 129});
            if matches!(reply, Reply::Cached) {
                usage["cachedContentTokenCount"] = json!(96);
            }
            json_body(json!({
                "responseId": "resp-1",
                "candidates": [{"content": {"role": "model", "parts": parts}, "finishReason": "STOP", "index": 0}],
                "usageMetadata": usage, "modelVersion": t.model()
            }))
        }
        Target::Ollama => {
            let message = match reply {
                Reply::Text(s) => json!({"role": "assistant", "content": s}),
                Reply::Cached => json!({"role": "assistant", "content": "Cached hello."}),
                Reply::Calls(n) => json!({"role": "assistant", "content": "", "tool_calls": calls_for(t, *n).into_iter().map(|(_, name, args)| json!({
                    "function": {"name": name, "arguments": args}
                })).collect::<Vec<_>>()}),
                Reply::Reasoning => json!({"role": "assistant", "content": "Tomatoes."}),
            };
            json_body(json!({
                "model": t.model(), "created_at": "2026-10-15T00:00:00Z", "message": message,
                "done": true, "done_reason": "stop", "prompt_eval_count": 120, "eval_count": 9
            }))
        }
        Target::OpenAiResponses | Target::ChatGpt => {
            let msg = |text: &str| {
                json!({"type": "message", "id": "msg_1", "status": "completed", "role": "assistant",
                       "content": [{"type": "output_text", "annotations": [], "text": text}]})
            };
            let output = match reply {
                Reply::Text(s) => json!([msg(s)]),
                Reply::Cached => json!([msg("Cached hello.")]),
                Reply::Calls(n) => Value::Array(
                    calls_for(t, *n)
                        .into_iter()
                        .enumerate()
                        .map(|(i, (id, name, args))| {
                            json!({"type": "function_call", "id": format!("fc_{}", i + 1), "call_id": id,
                                   "name": name, "arguments": args.to_string(), "status": "completed"})
                        })
                        .collect(),
                ),
                Reply::Reasoning => json!([
                    {"type": "reasoning", "id": "rs_reply", "summary": [{"type": "summary_text", "text": "The memory says tomatoes."}],
                     "encrypted_content": "enc-reply"},
                    msg("Tomatoes.")
                ]),
            };
            let mut usage = json!({"input_tokens": 120, "input_tokens_details": {"cached_tokens": 0},
                                   "output_tokens": 9, "output_tokens_details": {"reasoning_tokens": 0}, "total_tokens": 129});
            if matches!(reply, Reply::Cached) {
                usage["input_tokens_details"]["cached_tokens"] = json!(96);
            }
            let response = json!({
                "id": "resp_1", "object": "response", "created_at": 1760000000, "status": "completed",
                "error": null, "incomplete_details": null, "instructions": null, "max_output_tokens": null,
                "model": t.model(), "usage": usage, "output": output, "tools": []
            });
            if t == Target::ChatGpt {
                let sse = format!(
                    "event: response.created\ndata: {}\n\nevent: response.completed\ndata: {}\n\ndata: [DONE]\n\n",
                    json!({"type": "response.created", "response": {"id": "resp_1", "status": "in_progress"}}),
                    json!({"type": "response.completed", "response": response})
                );
                ("text/event-stream".to_string(), sse)
            } else {
                json_body(response)
            }
        }
    }
}

// llm::types → rig ---------------------------------------------------------------------------

fn rig_reasoning(t: Target, text: &str, pd: &Option<Value>) -> rm::Reasoning {
    let get = |k: &str| {
        pd.as_ref()
            .and_then(|p| p.get(k))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    match t {
        Target::Anthropic => match get("redacted") {
            Some(data) => rm::Reasoning::redacted(data),
            None => rm::Reasoning::new_with_signature(text, get("signature")),
        },
        Target::Gemini => rm::Reasoning::new_with_signature(text, get("thought_signature")),
        Target::OpenAiResponses | Target::ChatGpt => {
            let mut r = rm::Reasoning::summaries(vec![text.to_string()]);
            if let Some(id) = get("id") {
                r = r.with_id(id);
            }
            if let Some(enc) = get("encrypted_content") {
                r.content.push(rm::ReasoningContent::Encrypted(enc));
            }
            r
        }
        _ => rm::Reasoning::new(text),
    }
}

fn rig_user_part(part: &Part) -> rm::UserContent {
    match part {
        Part::Text(t) => rm::UserContent::text(t.clone()),
        Part::Attachment { data, .. } => rm::UserContent::image_base64(
            base64::engine::general_purpose::STANDARD.encode(data),
            Some(rm::ImageMediaType::PNG),
            None,
        ),
    }
}

fn to_rig(t: Target, messages: &[Message]) -> Vec<rm::Message> {
    let mut out = vec![];
    let mut results: Vec<rm::UserContent> = vec![];
    let flush = |out: &mut Vec<rm::Message>, results: &mut Vec<rm::UserContent>| {
        if !results.is_empty() {
            out.push(rm::Message::User {
                content: OneOrMany::many(std::mem::take(results)).unwrap(),
            });
        }
    };
    for m in messages {
        match m {
            Message::ToolResult { call_id, content } => {
                results.push(rm::UserContent::ToolResult(rm::ToolResult {
                    id: call_id.clone(),
                    call_id: Some(call_id.clone()),
                    content: OneOrMany::one(rm::ToolResultContent::text(
                        content.iter().filter_map(Part::text).collect::<Vec<_>>().join("\n"),
                    )),
                }))
            }
            Message::System(text) => {
                flush(&mut out, &mut results);
                out.push(rm::Message::system(text.clone()));
            }
            Message::User(parts) => {
                flush(&mut out, &mut results);
                out.push(rm::Message::User {
                    content: OneOrMany::many(parts.iter().map(rig_user_part).collect::<Vec<_>>())
                        .unwrap(),
                });
            }
            Message::Assistant(parts) => {
                flush(&mut out, &mut results);
                let content = parts
                    .iter()
                    .map(|p| match p {
                        AssistantPart::Text(s) => rm::AssistantContent::text(s.clone()),
                        AssistantPart::ToolCall {
                            id,
                            name,
                            arguments,
                            provider_data,
                        } => rm::AssistantContent::ToolCall(rm::ToolCall {
                            id: id.clone(),
                            call_id: Some(id.clone()),
                            function: rm::ToolFunction {
                                name: name.clone(),
                                arguments: arguments.clone(),
                            },
                            signature: provider_data
                                .as_ref()
                                .and_then(|p| p.get("thought_signature"))
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            additional_params: None,
                        }),
                        AssistantPart::Reasoning {
                            text,
                            provider_data,
                        } => rm::AssistantContent::Reasoning(rig_reasoning(t, text, provider_data)),
                    })
                    .collect::<Vec<_>>();
                out.push(rm::Message::Assistant {
                    id: None,
                    content: OneOrMany::many(content).unwrap(),
                });
            }
        }
    }
    flush(&mut out, &mut results);
    out
}

// rig → llm::types ---------------------------------------------------------------------------

fn from_rig_reasoning(t: Target, r: &rm::Reasoning) -> AssistantPart {
    match t {
        Target::Anthropic => {
            if let Some(rm::ReasoningContent::Redacted { data }) = r.content.first() {
                return AssistantPart::Reasoning {
                    text: String::new(),
                    provider_data: Some(json!({"redacted": data})),
                };
            }
            AssistantPart::Reasoning {
                text: r.display_text(),
                provider_data: r.first_signature().map(|s| json!({"signature": s})),
            }
        }
        Target::Gemini => AssistantPart::Reasoning {
            text: r.display_text(),
            provider_data: r.first_signature().map(|s| json!({"thought_signature": s})),
        },
        Target::OpenAiResponses | Target::ChatGpt => {
            let summaries: Vec<String> = r
                .content
                .iter()
                .filter_map(|c| match c {
                    rm::ReasoningContent::Summary(s) => Some(s.clone()),
                    rm::ReasoningContent::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect();
            let mut data = serde_json::Map::new();
            if let Some(id) = &r.id {
                data.insert("id".into(), json!(id));
            }
            if let Some(enc) = r.encrypted_content() {
                data.insert("encrypted_content".into(), json!(enc));
            }
            data.insert(
                "summary".into(),
                Value::Array(
                    summaries
                        .iter()
                        .map(|s| json!({"type": "summary_text", "text": s}))
                        .collect(),
                ),
            );
            AssistantPart::Reasoning {
                text: summaries.join("\n"),
                provider_data: Some(Value::Object(data)),
            }
        }
        _ => AssistantPart::Reasoning {
            text: r.display_text(),
            provider_data: None,
        },
    }
}

fn from_rig(
    t: Target,
    choice: &OneOrMany<rm::AssistantContent>,
    usage: &rig_core::completion::Usage,
    id: Option<String>,
) -> CompletionResponse {
    let content = choice
        .iter()
        .filter_map(|c| match c {
            rm::AssistantContent::Text(text) => Some(AssistantPart::Text(text.text.clone())),
            rm::AssistantContent::ToolCall(tc) => Some(AssistantPart::ToolCall {
                id: match t {
                    Target::OpenAiResponses | Target::ChatGpt => {
                        tc.call_id.clone().unwrap_or_else(|| tc.id.clone())
                    }
                    _ => tc.id.clone(),
                },
                name: tc.function.name.clone(),
                arguments: tc.function.arguments.clone(),
                provider_data: if t == Target::Gemini {
                    tc.signature.as_ref().map(|s| json!({"thought_signature": s}))
                } else {
                    None
                },
            }),
            rm::AssistantContent::Reasoning(r) => Some(from_rig_reasoning(t, r)),
            _ => None,
        })
        .collect();
    CompletionResponse {
        id,
        content,
        usage: Usage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens,
            cached_input_tokens: usage.cached_input_tokens,
            cache_creation_input_tokens: usage.cache_creation_input_tokens,
        },
    }
}

async fn run_rig<M: RigCompletionModel>(
    t: Target,
    model: &M,
    req: &CompletionRequest,
) -> Result<CompletionResponse, String> {
    let mut messages = to_rig(t, &req.messages);
    let prompt = messages.pop().unwrap();
    let tools = req
        .tools
        .iter()
        .map(|d| rig_core::completion::ToolDefinition {
            name: d.name.clone(),
            description: d.description.clone(),
            parameters: d.parameters.clone(),
        })
        .collect();
    let mut builder = model.completion_request(prompt).messages(messages).tools(tools);
    if let Some(max) = req.max_tokens {
        builder = builder.max_tokens(max);
    }
    if req.tool_choice == Some(ToolChoice::None) {
        builder = builder.tool_choice(rm::ToolChoice::None);
    }
    let response = model
        .completion(builder.build())
        .await
        .map_err(|e| e.to_string())?;
    Ok(from_rig(t, &response.choice, &response.usage, response.message_id))
}

fn fixture_dir(t: Target) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/llm/dialect/fixtures/golden")
        .join(t.dir())
}

/// The model settings `golden_tests` rebuilds the `ResolvedModel` from.
fn model_settings(t: Target, base: &str) -> Value {
    json!({
        "model": t.model(),
        "base_url": base,
        "max_output": if t == Target::Anthropic { json!(64000) } else { Value::Null },
        "reasoning": false,
        "stream_required": t == Target::ChatGpt,
        "default_instructions": if t == Target::ChatGpt { json!("You are ChatGPT, a helpful AI assistant.") } else { Value::Null },
    })
}

#[tokio::test]
#[ignore = "records fixtures from rig 0.39; run once with --ignored before removing rig"]
async fn golden_capture() {
    // SAFETY: test-only, single-threaded at this point; rig's ChatGPT client reads these.
    unsafe {
        std::env::remove_var("CHATGPT_DEFAULT_INSTRUCTIONS");
        std::env::remove_var("CHATGPT_ORIGINATOR");
    }
    let (url, state) = server().await;
    let targets = [
        Target::OpenAiChat,
        Target::DeepSeek,
        Target::Anthropic,
        Target::Gemini,
        Target::Ollama,
        Target::OpenAiResponses,
        Target::ChatGpt,
    ];

    for t in targets {
        let dir = fixture_dir(t);
        std::fs::create_dir_all(&dir).unwrap();
        // The base URL our side uses for the same endpoint rig called.
        let our_base = match t {
            Target::OpenAiChat | Target::OpenAiResponses => format!("{url}/v1"),
            Target::Gemini => format!("{url}/v1beta"),
            _ => url.clone(),
        };

        for s in scenarios(t) {
            let canned = canned(t, &s.reply);
            {
                let mut st = state.lock().unwrap();
                st.response = canned.clone();
                st.seen.clear();
            }

            let parsed = match t {
                Target::OpenAiChat => {
                    let client = rig_core::providers::openai::CompletionsClient::builder()
                        .api_key("test-key")
                        .base_url(format!("{url}/v1"))
                        .build()
                        .unwrap();
                    run_rig(t, &client.completion_model(t.model()), &s.request).await
                }
                Target::DeepSeek => {
                    let client = rig_core::providers::deepseek::Client::builder()
                        .api_key("test-key")
                        .base_url(&url)
                        .build()
                        .unwrap();
                    run_rig(t, &client.completion_model(t.model()), &s.request).await
                }
                Target::Anthropic => {
                    let client = rig_core::providers::anthropic::Client::builder()
                        .api_key("test-key")
                        .base_url(&url)
                        .build()
                        .unwrap();
                    let model = client.completion_model(t.model()).with_automatic_caching();
                    run_rig(t, &model, &s.request).await
                }
                Target::Gemini => {
                    let client = rig_core::providers::gemini::Client::builder()
                        .api_key("test-key")
                        .base_url(&url)
                        .build()
                        .unwrap();
                    run_rig(t, &client.completion_model(t.model()), &s.request).await
                }
                Target::Ollama => {
                    let client = rig_core::providers::ollama::Client::builder()
                        .api_key(rig_core::client::Nothing)
                        .base_url(&url)
                        .build()
                        .unwrap();
                    run_rig(t, &client.completion_model(t.model()), &s.request).await
                }
                Target::OpenAiResponses => {
                    let client = rig_core::providers::openai::Client::builder()
                        .api_key("test-key")
                        .base_url(format!("{url}/v1"))
                        .build()
                        .unwrap();
                    run_rig(t, &client.completion_model(t.model()), &s.request).await
                }
                Target::ChatGpt => {
                    let client = rig_core::providers::chatgpt::Client::builder()
                        .api_key(rig_core::providers::chatgpt::ChatGPTAuth::AccessToken {
                            access_token: "test-token".into(),
                            account_id: Some("acct-1".into()),
                        })
                        .base_url(&url)
                        .build()
                        .unwrap();
                    run_rig(t, &client.completion_model(t.model()), &s.request).await
                }
            };

            let (path, body, _headers) = state
                .lock()
                .unwrap()
                .seen
                .last()
                .cloned()
                .unwrap_or_else(|| panic!("{t:?}/{}: rig sent nothing", s.name));
            let path = path.split('?').next().unwrap().to_string();
            let parsed = match parsed {
                Ok(p) => serde_json::to_value(p).unwrap(),
                Err(e) => json!({"error": e}),
            };
            let write = |suffix: &str, v: &Value| {
                std::fs::write(
                    dir.join(format!("{}.{suffix}.json", s.name)),
                    serde_json::to_string_pretty(v).unwrap() + "\n",
                )
                .unwrap();
            };
            write(
                "input",
                &json!({
                    "settings": model_settings(t, "http://capture"),
                    "request": s.request,
                }),
            );
            let our_path = path
                .strip_prefix(our_base.strip_prefix(&url).unwrap_or(""))
                .unwrap_or(&path)
                .to_string();
            write("request", &json!({"path": our_path, "body": body}));
            let response = if canned.0 == "text/event-stream" {
                json!({"sse": canned.1})
            } else {
                serde_json::from_str::<Value>(&canned.1).unwrap()
            };
            write("response", &response);
            write("parsed", &parsed);
            println!("{}/{}: captured {path}", t.dir(), s.name);
        }
    }
}
