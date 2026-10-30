//! Bedrock's unified Converse API (`POST /model/{id}/converse`): one dialect for every Bedrock
//! chat family (research R4, contract `bedrock-wire.md` §3–4).
//!
//! The dialect is pure: it receives the resolved model and the prepared request and translates
//! both ways. Signing, the regional host and the model-id path segment are the transport's and
//! `llm::bedrock`'s business.

use base64::Engine;
use serde_json::{Value, json};

use super::{Dialect, PreparedRequest, SystemStyle, error_in_body, finish_reason, str_field};
use crate::llm::bedrock::{aws_uri_encode, model_path};
use crate::llm::error::LlmError;
use crate::llm::resolve::ResolvedModel;
use crate::llm::types::{
    AssistantPart, CompletionResponse, Message, Part, ToolChoice, ToolDefinition, Usage,
};

pub struct Bedrock;

/// Converse image blocks accept exactly these formats; anything else becomes the shared note.
fn image_format(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpeg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// The note `prepare` would have used for an attachment this dialect cannot send (kept in the
/// same words so a history can mix providers without the text changing).
fn attachment_note(mime: &str, name: Option<&str>) -> String {
    let name = name.unwrap_or("file");
    if mime.starts_with("image/") {
        format!("[image attached: {name}, but this model cannot view images]")
    } else if mime == "application/pdf" {
        format!("[document attached: {name}, but this model cannot read documents]")
    } else {
        format!("[file attached: {name} ({mime}), but this model cannot read it]")
    }
}

/// One person/tool content part as a Converse content block.
fn encode_part(part: &Part) -> Value {
    match part {
        Part::Text(text) => json!({"text": text}),
        Part::Attachment { mime, data, name } => match image_format(mime) {
            Some(format) => json!({
                "image": {
                    "format": format,
                    "source": {"bytes": b64(data)},
                }
            }),
            None => json!({"text": attachment_note(mime, name.as_deref())}),
        },
    }
}

/// One assistant part as a Converse content block; `None` for a block Converse should not see.
fn encode_assistant_part(part: &AssistantPart) -> Option<Value> {
    match part {
        AssistantPart::Text(text) if text.is_empty() => None,
        AssistantPart::Text(text) => Some(json!({"text": text})),
        AssistantPart::ToolCall {
            id,
            name,
            arguments,
            ..
        } => Some(json!({
            "toolUse": {
                "toolUseId": id,
                "name": name,
                "input": arguments,
            }
        })),
        // Reasoning is re-sent only with a signature, as Converse requires; without one the
        // provider cannot verify it, so it is dropped rather than rejected.
        AssistantPart::Reasoning {
            text,
            provider_data,
        } => {
            let signature = provider_data
                .as_ref()
                .and_then(|p| p.get("signature"))
                .and_then(Value::as_str)?;
            Some(json!({
                "reasoningContent": {
                    "reasoningText": {"text": text, "signature": signature}
                }
            }))
        }
    }
}

fn encode_tools(tools: &[ToolDefinition]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({
                "toolSpec": {
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": {"json": t.parameters},
                }
            })
        })
        .collect()
}

/// Append a message, merging consecutive messages of the same role (Converse requires strict
/// alternation, and its tool results are user content).
fn push_message(out: &mut Vec<Value>, role: &str, blocks: Vec<Value>) {
    if blocks.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(last) if last["role"] == role => {
            if let Some(content) = last.get_mut("content").and_then(Value::as_array_mut) {
                content.extend(blocks);
            }
        }
        _ => out.push(json!({"role": role, "content": blocks})),
    }
}

fn encode_messages(messages: &[Message]) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for message in messages {
        match message {
            Message::User(parts) => {
                push_message(&mut out, "user", parts.iter().map(encode_part).collect());
            }
            Message::ToolResult { call_id, content } => {
                push_message(
                    &mut out,
                    "user",
                    vec![json!({
                        "toolResult": {
                            "toolUseId": call_id,
                            "content": content.iter().map(encode_part).collect::<Vec<_>>(),
                        }
                    })],
                );
            }
            Message::Assistant(parts) => push_message(
                &mut out,
                "assistant",
                parts.iter().filter_map(encode_assistant_part).collect(),
            ),
            // `prepare` normalises system messages for `SystemStyle::Field`; a later one has
            // already become user text.
            Message::System(_) => {}
        }
    }
    out
}

