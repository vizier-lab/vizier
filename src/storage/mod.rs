use std::sync::Arc;

use crate::storage::{history::HistoryStorage, memory::MemoryStorage, task::TaskStorage};

pub mod history;
pub mod memory;
pub mod surreal;
pub mod task;

pub trait VizierStorageProvider
where
    Self: MemoryStorage + TaskStorage + HistoryStorage,
{
}

#[derive(Clone)]
pub struct VizierStorage(Arc<Box<dyn VizierStorageProvider + Sync + Send + 'static>>);

impl VizierStorage {
    pub fn new<Storage: VizierStorageProvider + Sync + Send + 'static>(storage: Storage) -> Self {
        Self(Arc::new(Box::new(storage)))
    }
}

impl VizierStorageProvider for VizierStorage {}
