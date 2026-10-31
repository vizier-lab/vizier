//! The `## Project` section of a project session's system prompt
//! (`specs/041-project-coding-workspace` contracts/coding-tools.md §4).
//!
//! It holds only what is fixed for the life of a chat: the project, the working folder, the
//! worktree's name, a fixed paragraph explaining both modes, and the bounded instruction
//! document. The mode, branch, shell switch and folder availability change between turns, so
//! they live in the per-message `## Project state` block instead (R17) — a mode or branch switch
//! must never invalidate the provider's prompt cache for the whole history.

use crate::projects::channel;
use crate::projects::context::ProjectTurnContext;

/// The section, after CORE and before the configured documents.
pub fn project_md(project: &ProjectTurnContext) -> String {
    let folder = channel::display_path(&project.cwd);
    let mut out = format!(
        "## Project\n\nYou are working in the project \"{}\", at folder {}",
        project.name, folder
    );
    match &project.worktree {
        Some(worktree) => out.push_str(&format!(", in its worktree \"{}\".\n", worktree.name)),
        None => out.push_str(".\n"),
    }
    if project.is_piece {
        out.push_str(
            "You were started from a project chat and report back to it. This chat has a Plan \
             mode (read and search only) and a Build mode (change files; run commands when the \
             shell is on). The current mode, branch and shell state are given with each message \
             under `## Project state`; follow it. You have no `propose_plan`.\n",
        );
    } else {
        out.push_str(
            "This chat has a Plan mode (read and search only; finish with `propose_plan`) and a \
             Build mode (change files; run commands when the shell is on). The current mode, \
             branch and shell state are given with each message under `## Project state`; follow \
             it.\n",
        );
    }

    if let Some(instructions) = &project.instructions {
        out.push_str("\n## Project Instructions\n\n");
        out.push_str(instructions);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        if project.instructions_truncated {
            out.push_str("\nThe project instructions were truncated at 32 KiB.\n");
        }
    }
    out
}

/// The per-message `## Project state` block: everything about the project that may change
/// between two turns of one chat.
pub fn project_state_md(project: &ProjectTurnContext) -> String {
    if !project.available {
        return "## Project state\nFolder unavailable — the folder is missing, so project tools \
                are off until it is restored."
            .to_string();
    }
    let mode = match (project.mode, project.shell_enabled) {
        (crate::schema::ProjectMode::Plan, _) => {
            "Mode: Plan — you can read and search the project but not change it. Finish with \
             propose_plan."
        }
        (crate::schema::ProjectMode::Build, true) => {
            "Mode: Build — you can change files and run commands."
        }
        (crate::schema::ProjectMode::Build, false) => {
            "Mode: Build — you can change files; commands are off for this project."
        }
    };
    let mut out = format!("## Project state\n{mode}");
    if let Some(git) = &project.git {
        match &git.branch {
            Some(branch) => out.push_str(&format!("\nBranch: {branch}")),
            None => {
                // A detached HEAD has no branch name; `git rev-parse --short HEAD` would need a
                // second call, so the state says only that it is detached.
                out.push_str("\nBranch: detached HEAD");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::*;
    use crate::projects::git::GitInfo;
    use crate::schema::{ProjectMode, WorktreeRef};

    fn ctx() -> ProjectTurnContext {
        ProjectTurnContext {
            project_id: "p3xY9aQ2mZ".into(),
            name: "alpha".into(),
            folder: PathBuf::from("/tmp/vizier-project-alpha"),
            cwd: PathBuf::from("/tmp/vizier-project-alpha"),
            worktree: None,
            mode: ProjectMode::Plan,
            git: None,
            available: true,
            shell_enabled: true,
            instructions: None,
            instructions_truncated: false,
            is_piece: false,
            ledger: Arc::new(Default::default()),
        }
    }

    #[test]
    fn a_project_session_is_told_its_name_folder_and_both_modes() {
        let md = project_md(&ctx());
        assert!(md.contains("## Project"));
        assert!(md.contains("alpha"));
        assert!(md.contains("/tmp/vizier-project-alpha"));
        assert!(md.contains("Plan mode"));
        assert!(md.contains("Build mode"));
        // The volatile facts moved to the context block.
        assert!(!md.contains("shell is turned off"));
        assert!(!md.contains("folder is unavailable"));
        assert!(!md.contains("Mode:"));
    }

    /// FR-017/R17: the system prompt is byte-identical for the life of a chat, so switching
    /// mode, the shell switch, the branch or losing the folder costs no prompt-cache miss.
    #[test]
    fn the_project_section_is_stable_across_volatile_changes() {
        let base = ctx();
        let mut other = ctx();
        other.mode = ProjectMode::Build;
        assert_eq!(project_md(&base), project_md(&other));
        other.shell_enabled = false;
        assert_eq!(project_md(&base), project_md(&other));
        other.git = Some(GitInfo { branch: Some("main".into()), toplevel: PathBuf::from("/tmp") });
        assert_eq!(project_md(&base), project_md(&other));
        other.available = false;
        assert_eq!(project_md(&base), project_md(&other));
        other.worktree = Some(WorktreeRef { id: "w".into(), name: "fix".into(), branch: "fix".into() });
        assert_ne!(project_md(&base), project_md(&other), "a worktree is fixed for the chat");
        assert!(project_md(&other).contains("worktree \"fix\""));
    }

    #[test]
    fn a_piece_is_told_it_has_no_propose_plan() {
        let mut project = ctx();
        project.is_piece = true;
        let md = project_md(&project);
        assert!(md.contains("started from a project chat"));
        assert!(md.contains("no `propose_plan`"));
    }

    #[test]
    fn instructions_are_included_and_a_truncated_one_is_flagged() {
        let mut project = ctx();
        project.instructions = Some("Always answer in French.".into());
        let md = project_md(&project);
        assert!(md.contains("## Project Instructions"));
        assert!(md.contains("Always answer in French."));
        project.instructions_truncated = true;
        assert!(project_md(&project).contains("truncated at 32 KiB"));
    }

    #[test]
    fn the_state_block_renders_each_case() {
        let mut project = ctx();
        assert!(project_state_md(&project).contains("Mode: Plan"));
        project.mode = ProjectMode::Build;
        assert!(project_state_md(&project).contains("Mode: Build — you can change files and run commands."));
        project.shell_enabled = false;
        assert!(project_state_md(&project).contains("commands are off"));
        project.git = Some(GitInfo { branch: Some("041-coding".into()), toplevel: PathBuf::from("/tmp") });
        assert!(project_state_md(&project).contains("Branch: 041-coding"));
        project.git = Some(GitInfo { branch: None, toplevel: PathBuf::from("/tmp") });
        assert!(project_state_md(&project).contains("detached"));
        project.available = false;
        let unavailable = project_state_md(&project);
        assert!(unavailable.contains("Folder unavailable"));
        assert!(!unavailable.contains("Mode:"));
    }
}
