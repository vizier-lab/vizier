use anyhow::Result;

use crate::{
    indexer::VizierIndexer,
    schema::{MemoryOpEnvelope, MemoryOpRequest, MemoryOpResponse},
    storage::{VizierStorage, memory::MemoryStorage},
};

pub async fn handle_memory_ops(
    rx: flume::Receiver<MemoryOpEnvelope>,
    indexer: VizierIndexer,
    agent_id: String,
    storage: VizierStorage,
) -> Result<()> {
    let mut rx = rx;
    while let Ok(envelope) = rx.recv_async().await {
        let result = dispatch_memory_op(&envelope.op, &agent_id, &storage, &indexer).await;
        let _ = envelope.response.send(result);
    }
    Ok(())
}

async fn dispatch_memory_op(
    op: &MemoryOpRequest,
    agent_id: &str,
    storage: &VizierStorage,
    indexer: &VizierIndexer,
) -> Result<MemoryOpResponse> {
    match op {
        MemoryOpRequest::Write {
            bundle,
            path,
            create_only,
            title,
            content,
            tags,
            attachments,
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
                attachments.clone(),
                origin,
                indexer,
            )
            .await
            .map(MemoryOpResponse::Memory),
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
                indexer,
            )
            .await
            .map(MemoryOpResponse::MemoryList),
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
            .delete_bundle(agent_id.to_string(), bundle.clone(), *force, origin, indexer)
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
            .diff_memory_revisions(agent_id.to_string(), bundle.clone(), path.clone(), *from, *to)
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
            )
            .await
            .map(MemoryOpResponse::Rollback),
    }
}
