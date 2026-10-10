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

    repair_tool_pairing(messages)
}

/// What a tool call with no recorded result replays as. Sessions saved before turns sealed
/// their own tool calls (`seal_tool_calls`) can hold calls whose results were never stored.
const NO_RESULT_RECORDED: &str = "[no result recorded: the turn ended before this tool returned]";

fn placeholder_result(call: &ToolCall, text: &str) -> UserContent {
    UserContent::ToolResult(rig_core::message::ToolResult {
        id: call.id.clone(),
        call_id: call.call_id.clone(),
        content: OneOrMany::one(ToolResultContent::text(text)),
    })
}

fn tool_calls_of(message: &Message) -> Vec<ToolCall> {
    let Message::Assistant { content, .. } = message else {
        return vec![];
    };
    content
        .iter()
        .filter_map(|item| match item {
            AssistantContent::ToolCall(call) => Some(call.clone()),
            _ => None,
        })
        .collect()
}

fn tool_result_ids(message: &Message) -> Vec<String> {
    let Message::User { content } = message else {
        return vec![];
    };
    content
        .iter()
        .filter_map(|item| match item {
            UserContent::ToolResult(result) => Some(result.id.clone()),
            _ => None,
        })
        .collect()
}

/// Answers every tool call of the last assistant message that has no result yet.
///
/// A turn that fails part-way through its tool calls would otherwise leave a `tool_use` with
/// no `tool_result` after it, which every later request replays and the provider rejects, so
/// the session can never continue. `completed` holds the results that came back before the
/// failure; each call still unanswered after them gets `reason` as its result.
pub fn seal_tool_calls(history: &mut Vec<Message>, completed: Vec<UserContent>, reason: &str) {
    let Some(index) = history
        .iter()
        .rposition(|message| matches!(message, Message::Assistant { .. }))
    else {
        return;
    };

    let mut answered: Vec<String> = history[index + 1..]
        .iter()
        .flat_map(tool_result_ids)
        .collect();
    answered.extend(completed.iter().filter_map(|item| match item {
        UserContent::ToolResult(result) => Some(result.id.clone()),
        _ => None,
    }));

    let mut content = completed;
    for call in tool_calls_of(&history[index]) {
        if !answered.contains(&call.id) {
            content.push(placeholder_result(&call, &format!("[tool did not return: {reason}]")));
        }
    }

    if let Ok(content) = OneOrMany::many(content) {
        history.push(Message::User { content });
    }
}

/// Makes every assistant tool call be followed directly by a result for each of its ids, and
/// drops tool results that answer no call just before them.
///
/// Providers reject either shape, and a stored history holding one fails every turn after it.
/// This repairs the replay only; the stored entries are left as they are.
fn repair_tool_pairing(messages: Vec<Message>) -> Vec<Message> {
    let mut repaired = Vec::with_capacity(messages.len());
    let mut expected: Vec<ToolCall> = vec![];

    for message in messages {
        let Message::User { content } = &message else {
            repaired.extend(unanswered_message(&mut expected));
            expected = tool_calls_of(&message);
            repaired.push(message);
            continue;
        };

        let mut kept: Vec<UserContent> = vec![];
        for item in content.iter() {
            match item {
                UserContent::ToolResult(result) => {
                    if let Some(position) = expected.iter().position(|call| call.id == result.id) {
                        expected.remove(position);
                        kept.push(item.clone());
                    }
                }
                other => kept.push(other.clone()),
            }
        }
        // Missing results go with the ones that did come back, ahead of anything else the
        // message carries: a tool result has to lead the message that follows its call.
        let (results, others): (Vec<UserContent>, Vec<UserContent>) = kept
            .into_iter()
            .partition(|item| matches!(item, UserContent::ToolResult(_)));
        let mut content: Vec<UserContent> = results;
        content.extend(
            expected
                .drain(..)
                .map(|call| placeholder_result(&call, NO_RESULT_RECORDED)),
        );
        content.extend(others);

        if let Ok(content) = OneOrMany::many(content) {
            repaired.push(Message::User { content });
        }
    }

    repaired.extend(unanswered_message(&mut expected));

    repaired
}

