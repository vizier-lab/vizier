use rig_core::{
    OneOrMany,
    message::{AssistantContent, Message, ToolCall, ToolFunction, ToolResultContent, UserContent},
};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


use crate::sandbox::ExecutionReport;
use crate::schema::{ReactionEntry, VizierRequest, VizierResponse, VizierResponseContent, VizierSession};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct SessionHistory {
    pub uid: String,
    pub vizier_session: VizierSession,
    pub content: SessionHistoryContent,
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub reactions: Vec<ReactionEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema, utoipa::ToSchema)]
pub enum SessionHistoryContent {
    Request(VizierRequest),
    Response(VizierResponse),
    AssistantMessage(String),
    ToolCall {
        call_id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        call_id: String,
        content: String,
    },
    Checkpoint(Option<String>),
    Command(String),
}

/// Convert rig Messages to SessionHistoryContent entries.
/// - Message::User with ToolResult → ToolResult entries
/// - Message::Assistant with ToolCall → ToolCall entries
/// - Message::Assistant with Text only (no tool calls) → Response entry (final)
/// - Message::Assistant with Text + ToolCall → AssistantMessage entry (intermediate)
/// - Message::System and user Text messages → skipped (caller handles those)
pub fn messages_to_history_entries(messages: &[Message]) -> Vec<SessionHistoryContent> {
    let mut entries = Vec::new();

    for msg in messages {
        match msg {
            Message::System { .. } => {}
            Message::User { content } => {
                let all_tool_results = content
                    .iter()
                    .all(|c| matches!(c, UserContent::ToolResult(_)));

                if all_tool_results {
                    for item in content.iter() {
                        if let UserContent::ToolResult(tr) = item {
                            entries.push(SessionHistoryContent::ToolResult {
                                call_id: tr.id.clone(),
                                content: tool_result_content_to_text(&tr.content),
                            });
                        }
                    }
                }
            }
            Message::Assistant { content, .. } => {
                let mut text_parts = Vec::new();
                let has_tool_calls = content
                    .iter()
                    .any(|c| matches!(c, AssistantContent::ToolCall(_)));

                for item in content.iter() {
                    match item {
                        AssistantContent::Text(text) => text_parts.push(text.to_string()),
                        AssistantContent::ToolCall(tc) => {
                            entries.push(SessionHistoryContent::ToolCall {
                                call_id: tc.id.clone(),
                                name: tc.function.name.clone(),
                                arguments: tc.function.arguments.clone(),
                            });
                        }
                        _ => {}
                    }
                }

                if !text_parts.is_empty() {
                    let text = text_parts.join("\n");
                    if has_tool_calls {
                        // Intermediate text during tool calling
                        entries.push(SessionHistoryContent::AssistantMessage(text));
                    } else {
                        // Final response
                        entries.push(SessionHistoryContent::Response(VizierResponse {
                            timestamp: chrono::Utc::now(),
                            content: VizierResponseContent::Message {
                                content: text,
                                stats: None,
                            },
                            attachments: vec![],
                        }));
                    }
                }
            }
        }
    }

    entries
}

