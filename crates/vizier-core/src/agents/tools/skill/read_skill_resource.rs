use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;

use super::scopes;

pub struct ReadSkillResource {
    workspace: String,
}

impl ReadSkillResource {
    pub fn new(_agent_id: Option<String>, deps: VizierDependencies) -> Self {
        Self { workspace: deps.config.workspace.clone() }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadSkillResourceArgs {
    #[schemars(description = "slug of the skill")]
    pub slug: String,

    #[schemars(description = "path to the resource file within the skill folder")]
    pub path: String,
}

#[async_trait::async_trait]
impl VizierTool for ReadSkillResource {
    type Input = ReadSkillResourceArgs;
    type Output = String;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Skills)
    }

    fn name() -> String {
        "read_skill_resource".to_string()
    }

    fn description(&self) -> String {
        "read a resource file from a skill folder (templates, references, scripts, etc.). The path \
         is relative to the skill's folder and may not leave it; a binary file is refused."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let Some(scoped) = scopes(ctx, &self.workspace).find(&args.slug) else {
            return Err(VizierError(format!("Skill '{}' not found", args.slug)));
        };
        match scoped.read_resource(&args.path)? {
            Some(content) => Ok(content),
            None => Err(VizierError(format!(
                "Resource '{}' not found in skill '{}'",
                args.path, args.slug
            ))),
        }
    }
}
