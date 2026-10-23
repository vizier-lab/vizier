//! The steps of a turn, recorded as they happen (`specs/023-persist-turn-activity`).
//!
//! `record_step` saves one history entry and pushes it to whoever watches the session, so a
//! page opened mid-turn reads the same trail as one that watched it live. `seal_open_calls`
//! closes a tool call a stop cut off, so history never holds a call without a result.

use crate::{
    llm::types::Message,
    schema::{
        SessionHistory, SessionHistoryContent, VizierSession, messages_to_history_entries,
        open_tool_calls,
    },
    storage::{VizierStorage, history::HistoryStorage},
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

/// How many times a turn may be resumed without a single new step before it is given up on.
pub const MAX_RESUMES_WITHOUT_PROGRESS: u32 = 3;

/// Why a turn stopped with a tool call still open. The text is what the agent reads as the
/// call's result, and the WebUI matches its `Stopped:`/`Interrupted:` prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Restart,
    Aborted,
    AgentRestarted,
    InternalError,
}

impl StopReason {
    pub fn notice(&self) -> &'static str {
        match self {
            StopReason::Restart => {
                "Interrupted: vizier restarted before this call finished, so its outcome is unknown. It was not run again."
            }
            StopReason::Aborted => {
                "Stopped: the person aborted the turn before this call finished, so its outcome is unknown. It was not run again."
            }
            StopReason::AgentRestarted => {
                "Stopped: the agent was restarted before this call finished, so its outcome is unknown. It was not run again."
            }
            StopReason::InternalError => {
                "Stopped: the turn failed with an internal error before this call finished, so its outcome is unknown. It was not run again."
            }
        }
    }
}

/// Saves one step and publishes it as `SessionFrame::HistoryEntry`. A failure to save is
/// logged and returns `None`; it never fails the turn.
pub async fn record_step(
    storage: &VizierStorage,
    transport: &VizierTransport,
    session: &VizierSession,
    content: SessionHistoryContent,
) -> Option<String> {
    match crate::storage::history::save_with_busy_retry(storage, session, content.clone()).await {
        Ok(uid) => {
            transport.publish_session_event(SessionEvent {
                session: session.clone(),
                frame: SessionFrame::HistoryEntry(SessionHistory {
                    uid: uid.clone(),
                    vizier_session: session.clone(),
                    content,
                    timestamp: chrono::Utc::now(),
                    reactions: vec![],
                    seq: None,
                }),
            });
            Some(uid)
        }
        Err(e) => {
            tracing::error!(
                agent_id = %session.0,
                "failed to record a turn step, the turn continues without it: {e}"
            );
            None
        }
    }
}

/// Answers every tool call after the request (the entry with `request_uid`, or the last
/// `Request` when `None`) that has no result, with `reason`'s notice. Returns how many.
pub async fn seal_open_calls(
    storage: &VizierStorage,
    transport: &VizierTransport,
    session: &VizierSession,
    request_uid: Option<&str>,
    reason: StopReason,
) -> usize {
    let entries = match storage
        .list_session_history(session.clone(), None, None, None)
        .await
    {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(agent_id = %session.0, "could not read history to seal open calls: {e}");
            return 0;
        }
    };

    let start = match request_uid {
        Some(uid) => entries.iter().position(|e| e.uid == uid),
        None => entries
            .iter()
            .rposition(|e| matches!(e.content, SessionHistoryContent::Request(_))),
    };
    let Some(start) = start else {
        return 0;
    };

    let open = open_tool_calls(&entries[start + 1..]);
    for call_id in &open {
        record_step(
            storage,
            transport,
            session,
            SessionHistoryContent::ToolResult {
                call_id: call_id.clone(),
                content: reason.notice().to_string(),
            },
        )
        .await;
    }
    open.len()
}

/// The history entries one model message produces.
pub fn entries_for(messages: &[Message]) -> Vec<SessionHistoryContent> {
    messages_to_history_entries(messages)
}

/// Records a turn's messages one at a time as the model loop produces them, and at the end
/// the entries it did not. `recorded` counts history entries already saved for the turn's
/// messages (a message may make several, or none), so `flush` can tell what is left.
pub struct StepRecorder {
    session: VizierSession,
    pub recorded: usize,
}

impl StepRecorder {
    pub fn new(session: VizierSession, recorded: usize) -> Self {
        Self { session, recorded }
    }

    pub async fn record(
        &mut self,
        storage: &VizierStorage,
        transport: &VizierTransport,
        message: &Message,
    ) {
        for content in entries_for(std::slice::from_ref(message)) {
            record_step(storage, transport, &self.session, content).await;
            self.recorded += 1;
        }
    }

    /// Records the entries of `delta` (the turn's messages, in order) beyond those already
    /// recorded.
    pub async fn flush(
        &mut self,
        storage: &VizierStorage,
        transport: &VizierTransport,
        delta: &[Message],
    ) {
        let entries = entries_for(delta);
        for content in entries.into_iter().skip(self.recorded) {
            record_step(storage, transport, &self.session, content).await;
            self.recorded += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::AssistantPart;
    use serde_json::json;

    fn call(id: &str) -> AssistantPart {
        AssistantPart::ToolCall {
            id: id.to_string(),
            name: "t".to_string(),
            arguments: json!({}),
            provider_data: None,
        }
    }

    fn kinds(entries: &[SessionHistoryContent]) -> Vec<String> {
        entries.iter().map(|e| format!("{e:?}").split(['(', ' ']).next().unwrap_or("").to_string()).collect()
    }

    #[test]
    fn recording_message_by_message_equals_converting_the_whole_delta() {
        let delta = vec![
            Message::Assistant(vec![AssistantPart::Text("narration".into()), call("a"), call("b")]),
            Message::tool_result("a".to_string(), "ra".to_string()),
            Message::tool_result("b".to_string(), "rb".to_string()),
            Message::Assistant(vec![call("c")]),
            Message::tool_result("c".to_string(), "rc".to_string()),
            Message::assistant_text("done".to_string()),
        ];
        let body = &delta[..delta.len() - 1];
        let one_by_one: Vec<_> = body
            .iter()
            .flat_map(|m| entries_for(std::slice::from_ref(m)))
            .collect();
        let whole = entries_for(body);
        assert_eq!(kinds(&one_by_one), kinds(&whole));
        assert_eq!(
            kinds(&whole),
            vec!["AssistantMessage", "ToolCall", "ToolCall", "ToolResult", "ToolResult", "ToolCall", "ToolResult"]
        );
    }

    #[test]
    fn every_stop_notice_is_recognisable_to_the_webui() {
        for reason in [StopReason::Restart, StopReason::Aborted, StopReason::AgentRestarted, StopReason::InternalError] {
            let n = reason.notice();
            assert!(n.starts_with("Stopped:") || n.starts_with("Interrupted:"));
        }
    }
}