/// Convert SessionHistory entries back to rig Messages.
/// Groups consecutive ToolCall entries into a single Message::Assistant.
/// Groups consecutive ToolResult entries into a single Message::User.
pub fn history_entries_to_messages(entries: &[SessionHistory]) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut pending_tool_calls: Vec<ToolCall> = Vec::new();
    let mut pending_tool_results: Vec<rig_core::message::ToolResult> = Vec::new();

    for entry in entries {
        match &entry.content {
            SessionHistoryContent::Request(req) => {
                flush_pending_tool_calls(&mut pending_tool_calls, &mut messages);
                flush_pending_tool_results(&mut pending_tool_results, &mut messages);

                if let Some(text) = req.to_prompt().ok() {
                    if !text.is_empty() {
                        messages.push(Message::user(text));
                    }
                }
            }
            SessionHistoryContent::Response(res) => {
                flush_pending_tool_calls(&mut pending_tool_calls, &mut messages);
                flush_pending_tool_results(&mut pending_tool_results, &mut messages);

                match &res.content {
                    VizierResponseContent::Message { content, .. } => {
                        if !content.is_empty() {
                            messages.push(Message::assistant(content.clone()));
                        }
                    }
                    VizierResponseContent::Error { kind, message } => {
                        let kind_str = match kind {
                            crate::schema::ErrorKind::Completion => "completion",
                            crate::schema::ErrorKind::ToolTimeout => "tool_timeout",
                            crate::schema::ErrorKind::PromptTimeout => "prompt_timeout",
                        };
                        messages.push(Message::user(format!("[Error: {}] {}", kind_str, message)));
                    }
                    _ => {}
                }
            }
            SessionHistoryContent::AssistantMessage(text) => {
                flush_pending_tool_calls(&mut pending_tool_calls, &mut messages);
                flush_pending_tool_results(&mut pending_tool_results, &mut messages);

                if !text.is_empty() {
                    messages.push(Message::assistant(text.clone()));
                }
            }
            SessionHistoryContent::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                pending_tool_calls.push(ToolCall {
                    id: call_id.clone(),
                    call_id: None,
                    function: ToolFunction {
                        name: name.clone(),
                        arguments: arguments.clone(),
                    },
                    signature: None,
                    additional_params: None,
                });
            }
            SessionHistoryContent::ToolResult { call_id, content } => {
                pending_tool_results.push(rig_core::message::ToolResult {
                    id: call_id.clone(),
                    call_id: None,
                    content: OneOrMany::one(ToolResultContent::text(content.clone())),
                });
            }
            SessionHistoryContent::Checkpoint(_) => {
                // Skip checkpoint entries - they are metadata, not conversation messages
            }
            SessionHistoryContent::Command(_) => {
                // Skip command entries - they are for display only, not agent context
            }
        }
    }

    flush_pending_tool_calls(&mut pending_tool_calls, &mut messages);
    flush_pending_tool_results(&mut pending_tool_results, &mut messages);

    messages
}

fn flush_pending_tool_calls(calls: &mut Vec<ToolCall>, messages: &mut Vec<Message>) {
    if !calls.is_empty() {
        let content: Vec<AssistantContent> = calls.drain(..).map(AssistantContent::ToolCall).collect();
        messages.push(Message::Assistant {
            id: None,
            content: OneOrMany::many(content).unwrap(),
        });
    }
}

fn flush_pending_tool_results(
    results: &mut Vec<rig_core::message::ToolResult>,
    messages: &mut Vec<Message>,
) {
    if !results.is_empty() {
        let content: Vec<UserContent> = results
            .drain(..)
            .map(UserContent::ToolResult)
            .collect();
        messages.push(Message::User {
            content: OneOrMany::many(content).unwrap(),
        });
    }
}

