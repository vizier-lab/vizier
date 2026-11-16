//! OpenAI Chat Completions (`/chat/completions`), which nearly every OpenAI-compatible
//! provider speaks. Field choices ported from rig-core 0.39 `providers/openai/completion`.

use base64::Engine;
use serde_json::{Value, json};

use super::{Dialect, PreparedRequest, SystemStyle, decode_openai_models, error_in_body};
use crate::llm::error::LlmError;
use crate::llm::model::ListedModel;
use crate::llm::resolve::ResolvedModel;
use crate::llm::types::{
    AssistantPart, CompletionResponse, Message, Part, ToolChoice, ToolDefinition, Usage,
};

pub struct OpenAiChat;

impl Dialect for OpenAiChat {
    fn system_style(&self) -> SystemStyle {
        SystemStyle::Inline
    }

    fn path(&self, _m: &ResolvedModel) -> String {
        "/chat/completions".into()
    }

    fn encode(&self, req: &PreparedRequest, m: &ResolvedModel) -> Value {
        let messages: Vec<Value> = req.messages.iter().filter_map(encode_message).collect();
        let mut body = json!({
            "model": m.model,
            "messages": messages,
        });
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(req.tools.iter().map(encode_tool).collect());
        }
        if let Some(choice) = req.tool_choice {
            body["tool_choice"] = json!(match choice {
                ToolChoice::Auto => "auto",
                ToolChoice::None => "none",
            });
        }
        if let Some(max) = req.max_tokens.or(m.max_tokens) {
            body["max_tokens"] = json!(max);
        }
        body
    }

    fn decode(&self, body: Value) -> Result<CompletionResponse, LlmError> {
        if let Some(err) = error_in_body(&body) {
            return Err(LlmError::Decode(err));
        }
        let message = body
            .pointer("/choices/0/message")
            .ok_or_else(|| LlmError::Decode("response contained no choices".into()))?;

        let mut content: Vec<AssistantPart> = text_parts(message.get("content"))
            .into_iter()
            .map(AssistantPart::Text)
            .collect();
        if let Some(reasoning) = message
            .get("reasoning_content")
            .and_then(Value::as_str)
            .filter(|r| !r.is_empty())
        {
            content.push(AssistantPart::Reasoning {
                text: reasoning.to_string(),
                provider_data: None,
            });
        }
        if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                content.push(AssistantPart::ToolCall {
                    id: call
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    name: call
                        .pointer("/function/name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    arguments: arguments_in_flight(call.pointer("/function/arguments")),
                    provider_data: None,
                });
            }
        }
        if content.is_empty() {
            return Err(LlmError::Decode(
                "response contained no message or tool call (empty)".into(),
            ));
        }

        let finish_reason = super::finish_reason(
            body.pointer("/choices/0/finish_reason").and_then(Value::as_str),
            &content,
        );
        Ok(CompletionResponse {
            id: super::str_field(&body, "id"),
            content,
            usage: decode_usage(body.get("usage")),
            model: super::str_field(&body, "model"),
            finish_reason,
        })
    }

    fn list_models_path(&self, _m: &ResolvedModel) -> Option<String> {
        Some("/models".into())
    }

    fn decode_models(&self, body: Value) -> Vec<ListedModel> {
        decode_openai_models(body)
    }
}

pub(crate) fn encode_tool(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        }
    })
}

fn data_url(mime: &str, data: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(data)
    )
}

fn encode_user_part(part: &Part) -> Value {
    match part {
        Part::Text(text) => json!({"type": "text", "text": text}),
        Part::Attachment { mime, data, name } if mime == "application/pdf" => json!({
            "type": "file",
            "file": {
                "file_data": data_url(mime, data),
                "filename": name.clone().unwrap_or_else(|| "document.pdf".into()),
            }
        }),
        Part::Attachment { mime, data, .. } => json!({
            "type": "image_url",
            "image_url": {"url": data_url(mime, data), "detail": "auto"},
        }),
    }
}

fn joined_text(parts: &[Part]) -> String {
    parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.clone(),
            Part::Attachment { name, .. } => format!(
                "[attachment {} omitted: tool results must be text]",
                name.as_deref().unwrap_or("file")
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn encode_message(message: &Message) -> Option<Value> {
    Some(match message {
        // Plain strings for system and assistant text: every OpenAI-compatible server accepts
        // them, and several (DeepSeek among them) accept nothing else.
        Message::System(text) => json!({"role": "system", "content": text}),
        Message::User(parts) => {
            let content = match parts.as_slice() {
                [Part::Text(text)] => json!(text),
                parts => Value::Array(parts.iter().map(encode_user_part).collect()),
            };
            json!({"role": "user", "content": content})
        }
        Message::Assistant(parts) => {
            let mut texts = vec![];
            let mut calls = vec![];
            let mut reasoning = String::new();
            for part in parts {
                match part {
                    AssistantPart::Text(text) => texts.push(text.as_str()),
                    AssistantPart::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    } => calls.push(json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": arguments_as_object(arguments).to_string(),
                        },
                    })),
                    AssistantPart::Reasoning { text, .. } => reasoning.push_str(text),
                }
            }
            // Reasoning alone is no message at all (rig 0.39 drops it too).
            if texts.is_empty() && calls.is_empty() {
                return None;
            }
            let mut msg = json!({"role": "assistant"});
            if !texts.is_empty() {
                msg["content"] = json!(texts.concat());
            }
            // DeepSeek and other OpenAI-compatible reasoners need their hidden reasoning echoed
            // back on assistant tool-call turns.
            if !reasoning.is_empty() {
                msg["reasoning_content"] = json!(reasoning);
            }
            if !calls.is_empty() {
                msg["tool_calls"] = Value::Array(calls);
            }
            msg
        }
        Message::ToolResult { call_id, content, .. } => json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": joined_text(content),
        }),
    })
}

