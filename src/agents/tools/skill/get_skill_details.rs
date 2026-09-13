use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::AgentId;
use crate::skill::SkillManager;

pub struct GetSkillDetails {
    global_manager: SkillManager,
    agent_manager: SkillManager,
}

impl GetSkillDetails {
    pub fn new(agent_id: AgentId, deps: VizierDependencies) -> Self {
        let workspace = deps.config.workspace.clone();
        Self {
            global_manager: SkillManager::new(&workspace),
            agent_manager: SkillManager::for_agent(&workspace, &agent_id),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetSkillDetailsArgs {
    #[schemars(description = "slug of the skill")]
    pub slug: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SkillDetails {
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub version: u32,
    pub resources: Vec<String>,
}

#[async_trait::async_trait]
impl VizierTool for GetSkillDetails {
    type Input = GetSkillDetailsArgs;
    type Output = SkillDetails;

    fn name() -> String {
        "get_skill_details".to_string()
    }

    fn description(&self) -> String {
        "get full metadata for a skill (description, keywords, version, resources) without \
         loading its full instructions. Use use_skill to load the instructions themselves."
            .into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        // Agent skill takes priority
        let skill = self
            .agent_manager
            .get_skill(&args.slug)
            .map_err(|e| VizierError(e.to_string()))?
            .or(
                self.global_manager
                    .get_skill(&args.slug)
                    .map_err(|e| VizierError(e.to_string()))?,
            )
            .ok_or_else(|| VizierError(format!("Skill '{}' not found", args.slug)))?;

        Ok(SkillDetails {
            name: skill.name,
            description: skill.description,
            keywords: skill.keywords,
            version: skill.version,
            resources: skill.resources,
        })
    }
}
