//! The `## Project` section of a project session's system prompt
//! (`specs/041-project-coding-workspace` contracts/coding-tools.md §4).
//!
//! It holds only what is fixed for the life of a chat: the project, the working folder, the
//! worktree's name, a fixed paragraph explaining both modes, and the bounded instruction
//! document. The mode, branch, shell switch and folder availability change between turns, so
//! they live in the per-message `## Project state` block instead (R17) — a mode or branch switch
//! must never invalidate the provider's prompt cache for the whole history.

use std::path::Path;

use crate::projects::channel;
use crate::projects::context::ProjectTurnContext;
use crate::skill::scopes::SkillScopes;

/// The most skills the section lists; beyond that it says how many are left
/// (`specs/041-project-coding-workspace` FR-040).
pub const MAX_LISTED_SKILLS: usize = 40;
/// The most of a skill's description the section shows: these are written to be read by a model
/// that can load the whole thing, and the longest here is 589 characters.
pub const MAX_DESCRIPTION_CHARS: usize = 400;

/// What the agent is told about the two things called hooks in a project folder. Prose, because
/// that is all they are in Vizier: spec-kit's extension hooks are instructions a skill's body tells
/// the agent to follow, and a harness's hook manifests are never read (FR-043).
const HOOKS_NOTE: &str = "\
If a skill you load tells you to check `.specify/extensions.yml` for extension hooks, those \
entries are instructions you may follow, not automation this program runs: read the file, then run \
the command it names with `workspace_shell` (Build mode only). Nothing enforces them and nothing \
reports on them. Before running a hook that commits, say what you are about to stage and commit. \
When there is no shell (Plan mode), say the hook was skipped and why. A harness hook manifest \
(`.claude/settings.local.json`, `.codex/hooks.json`, `.cursor/hooks.json`, `.github/hooks/*.json`, \
`.gemini/settings.json`) is never executed here, so never report that one ran.\n";

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

    if let Some(skills) = project_skills_md(&project.cwd) {
        out.push('\n');
        out.push_str(&skills);
    }

    out
}

/// The `## Project Skills` section: the skills this folder carries, read at prompt time like the
/// instruction document (`specs/041-project-coding-workspace` FR-040, contract §5).
///
/// None when the folder carries none, so a project without skills costs not one token. A skill
/// marked `disable-model-invocation` is left out: it is prose for a person to run, not something
/// the agent should pick up on its own.
pub fn project_skills_md(cwd: &Path) -> Option<String> {
    let found = SkillScopes::project(cwd).discover();
    let listed: Vec<_> = found
        .skills
        .into_iter()
        .filter(|scoped| !scoped.skill.disable_model_invocation)
        .collect();
    if listed.is_empty() {
        return None;
    }

    let mut out = String::from(
        "## Project Skills\n\nSkills this project carries in its own folder. Load one with \
         `use_skill(slug=\"…\")` in an `execute_python` script and follow it.\n\n",
    );

    for (index, scoped) in listed.iter().enumerate() {
        if index == MAX_LISTED_SKILLS {
            out.push_str(&format!(
                "- … and {} more (see `list_skills`)\n",
                listed.len() - MAX_LISTED_SKILLS
            ));
            break;
        }
        let skill = &scoped.skill;
        out.push_str(&format!("- **{}** — {}", skill.name, capped(&skill.description)));
        if let Some(hint) = &skill.argument_hint {
            out.push_str(&format!(" (takes: {})", hint.trim()));
        }
        if !skill.user_invocable {
            out.push_str(" (not runnable by name)");
        }
        out.push('\n');
    }

    out.push('\n');
    out.push_str(HOOKS_NOTE);
    Some(out)
}

