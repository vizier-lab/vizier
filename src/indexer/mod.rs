use std::sync::Arc;

use anyhow::Result;

pub mod sqlite;

use crate::schema::DocumentIndex;

#[async_trait::async_trait]
pub trait DocumentIndexer {
    async fn add_document_index(
        &self,
        context: String,
        path: String,
        content: String,
    ) -> Result<DocumentIndex>;
    async fn search_document_index(
        &self,
        context: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<DocumentIndex>>;
    async fn delete_index(&self, context: String, path: String) -> Result<()>;
}

impl VizierIndexer {
    pub fn build<Indexer: DocumentIndexer + Sync + Send + 'static>(indexer: Indexer) -> Self {
        Self(Arc::new(Box::new(indexer)))
    }
}

pub struct VizierIndexer(Arc<Box<dyn DocumentIndexer + Sync + Send + 'static>>);

impl Clone for VizierIndexer {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl VizierIndexer {
    pub async fn add_document_index(
        &self,
        context: String,
        path: String,
        content: String,
    ) -> Result<DocumentIndex> {
        self.0.add_document_index(context, path, content).await
    }

    pub async fn search_document_index(
        &self,
        context: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<DocumentIndex>> {
        self.0
            .search_document_index(context, query, limit, threshold)
            .await
    }

    pub async fn delete_index(&self, context: String, path: String) -> Result<()> {
        self.0.delete_index(context, path).await
    }
}