fn tool_result_content_to_text(content: &OneOrMany<ToolResultContent>) -> String {
    content
        .iter()
        .filter_map(|c| {
            if let ToolResultContent::Text(text) = c {
                Some(text.text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// A message as the provider should receive it, leaving the record kept in storage
/// (and shown in the WebUI) untouched.
///
/// Today this only narrows `execute_python` reports to
/// [`ExecutionReport::model_view`]. It is applied per request rather than where the
/// tool result is built, because that same value is what gets persisted: the agent
/// loop hands `full_history` to storage and the output of this function to the model.
pub fn message_for_model(message: &Message) -> Message {
    let Message::User { content } = message else {
        return message.clone();
    };
    if !content.iter().any(is_narrowable) {
        return message.clone();
    }

    let narrowed: Vec<UserContent> = content.iter().map(narrow_tool_result).collect();
    match OneOrMany::many(narrowed) {
        Ok(content) => Message::User { content },
        Err(_) => message.clone(),
    }
}

/// [`message_for_model`] over a whole history.
pub fn messages_for_model(messages: &[Message]) -> Vec<Message> {
    messages.iter().map(message_for_model).collect()
}

/// A cheap pre-check, so an ordinary tool result is never parsed as JSON.
fn is_narrowable(content: &UserContent) -> bool {
    let UserContent::ToolResult(result) = content else {
        return false;
    };
    result.content.iter().any(|part| match part {
        ToolResultContent::Text(text) => text.text.contains("\"tool_calls\""),
        _ => false,
    })
}

fn narrow_tool_result(content: &UserContent) -> UserContent {
    let UserContent::ToolResult(result) = content else {
        return content.clone();
    };

    let narrowed: Vec<ToolResultContent> = result
        .content
        .iter()
        .map(|part| match part {
            ToolResultContent::Text(text) => serde_json::from_str(&text.text)
                .ok()
                .as_ref()
                .and_then(ExecutionReport::model_view)
                .and_then(|view| serde_json::to_string(&view).ok())
                .map_or_else(|| part.clone(), ToolResultContent::text),
            _ => part.clone(),
        })
        .collect();

    match OneOrMany::many(narrowed) {
        Ok(content) => UserContent::ToolResult(rig_core::message::ToolResult {
            id: result.id.clone(),
            call_id: result.call_id.clone(),
            content,
        }),
        Err(_) => content.clone(),
    }
}

#[cfg(test)]
mod tests {
    use rig_core::message::ToolResultContent;
    use serde_json::json;

    use super::*;

    fn tool_result(id: &str, text: &str) -> Message {
        Message::User {
            content: OneOrMany::one(UserContent::tool_result(
                id.to_string(),
                OneOrMany::one(ToolResultContent::text(text)),
            )),
        }
    }

    fn report_json() -> String {
        json!({
            "ok": true,
            "result": { "hits": 2 },
            "stdout": "",
            "tool_calls": [{
                "seq": 1,
                "name": "memory_read",
                "arguments": { "query": "rust releases" },
                "ok": true,
                "duration_ms": 12,
            }],
            "duration_ms": 34,
        })
        .to_string()
    }

    fn first_tool_result_text(message: &Message) -> String {
        let Message::User { content } = message else {
            panic!("expected a user message");
        };
        let UserContent::ToolResult(result) = content.first() else {
            panic!("expected a tool result");
        };
        tool_result_content_to_text(&result.content)
    }

    #[test]
    fn a_report_reaches_the_model_without_nested_arguments() {
        let message = tool_result("call-1", &report_json());
        let narrowed = message_for_model(&message);

        let value: serde_json::Value =
            serde_json::from_str(&first_tool_result_text(&narrowed)).unwrap();
        assert!(!value["tool_calls"][0].as_object().unwrap().contains_key("arguments"));
        assert_eq!(value["result"]["hits"], 2);

        // The message the agent loop keeps for storage is not modified.
        let original: serde_json::Value =
            serde_json::from_str(&first_tool_result_text(&message)).unwrap();
        assert_eq!(original["tool_calls"][0]["arguments"]["query"], "rust releases");
    }

    #[test]
    fn an_ordinary_tool_result_passes_through_untouched() {
        for text in [r#"{"slug":"a-memory"}"#, "plain text", ""] {
            let message = tool_result("call-1", text);
            assert_eq!(first_tool_result_text(&message_for_model(&message)), text);
        }
    }

    #[test]
    fn a_tool_result_that_only_mentions_tool_calls_is_left_alone() {
        // Passes the cheap pre-check, then fails `looks_like`.
        let text = r#"{"note":"see \"tool_calls\" in the spec"}"#;
        let message = tool_result("call-1", text);
        assert_eq!(first_tool_result_text(&message_for_model(&message)), text);
    }

    #[test]
    fn non_user_messages_and_ids_are_preserved() {
        let assistant = Message::assistant("hello");
        assert!(matches!(message_for_model(&assistant), Message::Assistant { .. }));

        let narrowed = message_for_model(&tool_result("call-7", &report_json()));
        let Message::User { content } = &narrowed else {
            panic!("expected a user message");
        };
        let UserContent::ToolResult(result) = content.first() else {
            panic!("expected a tool result");
        };
        assert_eq!(result.id, "call-7");
    }

    #[test]
    fn messages_for_model_maps_a_whole_history() {
        let history = vec![
            Message::user("what changed?"),
            tool_result("call-1", &report_json()),
        ];
        let narrowed = messages_for_model(&history);

        assert_eq!(narrowed.len(), 2);
        let value: serde_json::Value =
            serde_json::from_str(&first_tool_result_text(&narrowed[1])).unwrap();
        assert!(!value["tool_calls"][0].as_object().unwrap().contains_key("arguments"));
    }
}
