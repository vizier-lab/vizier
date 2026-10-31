//! Projects (`specs/040-projects-mode/data-model.md`): a host folder mounted to exactly one
//! agent, and the derived session row its sidebar lists.

use std::path::PathBuf;
use std::time::Duration;

use schemars::JsonSchema;
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

/// What a project chat may do (`specs/041-project-coding-workspace` FR-009): Plan is read-only,
/// Build may change files and, with the shell switch on, run commands. No row means `Plan`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema,
)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProjectMode {
    #[default]
    Plan,
    Build,
}

impl ProjectMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Build => "build",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Plan => "Plan",
            Self::Build => "Build",
        }
    }
}

impl std::str::FromStr for ProjectMode {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "plan" => Ok(Self::Plan),
            "build" => Ok(Self::Build),
            _ => Err(()),
        }
    }
}

/// One isolated checkout of a project (`specs/041-project-coding-workspace` data-model §3),
/// bound to exactly one chat topic. The row mirrors storage; `available`, change counts and
/// `branch_merged` are derived at read time and never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worktree {
    pub id: String,
    pub project_id: String,
    pub agent_id: AgentId,
    /// The `user_id` of the person who created it.
    pub owner_id: String,
    /// The username at creation: the chat's channel user.
    pub owner_name: String,
    pub topic: String,
    pub name: String,
    pub branch: String,
    /// What it started from, for "is the branch merged?".
    pub base_ref: String,
    /// Canonical `{root}/.vizier/{agent_id}/{name}`.
    pub path: PathBuf,
    pub created_at: i64,
}

/// The worktree identity a turn needs, without the row's ownership bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct WorktreeRef {
    pub id: String,
    pub name: String,
    pub branch: String,
}

impl From<&Worktree> for WorktreeRef {
    fn from(worktree: &Worktree) -> Self {
        Self {
            id: worktree.id.clone(),
            name: worktree.name.clone(),
            branch: worktree.branch.clone(),
        }
    }
}

/// The project a subagent piece works in, copied from the launching turn (FR-007a). Set only
/// by `BackgroundJobs::dispatch_piece`, never by a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProjectScope {
    pub project_id: String,
    /// The launching chat's username.
    pub owner: String,
    /// The launching chat's topic: resolves its worktree.
    pub topic: String,
    /// The launching turn's mode, fixed for the piece.
    pub mode: ProjectMode,
}

/// One session of a project, as the project sessions endpoint projects it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSessionRow {
    pub topic: String,
    pub title: String,
    pub is_thinking: bool,
    /// `MAX(session_history.timestamp)` for the topic; `None` for a brand-new empty session.
    pub last_activity: Option<i64>,
    /// The chat's saved mode; no row means `Plan`.
    pub mode: ProjectMode,
    /// The worktree the chat is bound to, when it has one.
    pub worktree: Option<WorktreeRef>,
}
