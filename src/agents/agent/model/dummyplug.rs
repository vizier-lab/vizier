//! `dummyplug`: an offline test provider. It needs no keys and makes no network calls. It reads
//! the latest user message and the agent's tool definitions, and:
//!
//! - `tools` lists every tool;
//! - an exact tool name returns a sample `{"tool", "arguments"}` request built from its schema;
//! - such a request becomes a real `ToolCall`, so the agent loop runs the tool, and the
//!   follow-up completion echoes the tool result;
//! - anything else gets random lorem ipsum.
//!
//! See `specs/008-dummyplug-provider/` for the full chat protocol.

use anyhow::Result;
use rand::RngExt;
use rig_core::{
    OneOrMany,
    completion::{ToolDefinition, Usage},
    message::{
        AssistantContent, Message, ToolCall, ToolFunction, ToolResult, ToolResultContent,
        UserContent,
    },
};
use serde_json::{Value, json};

use super::VizierModelTrait;
use crate::{
    agents::agent::system_prompt::context::CONTEXT_HEADER,
    schema::AgentConfig,
    utils::{json_schema, markdown::parse_markdown_str},
};

pub struct DummyplugModel {
    context_window: Option<u64>,
}

impl DummyplugModel {
    pub fn new(agent_config: &AgentConfig) -> Self {
        Self {
            context_window: agent_config.context_window,
        }
    }
}

#[async_trait::async_trait]
impl VizierModelTrait for DummyplugModel {
    /// Dispatch rules, first match wins (research R5). User input errors are replies, never `Err`.
    async fn completion(
        &self,
        message: Message,
        history: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)> {
        let results = tool_results(&message);
        let reply = if !results.is_empty() {
            tracing::debug!("dummyplug: echoing {} tool result(s)", results.len());
            AssistantContent::text(reply_tool_results(&results, &history))
        } else {
            let text = command_text(&message);
            if text.eq_ignore_ascii_case("tools") {
                tracing::debug!("dummyplug: listing {} tool(s)", tools.len());
                AssistantContent::text(reply_tool_list(&tools))
            } else if text.eq_ignore_ascii_case("context") {
                tracing::debug!("dummyplug: echoing the context block");
                AssistantContent::text(reply_context(&message))
            } else if let Some(tool) = tools.iter().find(|t| t.name == text) {
                tracing::debug!("dummyplug: sample request for {}", tool.name);
                AssistantContent::text(reply_tool_sample(tool))
            } else if text.starts_with('{') {
                match parse_tool_request(&text, &tools) {
                    Ok(call) => {
                        tracing::debug!("dummyplug: calling tool {}", call.function.name);
                        AssistantContent::ToolCall(call)
                    }
                    Err(err) => {
                        tracing::debug!("dummyplug: rejected tool request");
                        AssistantContent::text(err)
                    }
                }
            } else {
                tracing::debug!("dummyplug: lorem ipsum");
                AssistantContent::text(lorem_ipsum())
            }
        };

        Ok((None, OneOrMany::one(reply), Usage::new()))
    }

    fn context_window(&self) -> Option<u64> {
        self.context_window
    }
}

