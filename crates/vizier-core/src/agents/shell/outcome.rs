//! The result of one workspace-shell command (`specs/040-projects-mode/data-model.md` §5).
//!
//! A non-zero exit code is a successful result: the tool only fails when the project is
//! unavailable, the shell switch is off, or the working directory escapes the project root.

use std::path::PathBuf;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One command to run: the line, an optional absolute working directory, and a time limit.
#[derive(Debug, Clone)]
pub struct ShellCommand {
    pub command: String,
    /// Already resolved and validated against the project root by the caller.
    pub workdir: PathBuf,
    pub timeout: Duration,
}

/// What a command produced, independent of the shell backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShellOutcome {
    /// UTF-8 lossy, capped at [`crate::schema::OUTPUT_CAP_BYTES`].
    pub stdout: String,
    /// UTF-8 lossy, capped at [`crate::schema::OUTPUT_CAP_BYTES`].
    pub stderr: String,
    /// The process exit code; `None` when the time limit killed it.
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    /// The limit was reached and the process tree was killed.
    pub timed_out: bool,
    /// Either stream hit the cap and was cut.
    pub truncated: bool,
}

impl ShellOutcome {
    /// The text contract the shell addon's `shell_exec` has always returned: stdout only.
    pub fn to_text(&self) -> String {
        self.stdout.clone()
    }
}
