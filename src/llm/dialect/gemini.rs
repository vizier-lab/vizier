//! Google Gemini `generateContent`. Ported from rig-core 0.39 `providers/gemini/completion.rs`,
//! including its tool-schema sanitising (Gemini accepts only an OpenAPI subset).

use base64::Engine;
use serde_json::{Map, Value, json};

use super::{Dialect, PreparedRequest, SystemStyle};
use crate::llm::error::LlmError;
use crate::llm::model::ListedModel;
use crate::llm::resolve::ResolvedModel;
use crate::llm::types::{AssistantPart, CompletionResponse, Message, Part, ToolChoice, Usage};

pub struct Gemini;

impl Dialect for Gemini {
    fn system_style(&self) -> SystemStyle {
        SystemStyle::Field
    }

    fn path(&self, m: &ResolvedModel) -> String {
        format!("/models/{}:generateContent", m.model)
    }

    fn encode(&self, req: &PreparedRequest, m: &ResolvedModel) -> Value {
        let mut body = json!({"contents": encode_contents(&req.messages)});
        if let Some(system) = req.system.as_ref().filter(|s| !s.is_empty()) {
            body["systemInstruction"] = json!({"role": "model", "parts": [text_part(system)]});
        }
        if !req.tools.is_empty() {
            let declarations: Vec<Value> = req
                .tools
                .iter()
                .map(|t| {
                    let mut decl = json!({"name": t.name, "description": t.description});
                    if let Some(schema) = tool_parameters_to_schema(&t.parameters) {
                        decl["parameters"] = schema;
                    }
                    decl
                })
                .collect();
            body["tools"] = json!([{"functionDeclarations": declarations}]);
        }
        if let Some(choice) = req.tool_choice {
            body["toolConfig"] = json!({"functionCallingConfig": {"mode": match choice {
                ToolChoice::Auto => "AUTO",
                ToolChoice::None => "NONE",
            }}});
        }
        if let Some(max) = req.max_tokens.or(m.max_tokens) {
            body["generationConfig"] = json!({"maxOutputTokens": max});
        }
        body
    }

