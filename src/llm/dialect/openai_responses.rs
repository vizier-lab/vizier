//! OpenAI Responses (`/responses`), always with `store: false`, so nothing is kept server side
//! and a tool call has one id: its `call_id` (contract S5). Ported from rig-core 0.39
//! `providers/openai/responses_api/` and `providers/chatgpt` (the SSE-only Codex backend).

use serde_json::{Map, Value, json};

use super::{BodyKind, Dialect, PreparedRequest, SystemStyle, decode_openai_models};
use crate::llm::error::LlmError;
use crate::llm::model::ListedModel;
use crate::llm::resolve::ResolvedModel;
use crate::llm::sse::parse_sse_body;
use crate::llm::types::{AssistantPart, CompletionResponse, Message, Part, ToolChoice, Usage};

pub struct OpenAiResponses;

impl OpenAiResponses {
    fn wants_encrypted_reasoning(m: &ResolvedModel) -> bool {
        m.stream_required || m.caps.reasoning == Some(true)
    }
}

impl Dialect for OpenAiResponses {
    fn system_style(&self) -> SystemStyle {
        SystemStyle::Instructions
    }

    fn path(&self, _m: &ResolvedModel) -> String {
        "/responses".into()
    }

    fn encode(&self, req: &PreparedRequest, m: &ResolvedModel) -> Value {
        let input: Vec<Value> = req.messages.iter().flat_map(encode_message).collect();
        let mut body = json!({
            "model": m.model,
            "input": input,
            "store": false,
        });

        let instructions = match (&m.default_instructions, &req.system) {
            (Some(default), Some(own)) if own.contains(default.as_str()) => Some(own.clone()),
            (Some(default), Some(own)) => Some(format!("{default}\n\n{own}")),
            (Some(default), None) => Some(default.clone()),
            (None, own) => own.clone(),
        };
        if let Some(instructions) = instructions.filter(|i| !i.trim().is_empty()) {
            body["instructions"] = json!(instructions);
        }
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(
                req.tools
                    .iter()
                    .map(|t| {
                        let mut parameters = t.parameters.clone();
                        sanitize_strict_schema(&mut parameters);
                        json!({
                            "type": "function",
                            "name": t.name,
                            "description": t.description,
                            "parameters": parameters,
                            "strict": true,
                        })
                    })
                    .collect(),
            );
        }
        if let Some(choice) = req.tool_choice {
            body["tool_choice"] = json!(match choice {
                ToolChoice::Auto => "auto",
                ToolChoice::None => "none",
            });
        }
        if Self::wants_encrypted_reasoning(m) {
            body["include"] = json!(["reasoning.encrypted_content"]);
        }
        if m.stream_required {
            // The ChatGPT backend only streams, and refuses an output cap.
            body["stream"] = json!(true);
        } else if let Some(max) = req.max_tokens.or(m.max_tokens) {
            body["max_output_tokens"] = json!(max);
        }
        body
    }

    fn decode(&self, body: Value) -> Result<CompletionResponse, LlmError> {
        if let Some(err) = super::error_in_body(&body) {
            return Err(LlmError::Decode(err));
        }
        if body.get("status").and_then(Value::as_str) == Some("failed") {
            return Err(LlmError::Decode(format!("response failed: {body}")));
        }
        let output = body
            .get("output")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut id = None;
        let mut content = vec![];
        for item in &output {
            match item.get("type").and_then(Value::as_str) {
                Some("message") => {
                    if id.is_none() {
                        id = item.get("id").and_then(Value::as_str).map(str::to_string);
                    }
                    for part in item
                        .get("content")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(text) = part
                            .get("text")
                            .or_else(|| part.get("refusal"))
                            .and_then(Value::as_str)
                        {
                            content.push(AssistantPart::Text(text.to_string()));
                        }
                    }
                }
                Some("function_call") => content.push(AssistantPart::ToolCall {
                    id: item
                        .get("call_id")
                        .or_else(|| item.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    name: item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    arguments: super::openai_chat::parse_arguments(item.get("arguments")),
                    provider_data: None,
                }),
                Some("reasoning") => {
                    let summary = item
                        .get("summary")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let text = summary
                        .iter()
                        .filter_map(|s| s.get("text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let mut data = Map::new();
                    for key in ["id", "encrypted_content"] {
                        if let Some(v) = item.get(key).filter(|v| !v.is_null()) {
                            data.insert(key.into(), v.clone());
                        }
                    }
                    data.insert("summary".into(), Value::Array(summary));
                    content.push(AssistantPart::Reasoning {
                        text,
                        provider_data: Some(Value::Object(data)),
                    });
                }
                _ => {}
            }
        }
        if content.is_empty() {
            return Err(LlmError::Decode(
                "response contained no message or tool call (empty)".into(),
            ));
        }

        let usage = body.get("usage");
        let get = |p: &str| {
            usage
                .and_then(|u| u.pointer(p))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        Ok(CompletionResponse {
            id,
            content,
            usage: Usage {
                input_tokens: get("/input_tokens"),
                output_tokens: get("/output_tokens"),
                total_tokens: get("/total_tokens"),
                cached_input_tokens: get("/input_tokens_details/cached_tokens"),
                cache_creation_input_tokens: 0,
            },
        })
    }

    fn read_body(&self, m: &ResolvedModel) -> BodyKind {
        if m.stream_required {
            BodyKind::Sse
        } else {
            BodyKind::Json
        }
    }

    fn aggregate_sse(&self, body: &str) -> Result<Value, LlmError> {
        aggregate_responses_sse(body)
    }

    fn list_models_path(&self, _m: &ResolvedModel) -> Option<String> {
        Some("/models".into())
    }

    fn decode_models(&self, body: Value) -> Vec<ListedModel> {
        decode_openai_models(body)
    }
}

/// One input item per part, as rig 0.39 sent them.
fn encode_message(message: &Message) -> Vec<Value> {
    match message {
        // Leading system messages are `instructions`; later ones go as developer messages.
        Message::System(text) => vec![json!({
            "type": "message",
            "role": "developer",
            "content": [{"type": "input_text", "text": text}],
        })],
        Message::User(parts) => parts
            .iter()
            .map(|part| {
                let content = match part {
                    Part::Text(text) => json!({"type": "input_text", "text": text}),
                    Part::Attachment { mime, data, name } if mime == "application/pdf" => json!({
                        "type": "input_file",
                        "file_data": data_url(mime, data),
                        "filename": name.clone().unwrap_or_else(|| "document.pdf".into()),
                    }),
                    Part::Attachment { mime, data, .. } => json!({
                        "type": "input_image",
                        "image_url": data_url(mime, data),
                        "detail": "auto",
                    }),
                };
                json!({"type": "message", "role": "user", "content": [content]})
            })
            .collect(),
        Message::Assistant(parts) => {
            let mut reasoning = vec![];
            let mut others = vec![];
            for part in parts {
                match part {
                    AssistantPart::Text(text) if text.is_empty() => {}
                    AssistantPart::Text(text) => others.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": text,
                    })),
                    AssistantPart::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    } => others.push(json!({
                        "type": "function_call",
                        "call_id": id,
                        "name": name,
                        "arguments": arguments.to_string(),
                        "status": "completed",
                    })),
                    // Without stored items a reasoning item is only accepted back with its
                    // encrypted content.
                    AssistantPart::Reasoning {
                        provider_data: Some(data),
                        ..
                    } if data.get("encrypted_content").is_some() && data.get("id").is_some() => {
                        let mut item = json!({
                            "type": "reasoning",
                            "id": data["id"],
                            "summary": data.get("summary").cloned().unwrap_or_else(|| json!([])),
                            "encrypted_content": data["encrypted_content"],
                        });
                        if item["summary"].is_null() {
                            item["summary"] = json!([]);
                        }
                        reasoning.push(item)
                    }
                    AssistantPart::Reasoning { .. } => {}
                }
            }
            reasoning.extend(others);
            reasoning
        }
        Message::ToolResult { call_id, content } => vec![json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": content
                .iter()
                .filter_map(Part::text)
                .collect::<Vec<_>>()
                .join("\n"),
            "status": "completed",
        })],
    }
}

