use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::skill::SkillManager;

pub struct ExecuteSkillResource {
    global_manager: SkillManager,
    agent_manager: Option<SkillManager>,
}

impl ExecuteSkillResource {
    pub fn new(agent_id: Option<String>, deps: VizierDependencies) -> Self {
        let workspace = deps.config.workspace.clone();
        Self {
            global_manager: SkillManager::new(&workspace),
            agent_manager: agent_id.map(|id| SkillManager::for_agent(&workspace, &id)),
        }
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
}

#[async_trait::async_trait]
impl VizierTool for ExecuteSkillResource {
    type Input = ExecuteSkillResourceArgs;
    type Output = ExecuteResult;

    fn name() -> String {
        "execute_skill_resource".to_string()
    }

    fn description(&self) -> String {
        "execute a script file from a skill folder (shell, python, etc.)".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        // Find the script file
        let script_path = self.find_script(&args.slug, &args.path)?;

        // Determine how to execute based on file extension
        let extension = std::path::Path::new(&args.path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let (program, mut cmd_args) = match extension {
            "sh" | "bash" => ("sh", vec![script_path.to_str().unwrap().to_string()]),
            "py" | "python" => ("python", vec![script_path.to_str().unwrap().to_string()]),
            "js" | "javascript" => ("node", vec![script_path.to_str().unwrap().to_string()]),
            "rb" | "ruby" => ("ruby", vec![script_path.to_str().unwrap().to_string()]),
            "pl" | "perl" => ("perl", vec![script_path.to_str().unwrap().to_string()]),
            _ => {
                // Try to execute directly
                (script_path.to_str().unwrap(), vec![])
            }
        };

        cmd_args.extend(args.args);

        let output = std::process::Command::new(program)
            .args(&cmd_args)
            .output()
            .map_err(|e| VizierError(format!("Failed to execute script: {}", e)))?;

        Ok(ExecuteResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

impl ExecuteSkillResource {
    fn find_script(&self, slug: &str, path: &str) -> crate::Result<std::path::PathBuf> {
        // Try agent skill first
        if let Some(ref agent_manager) = self.agent_manager {
            let resource_path = agent_manager.list_resources(slug)
                .map_err(|e| VizierError(e.to_string()))?;
            if resource_path.iter().any(|p| p.to_str() == Some(path)) {
                let skill_dir = std::path::PathBuf::from(agent_manager.skills_dir())
                    .join(slug)
                    .join(path);
                if skill_dir.exists() {
                    return Ok(skill_dir);
                }
            }
        }

        // Try global skill
        let resource_path = self.global_manager.list_resources(slug)
            .map_err(|e| VizierError(e.to_string()))?;
        if resource_path.iter().any(|p| p.to_str() == Some(path)) {
            let skill_dir = std::path::PathBuf::from(self.global_manager.skills_dir())
                .join(slug)
                .join(path);
            if skill_dir.exists() {
                return Ok(skill_dir);
            }
        }

        Err(VizierError(format!(
            "Script '{}' not found in skill '{}'",
            path, slug
        )))
    }
}
