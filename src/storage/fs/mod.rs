use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use tokio::sync::Mutex;

use crate::{embedding::VizierEmbedder, schema::DocumentIndex, storage::VizierStorageProvider};

mod history;
mod memory;
mod task;

const MEMORY_PATH: &'static str = "memory";
const TASK_PATH: &'static str = "tasks";
const HISTORY_PATH: &'static str = "history";

pub struct FileSystemStorage {
    workspace: String,
    memory_indices: Arc<Mutex<HashMap<String, DocumentIndex>>>,
    embedder: Option<Arc<VizierEmbedder>>,
}

impl FileSystemStorage {
    pub async fn new(workspace: String, embedder: Option<Arc<VizierEmbedder>>) -> Result<Self> {
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
