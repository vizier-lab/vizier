use anyhow::Result;

use crate::schema::{AgentId, DreamStage, dream_journal::DreamJournalEntry};
use crate::storage::{dream_journal::DreamJournalStorage, sqlite::SqliteStorage};

#[async_trait::async_trait]
impl DreamJournalStorage for SqliteStorage {
    async fn save_dream_entry(&self, entry: DreamJournalEntry) -> Result<()> {
        let data = serde_json::to_string(&entry)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO dream_journal (id, agent_id, dream_cycle_id, stage, timestamp, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                entry.id,
                entry.agent_id,
                entry.dream_cycle_id,
                serde_json::to_string(&entry.stage)?.trim_matches('"'),
                entry.timestamp.timestamp_millis(),
                data
            ],
        )?;
        Ok(())
    }

    async fn list_dream_entries(
        &self,
        agent_id: AgentId,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DreamJournalEntry>> {
        let conn = self.conn.lock();
        let offset_val = offset.unwrap_or(0) as i64;

        let sql = match limit {
            Some(l) => format!(
                "SELECT data FROM dream_journal WHERE agent_id = ?1 ORDER BY timestamp DESC LIMIT {} OFFSET {}",
                l, offset_val
            ),
            None => format!(
                "SELECT data FROM dream_journal WHERE agent_id = ?1 ORDER BY timestamp DESC OFFSET {}",
                offset_val
            ),
        };

        let mut stmt = conn.prepare(&sql)?;
        let entries = stmt
            .query_map(rusqlite::params![agent_id], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<DreamJournalEntry>(&data).ok())
            .collect();
        Ok(entries)
    }

    async fn get_dream_entry(
        &self,
        _agent_id: AgentId,
        entry_id: String,
    ) -> Result<Option<DreamJournalEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM dream_journal WHERE id = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![entry_id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn get_latest_dream_entry(
        &self,
        agent_id: AgentId,
        stage: DreamStage,
    ) -> Result<Option<DreamJournalEntry>> {
        let stage_str = serde_json::to_string(&stage)?.trim_matches('"').to_string();
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT data FROM dream_journal WHERE agent_id = ?1 AND stage = ?2 ORDER BY timestamp DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![agent_id, stage_str], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn list_dream_entries_by_cycle(
        &self,
        agent_id: AgentId,
        cycle_id: &str,
        stage: Option<DreamStage>,
    ) -> Result<Vec<DreamJournalEntry>> {
        let conn = self.conn.lock();
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match stage {
            Some(s) => {
                let stage_str = serde_json::to_string(&s)?.trim_matches('"').to_string();
                (
                    "SELECT data FROM dream_journal WHERE agent_id = ?1 AND dream_cycle_id = ?2 AND stage = ?3 ORDER BY timestamp ASC".to_string(),
                    vec![
                        Box::new(agent_id) as Box<dyn rusqlite::types::ToSql>,
                        Box::new(cycle_id.to_string()),
                        Box::new(stage_str),
                    ],
                )
            }
            None => (
                "SELECT data FROM dream_journal WHERE agent_id = ?1 AND dream_cycle_id = ?2 ORDER BY timestamp ASC".to_string(),
                vec![
                    Box::new(agent_id) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(cycle_id.to_string()),
                ],
            ),
        };

        let mut stmt = conn.prepare(&sql)?;
        let entries = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<DreamJournalEntry>(&data).ok())
            .collect();
        Ok(entries)
    }
}
