use anyhow::Result;

use crate::schema::{AgentId, DreamStage, dream_journal::DreamJournalEntry};

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
}
