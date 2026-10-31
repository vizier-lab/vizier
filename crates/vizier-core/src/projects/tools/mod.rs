//! The project coding tools (`specs/041-project-coding-workspace`): one registry that decides
//! what a turn is offered, how a script calls it, and what documentation it gets — so a new
//! tool is one entry, never a `match` on names.
//!
//! The tools exist only in a `Project` chat and in a subagent piece launched from one
//! (FR-007/FR-007a). Access is filtered by the chat's saved mode: Plan offers only reads and
//! `propose_plan`; Build adds writes and, with the project's shell switch on, commands.

mod edit_file;
mod git_diff;
mod git_log;
mod git_status;
mod glob;
mod grep;
mod list_files;
mod propose_plan;
mod read_file;
mod write_file;

pub mod walk;

use std::path::{Path, PathBuf};

use crate::{
    agents::tools::{
        CatalogueEntry, ToolContext, ToolTarget, VizierToolDyn, flagged_definition,
    },
    error::VizierError,
    projects::context::ProjectTurnContext,
    schema::ProjectMode,
};

/// What a tool may do to the folder. Plan mode offers `Read` and `Plan`; Build adds `Write` and
/// `Command`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Plan,
    Write,
    Command,
}

/// One registry entry: the tool, what it may do and whether it needs a repository.
pub struct ProjectToolSpec {
    pub access: Access,
    pub needs_git: bool,
    pub tool: &'static (dyn VizierToolDyn + Sync),
}

static READ_FILE: read_file::ReadFile = read_file::ReadFile;
static LIST_FILES: list_files::ListFiles = list_files::ListFiles;
static GREP: grep::Grep = grep::Grep;
static GLOB: glob::Glob = glob::Glob;
static GIT_STATUS: git_status::GitStatus = git_status::GitStatus;
static GIT_DIFF: git_diff::GitDiff = git_diff::GitDiff;
static GIT_LOG: git_log::GitLog = git_log::GitLog;
static PROPOSE_PLAN: propose_plan::ProposePlan = propose_plan::ProposePlan;
static WRITE_FILE: write_file::WriteFile = write_file::WriteFile;
static EDIT_FILE: edit_file::EditFile = edit_file::EditFile;
static WORKSPACE_SHELL: crate::projects::shell::WorkspaceShell =
    crate::projects::shell::WorkspaceShell;

/// Every project tool, in the order the agent should read them. `workspace_shell` stays first
/// among the commands it shares the category with.
pub static REGISTRY: &[ProjectToolSpec] = &[
    ProjectToolSpec { access: Access::Read, needs_git: false, tool: &READ_FILE },
    ProjectToolSpec { access: Access::Read, needs_git: false, tool: &LIST_FILES },
    ProjectToolSpec { access: Access::Read, needs_git: false, tool: &GREP },
    ProjectToolSpec { access: Access::Read, needs_git: false, tool: &GLOB },
    ProjectToolSpec { access: Access::Read, needs_git: true, tool: &GIT_STATUS },
    ProjectToolSpec { access: Access::Read, needs_git: true, tool: &GIT_DIFF },
    ProjectToolSpec { access: Access::Read, needs_git: true, tool: &GIT_LOG },
    ProjectToolSpec { access: Access::Plan, needs_git: false, tool: &PROPOSE_PLAN },
    ProjectToolSpec { access: Access::Write, needs_git: false, tool: &WRITE_FILE },
    ProjectToolSpec { access: Access::Write, needs_git: false, tool: &EDIT_FILE },
    ProjectToolSpec { access: Access::Command, needs_git: false, tool: &WORKSPACE_SHELL },
];

fn allowed(spec: &ProjectToolSpec, project: &ProjectTurnContext) -> bool {
    if spec.needs_git && project.git.is_none() {
        return false;
    }
    match spec.access {
        Access::Read => true,
        Access::Plan => project.plan_offered(),
        Access::Write => project.mode == ProjectMode::Build,
        Access::Command => project.mode == ProjectMode::Build && project.shell_enabled,
    }
}

fn entry(spec: &ProjectToolSpec) -> CatalogueEntry {
    let tool = spec.tool;
    CatalogueEntry {
        name: tool.tool_name(),
        category: crate::agents::tools::ToolCategory::Coding.name().to_string(),
        target: ToolTarget::Project,
        definition: flagged_definition(tool),
        ends_turn: tool.ends_turn(),
    }
}

