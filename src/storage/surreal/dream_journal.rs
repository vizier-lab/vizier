use anyhow::Result;

use crate::schema::{
    AgentId,
    DreamStage,
    dream_journal::DreamJournalEntry,
};
use crate::storage::{dream_journal::DreamJournalStorage, surreal::SurrealStorage};

#[async_trait::async_trait]
impl DreamJournalStorage for SurrealStorage {
    async fn save_dream_entry(&self, entry: DreamJournalEntry) -> Result<()> {
        let _: Option<DreamJournalEntry> = self
            .conn
            .create(("dream_journal", entry.id.clone()))
            .content(entry)
            .await?;

        Ok(())
    }

    async fn list_dream_entries(
        &self,
        agent_id: AgentId,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DreamJournalEntry>> {
        let offset_val = offset.unwrap_or(0);
        let query = if let Some(limit_val) = limit {
            format!(
                "SELECT * FROM dream_journal WHERE agent_id = $agent_id ORDER BY timestamp DESC LIMIT {} START {}",
                limit_val, offset_val
            )
        } else {
            format!(
                "SELECT * FROM dream_journal WHERE agent_id = $agent_id ORDER BY timestamp DESC START {}",
                offset_val
            )
        };

        let mut response = self
            .conn
            .query(query)
            .bind(("agent_id", agent_id))
            .await?;

        let entries: Vec<DreamJournalEntry> = response.take(0)?;
        Ok(entries)
    }

    async fn get_dream_entry(
        &self,
        _agent_id: AgentId,
        entry_id: String,
    ) -> Result<Option<DreamJournalEntry>> {
        let entry: Option<DreamJournalEntry> =
            self.conn.select(("dream_journal", entry_id)).await?;

        Ok(entry)
    }

    async fn get_latest_dream_entry(
        &self,
        agent_id: AgentId,
        stage: DreamStage,
    ) -> Result<Option<DreamJournalEntry>> {
        let mut response = self
            .conn
            .query(
                "SELECT * FROM dream_journal WHERE agent_id = $agent_id AND stage = $stage ORDER BY timestamp DESC LIMIT 1",
            )
            .bind(("agent_id", agent_id))
            .bind(("stage", stage))
            .await?;

        let entries: Vec<DreamJournalEntry> = response.take(0)?;
        Ok(entries.into_iter().next())
    }

    async fn list_dream_entries_by_cycle(
        &self,
        agent_id: AgentId,
        cycle_id: &str,
        stage: Option<DreamStage>,
    ) -> Result<Vec<DreamJournalEntry>> {
        let query = if stage.is_some() {
            "SELECT * FROM dream_journal WHERE agent_id = $agent_id AND dream_cycle_id = $cycle_id AND stage = $stage ORDER BY timestamp ASC"
        } else {
            "SELECT * FROM dream_journal WHERE agent_id = $agent_id AND dream_cycle_id = $cycle_id ORDER BY timestamp ASC"
        };

        let mut q = self
            .conn
            .query(query)
            .bind(("agent_id", agent_id))
            .bind(("cycle_id", cycle_id.to_string()));

        if let Some(stage) = stage {
            q = q.bind(("stage", stage));
        }

        let mut response = q.await?;
        let entries: Vec<DreamJournalEntry> = response.take(0)?;
        Ok(entries)
    }
}
