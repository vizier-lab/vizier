use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::AgentId;

use super::scopes;

pub struct GetSkillDetails {
    workspace: String,
}

impl GetSkillDetails {
    pub fn new(_agent_id: AgentId, deps: VizierDependencies) -> Self {
        Self { workspace: deps.config.workspace.clone() }
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
    pub version: String,
    pub resources: Vec<String>,
    /// `project`, `agent` or `global`.
    pub scope: String,
    /// The skill's folder: relative to the project root for a project skill, else its slug.
    pub folder: String,
    /// Whether a person may run it by name (`/slug`).
    pub user_invocable: bool,
}

#[async_trait::async_trait]
impl VizierTool for GetSkillDetails {
    type Input = GetSkillDetailsArgs;
    type Output = SkillDetails;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Skills)
    }

    fn name() -> String {
        "get_skill_details".to_string()
    }

    fn description(&self) -> String {
        "get full metadata for a skill (description, keywords, version, resources) without \
         loading its full instructions. Use use_skill to load the instructions themselves."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let Some(scoped) = scopes(ctx, &self.workspace).find(&args.slug) else {
            return Err(VizierError(format!("Skill '{}' not found", args.slug)));
        };
        let listing = scoped.resources();
        let scope = scoped.scope.label().to_string();
        let folder = scoped.folder();
        let skill = scoped.skill;

        Ok(SkillDetails {
            name: skill.name,
            description: skill.description,
            keywords: skill.keywords,
            version: skill.version.to_string(),
            resources: listing.paths,
            scope,
            folder,
            user_invocable: skill.user_invocable,
        })
    }
}
