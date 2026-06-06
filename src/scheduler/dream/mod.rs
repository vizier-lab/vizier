mod prompts;

use std::str::FromStr;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
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

            // Timeout protection: if dreaming for > 30 minutes, force-reset
            if let Some(ref dream_status) = status {
                let started_at = match dream_status {
                    DreamStatus::Extracting { started_at, .. } => Some(*started_at),
                    DreamStatus::Consolidating { started_at, .. } => Some(*started_at),
                    DreamStatus::Idle => None,
                };
                if let Some(started) = started_at
                    && Utc::now() - started > Duration::minutes(30)
                {
                    tracing::warn!(
                        "Dream cycle timed out for agent '{}', resetting to idle",
                        agent_id
                    );
                    self.deps
                        .storage
                        .set_dream_status(&agent_id, DreamStatus::Idle)
                        .await?;
                }
            }

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
                .unwrap_or(Utc::now() - Duration::hours(24));
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

            if next <= Utc::now()
                && let Err(e) = self.trigger_dream(&agent_id).await
            {
                tracing::error!("Failed to trigger dream for agent '{}': {}", agent_id, e);
            }
        }

        Ok(())
    }

    fn is_final_response(response: &VizierResponse) -> bool {
        matches!(
            response.content,
            VizierResponseContent::Message { .. }
                | VizierResponseContent::Abort
                | VizierResponseContent::Empty
        )
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
        let now = Utc::now();
        let last_dream = self
            .deps
            .storage
            .get_last_dream_time(agent_id)
            .await?
            .unwrap_or(now - Duration::hours(24));

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
        self.deps
            .storage
            .set_last_dream_time(agent_id, now)
            .await?;

        // Spawn background task for the full dream cycle
        let deps = self.deps.clone();
        let agent_id = agent_id.to_string();
        let source_session_slugs: Vec<String> = sessions.iter().map(|s| s.to_slug()).collect();

        tokio::spawn(async move {
            // Send all extraction requests, collect response channels
            let mut response_rxs = vec![];
            for session in &sessions {
                let (resp_tx, resp_rx) = flume::unbounded();

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
                            timestamp: Utc::now(),
                            user: agent_id.clone(),
                            content: VizierRequestContent::Task(EXTRACTION_PROMPT.to_string()),
                            metadata: serde_json::json!({
                                "dream_cycle_id": cycle_id,
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

            // Await ALL extraction responses, updating progress as each completes
            for (i, rx) in response_rxs.iter().enumerate() {
                while let Ok(response) = rx.recv_async().await {
                    if Self::is_final_response(&response) {
                        break;
                    }
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

            // Set status → Consolidating
            let _ = deps
                .storage
                .set_dream_status(
                    &agent_id,
                    DreamStatus::Consolidating {
                        started_at: Utc::now(),
                        cycle_id: cycle_id.clone(),
                    },
                )
                .await;

            // Trigger consolidation
            match Self::do_consolidation(&deps, &agent_id, &cycle_id, &source_session_slugs).await
            {
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
        source_sessions: &[String],
    ) -> Result<flume::Receiver<VizierResponse>> {
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
            let (tx, rx) = flume::unbounded();
            let _ = tx
                .send_async(VizierResponse {
                    timestamp: Utc::now(),
                    content: VizierResponseContent::Empty,
                    attachments: vec![],
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
        let now = Utc::now();
        let (resp_tx, resp_rx) = flume::unbounded();

        deps.transport
            .send_request(
                VizierSession(
                    agent_id.to_string(),
                    VizierChannelId::Dream(
                        Box::new(VizierSession(
                            agent_id.to_string(),
                            VizierChannelId::Heartbeat(now),
                            Some("dream-consolidation".to_string()),
                        )),
                        DreamStage::Consolidation,
                    ),
                    Some(cycle_id.to_string()),
                ),
                VizierRequest {
                    timestamp: now,
                    user: agent_id.to_string(),
                    content: VizierRequestContent::Task(prompt),
                    metadata: serde_json::json!({
                        "dream_cycle_id": cycle_id,
                        "source_sessions": source_sessions,
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
}
