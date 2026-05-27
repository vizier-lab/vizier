use std::sync::Arc;

use anyhow::Result;

use crate::{
    schema::DocumentIndex,
    storage::{
        VizierStorageProvider,
        indexer::{DocumentIndexer, VizierIndexer},
    },
};

mod history;
mod memory;
mod session;
mod shared_document;
mod skill;
mod state;
mod task;
mod user;

const MEMORY_PATH: &'static str = "memory";
const TASK_PATH: &'static str = "tasks";
const HISTORY_PATH: &'static str = "history";
const SESSION_PATH: &'static str = "session";
const STATE_PATH: &'static str = "state";

pub struct FileSystemStorage {
    workspace: String,
    indices: Arc<VizierIndexer>,
}

impl FileSystemStorage {
    pub async fn new(
        workspace: String,
        indices: Arc<VizierIndexer>,
        reindex: bool,
    ) -> Result<Self> {
        let storage = Self { workspace, indices };

        if reindex {
            storage.reindex_memory().await?;
            storage.reindex_shared_documents().await?;
        }

        Ok(storage)
    }
}

impl VizierStorageProvider for FileSystemStorage {}

#[async_trait::async_trait]
impl DocumentIndexer for FileSystemStorage {
    async fn add_document_index(&self, context: String, path: String) -> Result<DocumentIndex> {
        self.indices.add_document_index(context, path).await
    }
    async fn search_document_index(
        &self,
        context: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<DocumentIndex>> {
        self.indices
            .search_document_index(context, query, limit, threshold)
            .await
    }

    async fn delete_index(&self, context: String, path: String) -> Result<()> {
        self.indices.delete_index(context, path).await
    }
}
