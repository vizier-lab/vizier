use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::VizierTool;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::skill::SkillManager;

pub struct ReadSkillResource {
    global_manager: SkillManager,
    agent_manager: Option<SkillManager>,
}

impl ReadSkillResource {
    pub fn new(agent_id: Option<String>, deps: VizierDependencies) -> Self {
        let workspace = deps.config.workspace.clone();
        Self {
            global_manager: SkillManager::new(&workspace),
            agent_manager: agent_id.map(|id| SkillManager::for_agent(&workspace, &id)),
        }
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

    fn name() -> String {
        "read_skill_resource".to_string()
    }

    fn description(&self) -> String {
        "read a resource file from a skill folder (templates, references, scripts, etc.)".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        // Try agent skill first, then global
        if let Some(ref agent_manager) = self.agent_manager {
            if let Some(content) = agent_manager.read_resource(&args.slug, &args.path)
                .map_err(|e| VizierError(e.to_string()))?
            {
                return Ok(content);
            }
        }

        if let Some(content) = self.global_manager.read_resource(&args.slug, &args.path)
            .map_err(|e| VizierError(e.to_string()))?
        {
            return Ok(content);
        }

        Err(VizierError(format!(
            "Resource '{}' not found in skill '{}'",
            args.path, args.slug
        )))
    }
}
