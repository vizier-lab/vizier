//! Detached scripts (spec 058): one `detached_script` row per detached `execute_python` run.
//!
//! Every write that advances a script is fenced on its agent's scope (spec 057), and every
//! transition out of `running` is one guarded update, so of two racing causes exactly one wins.

use anyhow::Result;

use crate::{
    schema::{
        AgentId, Canceller, DetachedScript, DetachedScriptId, DetachedScriptState, VizierSession,
    },
    time::Timestamp,
};

/// What `cancel_detached_script` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelOutcome {
    /// The script moved to `cancelled`. The ids are it first, then every running script
    /// launched beneath it that the cascade cancelled too.
    Cancelled(Vec<DetachedScriptId>),
    /// The script was already final (it finished first, or was cancelled before).
    AlreadyFinished(DetachedScriptState),
    NotFound,
}

#[async_trait::async_trait]
pub trait DetachedScriptStorage {
    /// Record a launched script (state `running`).
    ///
    /// Fenced: refused with `OwnershipLost` when a newer owner holds `agent:{agent_id}`.
    async fn open_detached_script(&self, script: DetachedScript) -> Result<()>;

    /// Replace a script's tool-call list. Returns the row's current state (`None` if the row
    /// is gone), so a running task learns it was cancelled or interrupted elsewhere and stops.
    /// Only a `running` row is written.
    ///
    /// Fenced.
    async fn record_detached_progress(
        &self,
        id: &str,
        tool_calls: serde_json::Value,
    ) -> Result<Option<DetachedScriptState>>;

    /// The guarded transition `running → state` (a final, non-cancelled state) with the
    /// outcome and the last tool-call list. False when the row was no longer running.
    ///
    /// Fenced.
    async fn close_detached_script(
        &self,
        id: &str,
        state: DetachedScriptState,
        outcome: serde_json::Value,
        tool_calls: serde_json::Value,
        at: Timestamp,
    ) -> Result<bool>;

    /// The guarded transition `running → cancelled`, cascaded to every running script
    /// launched beneath it (by `parent_id`, at any depth). A cancelled script is never
    /// delivered.
    ///
    /// Fenced on the script's own agent.
    async fn cancel_detached_script(
        &self,
        id: &str,
        by: Canceller,
        at: Timestamp,
    ) -> Result<CancelOutcome>;

    /// Take the session's deliverable scripts (final, not cancelled, not delivered): sets
    /// `delivered_at` on them and returns them in `finished_at` order. Whoever calls first
    /// takes the rows; a second caller gets none.
    ///
    /// Fenced on the session's agent.
    async fn claim_deliverable_scripts(&self, session: &VizierSession)
    -> Result<Vec<DetachedScript>>;

    /// The session's running scripts and its last `final_limit` final ones, oldest first.
    async fn list_detached_scripts(
        &self,
        session: &VizierSession,
        final_limit: usize,
    ) -> Result<Vec<DetachedScript>>;

    async fn get_detached_script(&self, id: &str) -> Result<Option<DetachedScript>>;

    /// Move an agent's running scripts to `interrupted` and re-own them with the held fence.
    /// `below`: only rows written under an older fence (recovery on claim); `None` for all of
    /// them (the agent's instance stopped). `ids` restricts the sweep to those scripts.
    /// Returns the scripts it moved.
    ///
    /// Fenced.
    async fn interrupt_running_scripts(
        &self,
        agent_id: &str,
        below: Option<u64>,
        ids: Option<&[DetachedScriptId]>,
    ) -> Result<Vec<DetachedScript>>;

    /// Delete a conversation's scripts. Returns the ids that were still running, so their
    /// tasks can be stopped.
    async fn delete_session_scripts(&self, session: &VizierSession) -> Result<Vec<DetachedScriptId>>;

    /// Delete every script of an agent (the agent is being deleted).
    async fn delete_agent_scripts(&self, agent_id: &str) -> Result<()>;

    /// The agent's sessions that have at least one deliverable script.
    async fn deliverable_script_sessions(&self, agent_id: &AgentId) -> Result<Vec<VizierSession>>;

    /// The topic list's badge: running scripts of one agent and channel, by topic.
    async fn count_running_scripts(
        &self,
        agent_id: &str,
        channel: &crate::schema::VizierChannelId,
    ) -> Result<std::collections::HashMap<Option<crate::schema::TopicId>, usize>>;
}

#[async_trait::async_trait]
impl DetachedScriptStorage for super::VizierStorage {
    async fn open_detached_script(&self, script: DetachedScript) -> Result<()> {
        self.0.open_detached_script(script).await
    }

    async fn record_detached_progress(
        &self,
        id: &str,
        tool_calls: serde_json::Value,
    ) -> Result<Option<DetachedScriptState>> {
        self.0.record_detached_progress(id, tool_calls).await
    }

    async fn close_detached_script(
        &self,
        id: &str,
        state: DetachedScriptState,
        outcome: serde_json::Value,
        tool_calls: serde_json::Value,
        at: Timestamp,
    ) -> Result<bool> {
        self.0.close_detached_script(id, state, outcome, tool_calls, at).await
    }

    async fn cancel_detached_script(
        &self,
        id: &str,
        by: Canceller,
        at: Timestamp,
    ) -> Result<CancelOutcome> {
        self.0.cancel_detached_script(id, by, at).await
    }

    async fn claim_deliverable_scripts(
        &self,
        session: &VizierSession,
    ) -> Result<Vec<DetachedScript>> {
        self.0.claim_deliverable_scripts(session).await
    }

    async fn list_detached_scripts(
        &self,
        session: &VizierSession,
        final_limit: usize,
    ) -> Result<Vec<DetachedScript>> {
        self.0.list_detached_scripts(session, final_limit).await
    }

    async fn get_detached_script(&self, id: &str) -> Result<Option<DetachedScript>> {
        self.0.get_detached_script(id).await
    }

    async fn interrupt_running_scripts(
        &self,
        agent_id: &str,
        below: Option<u64>,
        ids: Option<&[DetachedScriptId]>,
    ) -> Result<Vec<DetachedScript>> {
        self.0.interrupt_running_scripts(agent_id, below, ids).await
    }

    async fn delete_session_scripts(&self, session: &VizierSession) -> Result<Vec<DetachedScriptId>> {
        self.0.delete_session_scripts(session).await
    }

    async fn delete_agent_scripts(&self, agent_id: &str) -> Result<()> {
        self.0.delete_agent_scripts(agent_id).await
    }

    async fn deliverable_script_sessions(&self, agent_id: &AgentId) -> Result<Vec<VizierSession>> {
        self.0.deliverable_script_sessions(agent_id).await
    }

    async fn count_running_scripts(
        &self,
        agent_id: &str,
        channel: &crate::schema::VizierChannelId,
    ) -> Result<std::collections::HashMap<Option<crate::schema::TopicId>, usize>> {
        self.0.count_running_scripts(agent_id, channel).await
    }
}
