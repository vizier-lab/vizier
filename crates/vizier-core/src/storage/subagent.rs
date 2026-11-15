//! Subagents (spec 059): one `subagent` row per worker.
//!
//! Every write that advances a worker is fenced on its agent's scope (spec 057), and every state
//! change is one guarded update, so of two racing causes exactly one wins. "Seen" and
//! "delivered" are sequence numbers against `outcome_seq` (research D8): an outcome is handed to
//! the main agent once, by a wait, a read or a delivery claim, whichever applies first.

use anyhow::Result;

use crate::{
    schema::{AgentId, Subagent, SubagentId, SubagentStopReason, VizierSession},
    time::Timestamp,
};

/// How a worker's turn ended (`settle_subagent`).
#[derive(Debug, Clone, PartialEq)]
pub enum Settled {
    Answered { result: String, truncated: bool },
    Failed(String),
    TimedOut(String),
}

/// What `stop_subagent` did.
#[derive(Debug, Clone, PartialEq)]
pub enum StopOutcome {
    /// The worker moved to `stopped`; the row as it is now.
    Stopped(Subagent),
    /// It was already final; the row as it is.
    AlreadyFinished(Subagent),
    NotFound,
}

#[async_trait::async_trait]
pub trait SubagentStorage {
    /// Record a spawned group (state `running`) in one transaction. Fenced.
    async fn open_subagents(&self, rows: Vec<Subagent>) -> Result<()>;

    /// `running → asking` with the question; `outcome_seq + 1`, the deadline cleared and the
    /// remaining time kept. `None` when the row was not running. Fenced.
    async fn ask_subagent(
        &self,
        id: &str,
        question: &str,
        remaining_ms: i64,
        at: Timestamp,
    ) -> Result<Option<Subagent>>;

    /// `asking → running` with a new deadline. `None` when the row was not asking. Fenced.
    async fn reply_subagent(
        &self,
        id: &str,
        deadline_at: Timestamp,
        at: Timestamp,
    ) -> Result<Option<Subagent>>;

    /// `running → answered | failed | timed_out`; `outcome_seq + 1`, `ended_at` set. `None` when
    /// the row was not running (it is asking, or already final). Fenced.
    async fn settle_subagent(
        &self,
        id: &str,
        settled: Settled,
        at: Timestamp,
    ) -> Result<Option<Subagent>>;

    /// `running | asking → stopped`. Fenced.
    async fn stop_subagent(
        &self,
        id: &str,
        reason: SubagentStopReason,
        at: Timestamp,
    ) -> Result<StopOutcome>;

    /// The main agent read these outcomes through a wait or a read: `seen_seq = outcome_seq`
    /// where still deliverable. Returns the ids it marked. Fenced.
    async fn mark_subagents_seen(&self, ids: &[SubagentId]) -> Result<Vec<SubagentId>>;

    /// Take the hiring conversation's deliverable outcomes: `delivered_seq = outcome_seq` where
    /// still deliverable; returns the rows it took, oldest change first. `exclude` are workers a
    /// `wait_subagents` is watching, which hands their outcome back itself. Fenced.
    async fn claim_subagent_deliveries(
        &self,
        session: &VizierSession,
        exclude: &[SubagentId],
    ) -> Result<Vec<Subagent>>;

    /// The worker whose own conversation has `worker_topic`.
    async fn subagent_by_worker(&self, worker_topic: &str) -> Result<Option<Subagent>>;

    async fn get_subagent(&self, id: &str) -> Result<Option<Subagent>>;

    /// The hiring conversation's running and asking workers, then its last `final_limit`
    /// final ones; oldest first.
    async fn list_subagents(
        &self,
        session: &VizierSession,
        final_limit: usize,
    ) -> Result<Vec<Subagent>>;

    /// Recovery on claim: the agent's `running` workers written under an older fence (`below`),
    /// re-owned with the held fence; returns them so their timers can be re-armed. Fenced.
    async fn reown_running_subagents(&self, agent_id: &AgentId, below: u64)
    -> Result<Vec<Subagent>>;

