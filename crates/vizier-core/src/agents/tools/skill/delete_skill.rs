use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::skill::{deindex_skill, global_scope, scope_for_agent};
use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::indexer::VizierIndexer;
use crate::schema::AgentId;
use crate::skill::SkillManager;

pub struct DeleteSkill {
    agent_id: AgentId,
    agent_manager: SkillManager,
    global_manager: SkillManager,
    indexer: Option<VizierIndexer>,
}

impl DeleteSkill {
    pub fn new(agent_id: AgentId, deps: VizierDependencies, indexer: Option<VizierIndexer>) -> Self {
        let workspace = deps.config.workspace.clone();
        Self {
            agent_manager: SkillManager::for_agent(&workspace, &agent_id),
            global_manager: SkillManager::new(&workspace),
            agent_id,
            indexer,
        }
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

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Skills)
    }

    fn name() -> String {
        "delete_skill".to_string()
    }

    fn description(&self) -> String {
        "delete a skill and all its resources".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        // The agent's own skill first, as `create_skill` writes there; then a global one.
        let scope = if self
            .agent_manager
            .delete_skill(&args.slug)
            .map_err(|e| VizierError(e.to_string()))?
        {
            scope_for_agent(&self.agent_id)
        } else if self
            .global_manager
            .delete_skill(&args.slug)
            .map_err(|e| VizierError(e.to_string()))?
        {
            global_scope().to_string()
        } else {
            return Err(VizierError(format!("Skill '{}' not found", args.slug)));
        };

        if let Some(indexer) = &self.indexer {
            let _ = deindex_skill(indexer, &scope, &args.slug).await;
        }
        Ok(format!("Skill '{}' deleted", args.slug))
    }
}
