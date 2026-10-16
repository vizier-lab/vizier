//! Golden fixtures recorded from rig 0.39 (`golden_capture.rs`): our `encode` must send what
//! rig sent, and our `decode` must read what rig read, for every dialect and scenario.
//!
//! Differences we chose on purpose are applied to rig's side before comparing, each in its
//! own normaliser below, with the reason. Everything else must match exactly. `null` and an
//! absent field are treated alike on both sides.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::anthropic::Anthropic;
use super::gemini::Gemini;
use super::ollama::Ollama;
use super::openai_chat::OpenAiChat;
use super::openai_responses::OpenAiResponses;
use super::{BodyKind, Dialect, DialectKind};
use crate::catalogue::data::Capabilities;
use crate::llm::prepare::prepare;
use crate::llm::resolve::ResolvedModel;
use crate::llm::types::{AssistantPart, CompletionRequest, CompletionResponse};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm/dialect/fixtures/golden")
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn strip_nulls(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            map.values_mut().for_each(strip_nulls);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_nulls),
        _ => {}
    }
}

fn dialect_for(target: &str) -> (DialectKind, Box<dyn Dialect>) {
    match target {
        "openai_chat" | "deepseek" => (DialectKind::OpenAiChat, Box::new(OpenAiChat)),
        "anthropic" => (DialectKind::Anthropic, Box::new(Anthropic)),
        "gemini" => (DialectKind::Gemini, Box::new(Gemini)),
        "ollama" => (DialectKind::Ollama, Box::new(Ollama)),
        "openai_responses" | "chatgpt" => (DialectKind::OpenAiResponses, Box::new(OpenAiResponses)),
        other => panic!("unknown golden target {other}"),
    }
}

fn model(settings: &Value, dialect: DialectKind) -> ResolvedModel {
    ResolvedModel {
        model: settings["model"].as_str().unwrap().to_string(),
        dialect,
        base_url: settings["base_url"].as_str().unwrap().to_string(),
        max_output: settings["max_output"].as_u64(),
        stream_required: settings["stream_required"].as_bool().unwrap_or(false),
        default_instructions: settings["default_instructions"].as_str().map(str::to_string),
        caps: Capabilities {
            reasoning: settings["reasoning"].as_bool(),
            ..Default::default()
        },
        ..ResolvedModel::test("unused")
    }
}

// Normalisers: rig 0.39's body → what we send, by documented decision. ------------------------

/// OpenAI chat: system and assistant text go as plain strings, not part arrays, because
/// several OpenAI-compatible servers (DeepSeek among them) accept only strings.
fn chat_string_content(body: &mut Value) {
    for m in body["messages"].as_array_mut().into_iter().flatten() {
        let role = m["role"].as_str().unwrap_or_default().to_string();
        if (role == "system" || role == "assistant")
            && let Some(parts) = m["content"].as_array()
        {
            let text: String = parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .concat();
            m["content"] = json!(text);
        }
        // An empty assistant text beside tool calls is omitted, as OpenAI's own client does.
        if role == "assistant" && m["content"] == json!("") && m.get("tool_calls").is_some() {
            m.as_object_mut().unwrap().remove("content");
        }
        // rig's DeepSeek client numbered every call `index: 0`; the index is a streaming field.
        for call in m
            .get_mut("tool_calls")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            call.as_object_mut().unwrap().remove("index");
        }
    }
}

/// Anthropic: one joined system block, and block-level cache breakpoints (last tool, system,
/// last message block) instead of rig's top-level automatic `cache_control`, since block-level
/// breakpoints work on every Anthropic-compatible endpoint.
fn anthropic_caching(body: &mut Value) {
    let obj = body.as_object_mut().unwrap();
    obj.remove("cache_control");
    if let Some(blocks) = obj.get("system").and_then(Value::as_array).cloned() {
        let text = blocks
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        obj.insert(
            "system".into(),
            json!([{"type": "text", "text": text, "cache_control": {"type": "ephemeral"}}]),
        );
    }
    if let Some(last) = obj.get_mut("tools").and_then(Value::as_array_mut).and_then(|t| t.last_mut()) {
        last["cache_control"] = json!({"type": "ephemeral"});
    }
    if let Some(block) = obj
        .get_mut("messages")
        .and_then(Value::as_array_mut)
        .and_then(|m| m.last_mut())
        .and_then(|m| m["content"].as_array_mut())
        .and_then(|c| c.last_mut())
        && matches!(
            block["type"].as_str(),
            Some("text" | "image" | "tool_result" | "document")
        )
    {
        block["cache_control"] = json!({"type": "ephemeral"});
    }
}