/// The user's plain text: the channel's frontmatter, the attachments trailer and one surrounding
/// code fence stripped, then trimmed (research R4).
fn command_text(message: &Message) -> String {
    let Message::User { content } = message else {
        return String::new();
    };
    let text = content
        .iter()
        .filter_map(|c| match c {
            UserContent::Text(t) if !t.text().starts_with(CONTEXT_HEADER) => Some(t.text()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let body = match parse_markdown_str::<serde_yaml::Value>(&text) {
        Ok((_, body)) => body,
        Err(_) => text,
    };
    let body = match body.find("\n\n# Attached Files\n") {
        Some(end) => &body[..end],
        None => &body[..],
    };
    strip_fence(body.trim()).trim().to_string()
}

/// §6: the per-request context block exactly as the agent received it this turn.
fn reply_context(message: &Message) -> String {
    let Message::User { content } = message else {
        return "(no context block)".to_string();
    };
    content
        .iter()
        .find_map(|c| match c {
            UserContent::Text(t) if t.text().starts_with(CONTEXT_HEADER) => {
                Some(t.text().to_string())
            }
            _ => None,
        })
        .unwrap_or_else(|| "(no context block)".to_string())
}

/// Drop one surrounding Markdown code fence, including the opening line's language tag.
fn strip_fence(text: &str) -> &str {
    if text.len() < 6 || !text.starts_with("```") || !text.ends_with("```") {
        return text;
    }
    let inner = &text[3..text.len() - 3];
    match inner.find('\n') {
        Some(line_end) => &inner[line_end + 1..],
        None => inner,
    }
}

fn tool_results(message: &Message) -> Vec<&ToolResult> {
    let Message::User { content } = message else {
        return vec![];
    };
    content
        .iter()
        .filter_map(|c| match c {
            UserContent::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

/// Echo each tool result, named by the matching call in the last assistant message (R8).
fn reply_tool_results(results: &[&ToolResult], history: &[Message]) -> String {
    let calls = history
        .iter()
        .rev()
        .find_map(|m| match m {
            Message::Assistant { content, .. } => Some(content),
            _ => None,
        })
        .map(|content| {
            content
                .iter()
                .filter_map(|c| match c {
                    AssistantContent::ToolCall(call) => Some(call),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    results
        .iter()
        .map(|result| {
            let name = calls
                .iter()
                .find(|call| call.id == result.id)
                .map(|call| call.function.name.as_str())
                .unwrap_or(&result.id);
            let text = result
                .content
                .iter()
                .map(|c| match c {
                    ToolResultContent::Text(t) => t.text().to_string(),
                    ToolResultContent::Image(_) => "[image]".to_string(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("**Tool result** (`{name}`):\n{text}")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Contract §1.
fn reply_tool_list(tools: &[ToolDefinition]) -> String {
    if tools.is_empty() {
        return "This agent has no tools available.".to_string();
    }
    let lines = tools
        .iter()
        .map(|tool| match summary(&tool.description) {
            Some(summary) => format!("- `{}` — {summary}", tool.name),
            None => format!("- `{}`", tool.name),
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "**Available tools** ({}):\n\n{lines}\n\nSend a tool name to get a sample request.",
        tools.len()
    )
}

/// The first line of a description, truncated to 120 characters.
fn summary(description: &str) -> Option<String> {
    const MAX: usize = 120;
    let first = description.trim().lines().next()?.trim();
    if first.is_empty() {
        return None;
    }
    if first.chars().count() <= MAX {
        return Some(first.to_string());
    }
    Some(format!("{}…", first.chars().take(MAX).collect::<String>()))
}

/// Contract §2.
fn reply_tool_sample(tool: &ToolDefinition) -> String {
    let mut reply = match tool.description.trim() {
        "" => format!("**`{}`**", tool.name),
        description => format!("**`{}`** — {description}", tool.name),
    };

    let properties = json_schema::properties(&tool.parameters);
    let mut arguments = String::new();
    for (heading, required) in [("Required:", true), ("Optional:", false)] {
        let group = properties
            .iter()
            .filter(|p| p.required == required)
            .collect::<Vec<_>>();
        if group.is_empty() {
            continue;
        }
        arguments.push_str(heading);
        for p in group {
            match &p.description {
                Some(description) => {
                    arguments.push_str(&format!("\n- `{}`: {description}", p.name))
                }
                None => arguments.push_str(&format!("\n- `{}`", p.name)),
            }
        }
        arguments.push('\n');
    }
    if !arguments.is_empty() {
        reply.push_str("\n\n");
        reply.push_str(arguments.trim_end());
    }

    let sample = match json_schema::sample_value(&tool.parameters) {
        sample @ Value::Object(_) => sample,
        _ => json!({}),
    };
    // Built by hand so `tool` comes before `arguments`; `serde_json::Map` would sort the keys.
    let sample = serde_json::to_string_pretty(&sample)
        .unwrap_or_else(|_| "{}".to_string())
        .replace('\n', "\n  ");
    reply.push_str(&format!(
        "\n\n```json\n{{\n  \"tool\": {},\n  \"arguments\": {sample}\n}}\n```\n\nSend the JSON back (edit the values first) to run the tool.",
        Value::String(tool.name.clone())
    ));
    reply
}

const REQUEST_SHAPE: &str = "Expected: `{\"tool\": \"<tool name>\", \"arguments\": { ... }}`";

/// Contract §3. `Err` holds the user-facing reply.
fn parse_tool_request(text: &str, tools: &[ToolDefinition]) -> Result<ToolCall, String> {
    let value = serde_json::from_str::<Value>(text)
        .map_err(|e| format!("Could not parse tool request: {e}\n\n{REQUEST_SHAPE}"))?;
    let invalid = |reason: &str| format!("Invalid tool request: {reason}\n\n{REQUEST_SHAPE}");

    let Some(request) = value.as_object() else {
        return Err(invalid("expected a JSON object"));
    };
    let name = match request.get("tool") {
        Some(Value::String(name)) => name,
        Some(_) => return Err(invalid("`tool` must be a string")),
        None => return Err(invalid("missing `tool`")),
    };
    let arguments = match request.get("arguments") {
        None => json!({}),
        Some(arguments @ Value::Object(_)) => arguments.clone(),
        Some(_) => return Err(invalid("`arguments` must be an object")),
    };
    if !tools.iter().any(|tool| &tool.name == name) {
        return Err(format!(
            "Unknown tool `{name}`. Send `tools` to list available tools."
        ));
    }

    Ok(ToolCall {
        id: format!("dummyplug-{}", uuid::Uuid::new_v4()),
        call_id: None,
        function: ToolFunction {
            name: name.clone(),
            arguments,
        },
        signature: None,
        additional_params: None,
    })
}

const WORDS: &[&str] = &[
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "ut",
    "labore",
    "et",
    "dolore",
    "magna",
    "aliqua",
    "enim",
    "ad",
    "minim",
    "veniam",
    "quis",
    "nostrud",
    "exercitation",
    "ullamco",
    "laboris",
    "nisi",
    "aliquip",
    "ex",
    "ea",
    "commodo",
    "consequat",
    "duis",
    "aute",
    "irure",
    "in",
    "reprehenderit",
    "voluptate",
    "velit",
    "esse",
    "cillum",
    "fugiat",
    "nulla",
    "pariatur",
    "excepteur",
    "sint",
    "occaecat",
    "cupidatat",
    "non",
    "proident",
    "sunt",
    "culpa",
    "qui",
    "officia",
    "deserunt",
    "mollit",
    "anim",
    "id",
    "est",
    "laborum",
];

/// 1–3 paragraphs of 2–5 sentences, each 6–14 words, capitalized and ending in `.`.
fn lorem_ipsum() -> String {
    let mut rng = rand::rng();
    let paragraphs = rng.random_range(1..=3);
    (0..paragraphs)
        .map(|_| {
            let sentences = rng.random_range(2..=5);
            (0..sentences)
                .map(|_| {
                    let words = rng.random_range(6..=14);
                    let mut sentence = (0..words)
                        .map(|_| WORDS[rng.random_range(0..WORDS.len())])
                        .collect::<Vec<_>>()
                        .join(" ");
                    if let Some(first) = sentence.get_mut(0..1) {
                        first.make_ascii_uppercase();
                    }
                    sentence.push('.');
                    sentence
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_config(context_window: Option<u64>) -> AgentConfig {
        AgentConfig {
            name: "tester".into(),
            owner_id: None,
            shared_to: vec![],
            system_prompt: None,
            description: None,
            provider: crate::config::provider::ProviderVariant::dummyplug,
            model: "dummyplug".into(),
            thinking_depth: 5,
            checkpoint_threshold: 0.8,
            tools: Default::default(),
            silent_read_initiative_chance: 0.0,
            max_tokens: None,
            context_window,
            include_documents: None,
            prompt_timeout: Default::default(),
            documents: vec![],
            heartbeat_interval: Default::default(),
            core: None,
            dream_enabled: false,
            dream_schedule: None,
            dream_provider: None,
            dream_model: None,
            discord_token: None,
            telegram_token: None,
            avatar_url: None,
            embedding: None,
            indexer: None,
            chunking: Default::default(),
            auto_context: Default::default(),
        }
    }

    #[test]
    fn lorem_ipsum_is_non_empty_and_ends_with_a_period() {
        let text = lorem_ipsum();
        assert!(!text.is_empty());
        assert!(text.ends_with('.'));
    }

    #[test]
    fn lorem_ipsum_varies_between_calls() {
        let texts = (0..5).map(|_| lorem_ipsum()).collect::<Vec<_>>();
        assert!(texts.iter().any(|t| t != &texts[0]));
    }

    #[tokio::test]
    async fn prose_gets_one_text_reply_and_zero_usage() {
        let model = DummyplugModel::new(&agent_config(None));
        let (id, content, usage) = model
            .completion(Message::user("hello"), vec![], vec![])
            .await
            .unwrap();
        assert!(id.is_none());
        assert_eq!(content.len(), 1);
        assert!(matches!(content.first(), AssistantContent::Text(_)));
        assert_eq!(usage, Usage::new());
    }

    #[test]
    fn context_window_passes_through_the_config() {
        assert_eq!(
            DummyplugModel::new(&agent_config(Some(1234))).context_window(),
            Some(1234)
        );
        assert_eq!(
            DummyplugModel::new(&agent_config(None)).context_window(),
            None
        );
    }

    // --- helpers ------------------------------------------------------------------------------

    use crate::schema::{VizierRequest, VizierRequestContent};

    /// A user message exactly as a channel frames it (frontmatter + body).
    fn user_message(text: &str) -> Message {
        VizierRequest {
            content: VizierRequestContent::Chat(text.into()),
            user: "tester".into(),
            metadata: json!({}),
            ..Default::default()
        }
        .to_message("")
        .unwrap()
    }

    fn echo_tool() -> ToolDefinition {
        ToolDefinition {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"]
            }),
        }
    }

    fn tool<T: schemars::JsonSchema>(name: &str, description: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::to_value(schemars::schema_for!(T)).unwrap(),
        }
    }

    async fn reply(
        message: Message,
        history: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> AssistantContent {
        let (_, content, _) = DummyplugModel::new(&agent_config(None))
            .completion(message, history, tools)
            .await
            .unwrap();
        assert_eq!(content.len(), 1);
        content.first()
    }

    async fn reply_text(message: Message, tools: Vec<ToolDefinition>) -> String {
        match reply(message, vec![], tools).await {
            AssistantContent::Text(t) => t.text().to_string(),
            other => panic!("expected text, got {other:?}"),
        }
    }

    async fn reply_call(message: Message, tools: Vec<ToolDefinition>) -> ToolCall {
        match reply(message, vec![], tools).await {
            AssistantContent::ToolCall(call) => call,
            other => panic!("expected a tool call, got {other:?}"),
        }
    }

    // --- command_text -------------------------------------------------------------------------

    #[test]
    fn command_text_strips_the_channel_frontmatter() {
        assert_eq!(command_text(&user_message("tools")), "tools");
    }

    #[test]
    fn command_text_strips_a_json_fence() {
        let message = user_message("```json\n{\"tool\":\"x\"}\n```");
        assert_eq!(command_text(&message), "{\"tool\":\"x\"}");
    }

    #[test]
    fn command_text_falls_back_to_the_whole_text_without_frontmatter() {
        assert_eq!(command_text(&Message::user("  hi  ")), "hi");
    }

    #[test]
    fn command_text_drops_the_attached_files_trailer() {
        let message = VizierRequest {
            content: VizierRequestContent::Chat("tools".into()),
            user: "tester".into(),
            metadata: json!({}),
            ..Default::default()
        }
        .to_prompt()
        .unwrap()
            + "\n\n# Attached Files\n- a.png (image/png)\nthe following files added to your session files.";
        assert_eq!(command_text(&Message::user(message)), "tools");
    }

    #[test]
    fn command_text_ignores_the_injected_context_block() {
        use crate::agents::agent::system_prompt::context::{context_md, with_context};

        let message = with_context(user_message("tools"), context_md(&[], &[], None));
        assert_eq!(command_text(&message), "tools");
    }

    // --- §6: context ---------------------------------------------------------------------------

    #[tokio::test]
    async fn context_replies_with_the_context_block() {
        use crate::agents::agent::system_prompt::context::{context_md, with_context};

        let message = with_context(user_message("context"), context_md(&[], &[], Some("- x")));
        let text = reply_text(message, vec![]).await;
        assert!(text.starts_with(CONTEXT_HEADER), "{text}");
        assert!(text.contains("## Reactions"), "{text}");
        assert!(text.ends_with("- x"), "{text}");
    }

    #[tokio::test]
    async fn context_without_a_block_says_so() {
        assert_eq!(reply_text(user_message("context"), vec![]).await, "(no context block)");
    }

    // --- US4: tool requests -------------------------------------------------------------------

    #[tokio::test]
    async fn valid_request_becomes_a_tool_call() {
        let call = reply_call(
            user_message(r#"{"tool": "echo", "arguments": {"text": "hi"}, "extra": 1}"#),
            vec![echo_tool()],
        )
        .await;
        assert_eq!(call.function.name, "echo");
        assert_eq!(call.function.arguments, json!({"text": "hi"}));
        assert!(call.id.starts_with("dummyplug-"));
        assert_eq!(call.call_id, None);
    }

    #[tokio::test]
    async fn omitted_arguments_default_to_an_empty_object() {
        let call = reply_call(user_message(r#"{"tool": "echo"}"#), vec![echo_tool()]).await;
        assert_eq!(call.function.arguments, json!({}));
    }

    #[tokio::test]
    async fn bad_requests_are_text_replies() {
        let unknown = reply_text(user_message(r#"{"tool": "nope"}"#), vec![echo_tool()]).await;
        assert!(unknown.starts_with("Unknown tool `nope`"), "{unknown}");

        let malformed = reply_text(user_message(r#"{"tool": "#), vec![echo_tool()]).await;
        assert!(
            malformed.starts_with("Could not parse tool request"),
            "{malformed}"
        );

        let bad_args = reply_text(
            user_message(r#"{"tool": "echo", "arguments": 1}"#),
            vec![echo_tool()],
        )
        .await;
        assert!(bad_args.starts_with("Invalid tool request"), "{bad_args}");

        // `[1,2]` doesn't start with `{`, so it's prose; either way, no tool call.
        let array = reply(user_message("[1,2]"), vec![], vec![echo_tool()]).await;
        assert!(matches!(array, AssistantContent::Text(_)));
    }

    #[tokio::test]
    async fn tool_results_are_echoed_never_called_again() {
        let call = ToolCall {
            id: "call-1".into(),
            call_id: None,
            function: ToolFunction {
                name: "echo".into(),
                arguments: json!({"text": "hi"}),
            },
            signature: None,
            additional_params: None,
        };
        let history = vec![
            user_message(r#"{"tool": "echo"}"#),
            Message::Assistant {
                id: None,
                content: OneOrMany::one(AssistantContent::ToolCall(call)),
            },
        ];
        let result = Message::User {
            content: OneOrMany::one(UserContent::tool_result(
                "call-1",
                OneOrMany::one(ToolResultContent::text("hi back")),
            )),
        };
        match reply(result, history, vec![echo_tool()]).await {
            AssistantContent::Text(t) => {
                assert_eq!(t.text(), "**Tool result** (`echo`):\nhi back")
            }
            other => panic!("expected text, got {other:?}"),
        }
    }

    // --- US2: tool listing --------------------------------------------------------------------

    #[tokio::test]
    async fn tools_lists_every_tool_in_order_ignoring_case_and_whitespace() {
        let tools = vec![
            echo_tool(),
            ToolDefinition {
                name: "mcp_fs__read_file".into(),
                description: "Read a file.\nSecond line.".into(),
                parameters: json!({}),
            },
        ];
        for input in ["tools", " Tools ", "TOOLS"] {
            let text = reply_text(user_message(input), tools.clone()).await;
            assert_eq!(
                text,
                "**Available tools** (2):\n\n- `echo` — Echo\n- `mcp_fs__read_file` — Read a file.\n\nSend a tool name to get a sample request."
            );
        }
    }

    #[tokio::test]
    async fn tools_with_no_tools() {
        assert_eq!(
            reply_text(user_message("tools"), vec![]).await,
            "This agent has no tools available."
        );
    }

    #[test]
    fn long_descriptions_are_truncated() {
        let long = "x".repeat(200);
        let summary = summary(&long).unwrap();
        assert_eq!(summary.chars().count(), 121);
        assert!(summary.ends_with('…'));
    }

    // --- US3: sample requests -----------------------------------------------------------------

    #[derive(schemars::JsonSchema, serde::Deserialize)]
    struct NoArgs {}

    #[derive(schemars::JsonSchema, serde::Deserialize)]
    struct Inner {
        x: bool,
    }

    #[derive(schemars::JsonSchema, serde::Deserialize)]
    #[allow(dead_code)]
    enum Mode {
        Fast,
        Slow,
    }

    #[derive(schemars::JsonSchema, serde::Deserialize)]
    #[allow(dead_code)]
    struct Node {
        children: Vec<Node>,
    }

    #[derive(schemars::JsonSchema, serde::Deserialize)]
    #[allow(dead_code)]
    struct WriteLike {
        /// Title of the memory
        title: String,
        content: String,
        /// Tags used to group related memories
        tags: Option<Vec<String>>,
        inner: Inner,
        mode: Mode,
        node: Option<Node>,
    }

    fn fenced_json(reply: &str) -> &str {
        let start = reply.find("```json").expect("no json fence");
        let end = reply[start + 7..].find("```").expect("unclosed fence") + start + 7 + 3;
        &reply[start..end]
    }

    #[tokio::test]
    async fn sample_round_trips_into_a_tool_call() {
        let tools = vec![
            tool::<NoArgs>("no_args", "Takes nothing."),
            tool::<WriteLike>("write_like", "Writes a thing."),
            tool::<Node>("recursive", "Recursive."),
        ];
        for t in &tools {
            let sample = reply_text(user_message(&t.name), tools.clone()).await;
            let call = reply_call(user_message(fenced_json(&sample)), tools.clone()).await;
            assert_eq!(call.function.name, t.name);
            // The arguments also still conform to the tool's input type.
            if t.name == "write_like" {
                serde_json::from_value::<WriteLike>(call.function.arguments).unwrap();
            }
        }
    }

    #[tokio::test]
    async fn sample_for_a_tool_without_parameters() {
        let text = reply_text(
            user_message("no_args"),
            vec![tool::<NoArgs>("no_args", "Takes nothing.")],
        )
        .await;
        assert_eq!(
            text,
            "**`no_args`** — Takes nothing.\n\n```json\n{\n  \"tool\": \"no_args\",\n  \"arguments\": {}\n}\n```\n\nSend the JSON back (edit the values first) to run the tool."
        );
    }

    #[tokio::test]
    async fn sample_lists_arguments_under_required_and_optional() {
        let text = reply_text(
            user_message("write_like"),
            vec![tool::<WriteLike>("write_like", "Writes.")],
        )
        .await;
        let args = &text[..text.find("```json").unwrap()];
        assert_eq!(
            args,
            "**`write_like`** — Writes.\n\nRequired:\n- `title`: Title of the memory\n- `content`\n- `inner`\n- `mode`\nOptional:\n- `node`\n- `tags`: Tags used to group related memories\n\n"
        );
        assert!(text.contains("\"tool\": \"write_like\""));
    }

    #[tokio::test]
    async fn tools_command_wins_over_a_tool_named_tools() {
        let tools = vec![ToolDefinition {
            name: "tools".into(),
            description: "Shadow".into(),
            parameters: json!({}),
        }];
        let text = reply_text(user_message("tools"), tools).await;
        assert!(text.starts_with("**Available tools** (1):"), "{text}");
    }
}