/// A tool-result message answering each of `expected`, or nothing when it is empty.
fn unanswered_message(expected: &mut Vec<ToolCall>) -> Option<Message> {
    let placeholders: Vec<UserContent> = expected
        .drain(..)
        .map(|call| placeholder_result(&call, NO_RESULT_RECORDED))
        .collect();
    OneOrMany::many(placeholders)
        .ok()
        .map(|content| Message::User { content })
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

    // ---------------------------------------------------------------------------------
    // Tool-call pairing: a failed turn must never leave a tool call without a result
    // (issue #58).
    // ---------------------------------------------------------------------------------

    fn call_entry(id: &str) -> SessionHistory {
        entry(SessionHistoryContent::ToolCall {
            call_id: id.to_string(),
            name: "memory_search".to_string(),
            arguments: json!({}),
        })
    }

    fn result_entry(id: &str, text: &str) -> SessionHistory {
        entry(SessionHistoryContent::ToolResult {
            call_id: id.to_string(),
            content: text.to_string(),
        })
    }

    fn error_entry() -> SessionHistory {
        entry(SessionHistoryContent::Response(VizierResponse {
            timestamp: chrono::Utc::now(),
            content: VizierResponseContent::Error {
                kind: crate::schema::ErrorKind::ToolTimeout,
                message: "Tool 'memory_search' timed out".to_string(),
            },
            attachments: vec![],
            ..Default::default()
        }))
    }

    fn calls_message(ids: &[&str]) -> Message {
        Message::Assistant {
            id: None,
            content: OneOrMany::many(
                ids.iter()
                    .map(|id| {
                        AssistantContent::ToolCall(ToolCall {
                            id: id.to_string(),
                            call_id: None,
                            function: ToolFunction {
                                name: "memory_search".to_string(),
                                arguments: json!({}),
                            },
                            signature: None,
                            additional_params: None,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        }
    }

    /// `(id, text)` for every tool result in a user message.
    fn results(message: &Message) -> Vec<(String, String)> {
        let Message::User { content } = message else {
            panic!("expected a user message, got {message:?}");
        };
        content
            .iter()
            .filter_map(|item| match item {
                UserContent::ToolResult(result) => {
                    Some((result.id.clone(), tool_result_content_to_text(&result.content)))
                }
                _ => None,
            })
            .collect()
    }

    /// Every tool call is answered by the message right after it, and every tool result
    /// answers a call in the message right before it.
    fn assert_paired(messages: &[Message]) {
        for (index, message) in messages.iter().enumerate() {
            let calls: Vec<String> = tool_calls_of(message).into_iter().map(|c| c.id).collect();
            let answered = messages.get(index + 1).map(tool_result_ids).unwrap_or_default();
            for id in &calls {
                assert!(answered.contains(id), "tool call {id} has no result after it");
            }

            let previous = index
                .checked_sub(1)
                .map(|i| tool_calls_of(&messages[i]))
                .unwrap_or_default();
            for id in tool_result_ids(message) {
                assert!(
                    previous.iter().any(|call| call.id == id),
                    "tool result {id} answers no call before it"
                );
            }
        }
    }

    #[test]
    fn a_tool_call_left_at_the_end_is_answered_on_replay() {
        let messages = history_entries_to_messages(&[call_entry("a")]);

        assert_paired(&messages);
        assert_eq!(messages.len(), 2);
        assert_eq!(results(&messages[1]), vec![("a".to_string(), NO_RESULT_RECORDED.to_string())]);
    }

    #[test]
    fn a_tool_call_followed_by_an_error_is_answered_before_the_error() {
        let messages = history_entries_to_messages(&[call_entry("a"), error_entry()]);

        assert_paired(&messages);
        let Message::User { content } = &messages[1] else {
            panic!("expected a user message after the call");
        };
        assert!(
            matches!(content.first(), UserContent::ToolResult(_)),
            "the tool result has to lead the message"
        );
        assert!(
            content.iter().any(|c| matches!(c, UserContent::Text(t) if t.text.starts_with("[Error"))),
            "the error is still replayed"
        );
    }

    #[test]
    fn a_partly_answered_batch_keeps_its_real_results() {
        let messages = history_entries_to_messages(&[
            call_entry("a"),
            call_entry("b"),
            result_entry("a", "two hits"),
            error_entry(),
        ]);

        assert_paired(&messages);
        assert_eq!(
            results(&messages[1]),
            vec![
                ("a".to_string(), "two hits".to_string()),
                ("b".to_string(), NO_RESULT_RECORDED.to_string()),
            ]
        );
    }

    #[test]
    fn a_tool_result_with_no_call_is_dropped() {
        let request = entry(SessionHistoryContent::Response(VizierResponse {
            timestamp: chrono::Utc::now(),
            content: VizierResponseContent::Message {
                content: "hello".to_string(),
                stats: None,
            },
            attachments: vec![],
            ..Default::default()
        }));
        let messages = history_entries_to_messages(&[request, result_entry("ghost", "stale")]);

        assert_paired(&messages);
        assert_eq!(messages.len(), 1, "the orphaned result is not replayed");
    }

    #[test]
    fn sealing_answers_only_the_calls_that_did_not_return() {
        let mut history = vec![Message::user("find it"), calls_message(&["a", "b", "c"])];
        let completed = vec![UserContent::tool_result(
            "a",
            OneOrMany::one(ToolResultContent::text("two hits")),
        )];

        seal_tool_calls(&mut history, completed, "timed out after 60s");

        assert_paired(&history);
        assert_eq!(
            results(&history[2]),
            vec![
                ("a".to_string(), "two hits".to_string()),
                ("b".to_string(), "[tool did not return: timed out after 60s]".to_string()),
                ("c".to_string(), "[tool did not return: timed out after 60s]".to_string()),
            ]
        );
    }

    #[test]
    fn sealing_an_answered_turn_changes_nothing() {
        let mut history = vec![calls_message(&["a"]), tool_result("a", "two hits")];
        seal_tool_calls(&mut history, vec![], "depth limit");
        assert_eq!(history.len(), 2);

        let mut history = vec![Message::user("hi"), Message::assistant("hello")];
        seal_tool_calls(&mut history, vec![], "depth limit");
        assert_eq!(history.len(), 2);
    }
}
