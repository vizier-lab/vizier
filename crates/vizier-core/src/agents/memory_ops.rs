use anyhow::Result;

use crate::{
    config::{ChunkLimits, MemoryLimits},
    indexer::VizierIndexer,
    schema::{MemoryOpRequest, MemoryOpResponse},
    transport::MemoryOpInbox,
    storage::{VizierStorage, memory::MemoryStorage, memory_bundle::WritePolicy},
};

/// The per-agent chunking and recall settings this handler applies to every memory op it
/// services. Carried here rather than looked up per request because they come from the agent
/// config the owning `agent_process` was spawned with.
#[derive(Clone)]
pub struct MemoryOpSettings {
    pub chunking: ChunkLimits,
    /// The size bounds every write through this handler is judged against
    /// (specs/043-unify-core-and-memory). The identity document is tighter regardless of this
    /// value, because the address decides that.
    pub limits: MemoryLimits,
    /// Cap on how many passages one document may contribute to a single search (FR-012).
    pub per_document: usize,
}

pub async fn handle_memory_ops(
    mut rx: MemoryOpInbox,
    indexer: VizierIndexer,
    agent_id: String,
    storage: VizierStorage,
    settings: MemoryOpSettings,
) -> Result<()> {
    while let Some((op, replier)) = rx.recv().await {
        let result = dispatch_memory_op(&op, &agent_id, &storage, &indexer, &settings).await;
        replier.answer(result).await;
    }
    Ok(())
}

async fn dispatch_memory_op(
    op: &MemoryOpRequest,
    agent_id: &str,
    storage: &VizierStorage,
    indexer: &VizierIndexer,
    settings: &MemoryOpSettings,
) -> Result<MemoryOpResponse> {
    match op {
        MemoryOpRequest::Write {
            bundle,
            path,
            create_only,
            title,
            content,
            tags,
            origin,
        } => storage
            .write_memory(
                agent_id.to_string(),
                bundle.clone(),
                path.clone(),
                *create_only,
                title.clone(),
                content.clone(),
                tags.clone(),
                origin,
                indexer,
                &settings.chunking,
                &WritePolicy::new(settings.limits.clone()),
            )
            .await
            .map(MemoryOpResponse::from_written),
        MemoryOpRequest::AttachArtifact {
            bundle,
            path,
            artifact_id,
            artifact_name,
            origin,
        } => storage
            .attach_memory_attachment(
                agent_id.to_string(),
                bundle.clone(),
                path.clone(),
                artifact_id.clone(),
                artifact_name.clone(),
                origin,
            )
            .await
            .map(MemoryOpResponse::from_memory),
        MemoryOpRequest::DetachAttachment {
            bundle,
            path,
            name,
            origin,
        } => storage
            .detach_memory_attachment(
                agent_id.to_string(),
                bundle.clone(),
                path.clone(),
                name.clone(),
                origin,
            )
            .await
            .map(MemoryOpResponse::from_memory),
        MemoryOpRequest::Query {
            bundle,
            query,
            limit,
            threshold,
        } => storage
            .query_memory(
                agent_id.to_string(),
                bundle.clone(),
                query.clone(),
                *limit,
                *threshold,
                settings.per_document,
                indexer,
                &settings.chunking,
            )
            .await
            .map(MemoryOpResponse::PassageList),
        MemoryOpRequest::GetById { bundle, path } => storage
            .get_memory_detail(agent_id.to_string(), bundle.clone(), path.clone())
            .await
            .map(MemoryOpResponse::MemoryOption),
        MemoryOpRequest::List { params } => storage
            .get_filtered_memories(params.clone())
            .await
            .map(MemoryOpResponse::Paginated),
        MemoryOpRequest::GetRelated { bundle, path } => storage
            .get_related_memories(agent_id.to_string(), bundle.clone(), path.clone())
            .await
            .map(MemoryOpResponse::MemoryList),
        MemoryOpRequest::GetGraph { bundle, search } => storage
            .get_memory_graph(agent_id.to_string(), bundle.clone(), search.clone())
            .await
            .map(MemoryOpResponse::Graph),
        MemoryOpRequest::Delete {
            bundle,
            path,
            origin,
        } => storage
            .delete_memory(
                agent_id.to_string(),
                bundle.clone(),
                path.clone(),
                origin,
                indexer,
            )
            .await
            .map(|_| MemoryOpResponse::Unit),
        MemoryOpRequest::ListBundles => storage
            .list_bundles(agent_id.to_string())
            .await
            .map(MemoryOpResponse::Bundles),
        MemoryOpRequest::DeleteBundle {
            bundle,
            force,
            origin,
        } => storage
            .delete_bundle(
                agent_id.to_string(),
                bundle.clone(),
                *force,
                origin,
                indexer,
            )
            .await
            .map(|_| MemoryOpResponse::Unit),
        MemoryOpRequest::ExportBundle { bundle } => storage
            .export_bundle(agent_id.to_string(), bundle.clone())
            .await
            .map(MemoryOpResponse::Export),
        MemoryOpRequest::ImportBundle {
            bundle,
            zip_bytes,
            origin,
        } => storage
            .import_bundle(
                agent_id.to_string(),
                bundle.clone(),
                zip_bytes.clone(),
                origin,
                indexer,
                &settings.chunking,
            )
            .await
            .map(MemoryOpResponse::Import),
        MemoryOpRequest::ListRevisions {
            bundle,
            path,
            offset,
            limit,
        } => storage
            .list_memory_revisions(
                agent_id.to_string(),
                bundle.clone(),
                path.clone(),
                *offset,
                *limit,
            )
            .await
            .map(MemoryOpResponse::Revisions),
        MemoryOpRequest::GetRevision { bundle, path, seq } => storage
            .get_memory_revision(agent_id.to_string(), bundle.clone(), path.clone(), *seq)
            .await
            .map(MemoryOpResponse::Revision),
        MemoryOpRequest::DiffRevisions {
            bundle,
            path,
            from,
            to,
        } => storage
            .diff_memory_revisions(
                agent_id.to_string(),
                bundle.clone(),
                path.clone(),
                *from,
                *to,
            )
            .await
            .map(MemoryOpResponse::Diff),
        MemoryOpRequest::Rollback {
            bundle,
            path,
            seq,
            origin,
        } => storage
            .rollback_memory(
                agent_id.to_string(),
                bundle.clone(),
                path.clone(),
                *seq,
                origin,
                indexer,
                &settings.chunking,
                &WritePolicy::new(settings.limits.clone()),
            )
            .await
            .map(MemoryOpResponse::Rollback),
    }
}