/// At most [`MAX_DESCRIPTION_CHARS`], cut on a character boundary, with an ellipsis when it was.
fn capped(description: &str) -> String {
    let description = description.trim();
    if description.chars().count() <= MAX_DESCRIPTION_CHARS {
        return description.to_string();
    }
    let cut: String = description.chars().take(MAX_DESCRIPTION_CHARS).collect();
    format!("{cut}…")
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

    /// A project folder with skills, written the way the ecosystem writes them.
    fn folder_with_skills() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let write = |root: &str, slug: &str, front: &str| {
            let path = dir.path().join(root).join(slug);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("SKILL.md"), format!("---\n{front}\n---\n\nbody\n")).unwrap();
        };
        write(
            ".agents/skills",
            "speckit-plan",
            "name: speckit-plan\ndescription: Execute the planning workflow.\n\
             argument-hint: \"Optional guidance\"\nuser-invocable: true",
        );
        write(
            ".agents/skills",
            "internal-only",
            "name: internal-only\ndescription: Agent picks this up.\n\
             disable-model-invocation: true",
        );
        write(
            ".agents/skills",
            "git-chore",
            "name: git-chore\ndescription: Commit chore.\nuser-invocable: false",
        );
        // The older home, carrying the same slug: it must not be a second entry.
        write(
            ".claude/skills",
            "speckit-plan",
            "name: speckit-plan\ndescription: Execute the planning workflow, the .claude copy.",
        );
        dir
    }

    #[test]
    fn the_project_skills_section_lists_one_entry_per_slug_with_its_flags() {
        let dir = folder_with_skills();
        let mut project = ctx();
        project.cwd = dir.path().to_path_buf();
        let md = project_md(&project);

        assert!(md.contains("## Project Skills"), "{md}");
        assert_eq!(md.matches("**speckit-plan**").count(), 1, "one entry, not two: {md}");
        assert!(md.contains("(takes: Optional guidance)"), "{md}");
        assert!(md.contains("(not runnable by name)"), "{md}");
        assert!(!md.contains("internal-only"), "disable-model-invocation is left out: {md}");
        assert!(
            md.contains("**speckit-plan** — Execute the planning workflow."),
            "the .agents copy wins: {md}"
        );
        assert!(!md.contains("the .claude copy"), "{md}");
    }

    #[test]
    fn the_hooks_note_says_what_is_prose_and_what_is_never_run() {
        let dir = folder_with_skills();
        let mut project = ctx();
        project.cwd = dir.path().to_path_buf();
        let md = project_md(&project);
        assert!(md.contains(".specify/extensions.yml"), "{md}");
        assert!(md.contains("not automation"), "{md}");
        assert!(md.contains("never executed here"), "{md}");
    }

    #[test]
    fn a_folder_without_skills_adds_no_section() {
        let dir = tempfile::tempdir().unwrap();
        let mut project = ctx();
        project.cwd = dir.path().to_path_buf();
        let md = project_md(&project);
        assert!(!md.contains("## Project Skills"), "{md}");
    }

    #[test]
    fn a_long_description_is_cut_on_a_character_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".agents/skills/long");
        std::fs::create_dir_all(&path).unwrap();
        let description = "é".repeat(600);
        std::fs::write(
            path.join("SKILL.md"),
            format!("---\nname: long\ndescription: \"{description}\"\n---\n\nbody\n"),
        )
        .unwrap();
        let md = project_skills_md(dir.path()).expect("a section");
        assert!(md.contains('…'), "cut with an ellipsis");
        let line = md.lines().find(|line| line.starts_with("- **long**")).unwrap();
        assert!(line.chars().count() < MAX_DESCRIPTION_CHARS + 40, "{}", line.chars().count());
    }

    /// FR-017/R17 again, for the new section: it is folder content, so it must not react to the
    /// facts that change between turns of one chat.
    #[test]
    fn the_skills_section_ignores_the_volatile_facts() {
        let dir = folder_with_skills();
        let mut base = ctx();
        base.cwd = dir.path().to_path_buf();

        for mutate in [
            (|p: &mut ProjectTurnContext| p.mode = ProjectMode::Build) as fn(&mut ProjectTurnContext),
            |p: &mut ProjectTurnContext| p.shell_enabled = false,
            |p: &mut ProjectTurnContext| p.available = false,
            |p: &mut ProjectTurnContext| {
                p.git = Some(GitInfo { branch: Some("main".into()), toplevel: PathBuf::from("/tmp") })
            },
        ] {
            let mut other = base.clone();
            mutate(&mut other);
            assert_eq!(project_md(&base), project_md(&other));
        }
    }
}