/// Non-empty text from a `content` that is a string or an array of `text`/`refusal` parts.
fn text_parts(content: Option<&Value>) -> Vec<String> {
    match content {
        Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .or_else(|| item.get("refusal"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            })
            .collect(),
        _ => vec![],
    }
}

/// Arguments as the wire wants them: always an object.
///
/// OpenAI's API rejects a call whose `function.arguments` is not a JSON object (400
/// "Assistant tool call function.arguments must be a JSON object"), and keeps rejecting it
/// on every later request that replays the call. A call can still reach the encoder holding
/// arguments that are not an object — see [`arguments_in_flight`] — so it is normalised here
/// rather than trusted. Empty arguments (`None`, `null`) are how a call with no arguments
/// travels, and `{}` is what a provider expects for those; any other non-object is wrapped so
/// no text the model produced is lost.
pub(crate) fn arguments_as_object(arguments: &Value) -> Value {
    match arguments {
        Value::Object(_) => arguments.clone(),
        Value::Null => json!({}),
        other => json!({ "arguments": other }),
    }
}

/// Tool-call arguments as kept in memory and in history, from a raw `arguments` value.
///
/// The normal case is a JSON string, or an object from a server that sends it that way. A
/// string that is not valid JSON — a reply cut off mid-arguments (a stream that ends early, a
/// disconnect) — is kept as the raw string rather than decoded into a value: turning it into
/// an object here would invent arguments the model never sent, and hide the truncation.
pub(crate) fn arguments_in_flight(raw: Option<&Value>) -> Value {
    match raw {
        Some(Value::String(s)) if s.trim().is_empty() => json!({}),
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or_else(|_| json!(s)),
        Some(other) => other.clone(),
        None => json!({}),
    }
}

