//! The git runner and read-only git views (`specs/041-project-coding-workspace` R12).
//!
//! `git` is an optional runtime program: absent git or a folder outside a repository hides
//! every git surface and leaves the file tools working (FR-024). Every command runs from an
//! argument vector, never a shell, with prompts and locks disabled, under a hard timeout.

pub mod status;
pub mod worktree;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;

use crate::{error::VizierError, projects::channel};

/// How long one git command may run before it is killed.
pub const TIMEOUT: Duration = Duration::from_secs(15);

/// What a folder's enclosing repository is, at the moment it was probed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInfo {
    /// The checked-out branch; `None` for a detached HEAD.
    pub branch: Option<String>,
    /// The repository's working-tree root (the folder may be a subdirectory of it).
    pub toplevel: PathBuf,
}

/// The parsed `--porcelain=v2 --branch` state of the worktree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub branch: Option<String>,
    pub detached: bool,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<StatusEntry>,
}

/// One changed path, before line counts are added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub kind: GitChangeKind,
}

/// The kind of a change, as the API and tools name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub enum GitChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

impl GitChangeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Modified => "modified",
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
            Self::Untracked => "untracked",
            Self::Conflicted => "conflicted",
        }
    }
}

/// A changed file with its added/removed line counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GitChange {
    pub path: String,
    pub old_path: Option<String>,
    pub kind: GitChangeKind,
    pub added: u64,
    pub removed: u64,
}

/// The header's branch and change count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GitSummary {
    pub available: bool,
    pub branch: Option<String>,
    pub detached: bool,
    pub ahead: u32,
    pub behind: u32,
    pub changed: usize,
}

impl GitSummary {
    pub fn unavailable() -> Self {
        Self {
            available: false,
            branch: None,
            detached: false,
            ahead: 0,
            behind: 0,
            changed: 0,
        }
    }
}

/// One file's unified diff against the last commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GitFileDiff {
    pub path: String,
    pub diff: String,
    pub truncated: bool,
    pub binary: bool,
}

/// One local branch, and the worktree already holding it (FR-023).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
    pub checked_out_in: Option<BranchHolder>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BranchHolder {
    /// `None` for the project's own folder.
    pub worktree_id: Option<String>,
    pub name: String,
}

/// One commit in `git_log`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GitCommit {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub subject: String,
}

/// The most a diff or a file tool returns.
pub const DIFF_CAP_BYTES: usize = 256 * 1024;
/// The most one file diff may return before it is truncated.
pub const FILE_DIFF_CAP_BYTES: usize = 1024 * 1024;

fn git_error(stderr: &str) -> VizierError {
    let text = stderr.trim();
    let code = if text.contains("not a git repository") {
        "git_unavailable"
    } else if text.contains("Your local changes to the following files would be overwritten") {
        "local_changes"
    } else if text.contains("already exists") {
        "branch_exists"
    } else if text.contains("is already checked out at") {
        "branch_checked_out"
    } else if text.contains("not fully merged") {
        "branch_not_merged"
    } else {
        "git_failed"
    };
    VizierError(format!("{code}: {text}"))
}

/// Whether `git` exists on this machine; probed once per process.
async fn available() -> bool {
    static AVAILABLE: tokio::sync::OnceCell<bool> = tokio::sync::OnceCell::const_new();
    *AVAILABLE
        .get_or_init(|| async {
            let output = Command::new("git")
                .arg("--version")
                .stdin(Stdio::null())
                .output()
                .await;
            output.is_ok_and(|output| output.status.success())
        })
        .await
}

/// The repository a folder lives in, or `None` when git is missing or the folder is not in a
/// repository (or does not exist).
pub async fn probe(dir: &Path) -> Option<GitInfo> {
    if !dir.is_dir() || !available().await {
        return None;
    }
    let git = Git::new(dir);
    let output = git
        .run_read(&["rev-parse", "--show-toplevel", "--git-common-dir", "--abbrev-ref", "HEAD"])
        .await
        .ok()?;
    let mut lines = output.lines();
    let toplevel = lines.next()?.to_string();
    let _common = lines.next();
    let branch = lines.next().filter(|branch| *branch != "HEAD");
    Some(GitInfo {
        branch: branch.map(str::to_string),
        toplevel: PathBuf::from(toplevel),
    })
}

/// A git command rooted at one folder.
#[derive(Debug, Clone)]
pub struct Git {
    pub dir: PathBuf,
}

