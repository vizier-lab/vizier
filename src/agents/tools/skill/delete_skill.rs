use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::skill::{deindex_skill, global_scope};
use crate::agents::tools::{ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::indexer::VizierIndexer;
use crate::skill::SkillManager;

pub struct DeleteSkill(SkillManager, Option<VizierIndexer>);

impl DeleteSkill {
    pub fn new(deps: VizierDependencies, indexer: Option<VizierIndexer>) -> Self {
        let workspace = deps.config.workspace.clone();
        Self(SkillManager::new(&workspace), indexer)
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
            if let Some(indexer) = &self.1 {
                let _ = deindex_skill(indexer, global_scope(), &args.slug).await;
            }
            Ok(format!("Skill '{}' deleted", args.slug))
        } else {
            Err(VizierError(format!("Skill '{}' not found", args.slug)))
        }
    }
}
