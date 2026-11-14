mod prompts;

use std::str::FromStr;

use anyhow::Result;
use chrono::Duration;
use croner::Cron;

use crate::{
    dependencies::VizierDependencies,
    schema::{
        DreamStage, DreamStatus, VizierChannelId, VizierRequest, VizierRequestContent,
        VizierResponse, VizierResponseContent, VizierSession,
    },
    storage::{
        agent::AgentStorage, dream::DreamStorage, dream_journal::DreamJournalStorage,
        history::HistoryStorage,
    },
};

pub use prompts::{CONSOLIDATION_PROMPT_TEMPLATE, EXTRACTION_PROMPT};

/// The extraction request's metadata key holding the start of the cycle's window.
pub(crate) const DREAM_SINCE_KEY: &str = "dream_since";

/// What one extraction reflects on: the conversation recorded in `(since, end]`, and the
/// handover of the last checkpoint before `since` as a preface, so the dream knows what came
/// before without reading it again. `None` when the window holds no conversation (only
/// checkpoint or command rows, such as the pre-dream checkpoint itself), so the session is
/// skipped.
///
/// It used to read the session from its very first entry every cycle, so a long-lived
/// conversation was paid for in full each night however little of it was new.
pub(crate) async fn extraction_source(
    storage: &crate::storage::VizierStorage,
    session: &VizierSession,
    since: crate::time::Timestamp,
    end: crate::time::Timestamp,
) -> Result<Option<(Vec<crate::schema::SessionHistory>, Option<String>)>> {
    let history = storage
        .list_session_by_time_window(session.clone(), Some(since), Some(end))
        .await?;
    if !has_conversation(&history) {
        return Ok(None);
    }
    let (_, handover) = storage
        .list_session_history_until_checkpoint(session.clone(), Some(since))
        .await?;
    Ok(Some((history, handover)))
}

/// Whether `history` holds anything a dream could learn from: a checkpoint or a command row
/// alone does not count.
fn has_conversation(history: &[crate::schema::SessionHistory]) -> bool {
    use crate::schema::SessionHistoryContent;
    history.iter().any(|entry| {
        !matches!(
            entry.content,
            SessionHistoryContent::Checkpoint(_) | SessionHistoryContent::Command(_)
        )
    })
}

pub struct DreamScheduler {
    deps: VizierDependencies,
}