impl Dialect for Bedrock {
    fn system_style(&self) -> SystemStyle {
        SystemStyle::Field
    }

    fn path(&self, m: &ResolvedModel) -> String {
        model_path(&m.model, "converse")
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("content-type", "application/json".into()),
            ("accept", "application/json".into()),
        ]
    }

    fn encode(&self, req: &PreparedRequest, m: &ResolvedModel) -> Value {
        let mut body = json!({"messages": encode_messages(&req.messages)});
        if let Some(system) = req.system.as_ref().filter(|s| !s.is_empty()) {
            body["system"] = json!([{"text": system}]);
        }
        if let Some(max_tokens) = req.max_tokens.or(m.max_tokens).or(m.max_output) {
            body["inferenceConfig"] = json!({"maxTokens": max_tokens});
        }
        if !req.tools.is_empty() {
            let mut config = json!({"tools": encode_tools(&req.tools)});
            // Converse's default is `auto`; `ToolChoice::None` has no equivalent, so the field
            // is omitted and the tools stay defined for histories that contain tool calls.
            if req.tool_choice == Some(ToolChoice::Auto) {
                config["toolChoice"] = json!({"auto": {}});
            }
            body["toolConfig"] = config;
        }
        body
    }

    fn decode(&self, body: Value) -> Result<CompletionResponse, LlmError> {
        if let Some(message) = error_in_body(&body) {
            return Err(LlmError::Decode(message));
        }

        let stop_reason = body.get("stopReason").and_then(Value::as_str);
        let mut content = vec![];
        for block in body
            .pointer("/output/message/content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                content.push(AssistantPart::Text(text.to_string()));
            } else if let Some(call) = block.get("toolUse") {
                let text = |key: &str| {
                    call.get(key)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                };
                content.push(AssistantPart::ToolCall {
                    id: text("toolUseId"),
                    name: text("name"),
                    arguments: call.get("input").cloned().unwrap_or_else(|| json!({})),
                    provider_data: None,
                });
            } else if let Some(reasoning) = block
                .get("reasoningContent")
                .and_then(|r| r.get("reasoningText"))
            {
                let text = reasoning
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let signature = reasoning.get("signature").and_then(Value::as_str);
                if !text.is_empty() || signature.is_some() {
                    content.push(AssistantPart::Reasoning {
                        text,
                        provider_data: signature.map(|s| json!({"signature": s})),
                    });
                }
            }
            // `citationsContent` and blocks this release does not know are ignored.
        }

        if content.is_empty() {
            // Converse documents empty `end_turn` replies after a tool-result round trip.
            if stop_reason == Some("end_turn") {
                content.push(AssistantPart::Text(String::new()));
            } else {
                return Err(LlmError::Decode(
                    "the Bedrock response contained no content".into(),
                ));
            }
        }

        let usage = body.get("usage");
        let count = |key: &str| {
            usage
                .and_then(|u| u.get(key))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        let input = count("inputTokens");
        let output = count("outputTokens");
        let total = usage
            .and_then(|u| u.get("totalTokens"))
            .and_then(Value::as_u64)
            .unwrap_or(input + output);
        let finish_reason = finish_reason(stop_reason, &content);

        Ok(CompletionResponse {
            // Converse echoes no response id; the requested id is what usage records.
            id: str_field(&body, "id"),
            model: None,
            finish_reason,
            content,
            usage: Usage {
                input_tokens: input,
                output_tokens: output,
                total_tokens: total,
                cached_input_tokens: count("cacheReadInputTokens"),
                cache_creation_input_tokens: count("cacheWriteInputTokens"),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::prepare::prepare;
    use crate::llm::types::{CompletionRequest, Message};
    use serde_json::json;

    fn model() -> ResolvedModel {
        ResolvedModel {
            base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            max_output: Some(64000),
            ..ResolvedModel::test("anthropic.claude-haiku-4-5-20251001-v1:0")
        }
    }

    fn encode(req: CompletionRequest) -> Value {
        let m = model();
        Bedrock.encode(&prepare(req, &m, SystemStyle::Field), &m)
    }

    fn tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.into(),
            description: "d".into(),
            parameters: json!({"type": "object"}),
        }
    }

    #[test]
    fn the_path_encodes_the_model_id() {
        assert_eq!(
            Bedrock.path(&model()),
            "/model/anthropic.claude-haiku-4-5-20251001-v1%3A0/converse"
        );
        for id in [
            "us.anthropic.claude-haiku-4-5-20251001-v1:0",
            "global.amazon.nova-2-lite-v1:0",
        ] {
            assert_eq!(
                Bedrock.path(&ResolvedModel {
                    model: id.into(),
                    ..model()
                }),
                format!("/model/{}/converse", id.replace(':', "%3A"))
            );
        }
        assert_eq!(
            aws_uri_encode("a/b:c"),
            "a%2Fb%3Ac",
            "the dialect uses the shared encoder"
        );
    }

    #[test]
    fn encodes_system_multi_turn_and_inference_config() {
        let body = encode(CompletionRequest {
            messages: vec![
                Message::system("top"),
                Message::user_text("hi").unwrap(),
                Message::assistant_text("hello"),
                Message::user_text("again").unwrap(),
            ],
            ..Default::default()
        });
        assert_eq!(body["system"], json!([{"text": "top"}]));
        assert_eq!(
            body["messages"],
            json!([
                {"role": "user", "content": [{"text": "hi"}]},
                {"role": "assistant", "content": [{"text": "hello"}]},
                {"role": "user", "content": [{"text": "again"}]}
            ])
        );
        assert_eq!(body["inferenceConfig"], json!({"maxTokens": 64000}));
        assert!(body.get("toolConfig").is_none());
    }

    #[test]
    fn consecutive_same_role_messages_are_merged() {
        let body = encode(CompletionRequest {
            messages: vec![
                Message::user_text("one").unwrap(),
                Message::user_text("two").unwrap(),
                Message::Assistant(vec![
                    AssistantPart::Text("a".into()),
                    AssistantPart::Text("b".into()),
                ]),
                Message::tool_result("t1", "r1"),
                Message::tool_result("t2", "r2"),
            ],
            ..Default::default()
        });
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3, "{messages:?}");
        assert_eq!(messages[0]["content"].as_array().unwrap().len(), 2);
        assert_eq!(messages[2]["content"].as_array().unwrap().len(), 2);
        assert_eq!(messages[2]["content"][0]["toolResult"]["toolUseId"], "t1");
        assert_eq!(
            messages[2]["content"][0]["toolResult"]["content"][0]["text"],
            "r1"
        );
    }

    #[test]
    fn decodes_text_and_usage_and_stop_reasons() {
        let response = Bedrock
            .decode(json!({
                "output": {"message": {"role": "assistant", "content": [{"text": "ok"}]}},
                "stopReason": "end_turn",
                "usage": {"inputTokens": 30, "outputTokens": 12, "totalTokens": 42,
                          "cacheReadInputTokens": 5, "cacheWriteInputTokens": 2}
            }))
            .unwrap();
        assert_eq!(response.content, vec![AssistantPart::Text("ok".into())]);
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert_eq!(response.usage.input_tokens, 30);
        assert_eq!(response.usage.output_tokens, 12);
        assert_eq!(response.usage.total_tokens, 42);
        assert_eq!(response.usage.cached_input_tokens, 5);
        assert_eq!(response.usage.cache_creation_input_tokens, 2);
        for (raw, expected) in [
            ("end_turn", "stop"),
            ("stop_sequence", "stop"),
            ("max_tokens", "length"),
            ("content_filtered", "content_filter"),
            ("guardrail_intervened", "content_filter"),
            ("weird_reason", "weird_reason"),
        ] {
            let r = Bedrock
                .decode(json!({
                    "output": {"message": {"role": "assistant", "content": [{"text": "x"}]}},
                    "stopReason": raw
                }))
                .unwrap();
            assert_eq!(r.finish_reason.as_deref(), Some(expected), "{raw}");
        }
    }

    #[test]
    fn an_empty_end_turn_is_empty_text_and_anything_else_is_a_decode_error() {
        let r = Bedrock
            .decode(json!({"output": {"message": {"role": "assistant", "content": []}}, "stopReason": "end_turn"}))
            .unwrap();
        assert_eq!(r.content, vec![AssistantPart::Text(String::new())]);
        assert!(Bedrock.decode(json!({"stopReason": "tool_use"})).is_err());
        assert!(
            Bedrock
                .decode(json!({"error": {"message": "bad"}}))
                .is_err()
        );
    }

    #[test]
    fn usage_falls_back_to_input_plus_output() {
        let r = Bedrock
            .decode(json!({
                "output": {"message": {"content": [{"text": "x"}]}},
                "stopReason": "end_turn",
                "usage": {"inputTokens": 3, "outputTokens": 4}
            }))
            .unwrap();
        assert_eq!(r.usage.total_tokens, 7);
    }

    #[test]
    fn tool_choice_and_tools_are_encoded_as_converse_expects() {
        let body = encode(CompletionRequest {
            messages: vec![Message::user_text("go").unwrap()],
            tools: vec![tool("memory_search")],
            tool_choice: Some(ToolChoice::Auto),
            ..Default::default()
        });
        assert_eq!(
            body["toolConfig"]["tools"][0]["toolSpec"]["name"],
            json!("memory_search")
        );
        assert_eq!(body["toolConfig"]["toolChoice"], json!({"auto": {}}));

        // None has no Converse equivalent: tools stay, toolChoice is omitted.
        let body = encode(CompletionRequest {
            messages: vec![Message::user_text("go").unwrap()],
            tools: vec![tool("memory_search")],
            tool_choice: Some(ToolChoice::None),
            ..Default::default()
        });
        assert!(body["toolConfig"].get("toolChoice").is_none());
        assert_eq!(
            body["toolConfig"]["tools"].as_array().unwrap().len(),
            1,
            "tools stay defined for histories with tool calls"
        );
    }

    #[test]
    fn images_are_encoded_and_unsupported_ones_become_a_note() {
        let png = Part::Attachment {
            mime: "image/png".into(),
            data: vec![1, 2, 3],
            name: Some("cat.png".into()),
        };
        let svg = Part::Attachment {
            mime: "image/svg+xml".into(),
            data: vec![1],
            name: Some("cat.svg".into()),
        };
        let pdf = Part::Attachment {
            mime: "application/pdf".into(),
            data: vec![1],
            name: Some("a.pdf".into()),
        };
        let body = encode(CompletionRequest {
            messages: vec![Message::User(vec![
                Part::Text("look".into()),
                png,
                svg,
                pdf,
            ])],
            ..Default::default()
        });
        let content = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content[0], json!({"text": "look"}));
        assert_eq!(content[1]["image"]["format"], json!("png"));
        assert_eq!(content[1]["image"]["source"]["bytes"], json!("AQID"));
        assert_eq!(
            content[2]["text"],
            json!("[image attached: cat.svg, but this model cannot view images]")
        );
        assert_eq!(
            content[3]["text"],
            json!("[document attached: a.pdf, but this model cannot read documents]")
        );
    }

    #[test]
    fn tool_results_and_images_round_trip_inside_one_user_message() {
        let body = encode(CompletionRequest {
            messages: vec![
                Message::user_text("go").unwrap(),
                Message::Assistant(vec![AssistantPart::ToolCall {
                    id: "t1".into(),
                    name: "f".into(),
                    arguments: json!({"x": 1}),
                    provider_data: None,
                }]),
                Message::ToolResult {
                    call_id: "t1".into(),
                    content: vec![Part::Text("done".into())],
                },
            ],
            ..Default::default()
        });
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages[1]["content"][0],
            json!({"toolUse": {"toolUseId": "t1", "name": "f", "input": {"x": 1}}})
        );
        assert_eq!(
            messages[2]["content"][0]["toolResult"],
            json!({"toolUseId": "t1", "content": [{"text": "done"}]})
        );
    }

    #[test]
    fn reasoning_is_re_sent_only_with_a_signature() {
        let signed = encode(CompletionRequest {
            messages: vec![Message::Assistant(vec![
                AssistantPart::Reasoning {
                    text: "plan".into(),
                    provider_data: Some(json!({"signature": "sig"})),
                },
                AssistantPart::Text("answer".into()),
            ])],
            ..Default::default()
        });
        assert_eq!(
            signed["messages"][0]["content"][0],
            json!({"reasoningContent": {"reasoningText": {"text": "plan", "signature": "sig"}}})
        );

        let unsigned = encode(CompletionRequest {
            messages: vec![Message::Assistant(vec![
                AssistantPart::Reasoning {
                    text: "plan".into(),
                    provider_data: None,
                },
                AssistantPart::Text("answer".into()),
            ])],
            ..Default::default()
        });
        assert_eq!(
            unsigned["messages"][0]["content"],
            json!([{"text": "answer"}])
        );
    }

    /// Several calls in one round decode to several `ToolCall`s; unknown blocks are ignored.
    #[test]
    fn several_tool_calls_and_unknown_blocks_decode() {
        let reply = Bedrock
            .decode(json!({
                "output": {"message": {"role": "assistant", "content": [
                    {"text": "two calls"},
                    {"toolUse": {"toolUseId": "c1", "name": "one", "input": {}}},
                    {"toolUse": {"toolUseId": "c2", "name": "two", "input": {"x": 1}}},
                    {"citationsContent": {"citations": []}}
                ]}},
                "stopReason": "tool_use"
            }))
            .unwrap();
        assert_eq!(reply.finish_reason.as_deref(), Some("tool_call"));
        let calls: Vec<_> = reply
            .content
            .iter()
            .filter_map(|p| match p {
                AssistantPart::ToolCall {
                    id,
                    name,
                    arguments,
                    ..
                } => Some((id.as_str(), name.as_str(), arguments.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(calls.len(), 2, "{:?}", reply.content);
        assert_eq!(calls[0], ("c1", "one", json!({})));
        assert_eq!(calls[1], ("c2", "two", json!({"x": 1})));

        // Two results go back as one user turn, in the same order.
        let next = encode(CompletionRequest {
            messages: vec![
                Message::user_text("go").unwrap(),
                Message::Assistant(reply.content),
                Message::tool_result("c1", "r1"),
                Message::tool_result("c2", "r2"),
            ],
            ..Default::default()
        });
        let results = next["messages"][2]["content"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["toolResult"]["toolUseId"], json!("c1"));
        assert_eq!(results[1]["toolResult"]["toolUseId"], json!("c2"));
    }

    #[test]
    fn a_tool_loop_round_trips_without_duplicating_the_call() {
        let reply = Bedrock
            .decode(json!({
                "output": {"message": {"role": "assistant", "content": [
                    {"text": "checking"},
                    {"toolUse": {"toolUseId": "call-1", "name": "memory_search", "input": {"query": "x"}}}
                ]}},
                "stopReason": "tool_use",
                "usage": {"inputTokens": 10, "outputTokens": 4}
            }))
            .unwrap();
        assert_eq!(reply.finish_reason.as_deref(), Some("tool_call"));
        let call = reply
            .content
            .iter()
            .find_map(|p| match p {
                AssistantPart::ToolCall {
                    id,
                    name,
                    arguments,
                    ..
                } => Some((id.clone(), name.clone(), arguments.clone())),
                _ => None,
            })
            .expect("the reply calls a tool");
        assert_eq!(call.0, "call-1");
        assert_eq!(call.1, "memory_search");

        // The agent loop appends the assistant turn and one result per call, then asks again.
        let next = encode(CompletionRequest {
            messages: vec![
                Message::user_text("search").unwrap(),
                Message::Assistant(reply.content.clone()),
                Message::tool_result(call.0.clone(), "found it"),
            ],
            tools: vec![tool("memory_search")],
            ..Default::default()
        });
        let messages = next["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3);
        let calls = messages[1]["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b.get("toolUse").is_some())
            .count();
        assert_eq!(calls, 1, "the call is sent once");
        assert_eq!(messages[2]["role"], json!("user"));
        assert_eq!(
            messages[2]["content"][0]["toolResult"],
            json!({"toolUseId": "call-1", "content": [{"text": "found it"}]})
        );
    }
}
