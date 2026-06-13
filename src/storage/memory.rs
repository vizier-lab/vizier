use anyhow::Result;

use crate::{
    indexer::VizierIndexer,
    schema::{
        Memory, MemoryGraph, MemoryGraphEdge, MemoryGraphNode, MemoryQueryParams,
        MemoryVisibility, PaginatedMemory, VizierAttachment,
    },
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait MemoryStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
        visibility: MemoryVisibility,
        shared_to: Vec<String>,
        tags: Vec<String>,
        attachments: Vec<VizierAttachment>,
        indexer: &VizierIndexer,
    ) -> Result<Memory>;

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        threshold: f64,
        indexer: &VizierIndexer,
    ) -> Result<Vec<Memory>>;

    async fn get_all_agent_memory(&self, agent_id: String) -> Result<Vec<Memory>>;

    async fn get_filtered_memories(
        &self,
        params: MemoryQueryParams,
    ) -> Result<PaginatedMemory>;

    async fn get_memory_detail(&self, agent_id: String, slug: String) -> Result<Option<Memory>>;

    async fn get_related_memories(
        &self,
        agent_id: String,
        slug: String,
    ) -> Result<Vec<Memory>>;

    async fn get_memory_graph(&self, agent_id: String) -> Result<MemoryGraph>;

    async fn delete_memory(
        &self,
        agent_id: String,
        slug: String,
        indexer: &VizierIndexer,
    ) -> Result<()>;
}

#[async_trait::async_trait]
impl MemoryStorage for VizierStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
        visibility: MemoryVisibility,
        shared_to: Vec<String>,
        tags: Vec<String>,
        attachments: Vec<VizierAttachment>,
        indexer: &VizierIndexer,
    ) -> Result<Memory> {
        self.0
            .write_memory(agent_id, slug, title, content, visibility, shared_to, tags, attachments, indexer)
            .await
    }

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        threshold: f64,
        indexer: &VizierIndexer,
    ) -> Result<Vec<Memory>> {
        self.0.query_memory(agent_id, query, limit, threshold, indexer).await
    }

    async fn get_all_agent_memory(&self, agent_id: String) -> Result<Vec<Memory>> {
        self.0.get_all_agent_memory(agent_id).await
    }

    async fn get_filtered_memories(
        &self,
        params: MemoryQueryParams,
    ) -> Result<PaginatedMemory> {
        self.0.get_filtered_memories(params).await
    }

    async fn get_memory_detail(&self, agent_id: String, slug: String) -> Result<Option<Memory>> {
        self.0.get_memory_detail(agent_id, slug).await
    }

    async fn get_related_memories(
        &self,
        agent_id: String,
        slug: String,
    ) -> Result<Vec<Memory>> {
        self.0.get_related_memories(agent_id, slug).await
    }

    async fn get_memory_graph(&self, agent_id: String) -> Result<MemoryGraph> {
        self.0.get_memory_graph(agent_id).await
    }

    async fn delete_memory(
        &self,
        agent_id: String,
        slug: String,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        self.0.delete_memory(agent_id, slug, indexer).await
    }
}
