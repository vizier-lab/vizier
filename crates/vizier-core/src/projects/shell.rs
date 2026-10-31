//! The workspace shell (`specs/040-projects-mode/contracts/workspace-shell.md`): the one tool an
//! agent can use only inside a project session, whose commands run in the project folder on the
//! host machine with no sandbox.

use std::path::{Path, PathBuf};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::{
        shell::{ShellCommand, ShellOutcome, local::LocalShell},
        tools::{CatalogueEntry, ToolCategory, ToolContext, ToolTarget, VizierTool, VizierToolDyn},
    },
    config::shell::LocalShellConfig,
    error::VizierError,
    llm::types::ToolDefinition,
    schema::{DEFAULT_COMMAND_TIMEOUT, MAX_COMMAND_TIMEOUT},
};

pub const NAME: &str = "workspace_shell";

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct WorkspaceShellInput {
    /// The command line to run in the project folder.
    pub command: String,
    /// Optional directory relative to the project root. Must stay inside it.
    #[serde(default)]
    pub workdir: Option<String>,
    /// Optional time limit; defaults to 120 seconds, maximum 600.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// A command in one project. It is per-turn: never in `BaseTools` and never in an agent's shell
/// addon, so it cannot appear in an ordinary conversation, task, dream or subagent piece.
pub struct WorkspaceShell;

impl WorkspaceShell {
    /// The catalogue entry `turn_catalogue` appends while a project session may use the tool.
    pub fn entry() -> CatalogueEntry {
        CatalogueEntry {
            name: NAME.to_string(),
            category: ToolCategory::Shell.name().to_string(),
            target: ToolTarget::Project,
            definition: ToolDefinition {
                name: NAME.to_string(),
                description: <WorkspaceShell as VizierTool>::description(&WorkspaceShell),
                parameters: <WorkspaceShell as VizierTool>::input_schema(),
            },
            ends_turn: false,
        }
    }
}

/// The project root and the working directory a call asked for, or the refusal. A relative
/// directory is canonicalized and must stay under the root; an absolute one is refused.
fn resolve_workdir(root: &Path, workdir: Option<&str>) -> Result<PathBuf, VizierError> {
    let Some(workdir) = workdir.map(str::trim).filter(|w| !w.is_empty()) else {
        return Ok(root.to_path_buf());
    };
    if Path::new(workdir).is_absolute() {
        return Err(VizierError(format!(
            "workdir_outside_project: {workdir:?} is absolute; use a path inside the project folder"
        )));
    }
    let candidate = root.join(workdir);
    let canonical = std::fs::canonicalize(&candidate).map_err(|err| {
        VizierError(format!(
            "workdir_outside_project: {workdir:?} cannot be resolved inside the project folder: {err}"
        ))
    })?;
    if !canonical.starts_with(root) {
        return Err(VizierError(format!(
            "workdir_outside_project: {workdir:?} leaves the project folder"
        )));
    }
    Ok(canonical)
}

#[async_trait::async_trait]
impl VizierTool for WorkspaceShell {
    type Input = WorkspaceShellInput;
    type Output = ShellOutcome;

    fn name() -> String {
        NAME.to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Shell)
    }

    fn description(&self) -> String {
        "Run a command in the project folder on the host machine, with the user's account. \
         There is no sandbox. Returns stdout, stderr, exit_code, duration_ms, timed_out and \
         truncated; a non-zero exit is a normal result. Optional relative workdir must stay \
         inside the project folder. Default timeout 120 seconds, maximum 600."
            .to_string()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
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
        if !project.shell_enabled {
            return Err(VizierError(
                "shell_disabled: the project's workspace shell is turned off".into(),
            ));
        }
        let workdir = resolve_workdir(&project.folder, args.workdir.as_deref())?;
        let timeout = Duration::from_secs(
            args.timeout_secs
                .unwrap_or(DEFAULT_COMMAND_TIMEOUT.as_secs())
                .clamp(1, MAX_COMMAND_TIMEOUT.as_secs()),
        );

        let shell = LocalShell::from_config(LocalShellConfig {
            path: workdir.to_string_lossy().into_owned(),
            env: None,
        });
        shell
            .run(ShellCommand {
                command: args.command,
                workdir,
                timeout,
            })
            .await
            .map_err(|err| VizierError(format!("spawn_failed: {err}")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        projects::context::ProjectTurnContext,
        schema::{VizierChannelId, VizierSession},
    };

    fn ctx(project: Option<ProjectTurnContext>) -> ToolContext {
        ToolContext {
            session: VizierSession(
                "dp-a".into(),
                VizierChannelId::HTTP("alice".into(), "project__p3xY9aQ2mZ".into()),
                Some("t1".into()),
            ),
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            loaded_images: Default::default(),
            recalled: Default::default(),
            deadline: None,
            client: None,
            secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
            dream_sources: vec![],
            project: project.map(Arc::new),
        }
    }

    fn project(folder: &std::path::Path, shell_enabled: bool) -> ProjectTurnContext {
        ProjectTurnContext {
            project_id: "p3xY9aQ2mZ".into(),
            name: "alpha".into(),
            folder: std::fs::canonicalize(folder).unwrap(),
            available: true,
            shell_enabled,
            instructions: None,
            instructions_truncated: false,
        }
    }

    fn input(command: &str) -> WorkspaceShellInput {
        WorkspaceShellInput {
            command: command.into(),
            workdir: None,
            timeout_secs: None,
        }
    }

    #[tokio::test]
    async fn runs_a_command_in_the_project_folder() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = WorkspaceShell
            .call(input("echo hello"), &ctx(Some(project(dir.path(), true))))
            .await
            .unwrap();
        assert!(outcome.stdout.contains("hello"), "got {outcome:?}");
        assert_eq!(outcome.exit_code, Some(0));
    }

    #[tokio::test]
    async fn a_non_zero_exit_is_a_normal_result() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = WorkspaceShell
            .call(input("exit 3"), &ctx(Some(project(dir.path(), true))))
            .await
            .unwrap();
        assert_eq!(outcome.exit_code, Some(3));
    }

    #[tokio::test]
    async fn an_absolute_or_escaping_workdir_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        for workdir in ["..", outside.path().to_str().unwrap()] {
            let mut args = input("pwd");
            args.workdir = Some(workdir.into());
            let err = WorkspaceShell
                .call(args, &ctx(Some(project(dir.path(), true))))
                .await
                .unwrap_err();
            assert!(err.0.contains("workdir_outside_project"), "got {err:?}");
        }
    }

    #[tokio::test]
    async fn a_relative_workdir_inside_the_project_runs_there() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let mut args = input(if cfg!(windows) { "Get-Location" } else { "pwd" });
        args.workdir = Some("sub".into());
        let outcome = WorkspaceShell
            .call(args, &ctx(Some(project(dir.path(), true))))
            .await
            .unwrap();
        assert!(outcome.stdout.trim().ends_with("sub"), "got {outcome:?}");
    }

    /// FR-027/SC-008: two sessions of one project do not serialise. There is no project-level
    /// lock, so the wall clock is the longer call, not the sum.
    #[tokio::test]
    async fn two_project_sessions_run_commands_at_the_same_time() {
        let dir = tempfile::tempdir().unwrap();
        let line = if cfg!(windows) { "Start-Sleep -Seconds 1" } else { "sleep 1" };
        let ctx = ctx(Some(project(dir.path(), true)));
        let started = std::time::Instant::now();
        let (a, b) = tokio::join!(
            WorkspaceShell.call(input(line), &ctx),
            WorkspaceShell.call(input(line), &ctx)
        );
        a.unwrap();
        b.unwrap();
        let elapsed = started.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(1800),
            "two 1 s commands took {elapsed:?}; they were serialised"
        );
    }

    #[tokio::test]
    async fn a_disabled_shell_and_a_missing_folder_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let err = WorkspaceShell
            .call(input("echo hi"), &ctx(Some(project(dir.path(), false))))
            .await
            .unwrap_err();
        assert!(err.0.contains("shell_disabled"), "got {err:?}");

        let mut missing = project(dir.path(), true);
        missing.available = false;
        let err = WorkspaceShell
            .call(input("echo hi"), &ctx(Some(missing)))
            .await
            .unwrap_err();
        assert!(err.0.contains("project_unavailable"), "got {err:?}");

        let err = WorkspaceShell.call(input("echo hi"), &ctx(None)).await.unwrap_err();
        assert!(err.0.contains("project_unavailable"), "got {err:?}");
    }
}
