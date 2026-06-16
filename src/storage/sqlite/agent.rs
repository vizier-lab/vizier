use anyhow::Result;

use crate::{
    schema::AgentConfig,
    storage::{agent::AgentStorage, sqlite::SqliteStorage},
};

#[async_trait::async_trait]
impl AgentStorage for SqliteStorage {
    async fn list_agents(&self) -> Result<Vec<(String, AgentConfig)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT agent_id, data FROM agent_config")?;
        let agents = stmt
            .query_map([], |row| {
                let agent_id: String = row.get(0)?;
                let data: String = row.get(1)?;
                Ok((agent_id, data))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(agent_id, data)| {
                serde_json::from_str::<AgentConfig>(&data)
                    .ok()
                    .map(|config| (agent_id, config))
            })
            .collect();
        Ok(agents)
    }

    async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentConfig>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM agent_config WHERE agent_id = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![agent_id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn create_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let data = serde_json::to_string(config)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO agent_config (agent_id, data) VALUES (?1, ?2)",
            rusqlite::params![agent_id, data],
        )?;
        Ok(())
    }

    async fn update_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let data = serde_json::to_string(config)?;
        let conn = self.conn.lock();
        let updated = conn.execute(
            "UPDATE agent_config SET data = ?1 WHERE agent_id = ?2",
            rusqlite::params![data, agent_id],
        )?;
        if updated == 0 {
            return Err(anyhow::anyhow!("Agent '{}' not found", agent_id));
        }
        Ok(())
    }

    async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM agent_config WHERE agent_id = ?1",
            rusqlite::params![agent_id],
        )?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("Agent '{}' not found", agent_id));
        }
        Ok(())
    }
}