/// The coding tools this turn may use. An unavailable folder offers none; the `coding`
/// category line is still listed by `turn_categories` (contracts/coding-tools.md §1).
pub fn offered(project: &ProjectTurnContext) -> Vec<CatalogueEntry> {
    if !project.available {
        return vec![];
    }
    REGISTRY
        .iter()
        .filter(|spec| allowed(spec, project))
        .map(entry)
        .collect()
}

fn refusal(spec: &ProjectToolSpec, project: &ProjectTurnContext) -> VizierError {
    let name = spec.tool.tool_name();
    let reason = match spec.access {
        Access::Plan => format!("not available in {} mode", project.mode.as_str()),
        Access::Write => format!("not available in {} mode", project.mode.as_str()),
        Access::Command if project.mode != ProjectMode::Build => {
            format!("not available in {} mode", project.mode.as_str())
        }
        Access::Command => "not available while the shell is off".to_string(),
        Access::Read if spec.needs_git => "not available without git".to_string(),
        Access::Read => "not available".to_string(),
    };
    VizierError(format!("tool_not_offered: {name} is {reason}"))
}

/// Call one registry tool by name. A name this turn was not offered is refused before any work,
/// so a script cannot reach a Build tool from Plan by knowing its name.
pub async fn call(
    name: &str,
    arguments: String,
    ctx: &ToolContext,
) -> Result<serde_json::Value, VizierError> {
    let Some(project) = &ctx.project else {
        return Err(VizierError(
            "project_unavailable: this conversation is not a project session".into(),
        ));
    };
    if !project.available {
        return Err(VizierError(
            "project_unavailable: the project folder is missing or not a directory".into(),
        ));
    }
    let Some(spec) = REGISTRY.iter().find(|spec| spec.tool.tool_name() == name) else {
        return Err(VizierError(format!("{name} not found")));
    };
    if !allowed(spec, project) {
        return Err(refusal(spec, project));
    }
    let output = spec.tool.tool_call(arguments, ctx).await?;
    serde_json::from_str(&output).map_err(|err| VizierError(err.to_string()))
}

/// The output schema of one registry tool, for `describe_tool_function`.
pub fn output_schema(name: &str) -> Option<serde_json::Value> {
    REGISTRY
        .iter()
        .find(|spec| spec.tool.tool_name() == name)
        .map(|spec| spec.tool.output_schema())
}

/// The chat's `cwd`, as every tool wants it.
pub(crate) fn cwd(ctx: &ToolContext) -> Result<PathBuf, VizierError> {
    let project = ctx.project.as_ref().ok_or_else(|| {
        VizierError("project_unavailable: this conversation is not a project session".into())
    })?;
    Ok(project.cwd.clone())
}

/// Record a file the chat has read, so a later edit may use it (R8).
pub(crate) fn record_read(ctx: &ToolContext, file: &Path, bytes: &[u8]) {
    if let Some(project) = &ctx.project {
        project
            .ledger
            .record(&ctx.session.to_slug(), file, bytes);
    }
}

/// Require that the chat read this file and that its bytes still match.
pub(crate) fn check_read(ctx: &ToolContext, file: &Path, bytes: &[u8]) -> Result<(), VizierError> {
    let project = ctx
        .project
        .as_ref()
        .ok_or_else(|| VizierError("project_unavailable: not a project session".into()))?;
    project
        .ledger
        .check(&ctx.session.to_slug(), file, bytes)
}

/// Write atomically: a temp file in the same folder, the original's permissions, then a rename.
/// A refusal never reaches this function, so a failed edit leaves the original byte-identical.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), VizierError> {
    let parent = path.parent().ok_or_else(|| {
        VizierError(format!("path_outside_project: {}", path.display()))
    })?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let temp = parent.join(format!(
        ".{name}.vizier-tmp-{}",
        nanoid::nanoid!(8, &nanoid::alphabet::SAFE)
    ));
    let write = || -> std::io::Result<()> {
        std::fs::write(&temp, bytes)?;
        if let Ok(meta) = std::fs::metadata(path) {
            std::fs::set_permissions(&temp, meta.permissions())?;
        }
        std::fs::rename(&temp, path)
    };
    match write() {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = std::fs::remove_file(&temp);
            Err(VizierError(format!("write_failed: {err}")))
        }
    }
}

/// Whether a folder is a project root's worktree area (used by callers that build messages).
pub fn is_in_worktree_area(cwd: &Path, path: &Path) -> bool {
    path.starts_with(cwd.join(".vizier"))
}
