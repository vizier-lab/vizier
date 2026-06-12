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
            slug,
            title,
            content,
            visibility,
            shared_to,
            tags,
        } => storage
            .write_memory(
                agent_id.to_string(),
                slug.clone(),
                title.clone(),
                content.clone(),
                visibility.clone(),
                shared_to.clone(),
                tags.clone(),
                indexer,
            )
            .await
            .map(MemoryOpResponse::Memory),
        MemoryOpRequest::Query {
            query,
            limit,
            threshold,
        } => storage
            .query_memory(
                agent_id.to_string(),
                query.clone(),
                *limit,
                *threshold,
                indexer,
            )
            .await
            .map(MemoryOpResponse::MemoryList),
        MemoryOpRequest::GetById { slug } => storage
            .get_memory_detail(agent_id.to_string(), slug.clone())
            .await
            .map(MemoryOpResponse::MemoryOption),
        MemoryOpRequest::List { params } => storage
            .get_filtered_memories(params.clone())
            .await
            .map(MemoryOpResponse::Paginated),
        MemoryOpRequest::GetRelated { slug } => storage
            .get_related_memories(agent_id.to_string(), slug.clone())
            .await
            .map(MemoryOpResponse::MemoryList),
        MemoryOpRequest::GetGraph => storage
            .get_memory_graph(agent_id.to_string())
            .await
            .map(MemoryOpResponse::Graph),
        MemoryOpRequest::Delete { slug } => storage
            .delete_memory(agent_id.to_string(), slug.clone(), indexer)
            .await
            .map(|_| MemoryOpResponse::Unit),
    }
}
