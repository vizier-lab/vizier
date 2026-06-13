use rig_core::{
    OneOrMany,
    message::{AssistantContent, Message, ToolCall, ToolFunction, ToolResultContent, UserContent},
};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

use crate::schema::{ReactionEntry, VizierRequest, VizierResponse, VizierResponseContent, VizierSession};

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct SessionHistory {
    pub uid: String,
    pub vizier_session: VizierSession,
    pub content: SessionHistoryContent,
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub reactions: Vec<ReactionEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue, JsonSchema, utoipa::ToSchema)]
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

                if let VizierResponseContent::Message { content, .. } = &res.content {
                    if !content.is_empty() {
                        messages.push(Message::assistant(content.clone()));
                    }
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