fn decode_usage(usage: Option<&Value>) -> Usage {
    let Some(u) = usage else {
        return Usage::new();
    };
    let get = |k: &str| u.get(k).and_then(Value::as_u64);
    let prompt = get("prompt_tokens").unwrap_or(0);
    let total = get("total_tokens").unwrap_or(0);
    Usage {
        input_tokens: prompt,
        output_tokens: get("completion_tokens").unwrap_or(total.saturating_sub(prompt)),
        total_tokens: total,
        // DeepSeek reports cache hits as `prompt_cache_hit_tokens`.
        cached_input_tokens: u
            .pointer("/prompt_tokens_details/cached_tokens")
            .and_then(Value::as_u64)
            .or_else(|| get("prompt_cache_hit_tokens"))
            .unwrap_or(0),
        cache_creation_input_tokens: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::prepare::prepare;
    use crate::llm::types::CompletionRequest;

    fn encode(req: CompletionRequest) -> Value {
        let m = ResolvedModel::test("gpt-x");
        OpenAiChat.encode(&prepare(req, &m, SystemStyle::Inline), &m)
    }

    fn call(id: &str, name: &str) -> AssistantPart {
        AssistantPart::ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: json!({"q": 1}),
            provider_data: None,
        }
    }

    #[test]
    fn encodes_text_tools_and_choice() {
        let body = encode(CompletionRequest {
            messages: vec![Message::system("sys"), Message::user_text("hi").unwrap()],
            tools: vec![ToolDefinition {
                name: "t".into(),
                description: "d".into(),
                parameters: json!({"type": "object"}),
            }],
            tool_choice: Some(ToolChoice::None),
            max_tokens: Some(99),
        });
        assert_eq!(
            body,
            json!({
                "model": "gpt-x",
                "messages": [
                    {"role": "system", "content": "sys"},
                    {"role": "user", "content": "hi"},
                ],
                "tools": [{"type": "function", "function": {"name": "t", "description": "d", "parameters": {"type": "object"}}}],
                "tool_choice": "none",
                "max_tokens": 99,
            })
        );
    }

    #[test]
    fn encodes_tool_calls_results_reasoning_and_images() {
        let body = encode(CompletionRequest {
            messages: vec![
                Message::User(vec![
                    Part::Text("see".into()),
                    Part::Attachment {
                        mime: "image/png".into(),
                        data: vec![1, 2, 3],
                        name: None,
                    },
                ]),
                Message::Assistant(vec![
                    AssistantPart::Reasoning {
                        text: "think".into(),
                        provider_data: None,
                    },
                    AssistantPart::Text("calling".into()),
                    call("c1", "a"),
                    call("c2", "b"),
                ]),
                Message::tool_result("c1", "r1"),
                Message::tool_result("c2", "r2"),
            ],
            ..Default::default()
        });
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(
            msgs[0]["content"][1],
            json!({"type": "image_url", "image_url": {"url": "data:image/png;base64,AQID", "detail": "auto"}})
        );
        assert_eq!(msgs[1]["reasoning_content"], "think");
        assert_eq!(msgs[1]["content"], "calling");
        assert_eq!(
            msgs[1]["tool_calls"][1],
            json!({"id": "c2", "type": "function", "function": {"name": "b", "arguments": "{\"q\":1}"}})
        );
        assert_eq!(msgs[2], json!({"role": "tool", "tool_call_id": "c1", "content": "r1"}));
        assert_eq!(msgs[3]["tool_call_id"], "c2");
        assert!(body.get("tools").is_none());
    }

    fn replay_call(arguments: serde_json::Value) -> serde_json::Value {
        let body = encode(CompletionRequest {
            messages: vec![
                Message::Assistant(vec![AssistantPart::ToolCall {
                    id: "c1".into(),
                    name: "think".into(),
                    arguments,
                    provider_data: None,
                }]),
                Message::tool_result("c1", "ok"),
            ],
            ..Default::default()
        });
        body["messages"][0]["tool_calls"][0]["function"]["arguments"].clone()
    }

    /// A reply cut off mid-arguments is kept raw on the way in, then replayed on every later
    /// request of the conversation. Sent as `"{\"intent\": \"...` it is a 400 from OpenAI's API
    /// on every message: "Assistant tool call function.arguments must be a JSON object".
    #[test]
    fn truncated_arguments_are_replayed_as_an_object() {
        let cut_off = "{\"intent\": \"plan the fix\", \"thought\": \"**FOUND A REAL DE";
        let replayed = replay_call(arguments_in_flight(Some(&json!(cut_off))));
        let sent: serde_json::Value = serde_json::from_str(replayed.as_str().unwrap()).unwrap();
        assert!(sent.is_object(), "the API rejects this: {sent}");
        assert_eq!(sent["arguments"], cut_off, "nothing the model said is lost");
    }

    /// A call with no arguments is `{}` on the wire, not `null` and not absent.
    #[test]
    fn absent_or_null_arguments_are_an_empty_object() {
        for arguments in [json!({}), serde_json::Value::Null] {
            assert_eq!(replay_call(arguments), "{}");
        }
    }

    #[test]
    fn valid_arguments_replay_unchanged() {
        let raw = json!("{\"a\":1}");
        assert_eq!(arguments_in_flight(Some(&raw)), json!({"a": 1}));
        assert_eq!(replay_call(arguments_in_flight(Some(&raw))), "{\"a\":1}");
    }

    #[test]
    fn reasoning_alone_is_dropped() {
        let body = encode(CompletionRequest {
            messages: vec![Message::Assistant(vec![AssistantPart::Reasoning {
                text: "x".into(),
                provider_data: None,
            }])],
            ..Default::default()
        });
        assert_eq!(body["messages"], json!([]));
    }

    #[test]
    fn decodes_text_reasoning_tool_calls_and_cached_usage() {
        let r = OpenAiChat
            .decode(json!({
                "id": "x", "choices": [{"index": 0, "finish_reason": "tool_calls", "message": {
                    "role": "assistant", "content": "hello", "reasoning_content": "hmm",
                    "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "f", "arguments": "{\"a\":2}"}}]
                }}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 3, "total_tokens": 13,
                          "prompt_tokens_details": {"cached_tokens": 8}}
            }))
            .unwrap();
        assert_eq!(
            r.content,
            vec![
                AssistantPart::Text("hello".into()),
                AssistantPart::Reasoning {
                    text: "hmm".into(),
                    provider_data: None
                },
                AssistantPart::ToolCall {
                    id: "c1".into(),
                    name: "f".into(),
                    arguments: json!({"a": 2}),
                    provider_data: None
                },
            ]
        );
        assert_eq!(
            r.usage,
            Usage {
                input_tokens: 10,
                output_tokens: 3,
                total_tokens: 13,
                cached_input_tokens: 8,
                cache_creation_input_tokens: 0
            }
        );
    }

    #[test]
    fn an_error_in_a_2xx_body_is_a_decode_error() {
        let err = OpenAiChat
            .decode(json!({"error": {"message": "overloaded"}}))
            .unwrap_err();
        assert_eq!(err, LlmError::Decode("overloaded".into()));
        assert!(OpenAiChat.decode(json!({"choices": []})).is_err());
    }
}
