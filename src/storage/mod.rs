use std::sync::Arc;

use anyhow::Result;

use crate::{
    schema::DocumentIndex,
    storage::{
        history::HistoryStorage, indexer::DocumentIndexer, memory::MemoryStorage,
        session::SessionStorage, skill::SkillStorage, task::TaskStorage,
    },
};

pub mod history;
pub mod indexer;
pub mod memory;
pub mod session;
pub mod skill;
pub mod task;

pub mod fs;
pub mod surreal;

pub trait VizierStorageProvider
where
    Self: MemoryStorage
        + TaskStorage
        + HistoryStorage
        + SkillStorage
        + SessionStorage
        + DocumentIndexer,
{
}

#[derive(Clone)]
pub struct VizierStorage(Arc<Box<dyn VizierStorageProvider + Sync + Send + 'static>>);

impl VizierStorage {
    pub fn new<Storage: VizierStorageProvider + Sync + Send + 'static>(storage: Storage) -> Self {
        Self(Arc::new(Box::new(storage)))
    }
}

#[async_trait::async_trait]
impl DocumentIndexer for VizierStorage {
    async fn add_document_index(&self, context: String, path: String) -> Result<DocumentIndex> {
        self.0.add_document_index(context, path).await
    }
    async fn search_document_index(
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

    async fn delete_index(&self, context: String, path: String) -> Result<()> {
        self.0.delete_index(context, path).await
    }
}

impl VizierStorageProvider for VizierStorage {}
