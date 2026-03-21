use anyhow::Result;

use crate::{schema::Memory, storage::VizierStorage};

#[async_trait::async_trait]
pub trait MemoryStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
    ) -> Result<()>;

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<Memory>>;

    async fn get_all_agent_memory(&self, agent_id: String) -> Result<Vec<Memory>>;
    async fn get_memory_detail(&self, agent_id: String, slug: String) -> Result<Option<Memory>>;
    async fn delete_memory(&self, agent_id: String, slug: String) -> Result<()>;
}

#[async_trait::async_trait]
impl MemoryStorage for VizierStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
    ) -> Result<()> {
        self.0.write_memory(agent_id, slug, title, content).await
    }

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<Memory>> {
        self.0.query_memory(agent_id, query, limit, threshold).await
    }

    async fn get_all_agent_memory(&self, agent_id: String) -> Result<Vec<Memory>> {
        self.0.get_all_agent_memory(agent_id).await
    }

    async fn get_memory_detail(&self, agent_id: String, slug: String) -> Result<Option<Memory>> {
        self.0.get_memory_detail(agent_id, slug).await
    }

    async fn delete_memory(&self, agent_id: String, slug: String) -> Result<()> {
        self.0.delete_memory(agent_id, slug).await
    }
}
