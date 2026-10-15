use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


use crate::llm::types::{AssistantPart, Message, Part};
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

/// Convert model messages to SessionHistoryContent entries (one-to-one, contract S4).
/// - `Message::ToolResult` → ToolResult entry
/// - `Message::Assistant` with tool calls → AssistantMessage (its narration, if any), then a
///   ToolCall entry per call
/// - `Message::Assistant` with text only → Response entry (final)
/// - System and user messages → skipped (the caller records the request itself)
/// - Reasoning is never persisted.
pub fn messages_to_history_entries(messages: &[Message]) -> Vec<SessionHistoryContent> {
    let mut entries = Vec::new();

    for msg in messages {
        match msg {
            Message::System(_) | Message::User(_) => {}
            Message::ToolResult { call_id, content } => {
                entries.push(SessionHistoryContent::ToolResult {
                    call_id: call_id.clone(),
                    content: parts_text(content),
                });
            }
            Message::Assistant(content) => {
                // Collected before anything is pushed, because the narration entry has to
                // land *before* the tool calls it accompanies: `history_entries_to_messages`
                // merges narration into the assistant message its tool calls flush as, and
                // it can only merge text it has already seen (FR-011).
                let mut text_parts = Vec::new();
                let mut calls = Vec::new();

                for item in content {
                    match item {
                        AssistantPart::Text(text) => text_parts.push(text.clone()),
                        AssistantPart::ToolCall {
                            id,
                            name,
                            arguments,
                            ..
                        } => calls.push(SessionHistoryContent::ToolCall {
                            call_id: id.clone(),
                            name: name.clone(),
                            arguments: arguments.clone(),
                        }),
                        AssistantPart::Reasoning { .. } => {}
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

/// Convert SessionHistory entries back to model messages. Consecutive ToolCall entries (with
/// the narration before them) become one assistant message; each ToolResult is its own
/// `Message::ToolResult`, and dialects group them as their wire format needs.
pub fn history_entries_to_messages(entries: &[SessionHistory]) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut pending_text: Option<String> = None;
    let mut pending_tool_calls: Vec<AssistantPart> = Vec::new();

    for entry in entries {
        match &entry.content {
            SessionHistoryContent::Request(req) => {
                flush_pending_tool_calls(&mut pending_text, &mut pending_tool_calls, &mut messages);

                if let Ok(text) = req.to_prompt()
                    && !text.is_empty()
                {
                    messages.push(Message::User(vec![Part::Text(text)]));
                }
            }
            SessionHistoryContent::Response(res) => {
                flush_pending_tool_calls(&mut pending_text, &mut pending_tool_calls, &mut messages);

                match &res.content {
                    VizierResponseContent::Message { content, .. } => {
                        if !content.is_empty() {
                            messages.push(Message::assistant_text(content.clone()));
                        }
                    }
                    VizierResponseContent::Error { kind, message } => {
                        messages.push(Message::User(vec![Part::Text(format!(
                            "[Error: {}] {}",
                            kind.as_str(),
                            message
                        ))]));
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

                if !text.is_empty() {
                    pending_text = Some(text.clone());
                }
            }
            SessionHistoryContent::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                pending_tool_calls.push(AssistantPart::ToolCall {
                    id: call_id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                    provider_data: None,
                });
            }
            SessionHistoryContent::ToolResult { call_id, content } => {
                flush_pending_tool_calls(&mut pending_text, &mut pending_tool_calls, &mut messages);
                messages.push(Message::tool_result(call_id.clone(), content.clone()));
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

    repair_tool_pairing(messages)
}

/// What a tool call with no recorded result replays as. Sessions saved before turns sealed
/// their own tool calls (`seal_tool_calls`) can hold calls whose results were never stored.
const NO_RESULT_RECORDED: &str = "[no result recorded: the turn ended before this tool returned]";

/// The ids of the tool calls an assistant message makes.
pub fn tool_call_ids(message: &Message) -> Vec<String> {
    let Message::Assistant(content) = message else {
        return vec![];
    };
    content
        .iter()
        .filter_map(|item| match item {
            AssistantPart::ToolCall { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn result_id(message: &Message) -> Option<&str> {
    match message {
        Message::ToolResult { call_id, .. } => Some(call_id),
        _ => None,
    }
}

/// Answers every tool call of the last assistant message that has no result yet.
///
/// A turn that fails part-way through its tool calls would otherwise leave a `tool_use` with
/// no `tool_result` after it, which every later request replays and the provider rejects, so
/// the session can never continue. `completed` holds the results that came back before the
/// failure; each call still unanswered after them gets `reason` as its result.
pub fn seal_tool_calls(history: &mut Vec<Message>, completed: Vec<Message>, reason: &str) {
    let Some(index) = history
        .iter()
        .rposition(|message| matches!(message, Message::Assistant(_)))
    else {
        return;
    };

    let mut answered: Vec<String> = history[index + 1..]
        .iter()
        .filter_map(result_id)
        .map(str::to_string)
        .collect();
    answered.extend(completed.iter().filter_map(result_id).map(str::to_string));

    let calls = tool_call_ids(&history[index]);
    history.extend(completed);
    for id in calls {
        if !answered.contains(&id) {
            history.push(Message::tool_result(
                id,
                format!("[tool did not return: {reason}]"),
            ));
        }
    }
}

/// Makes every assistant tool call be followed directly by a result for each of its ids, and
/// drops tool results that answer no call just before them.
///
/// Providers reject either shape, and a stored history holding one fails every turn after it.
/// This repairs the replay only; the stored entries are left as they are.
fn repair_tool_pairing(messages: Vec<Message>) -> Vec<Message> {
    let mut repaired = Vec::with_capacity(messages.len());
    let mut expected: Vec<String> = vec![];

    for message in messages {
        if let Some(id) = result_id(&message) {
            if let Some(position) = expected.iter().position(|call| call == id) {
                expected.remove(position);
                repaired.push(message);
            }
            continue;
        }
        // Missing results go right after the ones that did come back, ahead of anything else:
        // a tool result has to follow its call.
        repaired.extend(
            expected
                .drain(..)
                .map(|id| Message::tool_result(id, NO_RESULT_RECORDED)),
        );
        expected = tool_call_ids(&message);
        repaired.push(message);
    }

    repaired.extend(
        expected
            .drain(..)
            .map(|id| Message::tool_result(id, NO_RESULT_RECORDED)),
    );

    repaired
}

/// One assistant message out of the narration and the tool calls that came with it.
///
/// Narration leads, tool calls follow, which is the shape the model sent. Narration with no
/// tool calls after it — a turn that ended there — still becomes a message of its own, so
/// nothing recorded is silently dropped.
fn flush_pending_tool_calls(
    text: &mut Option<String>,
    calls: &mut Vec<AssistantPart>,
    messages: &mut Vec<Message>,
) {
    let text = text.take();
    if calls.is_empty() {
        if let Some(text) = text {
            messages.push(Message::assistant_text(text));
        }
        return;
    }

    let mut content: Vec<AssistantPart> = Vec::with_capacity(calls.len() + 1);
    content.extend(text.map(AssistantPart::Text));
    content.append(calls);
    messages.push(Message::Assistant(content));
}

fn parts_text(content: &[Part]) -> String {
    content
        .iter()
        .filter_map(Part::text)
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
    let Message::ToolResult { call_id, content } = message else {
        return message.clone();
    };
    // A cheap pre-check, so an ordinary tool result is never parsed as JSON.
    if !content
        .iter()
        .any(|p| p.text().is_some_and(|t| t.contains("\"tool_calls\"")))
    {
        return message.clone();
    }

    let narrowed = content
        .iter()
        .map(|part| match part {
            Part::Text(text) => serde_json::from_str(text)
                .ok()
                .as_ref()
                .and_then(ExecutionReport::model_view)
                .and_then(|view| serde_json::to_string(&view).ok())
                .map_or_else(|| part.clone(), Part::Text),
            other => other.clone(),
        })
        .collect();
    Message::ToolResult {
        call_id: call_id.clone(),
        content: narrowed,
    }
}

/// [`message_for_model`] over a whole history.
pub fn messages_for_model(messages: &[Message]) -> Vec<Message> {
    messages.iter().map(message_for_model).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn tool_result(id: &str, text: &str) -> Message {
        Message::tool_result(id, text)
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
        let Message::ToolResult { content, .. } = message else {
            panic!("expected a tool result");
        };
        parts_text(content)
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
    fn non_tool_messages_and_ids_are_preserved() {
        let assistant = Message::assistant_text("hello");
        assert_eq!(message_for_model(&assistant), assistant);

        let narrowed = message_for_model(&tool_result("call-7", &report_json()));
        assert_eq!(result_id(&narrowed), Some("call-7"));
    }

    #[test]
    fn messages_for_model_maps_a_whole_history() {
        let history = vec![
            Message::user_text("what changed?").unwrap(),
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

    fn call_part(id: &str) -> AssistantPart {
        AssistantPart::ToolCall {
            id: id.to_string(),
            name: "memory_search".to_string(),
            arguments: json!({ "query": "rust releases" }),
            provider_data: None,
        }
    }

    fn narrated_turn() -> Message {
        Message::Assistant(vec![
            AssistantPart::Text("Let me check the archives.".to_string()),
            call_part("call-1"),
        ])
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

    #[test]
    fn tool_results_and_reasoning_map_one_to_one() {
        let entries = messages_to_history_entries(&[
            Message::Assistant(vec![
                AssistantPart::Reasoning {
                    text: "hidden".into(),
                    provider_data: None,
                },
                call_part("a"),
                call_part("b"),
            ]),
            tool_result("a", "one"),
            tool_result("b", "two"),
        ]);
        assert_eq!(kinds(&entries), vec!["ToolCall", "ToolCall", "ToolResult", "ToolResult"]);
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
        assert_eq!(messages.len(), 2, "expected one assistant message then its result");
        let Message::Assistant(blocks) = &messages[0] else {
            panic!("expected an assistant message first");
        };
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            blocks[0],
            AssistantPart::Text("Let me check the archives.".to_string()),
            "narration leads"
        );
        assert!(matches!(blocks[1], AssistantPart::ToolCall { .. }));

        // H15: no two consecutive assistant messages.
        for pair in messages.windows(2) {
            assert!(
                !matches!((&pair[0], &pair[1]), (Message::Assistant(_), Message::Assistant(_))),
                "two consecutive assistant messages were emitted"
            );
        }

        // H16: every tool result follows the assistant message carrying its call.
        assert_paired(&messages);
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
        let Message::Assistant(blocks) = &messages[0] else {
            panic!("expected the tool call as an assistant message");
        };
        assert_eq!(blocks.len(), 1, "no empty text block is prepended");
        assert!(matches!(blocks[0], AssistantPart::ToolCall { .. }));
        assert!(matches!(messages[1], Message::ToolResult { .. }));
        assert_eq!(messages[2], Message::assistant_text("Two releases."));
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
        Message::Assistant(ids.iter().map(|id| call_part(id)).collect())
    }

    /// `(id, text)` for the run of tool results starting at `index`.
    fn results(messages: &[Message], index: usize) -> Vec<(String, String)> {
        messages[index..]
            .iter()
            .map_while(|m| match m {
                Message::ToolResult { call_id, content } => {
                    Some((call_id.clone(), parts_text(content)))
                }
                _ => None,
            })
            .collect()
    }

    /// Every tool call is answered by the results right after it, and every tool result
    /// answers a call in the assistant message right before its run.
    fn assert_paired(messages: &[Message]) {
        for (index, message) in messages.iter().enumerate() {
            let answered: Vec<String> = results(messages, index + 1).into_iter().map(|(id, _)| id).collect();
            for id in tool_call_ids(message) {
                assert!(answered.contains(&id), "tool call {id} has no result after it");
            }
            if let Some(id) = result_id(message) {
                let owner = messages[..index]
                    .iter()
                    .rev()
                    .find(|m| result_id(m).is_none())
                    .map(tool_call_ids)
                    .unwrap_or_default();
                assert!(owner.iter().any(|c| c == id), "tool result {id} answers no call before it");
            }
        }
    }

    #[test]
    fn a_tool_call_left_at_the_end_is_answered_on_replay() {
        let messages = history_entries_to_messages(&[call_entry("a")]);

        assert_paired(&messages);
        assert_eq!(messages.len(), 2);
        assert_eq!(results(&messages, 1), vec![("a".to_string(), NO_RESULT_RECORDED.to_string())]);
    }

    #[test]
    fn a_tool_call_followed_by_an_error_is_answered_before_the_error() {
        let messages = history_entries_to_messages(&[call_entry("a"), error_entry()]);

        assert_paired(&messages);
        assert!(
            matches!(messages[1], Message::ToolResult { .. }),
            "the tool result has to follow its call"
        );
        assert!(
            matches!(&messages[2], Message::User(parts) if parts[0].text().is_some_and(|t| t.starts_with("[Error"))),
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
            results(&messages, 1),
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
        let mut history = vec![Message::user_text("find it").unwrap(), calls_message(&["a", "b", "c"])];
        let completed = vec![tool_result("a", "two hits")];

        seal_tool_calls(&mut history, completed, "timed out after 60s");

        assert_paired(&history);
        assert_eq!(
            results(&history, 2),
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

        let mut history = vec![Message::user_text("hi").unwrap(), Message::assistant_text("hello")];
        seal_tool_calls(&mut history, vec![], "depth limit");
        assert_eq!(history.len(), 2);
    }
}
