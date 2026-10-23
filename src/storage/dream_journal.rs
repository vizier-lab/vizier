use anyhow::Result;

use chrono::{DateTime, Utc};

use crate::schema::{
    AgentId, DreamStage,
    dream_journal::{DreamEntryStub, DreamJournalEntry, DreamRevision},
};

use super::VizierStorage;

#[async_trait::async_trait]
pub trait DreamJournalStorage {
    async fn save_dream_entry(&self, entry: DreamJournalEntry) -> Result<()>;
    async fn list_dream_entries(
        &self,
        agent_id: AgentId,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DreamJournalEntry>>;
    async fn get_dream_entry(
        &self,
        agent_id: AgentId,
        entry_id: String,
    ) -> Result<Option<DreamJournalEntry>>;
    async fn get_latest_dream_entry(
        &self,
        agent_id: AgentId,
        stage: DreamStage,
    ) -> Result<Option<DreamJournalEntry>>;
    async fn list_dream_entries_by_cycle(
        &self,
        agent_id: AgentId,
        cycle_id: &str,
        stage: Option<DreamStage>,
    ) -> Result<Vec<DreamJournalEntry>>;

    /// Journal entries since `since`, oldest first, without their text (spec 024).
    async fn list_dream_entry_stubs(
        &self,
        _agent_id: AgentId,
        _since: DateTime<Utc>,
    ) -> Result<Vec<DreamEntryStub>> {
        Ok(Vec::new())
    }

    /// Every CORE and memory revision a dream made since `since`, oldest first (spec 024).
    async fn list_dream_revisions(
        &self,
        _agent_id: AgentId,
        _since: DateTime<Utc>,
    ) -> Result<Vec<DreamRevision>> {
        Ok(Vec::new())
    }
}

#[async_trait::async_trait]
impl DreamJournalStorage for VizierStorage {
    async fn save_dream_entry(&self, entry: DreamJournalEntry) -> Result<()> {
        self.0.save_dream_entry(entry).await
    }

    async fn list_dream_entries(
        &self,
        agent_id: AgentId,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DreamJournalEntry>> {
        self.0.list_dream_entries(agent_id, limit, offset).await
    }

    async fn get_dream_entry(
        &self,
        agent_id: AgentId,
        entry_id: String,
    ) -> Result<Option<DreamJournalEntry>> {
        self.0.get_dream_entry(agent_id, entry_id).await
    }

    async fn get_latest_dream_entry(
        &self,
        agent_id: AgentId,
        stage: DreamStage,
    ) -> Result<Option<DreamJournalEntry>> {
        self.0.get_latest_dream_entry(agent_id, stage).await
    }

    async fn list_dream_entries_by_cycle(
        &self,
        agent_id: AgentId,
        cycle_id: &str,
        stage: Option<DreamStage>,
    ) -> Result<Vec<DreamJournalEntry>> {
        self.0
            .list_dream_entries_by_cycle(agent_id, cycle_id, stage)
            .await
    }

    async fn list_dream_entry_stubs(
        &self,
        agent_id: AgentId,
        since: DateTime<Utc>,
    ) -> Result<Vec<DreamEntryStub>> {
        self.0.list_dream_entry_stubs(agent_id, since).await
    }

    async fn list_dream_revisions(
        &self,
        agent_id: AgentId,
        since: DateTime<Utc>,
    ) -> Result<Vec<DreamRevision>> {
        self.0.list_dream_revisions(agent_id, since).await
    }
}
