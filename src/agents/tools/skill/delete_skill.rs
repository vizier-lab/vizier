use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::skill::SkillManager;

pub struct DeleteSkill(SkillManager);

impl DeleteSkill {
    pub fn new(deps: VizierDependencies) -> Self {
        let workspace = deps.config.workspace.clone();
        Self(SkillManager::new(&workspace))
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DeleteSkillArgs {
    #[schemars(description = "slug of the skill to delete")]
    pub slug: String,
}

#[async_trait::async_trait]
impl VizierTool for DeleteSkill {
    type Input = DeleteSkillArgs;
    type Output = String;

    fn name() -> String {
        "delete_skill".to_string()
    }

    fn description(&self) -> String {
        "delete a skill and all its resources".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let deleted = self.0.delete_skill(&args.slug)
            .map_err(|e| VizierError(e.to_string()))?;

        if deleted {
            Ok(format!("Skill '{}' deleted", args.slug))
        } else {
            Err(VizierError(format!("Skill '{}' not found", args.slug)))
        }
    }
}
