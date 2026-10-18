use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{SkillResource, write_resource_files};
use crate::agents::skill::{global_scope, index_skill, scope_for_agent};
use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::indexer::VizierIndexer;
use crate::schema::AgentId;
use crate::skill::SkillManager;

pub struct UpdateSkill {
    agent_id: AgentId,
    agent_manager: SkillManager,
    global_manager: SkillManager,
    indexer: Option<VizierIndexer>,
}

impl UpdateSkill {
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
pub struct UpdateSkillArgs {
    #[schemars(description = "slug of the skill to update")]
    pub slug: String,

    #[schemars(description = "new content/instruction of the skill (optional)")]
    pub content: Option<String>,

    #[schemars(description = "new description of the skill (optional)")]
    pub description: Option<String>,

    #[schemars(description = "new keywords for matching (optional)")]
    pub keywords: Option<Vec<String>>,

    #[schemars(description = "resource files to add or update (optional, replaces all resources)")]
    pub resources: Option<Vec<SkillResource>>,
}

#[async_trait::async_trait]
impl VizierTool for UpdateSkill {
    type Input = UpdateSkillArgs;
    type Output = String;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Skills)
    }

    fn name() -> String {
        "update_skill".to_string()
    }

    fn description(&self) -> String {
        "update an existing skill's content, description, keywords, or resource files".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        // The agent's own skill first, as `create_skill` writes there; then a global one.
        let (manager, scope, mut skill) = match self
            .agent_manager
            .get_skill(&args.slug)
            .map_err(|e| VizierError(e.to_string()))?
        {
            Some(skill) => (&self.agent_manager, scope_for_agent(&self.agent_id), skill),
            None => {
                let skill = self
                    .global_manager
                    .get_skill(&args.slug)
                    .map_err(|e| VizierError(e.to_string()))?
                    .ok_or_else(|| VizierError(format!("Skill '{}' not found", args.slug)))?;
                (&self.global_manager, global_scope().to_string(), skill)
            }
        };

        if let Some(content) = args.content {
            skill.content = content;
        }
        if let Some(description) = args.description {
            skill.description = description;
        }
        if let Some(keywords) = args.keywords {
            skill.keywords = keywords;
        }

        if let Some(resources) = &args.resources {
            let skill_dir = manager.skill_dir(&args.slug);
            write_resource_files(&skill_dir, resources)?;
            skill.resources = resources.iter().map(|r| r.path.clone()).collect();
        }

        skill.version += 1;

        manager.save_skill(&skill)
            .map_err(|e| VizierError(e.to_string()))?;

        if let Some(indexer) = &self.indexer {
            let _ = index_skill(indexer, &scope, &skill).await;
        }

        Ok(format!("Skill '{}' updated to version {}", skill.name, skill.version))
    }
}
