//! Live-provider smoke tests (T095): one text turn, one tool call and its result through
//! `llm::build`, per dialect. `#[ignore]`d, and each skips itself when its key isn't set:
//!
//! ```sh
//! ANTHROPIC_API_KEY=… cargo test llm_smoke_ -- --ignored --nocapture
//! ```
//!
//! The model is the catalogue's default for the provider unless `LLM_SMOKE_<ID>_MODEL` names
//! one (`<ID>` upper-cased, `-` as `_`). `llm_smoke_extra` runs a provider vizier didn't
//! support before the catalogue: `LLM_SMOKE_EXTRA` (default `cerebras`).

use serde_json::json;

use super::resolve::{ModelOverrides, ResolveContext, resolve_with};
use super::types::{AssistantPart, CompletionRequest, Message, ToolChoice, ToolDefinition};
use crate::catalogue::{Catalogue, seed::load_seed};
use crate::config::provider::ProviderId;
use crate::schema::agent::CompletionRetryConfig;

fn model_for(catalogue: &Catalogue, id: &ProviderId) -> Option<String> {
    let var = format!("LLM_SMOKE_{}_MODEL", id.as_str().to_uppercase().replace(['-', '.'], "_"));
    std::env::var(var)
        .ok()
        .or_else(|| catalogue.models(id, false).first().map(|m| m.id.clone()))
}

fn weather_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_weather".into(),
        description: "Current weather for a city.".into(),
        parameters: json!({
            "type": "object",
            "properties": {"city": {"type": "string"}},
            "required": ["city"]
        }),
    }
}

/// Runs the three steps; `None` when the provider isn't configured here. `system` is sent as
/// the leading system prompt (long for the caching check).
async fn smoke(id: &str, needs_key: bool, system: &str) -> Option<Vec<super::Usage>> {
    let catalogue = Catalogue::new(load_seed());
    let id = ProviderId::new(id.to_string()).unwrap();
    let env = super::resolve::provider_env(&catalogue, &id);
    let credentials = crate::provider_keys::credentials_from_env(&id, &env).map(|(c, _)| c);
    if needs_key && credentials.is_none() {
        eprintln!("skip {id}: no key in {env:?}");
        return None;
    }
    let Some(model) = model_for(&catalogue, &id) else {
        eprintln!("skip {id}: no model (set LLM_SMOKE_…_MODEL)");
        return None;
    };
    let http = super::http::client();
    let resolved = resolve_with(
        &ResolveContext {
            catalogue: &catalogue,
            custom: None,
            credentials,
            http: http.clone(),
        },
        &id,
        &model,
        &ModelOverrides::default(),
    )
    .expect("resolve");
    eprintln!("{id}/{model} via {:?} at {}", resolved.dialect, resolved.base_url);
    let llm = super::build(resolved, http, CompletionRetryConfig::default());
    let mut usages = vec![];

    // 1. Text.
    let text = llm
        .complete(CompletionRequest {
            messages: vec![Message::system(system), Message::user_text("Reply with the single word: pong").unwrap()],
            tools: vec![],
            tool_choice: None,
            max_tokens: Some(256),
        })
        .await
        .unwrap_or_else(|e| panic!("{id}: text turn failed: {e}"));
    let said: String = text
        .content
        .iter()
        .filter_map(|p| match p {
            AssistantPart::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert!(said.to_lowercase().contains("pong"), "{id}: unexpected reply {said:?}");
    usages.push(text.usage.clone());

    // 2. A tool call.
    let mut messages = vec![
        Message::system(system),
        Message::user_text("What's the weather in Paris? Use the tool.").unwrap(),
    ];
    let call = llm
        .complete(CompletionRequest {
            messages: messages.clone(),
            tools: vec![weather_tool()],
            tool_choice: Some(ToolChoice::Auto),
            max_tokens: Some(1024),
        })
        .await
        .unwrap_or_else(|e| panic!("{id}: tool turn failed: {e}"));
    let (call_id, name) = call
        .content
        .iter()
        .find_map(|p| match p {
            AssistantPart::ToolCall { id, name, .. } => Some((id.clone(), name.clone())),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{id}: no tool call in {:?}", call.content));
    assert_eq!(name, "get_weather");
    usages.push(call.usage.clone());

    // 3. Its result, answered in text.
    messages.push(Message::Assistant(call.content));
    messages.push(Message::tool_result(call_id, "{\"city\":\"Paris\",\"temp_c\":21,\"sky\":\"clear\"}"));
    let answer = llm
        .complete(CompletionRequest {
            messages,
            tools: vec![weather_tool()],
            tool_choice: Some(ToolChoice::Auto),
            max_tokens: Some(1024),
        })
        .await
        .unwrap_or_else(|e| panic!("{id}: tool-result turn failed: {e}"));
    let said: String = answer
        .content
        .iter()
        .filter_map(|p| match p {
            AssistantPart::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert!(said.contains("21"), "{id}: the answer ignores the tool result: {said:?}");
    usages.push(answer.usage.clone());
    eprintln!("{id}: ok, usage {usages:?}");
    Some(usages)
}

const SHORT: &str = "You are a terse test assistant.";

#[tokio::test]
#[ignore]
async fn llm_smoke_openai_responses() {
    smoke("openai", true, SHORT).await;
}

#[tokio::test]
#[ignore]
async fn llm_smoke_anthropic_caches_the_prompt() {
    // Long enough to be cached (Anthropic's minimum is 1–4k tokens depending on the model).
    let system = format!("{SHORT}\n\n{}", "Reference: the quick brown fox jumps over the lazy dog. ".repeat(600));
    if let Some(usages) = smoke("anthropic", true, &system).await {
        // The second and third turns re-send the same system prompt and tool.
        assert!(
            usages.iter().skip(1).any(|u| u.cached_input_tokens > 0),
            "no cache hit: {usages:?}"
        );
    }
}

#[tokio::test]
#[ignore]
async fn llm_smoke_google() {
    smoke("google", true, SHORT).await;
}

#[tokio::test]
#[ignore]
async fn llm_smoke_deepseek_reasoning_and_tools() {
    smoke("deepseek", true, SHORT).await;
}

#[tokio::test]
#[ignore]
async fn llm_smoke_ollama() {
    if std::env::var("LLM_SMOKE_OLLAMA_MODEL").is_err() {
        eprintln!("skip ollama: set LLM_SMOKE_OLLAMA_MODEL to a pulled, tool-capable model");
        return;
    }
    smoke("ollama", false, SHORT).await;
}

#[tokio::test]
#[ignore]
async fn llm_smoke_extra() {
    let id = std::env::var("LLM_SMOKE_EXTRA").unwrap_or_else(|_| "cerebras".into());
    smoke(&id, true, SHORT).await;
}