fn data_url(mime: &str, data: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(data)
    )
}

/// The final `response.completed` payload of a streamed body. When it carries no output
/// (the Codex backend sometimes leaves it empty), the `response.output_item.done` items are
/// used instead, then the text deltas.
pub fn aggregate_responses_sse(body: &str) -> Result<Value, LlmError> {
    let mut payloads: Vec<Value> = vec![];
    for event in parse_sse_body(body) {
        match serde_json::from_str::<Value>(&event.data) {
            Ok(v) => payloads.push(v),
            // Several `data:` lines without blank lines between them read as one event.
            Err(_) => payloads.extend(
                event
                    .data
                    .lines()
                    .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok()),
            ),
        }
    }

    let mut completed = None;
    let mut failure = None;
    let mut items = vec![];
    let mut text = String::new();
    for p in payloads {
        match p.get("type").and_then(Value::as_str) {
            Some("response.completed") => {
                completed = p.get("response").cloned();
                break;
            }
            Some("response.failed") | Some("response.incomplete") => {
                let r = p.get("response");
                failure = Some(
                    r.and_then(|r| r.pointer("/error/message"))
                        .or_else(|| r.and_then(|r| r.pointer("/incomplete_details/reason")))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| p.to_string()),
                );
            }
            Some("response.output_item.done") => {
                if let Some(item) = p.get("item") {
                    items.push(item.clone());
                }
            }
            Some("response.output_text.delta") => {
                if let Some(d) = p.get("delta").and_then(Value::as_str) {
                    text.push_str(d);
                }
            }
            Some("error") => {
                failure = Some(
                    p.get("message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| p.to_string()),
                );
            }
            _ => {}
        }
    }

    let Some(mut response) = completed else {
        return Err(LlmError::Decode(failure.unwrap_or_else(|| {
            "the stream ended without a completed response".into()
        })));
    };
    let empty = response
        .get("output")
        .and_then(Value::as_array)
        .is_none_or(|o| o.is_empty());
    if empty {
        if !items.is_empty() {
            response["output"] = Value::Array(items);
        } else if !text.is_empty() {
            response["output"] = json!([{"type": "message", "role": "assistant",
                "content": [{"type": "output_text", "text": text}]}]);
        }
    }
    Ok(response)
}

