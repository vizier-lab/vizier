use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::AgentId;

use super::scopes;

pub struct ListSkills {
    workspace: String,
}

impl ListSkills {
    pub fn new(_agent_id: AgentId, deps: VizierDependencies) -> Self {
        Self { workspace: deps.config.workspace.clone() }
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
    /// `project`, `agent` or `global`.
    pub scope: String,
    /// The skill's folder: relative to the project root for a project skill, else its slug.
    pub folder: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListSkillsOutput {
    pub skills: Vec<SkillInfo>,
    /// Skill folders that could not be read, each already logged. Empty is the normal case.
    pub unreadable: Vec<String>,
}

#[async_trait::async_trait]
impl VizierTool for ListSkills {
    type Input = ListSkillsArgs;
    type Output = ListSkillsOutput;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Skills)
    }

    fn name() -> String {
        "list_skills".to_string()
    }

    fn description(&self) -> String {
        "list available skills (name + short description), optionally filtered by keyword. A skill \
         of this project appears first, then the agent's own and the global ones, each slug once. \
         Use get_skill_details for full metadata, or use_skill to load a skill's instructions."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let found = scopes(ctx, &self.workspace).discover();
        let keyword = args.keyword.map(|keyword| keyword.to_lowercase());

        let skills = found
            .skills
            .into_iter()
            .filter(|scoped| match &keyword {
                Some(keyword) => {
                    let skill = &scoped.skill;
                    skill.keywords.iter().any(|k| k.to_lowercase().contains(keyword))
                        || skill.name.to_lowercase().contains(keyword)
                        || skill.description.to_lowercase().contains(keyword)
                }
                None => true,
            })
            .map(|scoped| SkillInfo {
                scope: scoped.scope.label().to_string(),
                folder: scoped.folder(),
                name: scoped.skill.name.clone(),
                description: scoped.skill.description.clone(),
            })
            .collect();

        Ok(ListSkillsOutput {
            skills,
            unreadable: found
                .skipped
                .into_iter()
                .map(|skipped| format!("{}: {}", skipped.folder, skipped.reason))
                .collect(),
        })
    }
}
