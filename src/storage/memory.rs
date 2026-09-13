use anyhow::Result;

use crate::{
    indexer::VizierIndexer,
    schema::{
        BundleSummary, ImportReport, Memory, MemoryGraph, MemoryGraphNode, MemoryQueryParams,
        PaginatedMemory, VizierAttachment,
    },
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait MemoryStorage {
    /// Write (create or update) a concept document at `(bundle, path)`.
    ///
    /// `bundle: None` means the agent's default bundle. `path: None` derives a path from
    /// `slugify(title)`. When `create_only` is `true` (used by the HTTP `POST` create route),
    /// an existing `(bundle, path)` is rejected with `Err` rather than overwritten (FR-011,
    /// no auto-rename). When `false` (the agent-facing `memory_write` tool, and the HTTP `PUT`
    /// update route, which already knows the exact existing path it's revising), an existing
    /// document at that path is overwritten in place.
    #[allow(clippy::too_many_arguments)]
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
    ) -> Result<Memory>;

    /// Semantic search. `bundle: None` searches across all of the agent's bundles.
    async fn query_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        query: String,
        limit: usize,
        threshold: f64,
        indexer: &VizierIndexer,
    ) -> Result<Vec<Memory>>;

    /// `bundle: None` means all bundles.
    async fn get_all_agent_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
    ) -> Result<Vec<Memory>>;

    async fn get_filtered_memories(&self, params: MemoryQueryParams) -> Result<PaginatedMemory>;

    /// `bundle: None` means the agent's default bundle.
    async fn get_memory_detail(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Option<Memory>>;

    /// `bundle: None` means the agent's default bundle.
    async fn get_related_memories(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Vec<Memory>>;

    /// `bundle: None` returns the bundle-level graph (bundles as nodes); `Some(name)` returns
    /// that bundle's concept-level graph.
    async fn get_memory_graph(
        &self,
        agent_id: String,
        bundle: Option<String>,
        search: Option<String>,
    ) -> Result<MemoryGraph>;

    /// `bundle: None` means the agent's default bundle.
    async fn has_incoming_links(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<bool>;

    /// `bundle: None` means the agent's default bundle.
    async fn delete_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
        indexer: &VizierIndexer,
    ) -> Result<()>;

    /// `bundle: None` means the agent's default bundle.
    async fn increment_read_count(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<()>;

    async fn list_bundles(&self, agent_id: String) -> Result<Vec<BundleSummary>>;

    /// Deletes a bundle's `index.md`/`log.md` (and any other non-concept file left in it).
    /// Rejected with `Err` if the bundle still contains any concept document — a bundle must be
    /// emptied of concepts (via `delete_memory`) before it can be deleted itself.
    async fn delete_bundle(&self, agent_id: String, bundle: String) -> Result<()>;

    async fn export_bundle(&self, agent_id: String, bundle: String) -> Result<Vec<u8>>;

    async fn import_bundle(
        &self,
        agent_id: String,
        bundle: String,
        zip_bytes: Vec<u8>,
        indexer: &VizierIndexer,
    ) -> Result<ImportReport>;
}

#[async_trait::async_trait]
impl MemoryStorage for VizierStorage {
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
        self.0
            .write_memory(
                agent_id,
                bundle,
                path,
                create_only,
                title,
                content,
                tags,
                attachments,
                indexer,
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
        self.0
            .query_memory(agent_id, bundle, query, limit, threshold, indexer)
            .await
    }

    async fn get_all_agent_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
    ) -> Result<Vec<Memory>> {
        self.0.get_all_agent_memory(agent_id, bundle).await
    }

    async fn get_filtered_memories(&self, params: MemoryQueryParams) -> Result<PaginatedMemory> {
        self.0.get_filtered_memories(params).await
    }

    async fn get_memory_detail(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Option<Memory>> {
        self.0.get_memory_detail(agent_id, bundle, path).await
    }

    async fn get_related_memories(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Vec<Memory>> {
        self.0.get_related_memories(agent_id, bundle, path).await
    }

    async fn get_memory_graph(
        &self,
        agent_id: String,
        bundle: Option<String>,
        search: Option<String>,
    ) -> Result<MemoryGraph> {
        self.0.get_memory_graph(agent_id, bundle, search).await
    }

    async fn has_incoming_links(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<bool> {
        self.0.has_incoming_links(agent_id, bundle, path).await
    }

    async fn delete_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        self.0.delete_memory(agent_id, bundle, path, indexer).await
    }

    async fn increment_read_count(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<()> {
        self.0.increment_read_count(agent_id, bundle, path).await
    }

    async fn list_bundles(&self, agent_id: String) -> Result<Vec<BundleSummary>> {
        self.0.list_bundles(agent_id).await
    }

    async fn delete_bundle(&self, agent_id: String, bundle: String) -> Result<()> {
        self.0.delete_bundle(agent_id, bundle).await
    }

    async fn export_bundle(&self, agent_id: String, bundle: String) -> Result<Vec<u8>> {
        self.0.export_bundle(agent_id, bundle).await
    }

    async fn import_bundle(
        &self,
        agent_id: String,
        bundle: String,
        zip_bytes: Vec<u8>,
        indexer: &VizierIndexer,
    ) -> Result<ImportReport> {
        self.0
            .import_bundle(agent_id, bundle, zip_bytes, indexer)
            .await
    }
}

pub fn compute_initial_slugs(nodes: &[MemoryGraphNode], search: Option<&str>) -> Vec<String> {
    if let Some(q) = search {
        let q = q.trim().to_lowercase();
        if !q.is_empty() {
            return nodes
                .iter()
                .filter(|n| {
                    n.title.to_lowercase().contains(&q)
                        || n.slug.to_lowercase().contains(&q)
                        || n.tags.iter().any(|t| t.to_lowercase().contains(&q))
                })
                .map(|n| n.slug.clone())
                .collect();
        }
    }
    nodes.iter().map(|n| n.slug.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(slug: &str, tags: &[&str]) -> MemoryGraphNode {
        MemoryGraphNode {
            slug: slug.to_string(),
            bundle: "default".to_string(),
            title: slug.to_string(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            agent_id: "a".to_string(),
            boundary: false,
        }
    }

    #[test]
    fn search_returns_only_matches_case_insensitive() {
        let nodes = vec![node("kubernetes-basics", &["devops"]), node("intro", &["misc"])];
        let initial = compute_initial_slugs(&nodes, Some("KUBE"));
        assert_eq!(initial, vec!["kubernetes-basics".to_string()]);
    }

    #[test]
    fn empty_search_returns_all_nodes() {
        let nodes = vec![node("a", &["x"]), node("b", &["x"])];
        let initial = compute_initial_slugs(&nodes, Some(""));
        assert_eq!(initial, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn no_search_returns_all_nodes() {
        let nodes = vec![
            node("alone", &[]),
            node("hub", &["t"]),
            node("other", &["t"]),
        ];
        let initial = compute_initial_slugs(&nodes, None);
        assert_eq!(
            initial,
            vec!["alone".to_string(), "hub".to_string(), "other".to_string()]
        );
    }

    #[test]
    fn no_search_is_not_capped_per_tag() {
        let nodes: Vec<MemoryGraphNode> = (0..10)
            .map(|i| node(&format!("t-{i}"), &["t"]))
            .collect();
        let initial = compute_initial_slugs(&nodes, None);
        assert_eq!(initial.len(), nodes.len());
    }
}
