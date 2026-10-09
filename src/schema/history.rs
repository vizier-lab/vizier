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
    /// Explicit ordering position within the agent's history, breaking the ties that
    /// `timestamp` alone leaves ambiguous — a turn's entries are all flushed in one tight
    /// loop, so sharing a millisecond is the normal case.
    ///
    /// Assigned by the storage layer on insert and never by a caller. `None` means the entry
    /// was recorded before the column existed; such entries are not retroactively ordered
    /// (`specs/010-webui-reasoning-display`, FR-006/FR-007). It lives on the row rather than
    /// inside the serialized `data` blob, so reads read it from the column.
    #[serde(default)]
    pub seq: Option<i64>,
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
                // Collected before anything is pushed, because the narration entry has to
                // land *before* the tool calls it accompanies: `history_entries_to_messages`
                // merges narration into the assistant message its tool calls flush as, and
                // it can only merge text it has already seen (FR-011).
                let mut text_parts = Vec::new();
                let mut calls = Vec::new();

                for item in content.iter() {
                    match item {
                        AssistantContent::Text(text) => text_parts.push(text.to_string()),
                        AssistantContent::ToolCall(tc) => calls.push(SessionHistoryContent::ToolCall {
                            call_id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                        }),
                        _ => {}
                    }
                }

                let has_tool_calls = !calls.is_empty();

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
                            ..Default::default()
                        }));
                    }
                }

                entries.extend(calls);
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
    let mut pending_text: Option<String> = None;
    let mut pending_tool_calls: Vec<ToolCall> = Vec::new();
    let mut pending_tool_results: Vec<rig_core::message::ToolResult> = Vec::new();

    for entry in entries {
        match &entry.content {
            SessionHistoryContent::Request(req) => {
                flush_pending_tool_calls(&mut pending_text, &mut pending_tool_calls, &mut messages);
                flush_pending_tool_results(&mut pending_tool_results, &mut messages);

                if let Some(text) = req.to_prompt().ok() {
                    if !text.is_empty() {
                        messages.push(Message::user(text));
                    }
                }
            }
            SessionHistoryContent::Response(res) => {
                flush_pending_tool_calls(&mut pending_text, &mut pending_tool_calls, &mut messages);
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
                // Narration is held, not pushed. The model sent it as one assistant message
                // carrying a text block alongside its tool_use blocks, and replay has to
                // reconstruct that: pushing it on its own would emit either two consecutive
                // assistant messages or a message between a tool call and its result, both
                // of which providers reject (FR-012).
                flush_pending_tool_calls(&mut pending_text, &mut pending_tool_calls, &mut messages);
                flush_pending_tool_results(&mut pending_tool_results, &mut messages);

                if !text.is_empty() {
                    pending_text = Some(text.clone());
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

    flush_pending_tool_calls(&mut pending_text, &mut pending_tool_calls, &mut messages);
    flush_pending_tool_results(&mut pending_tool_results, &mut messages);

    messages
}

/// One assistant message out of the narration and the tool calls that came with it.
///
/// Narration leads, tool calls follow, which is the shape the model sent. Narration with no
/// tool calls after it — a turn that ended there — still becomes a message of its own, so
/// nothing recorded is silently dropped.
fn flush_pending_tool_calls(
    text: &mut Option<String>,
    calls: &mut Vec<ToolCall>,
    messages: &mut Vec<Message>,
) {
    let text = text.take();
    if calls.is_empty() {
        if let Some(text) = text {
            messages.push(Message::assistant(text));
        }
        return;
    }

    let mut content: Vec<AssistantContent> = Vec::with_capacity(calls.len() + 1);
    content.extend(text.map(AssistantContent::text));
    content.extend(calls.drain(..).map(AssistantContent::ToolCall));

    match OneOrMany::many(content) {
        Ok(content) => messages.push(Message::Assistant { id: None, content }),
        // Unreachable: `calls` is non-empty, so `content` is too. Dropping the turn is
        // still better than a panic in the middle of rebuilding an agent's context.
        Err(err) => tracing::error!("could not rebuild an assistant message from history: {err}"),
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
    // ---------------------------------------------------------------------------------
    // Narration: recording intermediate assistant text, and replaying it as a shape a
    // provider accepts. See `specs/010-webui-reasoning-display/contracts/history-api.md` §3.
    // ---------------------------------------------------------------------------------

    fn narrated_turn() -> Message {
        Message::Assistant {
            id: None,
            content: OneOrMany::many(vec![
                AssistantContent::text("Let me check the archives."),
                AssistantContent::ToolCall(ToolCall {
                    id: "call-1".to_string(),
                    call_id: None,
                    function: ToolFunction {
                        name: "memory_search".to_string(),
                        arguments: json!({ "query": "rust releases" }),
                    },
                    signature: None,
                    additional_params: None,
                }),
            ])
            .unwrap(),
        }
    }

    fn entry(content: SessionHistoryContent) -> SessionHistory {
        SessionHistory {
            uid: "uid".to_string(),
            vizier_session: VizierSession(
                "agent".to_string(),
                crate::schema::VizierChannelId::System,
                None,
            ),
            content,
            timestamp: chrono::Utc::now(),
            reactions: vec![],
            seq: None,
        }
    }

    fn kinds(entries: &[SessionHistoryContent]) -> Vec<&'static str> {
        entries
            .iter()
            .map(|e| match e {
                SessionHistoryContent::Request(_) => "Request",
                SessionHistoryContent::Response(_) => "Response",
                SessionHistoryContent::AssistantMessage(_) => "AssistantMessage",
                SessionHistoryContent::ToolCall { .. } => "ToolCall",
                SessionHistoryContent::ToolResult { .. } => "ToolResult",
                SessionHistoryContent::Checkpoint(_) => "Checkpoint",
                SessionHistoryContent::Command(_) => "Command",
            })
            .collect()
    }

    /// FR-011. The replay accumulator can only merge narration it has already seen by the
    /// time the tool calls arrive, so the write order is a hard precondition of H14.
    #[test]
    fn narration_is_recorded_before_the_tool_calls_it_accompanies() {
        let entries = messages_to_history_entries(&[narrated_turn()]);
        assert_eq!(kinds(&entries), vec!["AssistantMessage", "ToolCall"]);
    }

    /// H14, H15, H16.
    #[test]
    fn a_narrated_tool_turn_replays_as_one_assistant_message() {
        let history = vec![
            entry(SessionHistoryContent::AssistantMessage(
                "Let me check the archives.".to_string(),
            )),
            entry(SessionHistoryContent::ToolCall {
                call_id: "call-1".to_string(),
                name: "memory_search".to_string(),
                arguments: json!({ "query": "rust releases" }),
            }),
            entry(SessionHistoryContent::ToolResult {
                call_id: "call-1".to_string(),
                content: "two hits".to_string(),
            }),
        ];

        let messages = history_entries_to_messages(&history);

        // H14: exactly one assistant message, text first, then its tool call.
        assert_eq!(messages.len(), 2, "expected one assistant then one user");
        let Message::Assistant { content, .. } = &messages[0] else {
            panic!("expected an assistant message first");
        };
        let blocks: Vec<&AssistantContent> = content.iter().collect();
        assert_eq!(blocks.len(), 2);
        let AssistantContent::Text(text) = blocks[0] else {
            panic!("expected narration as the leading block");
        };
        assert_eq!(text.text, "Let me check the archives.");
        assert!(matches!(blocks[1], AssistantContent::ToolCall(_)));

        // H15: no two consecutive assistant messages.
        for pair in messages.windows(2) {
            assert!(
                !matches!(
                    (&pair[0], &pair[1]),
                    (Message::Assistant { .. }, Message::Assistant { .. })
                ),
                "two consecutive assistant messages were emitted"
            );
        }

        // H16: every tool result is immediately preceded by the assistant message
        // carrying its tool call.
        for (index, message) in messages.iter().enumerate() {
            let Message::User { content } = message else {
                continue;
            };
            let ids: Vec<String> = content
                .iter()
                .filter_map(|item| match item {
                    UserContent::ToolResult(result) => Some(result.id.clone()),
                    _ => None,
                })
                .collect();
            if ids.is_empty() {
                continue;
            }
            let Some(Message::Assistant { content, .. }) = index.checked_sub(1).map(|i| &messages[i])
            else {
                panic!("a tool result is not preceded by an assistant message");
            };
            let called: Vec<String> = content
                .iter()
                .filter_map(|item| match item {
                    AssistantContent::ToolCall(call) => Some(call.id.clone()),
                    _ => None,
                })
                .collect();
            for id in ids {
                assert!(called.contains(&id), "tool result {id} is not adjacent to its call");
            }
        }
    }

    /// H17, FR-013. The regression guard: the change has to be invisible for every
    /// conversation recorded before it, none of which contains an `AssistantMessage`.
    #[test]
    fn history_without_narration_replays_unchanged() {
        let history = vec![
            entry(SessionHistoryContent::ToolCall {
                call_id: "call-1".to_string(),
                name: "memory_search".to_string(),
                arguments: json!({ "query": "rust releases" }),
            }),
            entry(SessionHistoryContent::ToolResult {
                call_id: "call-1".to_string(),
                content: "two hits".to_string(),
            }),
            entry(SessionHistoryContent::Response(VizierResponse {
                timestamp: chrono::Utc::now(),
                content: VizierResponseContent::Message {
                    content: "Two releases.".to_string(),
                    stats: None,
                },
                attachments: vec![],
                ..Default::default()
            })),
        ];

        let messages = history_entries_to_messages(&history);

        assert_eq!(messages.len(), 3);
        let Message::Assistant { content, .. } = &messages[0] else {
            panic!("expected the tool call as an assistant message");
        };
        let blocks: Vec<&AssistantContent> = content.iter().collect();
        assert_eq!(blocks.len(), 1, "no empty text block is prepended");
        assert!(matches!(blocks[0], AssistantContent::ToolCall(_)));
        assert!(matches!(messages[1], Message::User { .. }));
        let Message::Assistant { content, .. } = &messages[2] else {
            panic!("expected the final response as an assistant message");
        };
        assert!(matches!(content.first(), AssistantContent::Text(_)));
    }
}
