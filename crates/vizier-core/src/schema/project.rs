//! Projects (`specs/040-projects-mode/data-model.md`): a host folder mounted to exactly one
//! agent, and the derived session row its sidebar lists.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::schema::AgentId;

/// The project id is a nanoid of this length, the same shape as a WebUI topic id.
pub const PROJECT_ID_LEN: usize = 10;
/// The workspace shell's default per-command time limit.
pub const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
/// The workspace shell's hard per-call limit an agent cannot exceed.
pub const MAX_COMMAND_TIMEOUT: Duration = Duration::from_secs(600);
/// Each captured stream is capped at this, UTF-8 safe.
pub const OUTPUT_CAP_BYTES: usize = 64 * 1024;
/// The project instruction document is read up to this many bytes.
pub const INSTRUCTIONS_CAP_BYTES: usize = 32 * 1024;
/// The title of a session created empty by `POST …/projects/{id}/sessions`; replaced after the
/// first request, like a topic slug.
pub const NEW_SESSION_TITLE: &str = "New session";

/// The long-lived entity: a folder mounted to one agent. `available` is derived at read time,
/// never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub agent_id: AgentId,
    /// Display name; defaults to the folder's final component.
    pub name: String,
    /// Canonical absolute path.
    pub folder: PathBuf,
    /// The `user_id` of the person who opened it.
    pub owner_id: String,
    pub shell_enabled: bool,
    /// Unix milliseconds.
    pub created_at: i64,
    /// `None` while open; set by soft removal.
    pub removed_at: Option<i64>,
}

impl Project {
    /// `folder.is_dir()` at read time. A missing folder refuses commands but keeps history
    /// readable.
    pub fn available(&self) -> bool {
        self.folder.is_dir()
    }

    /// The folder shown to people: the canonical path without Windows' `\\?\` verbatim prefix.
    pub fn display_folder(&self) -> String {
        crate::projects::channel::display_path(&self.folder)
    }
}

/// One session of a project, as the project sessions endpoint projects it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSessionRow {
    pub topic: String,
    pub title: String,
    pub is_thinking: bool,
    /// `MAX(session_history.timestamp)` for the topic; `None` for a brand-new empty session.
    pub last_activity: Option<i64>,
}
