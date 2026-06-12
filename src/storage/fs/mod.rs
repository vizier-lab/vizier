use anyhow::Result;

use crate::storage::VizierStorageProvider;

mod agent;
mod dream_journal;
mod global_config;
mod history;
mod memory;
mod provider;
mod session;
mod session_file;
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
}

impl FileSystemStorage {
    pub async fn new(workspace: String) -> Result<Self> {
        Ok(Self { workspace })
    }
}

impl VizierStorageProvider for FileSystemStorage {}