/// OpenAI strict-mode schema rules, as rig 0.39 `openai::sanitize_schema`: every object gets
/// `properties` and `additionalProperties: false`, every property is required, `oneOf` becomes
/// `anyOf`, and nothing sits beside a `$ref`.
pub fn sanitize_strict_schema(schema: &mut Value) {
    let Value::Object(obj) = schema else {
        return;
    };
    if obj.contains_key("$ref") {
        obj.retain(|k, _| k == "$ref");
        return;
    }
    let is_object = obj.get("type") == Some(&json!("object")) || obj.contains_key("properties");
    if is_object && !obj.contains_key("properties") {
        obj.insert("properties".into(), json!({}));
    }
    if is_object && !obj.contains_key("additionalProperties") {
        obj.insert("additionalProperties".into(), json!(false));
    }
    if let Some(Value::Object(props)) = obj.get("properties") {
        let keys = props.keys().cloned().map(Value::String).collect();
        obj.insert("required".into(), Value::Array(keys));
    }
    if let Some(Value::Object(defs)) = obj.get_mut("$defs") {
        for def in defs.values_mut() {
            sanitize_strict_schema(def);
        }
    }
    if let Some(Value::Object(props)) = obj.get_mut("properties") {
        for prop in props.values_mut() {
            sanitize_strict_schema(prop);
        }
    }
    if let Some(items) = obj.get_mut("items") {
        sanitize_strict_schema(items);
    }
    if let Some(one_of) = obj.remove("oneOf") {
        match obj.get_mut("anyOf") {
            Some(Value::Array(existing)) => {
                if let Value::Array(mut incoming) = one_of {
                    existing.append(&mut incoming);
                }
            }
            _ => {
                obj.insert("anyOf".into(), one_of);
            }
        }
    }
    for key in ["anyOf", "allOf"] {
        if let Some(Value::Array(variants)) = obj.get_mut(key) {
            for v in variants {
                sanitize_strict_schema(v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::data::Capabilities;
    use crate::llm::prepare::prepare;
    use crate::llm::types::{CompletionRequest, ToolDefinition};

    fn encode(req: CompletionRequest, m: &ResolvedModel) -> Value {
        OpenAiResponses.encode(&prepare(req, m, SystemStyle::Instructions), m)
    }

    #[test]
    fn encodes_instructions_items_and_strict_tools() {
        let m = ResolvedModel::test("gpt-5");
        let body = encode(
            CompletionRequest {
                messages: vec![
                    Message::system("sys"),
                    Message::user_text("hi").unwrap(),
                    Message::Assistant(vec![
                        AssistantPart::Text("calling".into()),
                        AssistantPart::ToolCall {
                            id: "call_1".into(),
                            name: "f".into(),
                            arguments: json!({"a": 1}),
                            provider_data: None,
                        },
                    ]),
                    Message::tool_result("call_1", "ok"),
                    Message::system("later"),
                ],
                tools: vec![ToolDefinition {
                    name: "f".into(),
                    description: "d".into(),
                    parameters: json!({"type": "object", "properties": {"a": {"type": "integer"}, "b": {"type": "string"}}}),
                }],
                tool_choice: Some(ToolChoice::None),
                max_tokens: Some(10),
            },
            &m,
        );
        assert_eq!(body["instructions"], "sys");
        assert_eq!(body["store"], false);
        assert_eq!(
            body["input"][2],
            json!({"type": "function_call", "call_id": "call_1", "name": "f", "arguments": "{\"a\":1}", "status": "completed"})
        );
        assert_eq!(
            body["input"][3],
            json!({"type": "function_call_output", "call_id": "call_1", "output": "ok", "status": "completed"})
        );
        assert_eq!(body["input"][4]["role"], "developer");
        assert_eq!(body["tools"][0]["strict"], true);
        assert_eq!(body["tools"][0]["parameters"]["required"], json!(["a", "b"]));
        assert_eq!(body["tools"][0]["parameters"]["additionalProperties"], false);
        assert_eq!(body["max_output_tokens"], 10);
        assert!(body.get("include").is_none());
    }

    #[test]
    fn round_trips_encrypted_reasoning() {
        let m = ResolvedModel {
            caps: Capabilities {
                reasoning: Some(true),
                ..Default::default()
            },
            ..ResolvedModel::test("o4")
        };
        let decoded = OpenAiResponses
            .decode(json!({"output": [
                {"type": "reasoning", "id": "rs_1", "summary": [{"type": "summary_text", "text": "s"}], "encrypted_content": "enc"},
                {"type": "function_call", "id": "fc_1", "call_id": "call_9", "name": "f", "arguments": "{}"}
            ], "usage": {"input_tokens": 5, "output_tokens": 1, "total_tokens": 6, "input_tokens_details": {"cached_tokens": 3}}}))
            .unwrap();
        assert!(matches!(&decoded.content[1], AssistantPart::ToolCall { id, .. } if id == "call_9"));
        assert_eq!(decoded.usage.cached_input_tokens, 3);

        let body = encode(
            CompletionRequest {
                messages: vec![
                    Message::user_text("go").unwrap(),
                    Message::Assistant(decoded.content),
                    Message::tool_result("call_9", "done"),
                ],
                ..Default::default()
            },
            &m,
        );
        assert_eq!(
            body["input"][1],
            json!({"type": "reasoning", "id": "rs_1", "summary": [{"type": "summary_text", "text": "s"}], "encrypted_content": "enc"})
        );
        assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    }

    #[test]
    fn chatgpt_streams_merges_default_instructions_and_drops_the_output_cap() {
        let m = ResolvedModel {
            stream_required: true,
            default_instructions: Some("You are ChatGPT.".into()),
            max_tokens: Some(50),
            ..ResolvedModel::test("gpt-5.4")
        };
        let body = encode(
            CompletionRequest {
                messages: vec![Message::system("be terse"), Message::user_text("hi").unwrap()],
                ..Default::default()
            },
            &m,
        );
        assert_eq!(body["instructions"], "You are ChatGPT.\n\nbe terse");
        assert_eq!(body["stream"], true);
        assert!(body.get("max_output_tokens").is_none());
        assert_eq!(OpenAiResponses.read_body(&m), BodyKind::Sse);
    }

    #[test]
    fn aggregates_an_sse_body() {
        // rig's own fixture: `data:` lines with no blank lines between them.
        let body = r#"data: {"type":"response.output_text.delta","delta":"hi"}
data: {"type":"response.completed","response":{"id":"resp_1","status":"completed","output":[{"type":"message","id":"msg_1","role":"assistant","content":[{"type":"output_text","annotations":[],"text":"hi"}]}],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}
data: [DONE]"#;
        let r = OpenAiResponses
            .decode(aggregate_responses_sse(body).unwrap())
            .unwrap();
        assert_eq!(r.content, vec![AssistantPart::Text("hi".into())]);
        assert_eq!(r.id.as_deref(), Some("msg_1"));

        let empty_completed = "event: response.output_item.done\n\
            data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"c\",\"name\":\"f\",\"arguments\":\"{}\"}}\n\n\
            event: response.completed\n\
            data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[]}}\n\n";
        let r = OpenAiResponses
            .decode(aggregate_responses_sse(empty_completed).unwrap())
            .unwrap();
        assert!(matches!(&r.content[0], AssistantPart::ToolCall { id, .. } if id == "c"));

        let failed = "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"nope\"}}}\n\n";
        assert_eq!(
            aggregate_responses_sse(failed).unwrap_err(),
            LlmError::Decode("nope".into())
        );
    }
}
