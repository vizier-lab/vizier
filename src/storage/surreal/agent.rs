use anyhow::Result;

use crate::{
    schema::AgentConfig,
    storage::{agent::AgentStorage, surreal::SurrealStorage},
};

#[async_trait::async_trait]
impl AgentStorage for SurrealStorage {
    async fn list_agents(&self) -> Result<Vec<(String, AgentConfig)>> {
        let mut result = self
            .conn
            .query("SELECT agent_id, config FROM agent_config")
            .await?;

        let rows: Vec<serde_json::Value> = result.take(0)?;
        let mut agents = Vec::new();
        for row in rows {
            let agent_id = row
                .get("agent_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if let Some(config_val) = row.get("config") {
                if let Ok(config) = serde_json::from_value::<AgentConfig>(config_val.clone()) {
                    agents.push((agent_id, config));
                }
            }
        }
        Ok(agents)
    }

    async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentConfig>> {
        let mut result = self
            .conn
            .query("SELECT config FROM agent_config WHERE agent_id = $id")
            .bind(("id", agent_id.to_string()))
            .await?;

        let rows: Vec<serde_json::Value> = result.take(0)?;
        Ok(rows.into_iter().next().and_then(|r| {
            r.get("config")
                .and_then(|v| serde_json::from_value::<AgentConfig>(v.clone()).ok())
        }))
    }

    async fn create_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let record = serde_json::json!({
            "agent_id": agent_id,
            "config": serde_json::to_value(config)?,
        });

        let _: Option<serde_json::Value> = self
            .conn
            .create(("agent_config", agent_id.to_string()))
            .content(record)
            .await?;

        Ok(())
    }

    async fn update_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let record = serde_json::json!({
            "agent_id": agent_id,
            "config": serde_json::to_value(config)?,
        });

        let _: Option<serde_json::Value> = self
            .conn
            .update(("agent_config", agent_id.to_string()))
            .content(record)
            .await?;

        Ok(())
    }

    async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        let _: Option<serde_json::Value> = self
            .conn
            .delete(("agent_config", agent_id.to_string()))
            .await?;

        Ok(())
    }
}
