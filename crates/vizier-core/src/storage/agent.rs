use anyhow::Result;

use crate::schema::{
    AgentConfig, CoreRevision, PaginatedCoreRevisions, RevisionDiff, RevisionOrigin,
    RollbackResponse,
};

#[async_trait::async_trait]
pub trait AgentStorage {
    async fn list_agents(&self) -> Result<Vec<(String, AgentConfig)>>;
    async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentConfig>>;
    async fn create_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()>;
    async fn update_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()>;
    async fn delete_agent(&self, agent_id: &str) -> Result<()>;

    async fn get_agent_core(&self, agent_id: &str) -> Result<Option<String>> {
        Ok(self.get_agent(agent_id).await?.and_then(|c| c.core))
    }

    /// `origin` describes who/what is saving; the sqlite backend records a `core_revision`
    /// entry for every save (specs/006-memory-version-history). The config-backed default
    /// keeps no history.
    async fn set_agent_core(
        &self,
        agent_id: &str,
        core: &str,
        _origin: &RevisionOrigin,
    ) -> Result<()> {
        let mut config = self
            .get_agent(agent_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", agent_id))?;
        config.core = Some(core.to_string());
        self.update_agent(agent_id, &config).await
    }

    // ---- CORE version history (specs/006-memory-version-history) ----
    // Only the sqlite backend keeps history; the defaults exist so the trait stays
    // implementable by a backend that doesn't (the legacy fs read-source, for one).

    /// Newest-first. Lazily seeds the baseline revision when a CORE exists but has no history.
    async fn list_core_revisions(
        &self,
        _agent_id: &str,
        _offset: usize,
        _limit: usize,
    ) -> Result<PaginatedCoreRevisions> {
        Err(anyhow::anyhow!("CORE version history is not supported by this storage backend"))
    }

    async fn get_core_revision(&self, _agent_id: &str, _seq: i64) -> Result<Option<CoreRevision>> {
        Err(anyhow::anyhow!("CORE version history is not supported by this storage backend"))
    }

    /// Changes introduced by `to` relative to `from` (`None` ⇒ `to - 1`). Errors on an unknown seq.
    async fn diff_core_revisions(
        &self,
        _agent_id: &str,
        _from: Option<i64>,
        _to: i64,
    ) -> Result<RevisionDiff> {
        Err(anyhow::anyhow!("CORE version history is not supported by this storage backend"))
    }

    /// Re-saves revision `seq` as a new revision with `trigger = rollback`; never rewrites history.
    async fn rollback_core(
        &self,
        _agent_id: &str,
        _seq: i64,
        _origin: &RevisionOrigin,
    ) -> Result<RollbackResponse> {
        Err(anyhow::anyhow!("CORE version history is not supported by this storage backend"))
    }
}
