//! Anthropic Messages (`/v1/messages`). Ported from rig-core 0.39
//! `providers/anthropic/completion.rs`, plus prompt caching on every request.

use base64::Engine;
use serde_json::{Value, json};

use super::{Dialect, PreparedRequest, SystemStyle, decode_openai_models};
use crate::llm::error::LlmError;
use crate::llm::model::ListedModel;
use crate::llm::resolve::ResolvedModel;
use crate::llm::types::{AssistantPart, CompletionResponse, Message, Part, ToolChoice, Usage};

pub const ANTHROPIC_VERSION: &str = "2023-06-01";
/// rig 0.39's fallback when a model has no known output limit.
const DEFAULT_MAX_TOKENS: u64 = 2048;

pub struct Anthropic;

/// Catalogue base URLs follow the AI SDK convention and already end in `/v1`; the bare host
/// (`https://api.anthropic.com`) is accepted too.
fn versioned(m: &ResolvedModel, path: &str) -> String {
    if m.base_url.trim_end_matches('/').ends_with("/v1") {
        path.to_string()
    } else {
        format!("/v1{path}")
    }
}

impl Dialect for Anthropic {
    fn system_style(&self) -> SystemStyle {
        SystemStyle::Field
    }

    fn path(&self, m: &ResolvedModel) -> String {
        versioned(m, "/messages")
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        vec![("anthropic-version", ANTHROPIC_VERSION.to_string())]
    }

    fn encode(&self, req: &PreparedRequest, m: &ResolvedModel) -> Value {
        let mut messages = encode_messages(&req.messages);
        let mut tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect();

        // Prompt caching: breakpoints on the tools layer, the system prompt and the newest
        // message block, so tools, system and every earlier turn are read from cache on the
        // next request (4 breakpoints are allowed; this uses at most 3).
        if let Some(last) = tools.last_mut() {
            last["cache_control"] = ephemeral();
        }
        if let Some(block) = messages
            .last_mut()
            .and_then(|m| m["content"].as_array_mut())
            .and_then(|c| c.last_mut())
            && matches!(
                block["type"].as_str(),
                Some("text" | "image" | "tool_result" | "document")
            )
        {
            block["cache_control"] = ephemeral();
        }

        let mut body = json!({
            "model": m.model,
            "messages": messages,
            "max_tokens": req
                .max_tokens
                .or(m.max_tokens)
                .or(m.max_output)
                .unwrap_or(DEFAULT_MAX_TOKENS),
        });
        if let Some(system) = req.system.as_ref().filter(|s| !s.is_empty()) {
            body["system"] = json!([{"type": "text", "text": system, "cache_control": ephemeral()}]);
        }
        if let Some(choice) = req.tool_choice {
            body["tool_choice"] = json!({"type": match choice {
                ToolChoice::Auto => "auto",
                ToolChoice::None => "none",
            }});
        }
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools);
        }
        body
    }

    fn decode(&self, body: Value) -> Result<CompletionResponse, LlmError> {
        if body.get("type").and_then(Value::as_str) == Some("error") {
            let message = body
                .pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| body.to_string());
            return Err(LlmError::Decode(message));
        }

        let mut content = vec![];
        for block in body
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let text = |k: &str| {
                block
                    .get(k)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            match block.get("type").and_then(Value::as_str) {
                Some("text") => content.push(AssistantPart::Text(text("text"))),
                Some("tool_use") => content.push(AssistantPart::ToolCall {
                    id: text("id"),
                    name: text("name"),
                    arguments: block.get("input").cloned().unwrap_or_else(|| json!({})),
                    provider_data: None,
                }),
                Some("thinking") => content.push(AssistantPart::Reasoning {
                    text: text("thinking"),
                    provider_data: block
                        .get("signature")
                        .and_then(Value::as_str)
                        .map(|s| json!({"signature": s})),
                }),
                Some("redacted_thinking") => content.push(AssistantPart::Reasoning {
                    text: String::new(),
                    provider_data: Some(json!({"redacted": text("data")})),
                }),
                _ => {}
            }
        }

        if content.is_empty() {
            // Anthropic documents empty `end_turn` replies after a tool-result round trip.
            if body.get("stop_reason").and_then(Value::as_str) == Some("end_turn") {
                content.push(AssistantPart::Text(String::new()));
            } else {
                return Err(LlmError::Decode("response contained no content".into()));
            }
        }

        let u = body.get("usage");
        let get = |k: &str| {
            u.and_then(|u| u.get(k))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        let (input, output) = (get("input_tokens"), get("output_tokens"));
        let (read, write) = (
            get("cache_read_input_tokens"),
            get("cache_creation_input_tokens"),
        );
        let finish_reason =
            super::finish_reason(body.get("stop_reason").and_then(Value::as_str), &content);
        Ok(CompletionResponse {
            id: super::str_field(&body, "id"),
            model: super::str_field(&body, "model"),
            finish_reason,
            content,
            usage: Usage {
                input_tokens: input,
                output_tokens: output,
                total_tokens: input + read + write + output,
                cached_input_tokens: read,
                cache_creation_input_tokens: write,
            },
        })
    }

    fn list_models_path(&self, m: &ResolvedModel) -> Option<String> {
        Some(versioned(m, "/models"))
    }

    fn decode_models(&self, body: Value) -> Vec<ListedModel> {
        decode_openai_models(body)
    }
}

