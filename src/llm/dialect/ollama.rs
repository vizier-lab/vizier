//! Ollama's native chat API (`/api/chat`). Ported from rig-core 0.39 `providers/ollama.rs`.

use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};

use super::{Dialect, PreparedRequest, SystemStyle, join_url};
use crate::llm::error::LlmError;
use crate::llm::http;
use crate::llm::model::ListedModel;
use crate::llm::resolve::ResolvedModel;
use crate::llm::types::{AssistantPart, CompletionResponse, Message, Part, Usage};

pub struct Ollama;

impl Dialect for Ollama {
    fn system_style(&self) -> SystemStyle {
        SystemStyle::Inline
    }

    fn path(&self, _m: &ResolvedModel) -> String {
        "/api/chat".into()
    }

    fn encode(&self, req: &PreparedRequest, m: &ResolvedModel) -> Value {
        if req.tool_choice.is_some() {
            tracing::debug!("ollama: tool_choice is not supported, sending tools as they are");
        }
        let mut options = json!({});
        if let Some(max) = req.max_tokens.or(m.max_tokens) {
            options["num_predict"] = json!(max);
        }
        let mut body = json!({
            "model": m.model,
            "messages": req.messages.iter().map(encode_message).collect::<Vec<_>>(),
            "stream": false,
            "think": false,
            "options": options,
        });
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(
                req.tools
                    .iter()
                    .map(|t| {
                        json!({"type": "function", "function": {
                            "name": t.name, "description": t.description, "parameters": t.parameters
                        }})
                    })
                    .collect(),
            );
        }
        body
    }

    fn decode(&self, body: Value) -> Result<CompletionResponse, LlmError> {
        if let Some(err) = body.get("error").and_then(Value::as_str) {
            return Err(LlmError::Decode(err.to_string()));
        }
        let message = body
            .get("message")
            .filter(|m| m.get("role").and_then(Value::as_str) == Some("assistant"))
            .ok_or_else(|| {
                LlmError::Decode("chat response does not include an assistant message".into())
            })?;

        let mut content = vec![];
        if let Some(thinking) = message
            .get("thinking")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
        {
            content.push(AssistantPart::Reasoning {
                text: thinking.to_string(),
                provider_data: None,
            });
        }
        if let Some(text) = message
            .get("content")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
        {
            content.push(AssistantPart::Text(text.to_string()));
        }
        for call in message
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = call
                .pointer("/function/name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            content.push(AssistantPart::ToolCall {
                // Ollama answers tool results by `tool_name`, so the call id is the name.
                id: name.clone(),
                name,
                arguments: super::openai_chat::parse_arguments(call.pointer("/function/arguments")),
                provider_data: None,
            });
        }
        if content.is_empty() {
            return Err(LlmError::Decode("no content provided".into()));
        }

        let prompt = body
            .get("prompt_eval_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let completion = body.get("eval_count").and_then(Value::as_u64).unwrap_or(0);
        let finish_reason =
            super::finish_reason(body.get("done_reason").and_then(Value::as_str), &content);
        Ok(CompletionResponse {
            // Ollama gives no response id.
            id: None,
            model: super::str_field(&body, "model"),
            finish_reason,
            content,
            usage: Usage {
                input_tokens: prompt,
                output_tokens: completion,
                total_tokens: prompt + completion,
                ..Usage::new()
            },
        })
    }

    fn list_models_path(&self, _m: &ResolvedModel) -> Option<String> {
        Some("/api/tags".into())
    }

    fn decode_models(&self, body: Value) -> Vec<ListedModel> {
        body.get("models")
            .and_then(Value::as_array)
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| {
                        let id = m.get("model").or_else(|| m.get("name"))?.as_str()?;
                        Some(ListedModel {
                            id: id.to_string(),
                            context: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn text_of(parts: &[Part], sep: &str) -> (String, Vec<String>) {
    let mut texts = vec![];
    let mut images = vec![];
    for part in parts {
        match part {
            Part::Text(t) => texts.push(t.clone()),
            Part::Attachment { mime, data, .. } if mime.starts_with("image/") => {
                images.push(base64::engine::general_purpose::STANDARD.encode(data))
            }
            Part::Attachment { name, .. } => texts.push(format!(
                "[attachment {} omitted: not supported by Ollama]",
                name.as_deref().unwrap_or("file")
            )),
        }
    }
    (texts.join(sep), images)
}

fn encode_message(message: &Message) -> Value {
    match message {
        Message::System(text) => json!({"role": "system", "content": text}),
        Message::User(parts) => {
            let (content, images) = text_of(parts, " ");
            let mut msg = json!({"role": "user", "content": content});
            if !images.is_empty() {
                msg["images"] = json!(images);
            }
            msg
        }
        Message::Assistant(parts) => {
            let mut texts = vec![];
            let mut thinking = None;
            let mut calls = vec![];
            for part in parts {
                match part {
                    AssistantPart::Text(t) => texts.push(t.clone()),
                    AssistantPart::Reasoning { text, .. } if !text.is_empty() => {
                        thinking = Some(text.clone())
                    }
                    AssistantPart::Reasoning { .. } => {}
                    AssistantPart::ToolCall {
                        name, arguments, ..
                    } => calls.push(json!({
                        "type": "function",
                        "function": {"name": name, "arguments": arguments},
                    })),
                }
            }
            let mut msg = json!({"role": "assistant", "content": texts.join(" "), "tool_calls": calls});
            if let Some(thinking) = thinking {
                msg["thinking"] = json!(thinking);
            }
            msg
        }
        Message::ToolResult { call_id, content } => {
            let (content, _) = text_of(content, "\n");
            json!({"role": "tool", "tool_name": call_id, "content": content})
        }
    }
}

/// The model's context length from `POST /api/show` (`model_info.<arch>.context_length`).
pub async fn show_context(http: &reqwest::Client, base_url: &str, model: &str) -> Option<u64> {
    let req = http
        .post(join_url(base_url, "/api/show"))
        .json(&json!({"model": model}))
        .timeout(Duration::from_secs(5));
    let body = http::send_json(req).await.ok()?;
    body.get("model_info")?
        .as_object()?
        .iter()
        .find(|(k, _)| k.ends_with(".context_length"))
        .and_then(|(_, v)| v.as_u64())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::prepare::prepare;
    use crate::llm::types::CompletionRequest;

    #[test]
    fn encodes_images_tool_calls_and_results() {
        let m = ResolvedModel::test("llama3.2");
        let req = CompletionRequest {
            messages: vec![
                Message::system("sys"),
                Message::User(vec![
                    Part::Text("see".into()),
                    Part::Attachment {
                        mime: "image/png".into(),
                        data: vec![1, 2, 3],
                        name: None,
                    },
                ]),
                Message::Assistant(vec![AssistantPart::ToolCall {
                    id: "f".into(),
                    name: "f".into(),
                    arguments: json!({"a": 1}),
                    provider_data: None,
                }]),
                Message::tool_result("f", "done"),
            ],
            max_tokens: Some(7),
            ..Default::default()
        };
        let body = Ollama.encode(&prepare(req, &m, SystemStyle::Inline), &m);
        assert_eq!(body["messages"][1], json!({"role": "user", "content": "see", "images": ["AQID"]}));
        assert_eq!(
            body["messages"][2]["tool_calls"][0],
            json!({"type": "function", "function": {"name": "f", "arguments": {"a": 1}}})
        );
        assert_eq!(
            body["messages"][3],
            json!({"role": "tool", "tool_name": "f", "content": "done"})
        );
        assert_eq!(body["stream"], false);
        assert_eq!(body["options"]["num_predict"], 7);
    }

    #[test]
    fn decodes_tool_calls_with_name_ids_and_usage() {
        let r = Ollama
            .decode(json!({
                "model": "m", "done": true,
                "message": {"role": "assistant", "content": "",
                            "tool_calls": [{"function": {"name": "f", "arguments": {"a": 1}}}]},
                "prompt_eval_count": 12, "eval_count": 4
            }))
            .unwrap();
        assert_eq!(
            r.content,
            vec![AssistantPart::ToolCall {
                id: "f".into(),
                name: "f".into(),
                arguments: json!({"a": 1}),
                provider_data: None
            }]
        );
        assert_eq!(r.usage.total_tokens, 16);
    }
}
