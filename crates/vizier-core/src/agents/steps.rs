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

/// Whether a turn resumed `resumes` times in a row without a new step has used up its chances.
pub fn exceeds_resume_limit(resumes: u32) -> bool {
    resumes > MAX_RESUMES_WITHOUT_PROGRESS
}

/// Which `session_turn` row a turn belongs to, and whether it is a resumption.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TurnRef {
    pub id: Option<i64>,
    pub resumed: bool,
}

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
            // A message an agent shows in a Workspace was written to the shared timeline by the
            // save itself; announce it and wake who it names (spec 053).
            if matches!(session.1, crate::schema::VizierChannelId::Workspace(_))
                && matches!(
                    content,
                    SessionHistoryContent::AssistantMessage(_) | SessionHistoryContent::Response(_)
                )
            {
                let hub = crate::workspaces::Hub::new(storage.clone(), transport.clone());
                crate::workspaces::after_save(&hub, &uid).await;
            }
            // A request keeps the time it was made, not the time it was saved.
            let timestamp = match &content {
                SessionHistoryContent::Request(request) => request.timestamp,
                _ => crate::time::now(),
            };
            transport.publish_session_event(SessionEvent {
                session: session.clone(),
                frame: SessionFrame::HistoryEntry(SessionHistory {
                    uid: uid.clone(),
                    vizier_session: session.clone(),
                    content,
                    timestamp,
                    reactions: vec![],
                    seq: None,
                }),
            });
            Some(uid)
        }
        // A newer owner holds this agent: the write was refused whole, and the watch on the
        // scope is what stops the agent here (specs/057 research R4). Not an error to log.
        Err(e) if crate::storage::fence::is_ownership_lost(&e) => {
            tracing::debug!(agent_id = %session.0, "a turn step was refused: {e}");
            None
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
        // A detached script's delivery is saved mid-turn as a `Request` (spec 058), so the
        // turn starts at the last request that is not one; a turn a delivery woke has only
        // deliveries since the last person's request, and starting earlier only scans calls
        // that are already answered.
        None => entries
            .iter()
            .rposition(|e| {
                matches!(&e.content, SessionHistoryContent::Request(r)
                    if !matches!(r.content, crate::schema::VizierRequestContent::DetachedDelivery(_)))
            })
            .or_else(|| {
                entries
                    .iter()
                    .position(|e| matches!(e.content, SessionHistoryContent::Request(_)))
            }),
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

    /// Saves the entries of `message` and returns the uids they were saved under, in entry
    /// order; a save that failed contributes none. A tool result's uid is the id a pruning
    /// placeholder names (spec 060).
    pub async fn record(
        &mut self,
        storage: &VizierStorage,
        transport: &VizierTransport,
        message: &Message,
    ) -> Vec<String> {
        let mut uids = Vec::new();
        for content in entries_for(std::slice::from_ref(message)) {
            uids.extend(record_step(storage, transport, &self.session, content).await);
            self.recorded += 1;
        }
        uids
    }

    /// Records one entry that is not a model message (a background delivery). It does not count toward
    /// `recorded`, which indexes the turn's messages.
    pub async fn record_entry(
        &mut self,
        storage: &VizierStorage,
        transport: &VizierTransport,
        content: SessionHistoryContent,
    ) {
        record_step(storage, transport, &self.session, content).await;
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
    fn the_resume_guard_trips_on_the_fourth_resume_without_progress() {
        assert!(!exceeds_resume_limit(1));
        assert!(!exceeds_resume_limit(MAX_RESUMES_WITHOUT_PROGRESS));
        assert!(exceeds_resume_limit(MAX_RESUMES_WITHOUT_PROGRESS + 1));
    }

    #[test]
    fn a_recorded_step_is_saved_and_published_together() {
        // What `run_checkpoint`, `run_lobotomy` and `answer_directly` all rely on: one call
        // saves the entry *and* pushes it, so a page watching the session gets a divider it
        // would otherwise only see after a reload.
        let storage = crate::agents::client::test_storage();
        let transport = VizierTransport::new();
        let session = VizierSession(
            "oz".into(),
            crate::schema::VizierChannelId::from_web("me", "c"),
            Some("t".into()),
        );
        let mut events = transport.subscribe_session_events();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        runtime.block_on(async {
            let uid = record_step(
                &storage,
                &transport,
                &session,
                SessionHistoryContent::Command("lobotomy".to_string()),
            )
            .await
            .expect("the step is recorded");

            // Published as part of the same call; delivery takes one hop through the event pump.
            let event = tokio::time::timeout(std::time::Duration::from_secs(5), events.recv())
                .await
                .expect("and published by the same call")
                .expect("an open channel");
            let SessionFrame::HistoryEntry(entry) = event.frame else {
                panic!("a recorded step is published as a history entry")
            };
            assert_eq!(entry.uid, uid);
            assert!(matches!(
                &entry.content,
                SessionHistoryContent::Command(command) if command == "lobotomy"
            ));

            let stored = crate::storage::history::HistoryStorage::list_session_history(
                &*storage, session.clone(), None, None, None,
            )
            .await
            .unwrap();
            assert_eq!(stored.len(), 1);
            assert!(matches!(
                &stored[0].content,
                SessionHistoryContent::Command(command) if command == "lobotomy"
            ));
        });
    }

    #[test]
    fn every_stop_notice_is_recognisable_to_the_webui() {
        for reason in [StopReason::Restart, StopReason::Aborted, StopReason::AgentRestarted, StopReason::InternalError] {
            let n = reason.notice();
            assert!(n.starts_with("Stopped:") || n.starts_with("Interrupted:"));
        }
    }
}