fn ephemeral() -> Value {
    json!({"type": "ephemeral"})
}

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn encode_part(part: &Part) -> Value {
    match part {
        Part::Text(text) => json!({"type": "text", "text": text}),
        Part::Attachment { mime, data, .. } if mime == "application/pdf" => json!({
            "type": "document",
            "source": {"type": "base64", "media_type": mime, "data": b64(data)},
        }),
        Part::Attachment { mime, data, .. } => json!({
            "type": "image",
            "source": {"type": "base64", "media_type": mime, "data": b64(data)},
        }),
    }
}

fn encode_assistant_part(part: &AssistantPart) -> Value {
    match part {
        AssistantPart::Text(text) => json!({"type": "text", "text": text}),
        AssistantPart::ToolCall {
            id,
            name,
            arguments,
            ..
        } => json!({"type": "tool_use", "id": id, "name": name, "input": arguments}),
        AssistantPart::Reasoning {
            text,
            provider_data,
        } => {
            if let Some(data) = provider_data
                .as_ref()
                .and_then(|p| p.get("redacted"))
                .and_then(Value::as_str)
            {
                json!({"type": "redacted_thinking", "data": data})
            } else {
                let mut block = json!({"type": "thinking", "thinking": text});
                if let Some(sig) = provider_data
                    .as_ref()
                    .and_then(|p| p.get("signature"))
                    .and_then(Value::as_str)
                {
                    block["signature"] = json!(sig);
                }
                block
            }
        }
    }
}

