//! An agent's scratch files (`specs/020-scratchpad-tools`): named plain-text notes private to
//! one conversation, which last while it has a turn running or queued and are cleared once it
//! goes idle (`agents/process.rs`), on restart and with the conversation or agent.
//!
//! Limits are checked here, atomically with the write, and a refusal is a value, never an
//! `Err` (research R3): an over-limit write leaves the file as it was.

use anyhow::Result;

use crate::{schema::VizierSession, storage::VizierStorage};

/// Largest a scratch file may be, in UTF-8 bytes.
pub const MAX_SCRATCH_FILE_BYTES: u64 = 1_048_576;
/// Most scratch files one conversation may hold.
pub const MAX_SCRATCH_FILES: usize = 64;
/// Longest a scratch file name may be, in bytes.
pub const MAX_SCRATCH_NAME_BYTES: usize = 128;

/// What a write or append did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScratchWrite {
    /// Saved; `size` is the file's size afterwards.
    Written { size: u64 },
    /// The file would pass `limit`; `current` is its size now (0 if absent).
    TooLarge { limit: u64, current: u64 },
    /// A new name, but the conversation already holds `limit` files, named in `names`.
    TooMany { limit: usize, names: Vec<String> },
}

#[async_trait::async_trait]
pub trait ScratchStorage {
    /// Create the file or replace all of its content.
    async fn write_scratch(&self, session: &VizierSession, name: &str, content: &str) -> Result<ScratchWrite>;

    /// Add `content` to the end of the file exactly as given, creating it if absent.
    async fn append_scratch(&self, session: &VizierSession, name: &str, content: &str) -> Result<ScratchWrite>;

    /// The file's content, or `None` if the session has no file of that name.
    async fn read_scratch(&self, session: &VizierSession, name: &str) -> Result<Option<String>>;

    /// The names of the session's files, sorted.
    async fn scratch_names(&self, session: &VizierSession) -> Result<Vec<String>>;

    /// Delete every file of the session, when it goes idle; returns how many there were.
    async fn clear_scratch(&self, session: &VizierSession) -> Result<usize>;

    /// Startup sweep: nothing can be running yet, so every row is a leftover.
    async fn clear_all_scratch(&self) -> Result<usize>;
}

#[async_trait::async_trait]
impl ScratchStorage for VizierStorage {
    async fn write_scratch(&self, session: &VizierSession, name: &str, content: &str) -> Result<ScratchWrite> {
        self.0.write_scratch(session, name, content).await
    }

    async fn append_scratch(&self, session: &VizierSession, name: &str, content: &str) -> Result<ScratchWrite> {
        self.0.append_scratch(session, name, content).await
    }

    async fn read_scratch(&self, session: &VizierSession, name: &str) -> Result<Option<String>> {
        self.0.read_scratch(session, name).await
    }

    async fn scratch_names(&self, session: &VizierSession) -> Result<Vec<String>> {
        self.0.scratch_names(session).await
    }

    async fn clear_scratch(&self, session: &VizierSession) -> Result<usize> {
        self.0.clear_scratch(session).await
    }

    async fn clear_all_scratch(&self) -> Result<usize> {
        self.0.clear_all_scratch().await
    }
}
