use anyhow::Result;

use crate::{indexer::DocumentIndexer, schema::DocumentIndex};

/// A `DocumentIndexer` that does nothing — used where an embedding-backed indexer isn't
/// available (e.g. during startup migration of a legacy memory belonging to an agent whose
/// embedding config can't be resolved). Content is still preserved via `DocumentStore`/the
/// Memory Graph Index; only semantic search over it lags until the agent rewrites it.
pub struct NoopIndexer;

#[async_trait::async_trait]
impl DocumentIndexer for NoopIndexer {
    async fn add_document_index(
        &self,
        context: String,
        path: String,
        _content: String,
    ) -> Result<DocumentIndex> {
        Ok(DocumentIndex {
            path,
            embedding: vec![],
            context,
        })
    }

    async fn search_document_index(
        &self,
        _context: String,
        _query: String,
        _limit: usize,
        _threshold: f64,
    ) -> Result<Vec<DocumentIndex>> {
        Ok(vec![])
    }

    async fn delete_index(&self, _context: String, _path: String) -> Result<()> {
        Ok(())
    }
}