impl DreamScheduler {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }

    /// Check all agents and trigger dream cycles as needed (called every scheduler tick)
    pub async fn tick(&self) -> Result<()> {
        let agents = self.deps.storage.list_agents().await?;

        for (agent_id, config) in agents {
            if !config.dream_enabled {
                continue;
            }

            // Check if already dreaming
            let status = self
                .deps
                .storage
                .get_dream_status(&agent_id)
                .await
                .unwrap_or(None);

            if !matches!(status, Some(DreamStatus::Idle) | None) {
                continue;
            }

            // Parse cron
            let cron_str = match &config.dream_schedule {
                Some(s) => s.clone(),
                None => continue,
            };
            let cron = match Cron::from_str(&cron_str) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "Invalid dream cron expression for agent '{}': {}",
                        agent_id,
                        e
                    );
                    continue;
                }
            };
            let last = self
                .deps
                .storage
                .get_last_dream_time(&agent_id)
                .await?
                .unwrap_or(crate::time::now() - Duration::hours(24));
            let next = match cron.find_next_occurrence(&last, true) {
                Ok(n) => n,
                Err(e) => {
                    tracing::warn!(
                        "Failed to find next dream occurrence for agent '{}': {}",
                        agent_id,
                        e
                    );
                    continue;
                }
            };

            if next <= crate::time::now()
                && let Err(e) = self.trigger_dream(&agent_id).await
            {
                tracing::error!("Failed to trigger dream for agent '{}': {}", agent_id, e);
            }
        }

        Ok(())
    }

    /// Whether a frame ends a dream turn: the shared predicate, so a failed extraction is over
    /// like an answered one and the cycle never waits on a channel that stays open (FR-017).
    fn is_final_response(response: &VizierResponse) -> bool {
        response.ends_turn()
    }

    /// The ended-session explanation a dream turn failed with, when that is what ended it
    /// (FR-016, FR-017). Every extraction of one cycle uses the same session, so the first one
    /// that names it is the cycle's answer: continuing would be one failing turn per session and
    /// a consolidation that cannot work either.
    fn session_failure(response: &VizierResponse) -> Option<&str> {
        match &response.content {
            VizierResponseContent::Error { message, .. }
                if crate::llm::sso::is_session_failure_message(message) =>
            {
                Some(message)
            }
            _ => None,
        }
    }

    /// Brings the agent's idle Workspace sessions up to date, so the cycle sees what it read
    /// but was never woken for. A session with a turn pending is left alone: that turn catches
    /// up itself. Best effort; a failure only means the dream sees a little less.
    async fn catch_up_workspaces(&self, agent_id: &str) {
        use crate::storage::turn::TurnStorage;
        use crate::storage::workspace::WorkspaceStorage;
        let hub = crate::workspaces::Hub::from(&self.deps);
        let storage = &self.deps.storage;
        let busy = storage.sessions_with_pending_turns().await.unwrap_or_default();
        let workspaces = match storage.workspaces_of_agent(agent_id).await {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!("Workspace catch-up skipped for '{}': {}", agent_id, e);
                return;
            }
        };
        for workspace_id in workspaces {
            let topics = storage.list_topics(&workspace_id).await.unwrap_or_default();
            for topic in topics {
                let session = crate::workspaces::session_of(agent_id, &workspace_id, &topic.id);
                if busy.iter().any(|b| b.to_slug() == session.to_slug()) {
                    continue;
                }
                if let Some(Err(e)) = crate::workspaces::catch_up::to_head(&hub, &session).await {
                    tracing::warn!("Workspace catch-up failed for {}: {}", session.to_slug(), e);
                }
            }
        }
    }

    /// Trigger a dream cycle for a specific agent.
    /// Returns immediately — the actual dream runs in a background task.
    pub async fn trigger_dream(&self, agent_id: &str) -> Result<()> {
        let config = self
            .deps
            .storage
            .get_agent(agent_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("agent not found"))?;

        if !config.dream_enabled {
            return Ok(());
        }

        let cycle_id = uuid::Uuid::new_v4().to_string();
        let now = crate::time::now();
        let last_dream = self
            .deps
            .storage
            .get_last_dream_time(agent_id)
            .await?
            .unwrap_or(now - Duration::hours(24));

        self.catch_up_workspaces(agent_id).await;

        // Collect user-interaction sessions in window
        let sessions = self
            .deps
            .storage
            .list_user_sessions_in_window(agent_id, last_dream, now)
            .await?;

        if sessions.is_empty() {
            tracing::info!("No sessions to dream about for agent '{}'", agent_id);
            return Ok(());
        }

        let total = sessions.len();

        // Pre-dream checkpoint: delegate to the agent's checkpoint command for each
        // source session so extraction sees a Checkpoint row as the prior context.
        if let Err(e) = self.pre_check_sessions(agent_id, &sessions).await {
            tracing::warn!(
                "Pre-dream checkpoint phase failed for agent '{}': {}",
                agent_id,
                e
            );
        }

        // Set status → Extracting
        self.deps
            .storage
            .set_dream_status(
                agent_id,
                DreamStatus::Extracting {
                    started_at: now,
                    cycle_id: cycle_id.clone(),
                    total_sessions: total,
                    completed_sessions: 0,
                },
            )
            .await?;

        // Update last dream time
        self.deps.storage.set_last_dream_time(agent_id, now).await?;

        // Spawn background task for the full dream cycle
        let deps = self.deps.clone();
        let agent_id = agent_id.to_string();
        let source_sessions: Vec<VizierSession> = sessions.clone();
        let source_session_slugs: Vec<String> = sessions.iter().map(|s| s.to_slug()).collect();

        tokio::spawn(async move {
            // Send all extraction requests, collect response channels
            let mut response_rxs = vec![];
            for session in &sessions {
                let Ok((resp_tx, resp_rx)) = deps.transport.reply_channel().await else {
                    continue;
                };

                if let Err(e) = deps
                    .transport
                    .send_request(
                        VizierSession(
                            agent_id.clone(),
                            VizierChannelId::Dream(
                                Box::new(session.clone()),
                                DreamStage::Extraction,
                            ),
                            Some(cycle_id.clone()),
                        ),
                        VizierRequest {
                            timestamp: crate::time::now(),
                            user: agent_id.clone(),
                            content: VizierRequestContent::Unattended(
                                EXTRACTION_PROMPT.to_string(),
                            ),
                            metadata: serde_json::json!({
                                "dream_cycle_id": cycle_id,
                                // Where this cycle's window starts: extraction reads only the
                                // conversation since then (see `extraction_source`).
                                DREAM_SINCE_KEY: last_dream,
                            }),
                            ..Default::default()
                        },
                        Some(resp_tx),
                    )
                    .await
                {
                    tracing::error!(
                        "Failed to send extraction request for '{}': {}",
                        session.to_slug(),
                        e
                    );
                    continue;
                }
                response_rxs.push(resp_rx);
            }

            // Await ALL extraction responses, updating progress as each completes — unless one
            // says the session ended, which fails every remaining extraction the same way.
            let mut ended_session: Option<String> = None;
            for (i, rx) in response_rxs.iter().enumerate() {
                while let Ok(response) = rx.recv_async().await {
                    if let Some(message) = Self::session_failure(&response) {
                        ended_session = Some(message.to_string());
                    }
                    if Self::is_final_response(&response) {
                        break;
                    }
                }

                if ended_session.is_some() {
                    break;
                }

                // Update progress
                let _ = deps
                    .storage
                    .set_dream_status(
                        &agent_id,
                        DreamStatus::Extracting {
                            started_at: now,
                            cycle_id: cycle_id.clone(),
                            total_sessions: total,
                            completed_sessions: i + 1,
                        },
                    )
                    .await;
            }

            // A cycle whose session ended stops here: nothing it could consolidate was
            // extracted, and the people who need to know are the ones the connection's health
            // reports it to (FR-017, FR-018).
            if let Some(message) = ended_session {
                tracing::warn!(
                    agent_id,
                    "the dream cycle for '{agent_id}' stopped: {message}"
                );
                let _ = deps
                    .storage
                    .set_dream_status(&agent_id, DreamStatus::Idle)
                    .await;
                return;
            }

            // Set status → Consolidating
            let _ = deps
                .storage
                .set_dream_status(
                    &agent_id,
                    DreamStatus::Consolidating {
                        started_at: crate::time::now(),
                        cycle_id: cycle_id.clone(),
                    },
                )
                .await;

            // Trigger consolidation
            match Self::do_consolidation(&deps, &agent_id, &cycle_id, &source_sessions).await {
                Ok(resp_rx) => {
                    // Await consolidation response
                    while let Ok(response) = resp_rx.recv_async().await {
                        if Self::is_final_response(&response) {
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Consolidation failed for '{}': {}", agent_id, e);
                }
            }

            // A cycle that journaled something is news; a cycle that wrote nothing is not.
            let entries = deps
                .storage
                .list_dream_entries_by_cycle(agent_id.clone(), &cycle_id, None)
                .await
                .map(|entries| entries.len() as u64)
                .unwrap_or(0);
            deps.notifications
                .dream_ended(&agent_id, &cycle_id, entries, crate::time::now())
                .await;

            // Set status → Idle
            let _ = deps
                .storage
                .set_dream_status(&agent_id, DreamStatus::Idle)
                .await;
        });

        Ok(())
    }

    /// Send consolidation request and return the response channel receiver.
    async fn do_consolidation(
        deps: &VizierDependencies,
        agent_id: &str,
        cycle_id: &str,
        source_sessions: &[VizierSession],
    ) -> Result<crate::transport::ResponseStream> {
        let source_session_slugs: Vec<String> =
            source_sessions.iter().map(|s| s.to_slug()).collect();
        // Read all extraction entries for this cycle
        let extractions = deps
            .storage
            .list_dream_entries_by_cycle(
                agent_id.to_string(),
                cycle_id,
                Some(DreamStage::Extraction),
            )
            .await?;

        if extractions.is_empty() {
            tracing::warn!(
                "No extraction entries found for cycle '{}', skipping consolidation",
                cycle_id
            );
            // Return a channel that immediately gets an empty response
            let (tx, rx) = deps.transport.reply_channel().await?;
            let _ = tx
                .send_async(VizierResponse {
                    timestamp: crate::time::now(),
                    content: VizierResponseContent::Empty,
                    attachments: vec![],
                    ..Default::default()
                })
                .await;
            return Ok(rx);
        }

        // Aggregate extraction content
        let aggregated = extractions
            .iter()
            .map(|e| {
                let session = e.session_context.as_deref().unwrap_or("unknown");
                format!("---\nSession: {}\n{}\n", session, e.content)
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Build consolidation prompt
        let prompt = CONSOLIDATION_PROMPT_TEMPLATE.replace("{extraction_content}", &aggregated);

        // Send consolidation request with response channel
        let now = crate::time::now();
        let (resp_tx, resp_rx) = deps.transport.reply_channel().await?;

        deps.transport
            .send_request(
                VizierSession(
                    agent_id.to_string(),
                    VizierChannelId::Dream(
                        Box::new(VizierSession(
                            agent_id.to_string(),
                            VizierChannelId::System,
                            Some("dream-consolidation".to_string()),
                        )),
                        DreamStage::Consolidation,
                    ),
                    Some(cycle_id.to_string()),
                ),
                VizierRequest {
                    timestamp: now,
                    user: agent_id.to_string(),
                    content: VizierRequestContent::Unattended(prompt),
                    metadata: serde_json::json!({
                        "dream_cycle_id": cycle_id,
                        "source_sessions": source_session_slugs,
                        // The actual sessions, so a dream turn can read/promote from exactly
                        // the conversations this cycle reflects on (specs/035 R16).
                        "source_session_data": source_sessions,
                    }),
                    ..Default::default()
                },
                Some(resp_tx),
            )
            .await?;

        Ok(resp_rx)
    }

    /// Get dream status for an agent
    pub async fn get_status(&self, agent_id: &str) -> Result<DreamStatus> {
        Ok(self
            .deps
            .storage
            .get_dream_status(agent_id)
            .await?
            .unwrap_or(DreamStatus::Idle))
    }

    /// Delegate checkpointing to the agent's existing `Command("checkpoint")`
    /// handler for each source session. The agent's handler aborts the session,
    /// generates a handover, persists a `Checkpoint` row, and replies with a
    /// `VizierResponseContent::Checkpoint` we wait for.
    ///
    /// Sessions with no new history since the last checkpoint are skipped so
    /// we don't redundantly regenerate the same handover.
    async fn pre_check_sessions(&self, agent_id: &str, sessions: &[VizierSession]) -> Result<()> {
        if sessions.is_empty() {
            return Ok(());
        }

        for session in sessions {
            let slug = session.to_slug();

            let (history, _prior_handover) = self
                .deps
                .storage
                .list_session_history_until_checkpoint(session.clone(), None)
                .await?;

            if history.is_empty() {
                tracing::debug!(
                    "Pre-dream checkpoint skipped for '{}': no new history",
                    slug
                );
                continue;
            }

            let (resp_tx, resp_rx) = self.deps.transport.reply_channel().await?;
            if let Err(e) = self
                .deps
                .transport
                .send_request(
                    session.clone(),
                    VizierRequest {
                        timestamp: crate::time::now(),
                        user: agent_id.to_string(),
                        content: VizierRequestContent::Command("checkpoint".to_string()),
                        ..Default::default()
                    },
                    Some(resp_tx),
                )
                .await
            {
                tracing::warn!("Pre-dream checkpoint dispatch failed for '{}': {}", slug, e);
                continue;
            }

            while let Ok(response) = resp_rx.recv_async().await {
                if matches!(response.content, VizierResponseContent::Checkpoint { .. }) {
                    break;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::llm::sso::SsoError;
    use crate::schema::ErrorKind;

    fn frame(content: VizierResponseContent) -> VizierResponse {
        VizierResponse {
            content,
            ..Default::default()
        }
    }

    /// FR-017: the first extraction that names the ended session is the cycle's answer, and the
    /// frame ends the turn like any other. A dream therefore fails once, promptly, instead of
    /// one failing turn per source session.
    #[tokio::test]
    async fn an_ended_session_stops_a_dream_cycle_promptly() {
        let (tx, rx) = flume::unbounded::<VizierResponse>();
        tx.send(frame(VizierResponseContent::Error {
            kind: ErrorKind::Completion,
            message: SsoError::SessionExpired.to_string(),
        }))
        .unwrap();

        let stopped = tokio::time::timeout(Duration::from_secs(1), async {
            while let Ok(response) = rx.recv_async().await {
                if let Some(message) = DreamScheduler::session_failure(&response) {
                    return Some(message.to_string());
                }
                if DreamScheduler::is_final_response(&response) {
                    return None;
                }
            }
            None
        })
        .await
        .expect("the cycle must not wait on the channel");

        let message = stopped.expect("the frame names the ended session");
        assert!(message.contains("AWS SSO session has ended"), "{message}");
        assert!(message.contains("sign in again"), "{message}");
        drop(tx);
    }

    fn chat(text: &str) -> crate::schema::SessionHistoryContent {
        crate::schema::SessionHistoryContent::Request(VizierRequest {
            content: VizierRequestContent::Chat(text.into()),
            ..Default::default()
        })
    }

    fn texts(history: &[crate::schema::SessionHistory]) -> Vec<String> {
        history
            .iter()
            .filter_map(|entry| match &entry.content {
                crate::schema::SessionHistoryContent::Request(req) => match &req.content {
                    VizierRequestContent::Chat(text) => Some(text.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// Extraction reads only the conversation since the last dream, with the handover of the
    /// checkpoint before it as a preface — never the session from its first entry.
    #[tokio::test]
    async fn extraction_reads_only_the_window_with_the_earlier_handover() {
        let storage = crate::agents::client::test_storage();
        let session = VizierSession("agent".into(), VizierChannelId::System, Some("t".into()));
        let tick = || tokio::time::sleep(Duration::from_millis(5));

        storage.save_session_history(session.clone(), chat("dreamed already")).await.unwrap();
        tick().await;
        storage
            .save_checkpoint(session.clone(), Some("earlier summary".into()))
            .await
            .unwrap();
        tick().await;
        let since = crate::time::now();
        tick().await;
        storage.save_session_history(session.clone(), chat("new today")).await.unwrap();
        tick().await;
        // The pre-dream checkpoint lands inside the window; it is skipped, not a stopping point.
        storage
            .save_checkpoint(session.clone(), Some("pre-dream summary".into()))
            .await
            .unwrap();
        tick().await;
        let end = crate::time::now();

        let (history, handover) = extraction_source(&storage, &session, since, end)
            .await
            .unwrap()
            .expect("the window holds a message");
        assert_eq!(texts(&history), vec!["new today".to_string()]);
        assert_eq!(handover.as_deref(), Some("earlier summary"));
    }

    /// A window holding only the pre-dream checkpoint has nothing to extract.
    #[tokio::test]
    async fn a_window_with_only_a_checkpoint_is_skipped() {
        let storage = crate::agents::client::test_storage();
        let session = VizierSession("agent".into(), VizierChannelId::System, Some("t".into()));
        storage.save_session_history(session.clone(), chat("old")).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        let since = crate::time::now();
        tokio::time::sleep(Duration::from_millis(5)).await;
        storage.save_checkpoint(session.clone(), None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;

        let source = extraction_source(&storage, &session, since, crate::time::now())
            .await
            .unwrap();
        assert!(source.is_none());
    }

    /// A dream turn that failed for any other reason is not a session failure: the cycle carries
    /// on with the other sessions rather than blaming the connection.
    #[test]
    fn only_a_session_failure_stops_the_cycle() {
        for other in [
            SsoError::Throttled,
            SsoError::ModelAccessNotEnabled {
                model: "anthropic.claude-sonnet-4-5".into(),
            },
            SsoError::RegionMismatch {
                arn_region: "us-east-1".into(),
                connection_region: "eu-west-1".into(),
            },
        ] {
            let frame = frame(VizierResponseContent::Error {
                kind: ErrorKind::Completion,
                message: other.to_string(),
            });
            assert!(DreamScheduler::session_failure(&frame).is_none(), "{other:?}");
        }
        assert!(
            DreamScheduler::session_failure(&frame(VizierResponseContent::Message {
                content: "done".into(),
                stats: None,
            }))
            .is_none()
        );
    }
}
