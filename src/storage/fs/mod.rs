use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use tokio::sync::Mutex;

use crate::{schema::DocumentIndex, storage::VizierStorageProvider};

mod history;
mod memory;
mod task;

const MEMORY_PATH: &'static str = "documents";
const TASK_PATH: &'static str = "tasks";
const HISTORY_PATH: &'static str = "history";

pub struct FileSystemStorage {
    workspace: String,
    memory_indices: Arc<Mutex<HashMap<String, DocumentIndex>>>,
    embedder: Option<Arc<crate::embedding::EmbeddingModel>>,
}

impl FileSystemStorage {
    pub async fn new(
        workspace: String,
        embedder: Option<Arc<crate::embedding::EmbeddingModel>>,
    ) -> Result<Self> {
        let storage = Self {
            workspace,
            memory_indices: Arc::new(Mutex::new(HashMap::new())),
            embedder,
        };

        storage.reindex_memory().await?;

        Ok(storage)
    }
}

impl VizierStorageProvider for FileSystemStorage {}
