//! Worktree helpers (`specs/041-project-coding-workspace` R13): the repository metadata that
//! must change before a worktree is created, without touching any tracked file (FR-026).

use std::path::PathBuf;

use crate::{error::VizierError, projects::git::Git};

/// Append `/.vizier/` to the repository's `info/exclude` once, so the worktree area is hidden
/// from `git status` in the main folder without changing a tracked `.gitignore`.
pub async fn ensure_excluded(git: &Git) -> Result<(), VizierError> {
    let common = git
        .run_read(&["rev-parse", "--git-common-dir"])
        .await?
        .trim()
        .to_string();
    let common = PathBuf::from(common);
    let common = if common.is_absolute() {
        common
    } else {
        git.dir.join(common)
    };
    let info = common.join("info");
    std::fs::create_dir_all(&info)
        .map_err(|err| VizierError(format!("git_failed: cannot create {}: {err}", info.display())))?;
    let exclude = info.join("exclude");
    let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
    let present = existing.lines().any(|line| {
        let line = line.trim();
        line == "/.vizier/" || line == ".vizier/" || line == ".vizier"
    });
    if present {
        return Ok(());
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("/.vizier/\n");
    std::fs::write(&exclude, text)
        .map_err(|err| VizierError(format!("git_failed: cannot write {}: {err}", exclude.display())))
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn git_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    async fn repo() -> Option<tempfile::TempDir> {
        if !git_available() {
            return None;
        }
        let dir = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "-q", "-b", "master"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "T"],
        ] {
            assert!(Command::new("git").current_dir(dir.path()).args(&args).status().unwrap().success());
        }
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        assert!(Command::new("git").current_dir(dir.path()).args(["add", "-A"]).status().unwrap().success());
        assert!(Command::new("git").current_dir(dir.path()).args(["commit", "-qm", "init"]).status().unwrap().success());
        Some(dir)
    }

    #[tokio::test]
    async fn the_worktree_area_is_excluded_once() {
        let Some(dir) = repo().await else { return };
        let git = Git::new(dir.path());
        ensure_excluded(&git).await.unwrap();
        ensure_excluded(&git).await.unwrap();
        let exclude = dir.path().join(".git/info/exclude");
        let text = std::fs::read_to_string(&exclude).unwrap();
        assert_eq!(text.matches("/.vizier/").count(), 1, "{text}");
        // No tracked file changed.
        let status = git.run_read(&["status", "--porcelain"]).await.unwrap();
        assert!(status.trim().is_empty(), "{status}");
    }
}