    fn decode(&self, body: Value) -> Result<CompletionResponse, LlmError> {
        if let Some(err) = super::error_in_body(&body) {
            return Err(LlmError::Decode(err));
        }
        let candidate = body
            .pointer("/candidates/0")
            .ok_or_else(|| LlmError::Decode("no response candidates in response".into()))?;

        let reason = candidate.get("finishReason").and_then(Value::as_str);
        let finish_message = candidate
            .get("finishMessage")
            .and_then(Value::as_str)
            .unwrap_or("no finish message provided");
        if let Some(reason) = reason
            && matches!(
                reason,
                "MALFORMED_FUNCTION_CALL"
                    | "UNEXPECTED_TOOL_CALL"
                    | "MISSING_THOUGHT_SIGNATURE"
                    | "TOO_MANY_TOOL_CALLS"
                    | "MALFORMED_RESPONSE"
            )
        {
            return Err(LlmError::Decode(format!(
                "Gemini stopped with finish_reason={reason}: {finish_message}"
            )));
        }

        let parts = candidate
            .pointer("/content/parts")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                LlmError::Decode(format!(
                    "Gemini candidate missing content (finish_reason={}, finish_message={finish_message})",
                    reason.unwrap_or("<unknown>")
                ))
            })?;

        let mut content = vec![];
        for part in parts {
            let signature = part
                .get("thoughtSignature")
                .and_then(Value::as_str)
                .map(|s| json!({"thought_signature": s}));
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                if part.get("thought").and_then(Value::as_bool) == Some(true) {
                    content.push(AssistantPart::Reasoning {
                        text: text.to_string(),
                        provider_data: signature,
                    });
                } else {
                    content.push(AssistantPart::Text(text.to_string()));
                }
            } else if let Some(call) = part.get("functionCall") {
                let name = call
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                content.push(AssistantPart::ToolCall {
                    // Gemini names results by function, so the call id is the function name.
                    id: name.clone(),
                    name,
                    arguments: call.get("args").cloned().unwrap_or_else(|| json!({})),
                    provider_data: signature,
                });
            } else if part.get("inlineData").is_some() {
                tracing::debug!("gemini: dropping an inline data part from the reply");
            } else {
                return Err(LlmError::Decode(
                    "response did not contain a message or tool call".into(),
                ));
            }
        }
        if content.is_empty() {
            return Err(LlmError::Decode(
                "response contained no message or tool call (empty)".into(),
            ));
        }

        let u = body.get("usageMetadata");
        let get = |k: &str| {
            u.and_then(|u| u.get(k))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        Ok(CompletionResponse {
            id: None,
            content,
            usage: Usage {
                input_tokens: get("promptTokenCount"),
                output_tokens: get("candidatesTokenCount"),
                total_tokens: get("totalTokenCount"),
                cached_input_tokens: get("cachedContentTokenCount"),
                cache_creation_input_tokens: 0,
            },
        })
    }

    fn list_models_path(&self, _m: &ResolvedModel) -> Option<String> {
        Some("/models".into())
    }

    fn decode_models(&self, body: Value) -> Vec<ListedModel> {
        body.get("models")
            .and_then(Value::as_array)
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| {
                        let name = m.get("name")?.as_str()?;
                        Some(ListedModel {
                            id: name.strip_prefix("models/").unwrap_or(name).to_string(),
                            context: m.get("inputTokenLimit").and_then(Value::as_u64),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn text_part(text: &str) -> Value {
    json!({"thought": false, "text": text})
}

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn encode_user_part(part: &Part) -> Value {
    match part {
        Part::Text(text) => text_part(text),
        Part::Attachment { mime, data, .. } => {
            json!({"thought": false, "inlineData": {"mimeType": mime, "data": b64(data)}})
        }
    }
}

fn encode_assistant_part(part: &AssistantPart) -> Value {
    let signature = |pd: &Option<Value>| {
        pd.as_ref()
            .and_then(|p| p.get("thought_signature"))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    match part {
        AssistantPart::Text(text) => text_part(text),
        AssistantPart::ToolCall {
            name,
            arguments,
            provider_data,
            ..
        } => {
            let mut p = json!({"thought": false, "functionCall": {"name": name, "args": arguments}});
            if let Some(sig) = signature(provider_data) {
                p["thoughtSignature"] = json!(sig);
            }
            p
        }
        AssistantPart::Reasoning {
            text,
            provider_data,
        } => {
            let mut p = json!({"thought": true, "text": text});
            if let Some(sig) = signature(provider_data) {
                p["thoughtSignature"] = json!(sig);
            }
            p
        }
    }
}

/// A tool result's response: the text parsed as JSON when it is JSON, wrapped as
/// `{"result": …}`; a second text part lands under `text`, as rig 0.39 did.
fn function_response(call_id: &str, content: &[Part]) -> Value {
    let mut response: Option<Value> = None;
    let mut parts = vec![];
    for part in content {
        match part {
            Part::Text(text) => {
                let result = serde_json::from_str::<Value>(text).unwrap_or_else(|_| json!(text));
                response = Some(match response {
                    Some(mut existing) => {
                        if let Value::Object(map) = &mut existing {
                            map.insert("text".into(), result);
                        }
                        existing
                    }
                    None => json!({"result": result}),
                });
            }
            Part::Attachment { mime, data, .. } => {
                parts.push(json!({"inlineData": {"mimeType": mime, "data": b64(data)}}))
            }
        }
    }
    let mut fr = json!({"name": call_id});
    if let Some(response) = response {
        fr["response"] = response;
    }
    if !parts.is_empty() {
        fr["parts"] = Value::Array(parts);
    }
    json!({"thought": false, "functionResponse": fr})
}

/// Consecutive tool results go into one user turn of `functionResponse` parts.
fn encode_contents(messages: &[Message]) -> Vec<Value> {
    let mut out = vec![];
    let mut results = vec![];
    let flush = |out: &mut Vec<Value>, results: &mut Vec<Value>| {
        if !results.is_empty() {
            out.push(json!({"role": "user", "parts": std::mem::take(results)}));
        }
    };
    for message in messages {
        match message {
            Message::ToolResult { call_id, content } => {
                results.push(function_response(call_id, content))
            }
            Message::User(parts) => {
                flush(&mut out, &mut results);
                out.push(json!({
                    "role": "user",
                    "parts": parts.iter().map(encode_user_part).collect::<Vec<_>>(),
                }));
            }
            Message::Assistant(parts) => {
                flush(&mut out, &mut results);
                if !parts.is_empty() {
                    out.push(json!({
                        "role": "model",
                        "parts": parts.iter().map(encode_assistant_part).collect::<Vec<_>>(),
                    }));
                }
            }
            Message::System(_) => {}
        }
    }
    flush(&mut out, &mut results);
    out
}

// Tool schema sanitising, as rig 0.39 `gemini_api_types::Schema` -------------------------------

/// `None` for a tool without parameters (Gemini rejects an empty object schema).
pub fn tool_parameters_to_schema(parameters: &Value) -> Option<Value> {
    if parameters.is_null() || *parameters == json!({"type": "object", "properties": {}}) {
        return None;
    }
    to_schema(flatten_schema(parameters.clone()))
}

/// Inline `$ref`s to `$defs`/`definitions`, then drop the definitions.
fn flatten_schema(mut schema: Value) -> Value {
    let defs = schema
        .as_object()
        .and_then(|o| o.get("$defs").or_else(|| o.get("definitions")))
        .and_then(Value::as_object)
        .cloned();
    let Some(defs) = defs else {
        return schema;
    };
    resolve_refs(&mut schema, &defs, 0);
    if let Some(obj) = schema.as_object_mut() {
        obj.remove("$defs");
        obj.remove("definitions");
    }
    schema
}

fn resolve_refs(value: &mut Value, defs: &Map<String, Value>, depth: usize) {
    if depth > 32 {
        return;
    }
    match value {
        Value::Object(obj) => {
            if let Some(name) = obj
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| {
                    r.strip_prefix("#/$defs/")
                        .or_else(|| r.strip_prefix("#/definitions/"))
                })
                && let Some(def) = defs.get(name)
            {
                let mut resolved = def.clone();
                resolve_refs(&mut resolved, defs, depth + 1);
                *value = resolved;
                return;
            }
            for v in obj.values_mut() {
                resolve_refs(v, defs, depth + 1);
            }
        }
        Value::Array(items) => {
            for item in items {
                resolve_refs(item, defs, depth + 1);
            }
        }
        _ => {}
    }
}

fn extract_type(type_value: &Value) -> Option<String> {
    if let Some(t) = type_value.as_str() {
        return Some(t.to_string());
    }
    type_value.as_array().and_then(|arr| {
        arr.iter()
            .filter_map(Value::as_str)
            .find(|t| *t != "null")
            .or_else(|| arr.iter().find_map(Value::as_str))
            .map(str::to_owned)
    })
}

fn schema_is_null(obj: &Map<String, Value>) -> bool {
    obj.get("type").and_then(extract_type).as_deref() == Some("null")
}

fn schema_is_nullable(obj: &Map<String, Value>) -> bool {
    obj.get("nullable").and_then(Value::as_bool).unwrap_or(false)
        || obj
            .get("type")
            .and_then(Value::as_array)
            .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("null")))
        || ["anyOf", "oneOf", "allOf"].iter().any(|key| {
            obj.get(*key).and_then(Value::as_array).is_some_and(|arr| {
                arr.iter()
                    .filter_map(Value::as_object)
                    .any(schema_is_null)
            })
        })
}

fn extract_type_from_composition(composition: &Value) -> Option<String> {
    composition.as_array()?.iter().find_map(|schema| {
        let obj = schema.as_object()?;
        if schema_is_null(obj) {
            return None;
        }
        obj.get("type").and_then(extract_type).or_else(|| {
            if obj.contains_key("properties") {
                Some("object".into())
            } else if obj.contains_key("enum") {
                Some("string".into())
            } else {
                None
            }
        })
    })
}

fn extract_schema_from_composition(composition: &Value) -> Option<Map<String, Value>> {
    composition.as_array()?.iter().find_map(|schema| {
        let obj = schema.as_object()?;
        (!schema_is_null(obj)).then(|| obj.clone())
    })
}

fn infer_type(obj: &Map<String, Value>) -> String {
    if let Some(t) = obj.get("type").and_then(extract_type) {
        return t;
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(t) = obj.get(key).and_then(extract_type_from_composition) {
            return t;
        }
    }
    if obj.contains_key("properties") {
        "object".into()
    } else if obj.contains_key("enum") {
        "string".into()
    } else {
        String::new()
    }
}

/// One Gemini `Schema` object: only `type`, `format`, `description`, `nullable`, `enum`,
/// `maxItems`, `minItems`, `properties`, `required` and `items` survive.
fn to_schema(value: Value) -> Option<Value> {
    let value = flatten_schema(value);
    let obj = value.as_object()?;

    let composition = ["anyOf", "oneOf", "allOf"]
        .iter()
        .find_map(|k| obj.get(*k).and_then(extract_schema_from_composition));
    let props_source = if obj.get("properties").is_none() {
        composition.clone().unwrap_or_else(|| obj.clone())
    } else {
        obj.clone()
    };

    let schema_type = infer_type(obj);
    let mut out = Map::new();
    out.insert("type".into(), json!(schema_type));

    let pick_str = |k: &str| {
        obj.get(k)
            .or_else(|| props_source.get(k))
            .and_then(Value::as_str)
            .map(|s| json!(s))
    };
    if let Some(v) = pick_str("format") {
        out.insert("format".into(), v);
    }
    if let Some(v) = pick_str("description") {
        out.insert("description".into(), v);
    }
    if schema_is_nullable(obj) || composition.as_ref().is_some_and(schema_is_nullable) {
        out.insert("nullable".into(), json!(true));
    }
    if let Some(values) = obj
        .get("enum")
        .or_else(|| props_source.get("enum"))
        .and_then(Value::as_array)
    {
        let values: Vec<Value> = values
            .iter()
            .filter_map(|v| v.as_str().map(|s| json!(s)))
            .collect();
        out.insert("enum".into(), Value::Array(values));
    }
    for (src, dst) in [("maxItems", "maxItems"), ("minItems", "minItems")] {
        if let Some(n) = obj.get(src).and_then(Value::as_i64) {
            out.insert(dst.into(), json!(n));
        }
    }
    if let Some(props) = props_source.get("properties").and_then(Value::as_object) {
        let props: Map<String, Value> = props
            .iter()
            .filter_map(|(k, v)| to_schema(v.clone()).map(|s| (k.clone(), s)))
            .collect();
        out.insert("properties".into(), Value::Object(props));
    }
    if let Some(required) = props_source.get("required").and_then(Value::as_array) {
        let required: Vec<Value> = required
            .iter()
            .filter_map(|v| v.as_str().map(|s| json!(s)))
            .collect();
        out.insert("required".into(), Value::Array(required));
    }
    let items = obj
        .get("items")
        .or_else(|| props_source.get("items"))
        .and_then(|v| to_schema(v.clone()));
    match items {
        Some(items) => {
            out.insert("items".into(), items);
        }
        // Gemini requires `items` on arrays.
        None if schema_type == "array" => {
            out.insert("items".into(), json!({"type": "string"}));
        }
        None => {}
    }
    Some(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::prepare::prepare;
    use crate::llm::types::{CompletionRequest, ToolDefinition};

    #[test]
    fn sanitises_a_real_mcp_tool_schema() {
        // The input schema of the GitHub MCP server's `create_issue` tool, as rmcp lists it.
        let schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "owner": {"type": "string", "description": "Repository owner"},
                "repo": {"type": "string", "description": "Repository name"},
                "title": {"type": "string", "description": "Issue title"},
                "body": {"type": ["string", "null"], "description": "Issue body content"},
                "labels": {"type": "array", "description": "Labels to apply", "items": {"type": "string"}},
                "assignees": {"type": "array", "description": "Usernames to assign"},
                "milestone": {"anyOf": [{"$ref": "#/$defs/Milestone"}, {"type": "null"}]},
                "type": {"type": "string", "format": "uri-reference", "enum": ["bug", "feature"]}
            },
            "required": ["owner", "repo", "title"],
            "$defs": {"Milestone": {"type": "object", "properties": {"number": {"type": "integer", "minimum": 1}}, "required": ["number"]}}
        });
        let out = tool_parameters_to_schema(&schema).unwrap();
        assert_eq!(
            out,
            json!({
                "type": "object",
                "properties": {
                    "owner": {"type": "string", "description": "Repository owner"},
                    "repo": {"type": "string", "description": "Repository name"},
                    "title": {"type": "string", "description": "Issue title"},
                    "body": {"type": "string", "description": "Issue body content", "nullable": true},
                    "labels": {"type": "array", "description": "Labels to apply", "items": {"type": "string"}},
                    "assignees": {"type": "array", "description": "Usernames to assign", "items": {"type": "string"}},
                    "milestone": {"type": "object", "nullable": true,
                                  "properties": {"number": {"type": "integer"}}, "required": ["number"]},
                    "type": {"type": "string", "format": "uri-reference", "enum": ["bug", "feature"]}
                },
                "required": ["owner", "repo", "title"]
            })
        );
    }

    #[test]
    fn an_empty_object_schema_has_no_parameters() {
        assert_eq!(
            tool_parameters_to_schema(&json!({"type": "object", "properties": {}})),
            None
        );
    }

    #[test]
    fn encodes_function_calls_results_and_signatures() {
        let m = ResolvedModel::test("gemini-x");
        let req = CompletionRequest {
            messages: vec![
                Message::system("sys"),
                Message::user_text("go").unwrap(),
                Message::Assistant(vec![AssistantPart::ToolCall {
                    id: "f".into(),
                    name: "f".into(),
                    arguments: json!({"a": 1}),
                    provider_data: Some(json!({"thought_signature": "sig"})),
                }]),
                Message::tool_result("f", r#"{"ok": true}"#),
                Message::tool_result("f", "plain"),
            ],
            tools: vec![ToolDefinition {
                name: "f".into(),
                description: "d".into(),
                parameters: json!({"type": "object", "properties": {"a": {"type": "integer"}}}),
            }],
            tool_choice: Some(ToolChoice::None),
            max_tokens: None,
        };
        let body = Gemini.encode(&prepare(req, &m, SystemStyle::Field), &m);
        assert_eq!(
            body["systemInstruction"],
            json!({"role": "model", "parts": [{"thought": false, "text": "sys"}]})
        );
        assert_eq!(body["contents"][1]["parts"][0]["thoughtSignature"], "sig");
        assert_eq!(
            body["contents"][2]["parts"],
            json!([
                {"thought": false, "functionResponse": {"name": "f", "response": {"result": {"ok": true}}}},
                {"thought": false, "functionResponse": {"name": "f", "response": {"result": "plain"}}}
            ])
        );
        assert_eq!(body["toolConfig"]["functionCallingConfig"]["mode"], "NONE");
        assert!(body.get("generationConfig").is_none());
    }

    #[test]
    fn decodes_thoughts_calls_and_usage() {
        let r = Gemini
            .decode(json!({
                "candidates": [{"content": {"role": "model", "parts": [
                    {"text": "thinking", "thought": true, "thoughtSignature": "s"},
                    {"functionCall": {"name": "f", "args": {"x": 1}}, "thoughtSignature": "s2"},
                    {"text": "done"}
                ]}, "finishReason": "STOP"}],
                "usageMetadata": {"promptTokenCount": 9, "candidatesTokenCount": 3, "totalTokenCount": 15, "cachedContentTokenCount": 4}
            }))
            .unwrap();
        assert_eq!(r.content.len(), 3);
        assert_eq!(
            r.content[1],
            AssistantPart::ToolCall {
                id: "f".into(),
                name: "f".into(),
                arguments: json!({"x": 1}),
                provider_data: Some(json!({"thought_signature": "s2"}))
            }
        );
        assert_eq!(r.usage.cached_input_tokens, 4);
        assert_eq!(r.usage.total_tokens, 15);

        let err = Gemini
            .decode(json!({"candidates": [{"finishReason": "MALFORMED_FUNCTION_CALL"}]}))
            .unwrap_err();
        assert!(matches!(err, LlmError::Decode(m) if m.contains("MALFORMED_FUNCTION_CALL")));
    }
}