impl Git {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    fn command(&self, args: &[&str], optional_locks: bool) -> Command {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&self.dir)
            .args(["-c", "core.quotepath=off"])
            .args(["-c", "color.ui=false"])
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", if optional_locks { "0" } else { "1" })
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command
    }

    async fn run_with(&self, args: &[&str], optional_locks: bool) -> Result<Vec<u8>, VizierError> {
        if !available().await {
            return Err(VizierError(
                "git_unavailable: git is not installed on this machine".into(),
            ));
        }
        let mut child = self
            .command(args, optional_locks)
            .spawn()
            .map_err(|err| VizierError(format!("git_unavailable: {err}")))?;
        let output = match tokio::time::timeout(TIMEOUT, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(err)) => return Err(VizierError(format!("git_failed: {err}"))),
            Err(_) => return Err(VizierError("git_failed: timed out after 15 seconds".into())),
        };
        if !output.status.success() {
            return Err(git_error(&String::from_utf8_lossy(&output.stderr)));
        }
        Ok(output.stdout)
    }

    /// Run a read-only command (`GIT_OPTIONAL_LOCKS=0`) and return its stdout as text.
    pub async fn run_read(&self, args: &[&str]) -> Result<String, VizierError> {
        let bytes = self.run_with(args, true).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Run a command that changes the repository or the worktree.
    pub async fn run_write(&self, args: &[&str]) -> Result<String, VizierError> {
        let bytes = self.run_with(args, false).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// `true` when `folder` is `dir` itself or inside it.
    pub fn contains(&self, folder: &Path) -> bool {
        folder == self.dir || folder.starts_with(&self.dir)
    }

    /// The folder as a person reads it.
    pub fn display(&self) -> String {
        channel::display_path(&self.dir)
    }

    pub async fn status(&self) -> Result<Status, VizierError> {
        let text = self
            .run_read(&["status", "--porcelain=v2", "-z", "--branch", "--untracked-files=all"])
            .await?;
        Ok(status::parse_status(&text))
    }

    pub async fn changes(&self) -> Result<Vec<GitChange>, VizierError> {
        let status = self.status().await?;
        let numstat = self.run_read(&["diff", "HEAD", "--numstat", "-z"]).await?;
        Ok(status::changes_with_counts(&status, &numstat, &self.dir))
    }

    pub async fn summary(&self) -> GitSummary {
        match self.status().await {
            Ok(status) => GitSummary {
                available: true,
                branch: status.branch,
                detached: status.detached,
                ahead: status.ahead,
                behind: status.behind,
                changed: status.entries.len(),
            },
            Err(_) => GitSummary::unavailable(),
        }
    }

    /// One file's unified diff against `HEAD` (or the index when `staged`).
    pub async fn file_diff(
        &self,
        path: &str,
        staged: bool,
    ) -> Result<GitFileDiff, VizierError> {
        let tracked = self
            .run_read(&["ls-files", "--error-unmatch", "--", path])
            .await
            .is_ok();
        let (mut diff, mut binary) = if tracked || staged {
            let text = if staged {
                self.run_read(&["diff", "--cached", "--", path]).await?
            } else {
                self.run_read(&["diff", "HEAD", "--", path]).await?
            };
            let binary = text.contains("Binary files");
            (text, binary)
        } else {
            // An untracked file has no diff; synthesise an all-added one.
            let full = self.dir.join(path);
            let bytes = match std::fs::read(&full) {
                Ok(bytes) => bytes,
                Err(err) => return Err(VizierError(format!("not_found: {path} ({err})"))),
            };
            let binary = bytes.iter().take(8192).any(|b| *b == 0);
            (status::synthetic_untracked_diff(path, &bytes, binary), binary)
        };
        let truncated = diff.len() > FILE_DIFF_CAP_BYTES;
        if truncated {
            diff.truncate(FILE_DIFF_CAP_BYTES);
        }
        Ok(GitFileDiff {
            path: path.to_string(),
            diff,
            truncated,
            binary,
        })
    }

    pub async fn branches(&self) -> Result<Vec<GitBranch>, VizierError> {
        let refs = self
            .run_read(&[
                "for-each-ref",
                "refs/heads",
                "--format=%(refname:short)%1f%(upstream:short)",
            ])
            .await?;
        let current = self
            .run_read(&["rev-parse", "--abbrev-ref", "HEAD"])
            .await
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| s != "HEAD");
        let worktrees = self.run_read(&["worktree", "list", "--porcelain"]).await?;
        Ok(status::parse_branches(&refs, current.as_deref(), &worktrees))
    }

    pub async fn log(&self, n: usize, path: Option<&str>) -> Result<Vec<GitCommit>, VizierError> {
        let n = n.clamp(1, 50).to_string();
        let mut args = vec!["log", "-n", n.as_str(), "--format=%H%x1f%an%x1f%aI%x1f%s"];
        if let Some(path) = path {
            args.push("--");
            args.push(path);
        }
        let text = self.run_read(&args).await?;
        Ok(status::parse_log(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_errors_become_stable_codes() {
        assert!(git_error("fatal: not a git repository").0.starts_with("git_unavailable:"));
        assert!(git_error("error: Your local changes to the following files would be overwritten by checkout")
            .0
            .starts_with("local_changes:"));
        assert!(git_error("fatal: a branch named 'x' already exists").0.starts_with("branch_exists:"));
        assert!(git_error("fatal: 'x' is already checked out at '/tmp/wt'").0.starts_with("branch_checked_out:"));
    }

    #[tokio::test]
    async fn no_repository_has_no_git() {
        let dir = tempfile::tempdir().unwrap();
        assert!(probe(dir.path()).await.is_none());
    }
}