/// Consecutive tool results go into one user turn of `tool_result` blocks.
fn encode_messages(messages: &[Message]) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    let mut results: Vec<Value> = vec![];

    let flush = |out: &mut Vec<Value>, results: &mut Vec<Value>| {
        if !results.is_empty() {
            out.push(json!({"role": "user", "content": std::mem::take(results)}));
        }
    };

    for message in messages {
        match message {
            Message::ToolResult { call_id, content } => results.push(json!({
                "type": "tool_result",
                "tool_use_id": call_id,
                "content": content.iter().map(encode_part).collect::<Vec<_>>(),
            })),
            Message::User(parts) => {
                flush(&mut out, &mut results);
                out.push(json!({
                    "role": "user",
                    "content": parts.iter().map(encode_part).collect::<Vec<_>>(),
                }));
            }
            Message::Assistant(parts) => {
                flush(&mut out, &mut results);
                if !parts.is_empty() {
                    out.push(json!({
                        "role": "assistant",
                        "content": parts.iter().map(encode_assistant_part).collect::<Vec<_>>(),
                    }));
                }
            }
            // Normalised away by `prepare` (SystemStyle::Field).
            Message::System(_) => {}
        }
    }
    flush(&mut out, &mut results);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::prepare::prepare;
    use crate::llm::types::{CompletionRequest, ToolDefinition};

    fn model() -> ResolvedModel {
        ResolvedModel {
            base_url: "https://api.anthropic.com/v1".into(),
            max_output: Some(64000),
            ..ResolvedModel::test("claude-x")
        }
    }

    fn encode(req: CompletionRequest) -> Value {
        let m = model();
        Anthropic.encode(&prepare(req, &m, SystemStyle::Field), &m)
    }

    fn tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.into(),
            description: "d".into(),
            parameters: json!({"type": "object"}),
        }
    }

    #[test]
    fn places_cache_breakpoints_on_tools_system_and_the_last_block() {
        let body = encode(CompletionRequest {
            messages: vec![
                Message::system("a"),
                Message::system("b"),
                Message::user_text("hi").unwrap(),
            ],
            tools: vec![tool("one"), tool("two")],
            ..Default::default()
        });
        assert_eq!(
            body["system"],
            json!([{"type": "text", "text": "a\n\nb", "cache_control": {"type": "ephemeral"}}])
        );
        assert!(body["tools"][0].get("cache_control").is_none());
        assert_eq!(body["tools"][1]["cache_control"], ephemeral());
        assert_eq!(body["messages"][0]["content"][0]["cache_control"], ephemeral());
        assert_eq!(body["max_tokens"], 64000);
    }

    #[test]
    fn groups_consecutive_tool_results_and_round_trips_thinking() {
        let body = encode(CompletionRequest {
            messages: vec![
                Message::user_text("go").unwrap(),
                Message::Assistant(vec![
                    AssistantPart::Reasoning {
                        text: "plan".into(),
                        provider_data: Some(json!({"signature": "sig"})),
                    },
                    AssistantPart::ToolCall {
                        id: "t1".into(),
                        name: "a".into(),
                        arguments: json!({}),
                        provider_data: None,
                    },
                    AssistantPart::ToolCall {
                        id: "t2".into(),
                        name: "b".into(),
                        arguments: json!({}),
                        provider_data: None,
                    },
                ]),
                Message::tool_result("t1", "r1"),
                Message::tool_result("t2", "r2"),
            ],
            tool_choice: Some(ToolChoice::None),
            ..Default::default()
        });
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(
            msgs[1]["content"][0],
            json!({"type": "thinking", "thinking": "plan", "signature": "sig"})
        );
        assert_eq!(msgs[2]["content"].as_array().unwrap().len(), 2);
        assert_eq!(msgs[2]["content"][0]["tool_use_id"], "t1");
        assert_eq!(msgs[2]["content"][1]["cache_control"], ephemeral());
        assert_eq!(body["tool_choice"], json!({"type": "none"}));
    }

    #[test]
    fn a_later_system_message_becomes_user_text() {
        let body = encode(CompletionRequest {
            messages: vec![
                Message::system("top"),
                Message::user_text("hi").unwrap(),
                Message::assistant_text("yo"),
                Message::system("checkpoint"),
            ],
            ..Default::default()
        });
        assert_eq!(body["system"][0]["text"], "top");
        assert_eq!(
            body["messages"][2]["content"][0]["text"],
            "<system>\ncheckpoint\n</system>"
        );
    }

    #[test]
    fn decodes_thinking_tool_use_and_cache_usage() {
        let r = Anthropic
            .decode(json!({
                "content": [
                    {"type": "thinking", "thinking": "hm", "signature": "s1"},
                    {"type": "text", "text": "ok"},
                    {"type": "tool_use", "id": "t", "name": "f", "input": {"x": 1}}
                ],
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 5, "output_tokens": 2, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 7}
            }))
            .unwrap();
        assert_eq!(
            r.content[0],
            AssistantPart::Reasoning {
                text: "hm".into(),
                provider_data: Some(json!({"signature": "s1"}))
            }
        );
        assert_eq!(r.usage.cached_input_tokens, 100);
        assert_eq!(r.usage.cache_creation_input_tokens, 7);
        assert_eq!(r.usage.total_tokens, 114);
    }

    #[test]
    fn an_empty_end_turn_is_an_empty_text() {
        let r = Anthropic
            .decode(json!({"content": [], "stop_reason": "end_turn", "usage": {}}))
            .unwrap();
        assert_eq!(r.content, vec![AssistantPart::Text(String::new())]);
    }

    #[test]
    fn path_follows_the_base_url() {
        assert_eq!(Anthropic.path(&model()), "/messages");
        let bare = ResolvedModel {
            base_url: "https://api.anthropic.com".into(),
            ..model()
        };
        assert_eq!(Anthropic.path(&bare), "/v1/messages");
    }
}