/// Gemini: one joined system part; and `max_tokens` is sent as `maxOutputTokens` (rig 0.39 only
/// sent it when a generation config was already present, so it was silently dropped).
fn gemini_system_and_max(body: &mut Value, req: &CompletionRequest) {
    if let Some(parts) = body["systemInstruction"]["parts"].as_array().cloned() {
        let text = parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        body["systemInstruction"]["parts"] = json!([{"thought": false, "text": text}]);
    }
    if let Some(max) = req.max_tokens {
        body["generationConfig"] = json!({"maxOutputTokens": max});
    }
}

/// Ollama: the output cap is `options.num_predict` (rig 0.39 sent a top-level `max_tokens`,
/// which Ollama ignores).
fn ollama_num_predict(body: &mut Value) {
    if let Some(max) = body.as_object_mut().unwrap().remove("max_tokens") {
        body["options"]["num_predict"] = max;
    }
    if body.get("options").is_none() {
        body["options"] = json!({});
    }
}

/// Responses: leading system messages are `instructions` and later ones developer messages,
/// rather than system input items; `store: false` is always sent; and a function call carries
/// only its `call_id`, since with nothing stored an item id can't be resolved (contract S5).
fn responses_items(body: &mut Value, chatgpt: bool) {
    let obj = body.as_object_mut().unwrap();
    let input = obj.get_mut("input").and_then(Value::as_array_mut).unwrap();
    let mut leading = vec![];
    while input
        .first()
        .is_some_and(|i| i["role"] == "system" && i["type"] == "message")
    {
        let item = input.remove(0);
        leading.extend(
            item["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["text"].as_str().map(str::to_string)),
        );
    }
    for item in input.iter_mut() {
        if item["role"] == "system" {
            item["role"] = json!("developer");
        }
        if item["type"] == "function_call" {
            item.as_object_mut().unwrap().remove("id");
        }
    }
    if !chatgpt && !leading.is_empty() {
        obj.insert("instructions".into(), json!(leading.join("\n\n")));
    }
    obj.insert("store".into(), json!(false));
}

/// DeepSeek: `tool_choice` is sent (rig 0.39's DeepSeek client dropped it, so an answer-only
/// completion could still call tools). DeepSeek accepts `"none"` like OpenAI.
fn deepseek_tool_choice(body: &mut Value, req: &CompletionRequest) {
    if req.tool_choice == Some(crate::llm::types::ToolChoice::None) {
        body["tool_choice"] = json!("none");
    }
}

fn normalise_rig(target: &str, mut body: Value, req: &CompletionRequest) -> Value {
    strip_nulls(&mut body);
    match target {
        "openai_chat" => chat_string_content(&mut body),
        "deepseek" => {
            chat_string_content(&mut body);
            deepseek_tool_choice(&mut body, req);
        }
        "anthropic" => anthropic_caching(&mut body),
        "gemini" => gemini_system_and_max(&mut body, req),
        "ollama" => ollama_num_predict(&mut body),
        "openai_responses" => responses_items(&mut body, false),
        "chatgpt" => responses_items(&mut body, true),
        _ => {}
    }
    body
}

/// Parts order: we keep each provider's own order, except that reasoning is compared apart
/// from its position (rig 0.39's DeepSeek client appended it last; it is never persisted).
///
/// The response's id, model and finish reason are not rig's: rig kept only the Responses
/// API's message item id and nothing else, while we read what each provider reports, for
/// telemetry. They are checked on their own by `every_golden_response_reports_its_metadata`.
fn normalise_parsed(mut r: CompletionResponse) -> Value {
    r.content
        .sort_by_key(|p| !matches!(p, AssistantPart::Reasoning { .. }));
    r.id = None;
    r.model = None;
    r.finish_reason = None;
    serde_json::to_value(r).unwrap()
}

fn golden_cases() -> Vec<(String, String)> {
    let mut cases = vec![];
    for entry in std::fs::read_dir(root()).unwrap() {
        let dir = entry.unwrap().path();
        let target = dir.file_name().unwrap().to_string_lossy().to_string();
        for file in std::fs::read_dir(&dir).unwrap() {
            let name = file.unwrap().file_name().to_string_lossy().to_string();
            if let Some(scenario) = name.strip_suffix(".input.json") {
                cases.push((target.clone(), scenario.to_string()));
            }
        }
    }
    cases.sort();
    cases
}

#[test]
fn every_golden_request_matches_rig() {
    let cases = golden_cases();
    assert!(cases.len() >= 60, "only {} golden cases found", cases.len());
    let mut failures = vec![];
    for (target, scenario) in &cases {
        let base = root().join(target);
        let input = read(&base.join(format!("{scenario}.input.json")));
        let recorded = read(&base.join(format!("{scenario}.request.json")));
        let req: CompletionRequest = serde_json::from_value(input["request"].clone()).unwrap();
        let (kind, dialect) = dialect_for(target);
        let m = model(&input["settings"], kind);

        let path = dialect.path(&m);
        if path != recorded["path"].as_str().unwrap() {
            failures.push(format!("{target}/{scenario}: path {path} != {}", recorded["path"]));
        }

        let mut ours = dialect.encode(&prepare(req.clone(), &m, dialect.system_style()), &m);
        strip_nulls(&mut ours);
        let expected = normalise_rig(target, recorded["body"].clone(), &req);
        if ours != expected {
            failures.push(format!(
                "{target}/{scenario}: request differs\n ours: {}\n  rig: {}",
                serde_json::to_string(&ours).unwrap(),
                serde_json::to_string(&expected).unwrap()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn every_golden_response_decodes_like_rig() {
    let mut failures = vec![];
    for (target, scenario) in golden_cases() {
        let base = root().join(&target);
        let input = read(&base.join(format!("{scenario}.input.json")));
        let response = read(&base.join(format!("{scenario}.response.json")));
        let parsed = read(&base.join(format!("{scenario}.parsed.json")));
        let (kind, dialect) = dialect_for(&target);
        let m = model(&input["settings"], kind);

        let body = match dialect.read_body(&m) {
            BodyKind::Json => response,
            BodyKind::Sse => dialect
                .aggregate_sse(response["sse"].as_str().unwrap())
                .unwrap(),
        };
        let ours = normalise_parsed(dialect.decode(body).unwrap());
        let rig = normalise_parsed(serde_json::from_value(parsed).unwrap());
        if ours != rig {
            failures.push(format!(
                "{target}/{scenario}: decode differs\n ours: {ours}\n  rig: {rig}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every recorded reply gives its id and model where the provider has them, and a finish
/// reason in the GenAI conventions' terms. None of the recordings was cut short, so each is
/// `tool_call` or `stop`; the other values are `dialect::finish_reason`'s unit tests.
#[test]
fn every_golden_response_reports_its_metadata() {
    let mut failures = vec![];
    for (target, scenario) in golden_cases() {
        let base = root().join(&target);
        let input = read(&base.join(format!("{scenario}.input.json")));
        let response = read(&base.join(format!("{scenario}.response.json")));
        let (kind, dialect) = dialect_for(&target);
        let m = model(&input["settings"], kind);

        let body = match dialect.read_body(&m) {
            BodyKind::Json => response,
            BodyKind::Sse => dialect
                .aggregate_sse(response["sse"].as_str().unwrap())
                .unwrap(),
        };
        let field = |key: &str| body.get(key).and_then(Value::as_str).map(str::to_string);
        let (id, model) = match target.as_str() {
            "gemini" => (field("responseId"), field("modelVersion")),
            "ollama" => (None, field("model")),
            _ => (field("id"), field("model")),
        };
        let r = dialect.decode(body.clone()).unwrap();
        let calls_a_tool = r
            .content
            .iter()
            .any(|p| matches!(p, AssistantPart::ToolCall { .. }));
        let finish = if calls_a_tool { "tool_call" } else { "stop" };

        if id.is_none() && target != "ollama" {
            failures.push(format!("{target}/{scenario}: the recording has no id"));
        }
        if r.id != id || r.model != model || r.finish_reason.as_deref() != Some(finish) {
            failures.push(format!(
                "{target}/{scenario}: got id {:?}, model {:?}, finish {:?}; expected {id:?}, {model:?}, {finish:?}",
                r.id, r.model, r.finish_reason
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