    /// The agent's hiring conversations with a deliverable worker.
    async fn subagent_delivery_sessions(&self, agent_id: &AgentId) -> Result<Vec<VizierSession>>;

    /// Stop every unfinished worker of a hiring conversation; returns the rows it stopped.
    /// Fenced.
    async fn stop_session_subagents(
        &self,
        session: &VizierSession,
        reason: SubagentStopReason,
        at: Timestamp,
    ) -> Result<Vec<Subagent>>;

    /// Delete a hiring conversation's workers; returns every deleted row (their worker
    /// conversations go too, by the caller).
    async fn delete_session_subagents(&self, session: &VizierSession) -> Result<Vec<Subagent>>;

    /// Delete every worker of an agent (the agent is being deleted).
    async fn delete_agent_subagents(&self, agent_id: &str) -> Result<()>;
}

#[async_trait::async_trait]
impl SubagentStorage for super::VizierStorage {
    async fn open_subagents(&self, rows: Vec<Subagent>) -> Result<()> {
        self.0.open_subagents(rows).await
    }

    async fn ask_subagent(
        &self,
        id: &str,
        question: &str,
        remaining_ms: i64,
        at: Timestamp,
    ) -> Result<Option<Subagent>> {
        self.0.ask_subagent(id, question, remaining_ms, at).await
    }

    async fn reply_subagent(
        &self,
        id: &str,
        deadline_at: Timestamp,
        at: Timestamp,
    ) -> Result<Option<Subagent>> {
        self.0.reply_subagent(id, deadline_at, at).await
    }

    async fn settle_subagent(
        &self,
        id: &str,
        settled: Settled,
        at: Timestamp,
    ) -> Result<Option<Subagent>> {
        self.0.settle_subagent(id, settled, at).await
    }

    async fn stop_subagent(
        &self,
        id: &str,
        reason: SubagentStopReason,
        at: Timestamp,
    ) -> Result<StopOutcome> {
        self.0.stop_subagent(id, reason, at).await
    }

    async fn mark_subagents_seen(&self, ids: &[SubagentId]) -> Result<Vec<SubagentId>> {
        self.0.mark_subagents_seen(ids).await
    }

    async fn claim_subagent_deliveries(
        &self,
        session: &VizierSession,
        exclude: &[SubagentId],
    ) -> Result<Vec<Subagent>> {
        self.0.claim_subagent_deliveries(session, exclude).await
    }

    async fn subagent_by_worker(&self, worker_topic: &str) -> Result<Option<Subagent>> {
        self.0.subagent_by_worker(worker_topic).await
    }

    async fn get_subagent(&self, id: &str) -> Result<Option<Subagent>> {
        self.0.get_subagent(id).await
    }

    async fn list_subagents(
        &self,
        session: &VizierSession,
        final_limit: usize,
    ) -> Result<Vec<Subagent>> {
        self.0.list_subagents(session, final_limit).await
    }

    async fn reown_running_subagents(
        &self,
        agent_id: &AgentId,
        below: u64,
    ) -> Result<Vec<Subagent>> {
        self.0.reown_running_subagents(agent_id, below).await
    }

    async fn subagent_delivery_sessions(&self, agent_id: &AgentId) -> Result<Vec<VizierSession>> {
        self.0.subagent_delivery_sessions(agent_id).await
    }

    async fn stop_session_subagents(
        &self,
        session: &VizierSession,
        reason: SubagentStopReason,
        at: Timestamp,
    ) -> Result<Vec<Subagent>> {
        self.0.stop_session_subagents(session, reason, at).await
    }

    async fn delete_session_subagents(&self, session: &VizierSession) -> Result<Vec<Subagent>> {
        self.0.delete_session_subagents(session).await
    }

    async fn delete_agent_subagents(&self, agent_id: &str) -> Result<()> {
        self.0.delete_agent_subagents(agent_id).await
    }
}
