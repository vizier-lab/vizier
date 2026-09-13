use anyhow::Result;

use crate::{
    indexer::VizierIndexer,
    schema::{
        BundleSummary, ImportReport, Memory, MemoryGraph, MemoryQueryParams, PaginatedMemory,
        VizierAttachment,
    },
    storage::{memory::MemoryStorage, sqlite::SqliteStorage},
};

#[async_trait::async_trait]
impl MemoryStorage for SqliteStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: Option<String>,
        create_only: bool,
        title: String,
        content: String,
        tags: Vec<String>,
        attachments: Vec<VizierAttachment>,
        indexer: &VizierIndexer,
    ) -> Result<Memory> {
        self.bundle_store()
            .write_memory(
                agent_id, bundle, path, create_only, title, content, tags, attachments, indexer,
            )
            .await
    }

    async fn query_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        query: String,
        limit: usize,
        threshold: f64,
        indexer: &VizierIndexer,
    ) -> Result<Vec<Memory>> {
        self.bundle_store()
            .query_memory(agent_id, bundle, query, limit, threshold, indexer)
            .await
    }

    async fn get_all_agent_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
    ) -> Result<Vec<Memory>> {
        self.bundle_store().get_all_agent_memory(agent_id, bundle).await
    }

    async fn get_filtered_memories(&self, params: MemoryQueryParams) -> Result<PaginatedMemory> {
        self.bundle_store().get_filtered_memories(params).await
    }

    async fn get_memory_detail(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Option<Memory>> {
        self.bundle_store().get_memory_detail(agent_id, bundle, path).await
    }

    async fn get_related_memories(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Vec<Memory>> {
        self.bundle_store().get_related_memories(agent_id, bundle, path).await
    }

    async fn get_memory_graph(
        &self,
        agent_id: String,
        bundle: Option<String>,
        search: Option<String>,
    ) -> Result<MemoryGraph> {
        self.bundle_store().get_memory_graph(agent_id, bundle, search).await
    }

    async fn has_incoming_links(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<bool> {
        self.bundle_store().has_incoming_links(agent_id, bundle, path).await
    }

    async fn delete_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        self.bundle_store()
            .delete_memory(agent_id, bundle, path, indexer)
            .await
    }

    async fn increment_read_count(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<()> {
        self.bundle_store()
            .increment_read_count(agent_id, bundle, path)
            .await
    }

    async fn list_bundles(&self, agent_id: String) -> Result<Vec<BundleSummary>> {
        self.bundle_store().list_bundles(agent_id).await
    }

    async fn delete_bundle(
        &self,
        agent_id: String,
        bundle: String,
        force: bool,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        self.bundle_store()
            .delete_bundle(agent_id, bundle, force, indexer)
            .await
    }

    async fn export_bundle(&self, agent_id: String, bundle: String) -> Result<Vec<u8>> {
        self.bundle_store().export_bundle(agent_id, bundle).await
    }

    async fn import_bundle(
        &self,
        agent_id: String,
        bundle: String,
        zip_bytes: Vec<u8>,
        indexer: &VizierIndexer,
    ) -> Result<ImportReport> {
        self.bundle_store()
            .import_bundle(agent_id, bundle, zip_bytes, indexer)
            .await
    }
}
