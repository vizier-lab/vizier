use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::AgentId;
use crate::skill::SkillManager;

pub struct UseSkill {
    global_manager: SkillManager,
    agent_manager: SkillManager,
}

impl UseSkill {
    pub fn new(agent_id: AgentId, deps: VizierDependencies) -> Self {
        let workspace = deps.config.workspace.clone();
        Self {
            global_manager: SkillManager::new(&workspace),
            agent_manager: SkillManager::for_agent(&workspace, &agent_id),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UseSkillArgs {
    #[schemars(description = "slug of the skill to use")]
    pub slug: String,
}

#[async_trait::async_trait]
impl VizierTool for UseSkill {
    type Input = UseSkillArgs;
    type Output = String;

    fn name() -> String {
        "use_skill".to_string()
    }

    fn description(&self) -> String {
        "load a skill's full instructions/content into context. Call list_skills or \
         get_skill_details first if you're not sure which skill to use.".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        // Agent skill takes priority
        if let Some(skill) = self
            .agent_manager
            .get_skill(&args.slug)
            .map_err(|e| VizierError(e.to_string()))?
        {
            return Ok(skill.content);
        }

        if let Some(skill) = self
            .global_manager
            .get_skill(&args.slug)
            .map_err(|e| VizierError(e.to_string()))?
        {
            return Ok(skill.content);
        }

        Err(VizierError(format!("Skill '{}' not found", args.slug)))
    }
}
