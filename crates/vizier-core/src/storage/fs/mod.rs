use anyhow::Result;

// `FileSystemStorage` is no longer a selectable `VizierStorageProvider` backend (sqlite is now
// the sole runtime backend, research.md §10 / FR-025) — it survives here only so
// `VizierDependencies::new`'s one-time `migrate_filesystem_backend_to_sqlite` migration can read
// a pre-existing `--storage filesystem` deployment's data via these trait impls before copying
// it into `SqliteStorage`. Its `MemoryStorage` impl is gone entirely: Part A of that migration
// reads legacy flat memory files directly (the visibility model they were written under no
// longer exists), not through this struct.
pub mod agent;
pub mod dream_journal;
pub mod global_config;
pub mod history;
pub mod provider;
pub mod session;
pub mod session_file;
pub mod state;
pub mod task;
pub mod user;

pub(crate) const TASK_PATH: &str = "tasks";
pub(crate) const HISTORY_PATH: &str = "history";
pub(crate) const SESSION_PATH: &str = "session";
pub(crate) const STATE_PATH: &str = "state";

pub struct FileSystemStorage {
    pub(crate) workspace: String,
}

impl FileSystemStorage {
    pub async fn new(workspace: String) -> Result<Self> {
        Ok(Self { workspace })
    }
}
