use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::AgentId;
use crate::skill::SkillManager;

pub struct ListSkills {
    global_manager: SkillManager,
    agent_manager: SkillManager,
}

impl ListSkills {
    pub fn new(agent_id: AgentId, deps: VizierDependencies) -> Self {
        let workspace = deps.config.workspace.clone();
        Self {
            global_manager: SkillManager::new(&workspace),
            agent_manager: SkillManager::for_agent(&workspace, &agent_id),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListSkillsArgs {
    #[schemars(description = "optional keyword to filter skills")]
    pub keyword: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
}

#[async_trait::async_trait]
impl VizierTool for ListSkills {
    type Input = ListSkillsArgs;
    type Output = Vec<SkillInfo>;

    fn name() -> String {
        "list_skills".to_string()
    }

    fn description(&self) -> String {
        "list available skills (name + short description), optionally filtered by keyword. \
         Use get_skill_details for full metadata, or use_skill to load a skill's instructions."
            .into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let mut skills = self
            .global_manager
            .list_skills()
            .map_err(|e| VizierError(e.to_string()))?;

        for agent_skill in self
            .agent_manager
            .list_skills()
            .map_err(|e| VizierError(e.to_string()))?
        {
            if !skills.iter().any(|s| s.name == agent_skill.name) {
                skills.push(agent_skill);
            }
        }

        let filtered: Vec<SkillInfo> = skills
            .iter()
            .filter(|skill| {
                if let Some(ref keyword) = args.keyword {
                    let keyword_lower = keyword.to_lowercase();
                    skill.keywords.iter().any(|k| k.to_lowercase().contains(&keyword_lower))
                        || skill.name.to_lowercase().contains(&keyword_lower)
                        || skill.description.to_lowercase().contains(&keyword_lower)
                } else {
                    true
                }
            })
            .map(|skill| SkillInfo {
                name: skill.name.clone(),
                description: skill.description.clone(),
            })
            .collect();

        Ok(filtered)
    }
}
