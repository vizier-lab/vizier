//! The `## Project` section of a project session's system prompt (`specs/040-projects-mode`
//! contracts/workspace-shell.md §5), with the folder, the shell state and the bounded project
//! instruction document. It is built per turn and never added to any other session kind.

use crate::projects::context::ProjectTurnContext;

/// The section, after CORE and before the configured documents.
pub fn project_md(project: &ProjectTurnContext) -> String {
    let folder = crate::projects::channel::display_path(&project.folder);
    let mut out = format!(
        "## Project\n\nYou are working in the project \"{}\", at folder {}.\n",
        project.name, folder
    );
    if project.available {
        if project.shell_enabled {
            out.push_str(
                "The `workspace_shell` tool runs commands in that folder on the host machine, \
                 with the user's account. It is not sandboxed.\n",
            );
        } else {
            out.push_str(
                "The project's workspace shell is turned off, so you cannot run commands here.\n",
            );
        }
    } else {
        out.push_str(
            "The project folder is unavailable; commands cannot run until it is restored. \
             The conversation history is still available.\n",
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn ctx(available: bool, shell_enabled: bool) -> ProjectTurnContext {
        ProjectTurnContext {
            project_id: "p3xY9aQ2mZ".into(),
            name: "alpha".into(),
            folder: PathBuf::from("/tmp/vizier-project-alpha"),
            available,
            shell_enabled,
            instructions: None,
            instructions_truncated: false,
        }
    }

    #[test]
    fn a_project_session_is_told_its_name_and_folder() {
        let md = project_md(&ctx(true, true));
        assert!(md.contains("## Project"));
        assert!(md.contains("alpha"));
        assert!(md.contains("/tmp/vizier-project-alpha"));
        assert!(md.contains("workspace_shell"));
        assert!(md.contains("not sandboxed"));
        assert!(!md.contains("## Project Instructions"));
    }

    #[test]
    fn a_disabled_shell_says_so() {
        let md = project_md(&ctx(true, false));
        assert!(md.contains("turned off"));
        assert!(!md.contains("It is not sandboxed."));
    }

    #[test]
    fn an_unavailable_folder_says_commands_are_refused() {
        let md = project_md(&ctx(false, true));
        assert!(md.contains("unavailable"));
        assert!(md.contains("history is still available"));
    }

    #[test]
    fn instructions_are_included_and_a_truncated_one_is_flagged() {
        let mut project = ctx(true, true);
        project.instructions = Some("Always answer in French.".into());
        let md = project_md(&project);
        assert!(md.contains("## Project Instructions"));
        assert!(md.contains("Always answer in French."));

        project.instructions_truncated = true;
        assert!(project_md(&project).contains("truncated at 32 KiB"));
    }
}
