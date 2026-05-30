use anyhow::Result;

use crate::schema::AgentConfig;

#[async_trait::async_trait]
pub trait AgentStorage {
    async fn list_agents(&self) -> Result<Vec<(String, AgentConfig)>>;
    async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentConfig>>;
    async fn create_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()>;
    async fn update_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()>;
    async fn delete_agent(&self, agent_id: &str) -> Result<()>;
}
