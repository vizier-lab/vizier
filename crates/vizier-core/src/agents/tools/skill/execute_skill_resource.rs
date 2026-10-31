use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;

use super::scopes;

/// The most a resource script may run for. Not configurable: it is a fallback for skill scripts,
/// and a project has `workspace_shell` for a command that needs to take its time.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(60);
/// Each captured stream is capped at this, UTF-8 safe.
const OUTPUT_CAP_BYTES: usize = 32 * 1024;

pub struct ExecuteSkillResource {
    workspace: String,
}

impl ExecuteSkillResource {
    pub fn new(_agent_id: Option<String>, deps: VizierDependencies) -> Self {
        Self { workspace: deps.config.workspace.clone() }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ExecuteSkillResourceArgs {
    #[schemars(description = "slug of the skill")]
    pub slug: String,

    #[schemars(description = "path to the script file within the skill folder")]
    pub path: String,

    #[schemars(description = "optional arguments to pass to the script")]
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ExecuteResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub truncated: bool,
}

#[async_trait::async_trait]
impl VizierTool for ExecuteSkillResource {
    type Input = ExecuteSkillResourceArgs;
    type Output = ExecuteResult;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Skills)
    }

    fn name() -> String {
        "execute_skill_resource".to_string()
    }

    fn description(&self) -> String {
        "execute a script file from a skill folder (shell, python, etc.). Not available in a \
         project chat: run skill scripts with workspace_shell there.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        // In a project chat this tool would run a repository's script on the host with no cwd, no
        // mode gate and no bound. `workspace_shell` has all three, so this is where the refusal
        // goes (`specs/041-project-coding-workspace` FR-042).
        if let Some(project) = &ctx.project
            && project.available
        {
            return Err(VizierError(
                "project_skill_resource: run skill scripts with workspace_shell in a project chat, \
                 so the command stays in the project folder and in Build mode"
                    .into(),
            ));
        }

        let Some(scoped) = scopes(ctx, &self.workspace).find(&args.slug) else {
            return Err(VizierError(format!("Skill '{}' not found", args.slug)));
        };
        crate::skill::validate_resource_path(&args.path)?;

        let script = scoped.dir.join(&args.path);
        if !script.is_file() {
            return Err(VizierError(format!(
                "Script '{}' not found in skill '{}'",
                args.path, args.slug
            )));
        }

        let extension = std::path::Path::new(&args.path)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if cfg!(windows) && matches!(extension.as_str(), "sh" | "bash") {
            return Err(VizierError(
                "shell scripts (.sh) cannot run on Windows; use a .py or .js resource instead".into(),
            ));
        }

        let script_arg = script.to_string_lossy().to_string();
        let (program, mut cmd_args): (&str, Vec<String>) = match extension.as_str() {
            "sh" | "bash" => ("sh", vec![script_arg]),
            "py" | "python" => ("python", vec![script_arg]),
            "js" | "javascript" => ("node", vec![script_arg]),
            "rb" | "ruby" => ("ruby", vec![script_arg]),
            "pl" | "perl" => ("perl", vec![script_arg]),
            _ => (script_arg.as_str(), vec![]),
        };
        cmd_args.extend(args.args);

        // Run from the skill's own folder, bounded in time, so a script that ignores its neighbours
        // still finishes and one that reads `./next-to-it` works.
        let dir: PathBuf = scoped.dir.clone();
        let mut command = tokio::process::Command::new(program);
        command
            .args(&cmd_args)
            .current_dir(&dir)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);

        let output = match tokio::time::timeout(SCRIPT_TIMEOUT, command.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(err)) => {
                return Err(VizierError(format!("Failed to execute script: {err}")));
            }
            Err(_) => {
                return Ok(ExecuteResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("timed out after {}s", SCRIPT_TIMEOUT.as_secs()),
                    timed_out: true,
                    truncated: false,
                });
            }
        };

        let (stdout, out_cut) = capped(&output.stdout);
        let (stderr, err_cut) = capped(&output.stderr);
        Ok(ExecuteResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout,
            stderr,
            timed_out: false,
            truncated: out_cut || err_cut,
        })
    }
}

/// A captured stream, cut on a character boundary.
fn capped(bytes: &[u8]) -> (String, bool) {
    let cut = bytes.len() > OUTPUT_CAP_BYTES;
    let slice = if cut { &bytes[..OUTPUT_CAP_BYTES] } else { bytes };
    let text = match std::str::from_utf8(slice) {
        Ok(text) => text.to_string(),
        Err(err) => String::from_utf8_lossy(&slice[..err.valid_up_to()]).into_owned(),
    };
    (text, cut)
}
