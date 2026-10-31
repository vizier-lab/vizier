use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::agents::skill::{index_skill, scope_for_agent};
use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::indexer::VizierIndexer;
use crate::schema::{AgentId, Skill};
use crate::skill::SkillManager;

use crate::skill::scopes::SkillScopes;

/// The skill scopes of this turn: the project's own skills when the chat is in a project, then the
/// agent's, then the global ones. Resolved per call, never in `new()`, because a tool is built once
/// per agent and a project is per conversation.
pub(crate) fn scopes(ctx: &ToolContext, workspace: &str) -> SkillScopes {
    let cwd = ctx.project.as_ref().map(|project| project.cwd.as_path());
    SkillScopes::for_turn(workspace, &ctx.session.0, cwd)
}

pub mod delete_skill;
pub mod execute_skill_resource;
pub mod get_skill_details;
pub mod list_skills;
pub mod read_skill_resource;
pub mod update_skill;
pub mod use_skill;

pub use delete_skill::DeleteSkill;
pub use execute_skill_resource::ExecuteSkillResource;
pub use get_skill_details::GetSkillDetails;
pub use list_skills::ListSkills;
pub use read_skill_resource::ReadSkillResource;
pub use update_skill::UpdateSkill;
pub use use_skill::UseSkill;

fn write_resource_files(skill_dir: &Path, resources: &[SkillResource]) -> Result<(), VizierError> {
    for resource in resources {
        crate::skill::validate_resource_path(&resource.path)?;
        let file_path = skill_dir.join(&resource.path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VizierError(format!("Failed to create resource directory: {}", e)))?;
        }
        std::fs::write(&file_path, &resource.content).map_err(|e| {
            VizierError(format!("Failed to write resource {}: {}", resource.path, e))
        })?;
    }
    Ok(())
}

pub struct CreateSkill(AgentId, SkillManager, Option<VizierIndexer>);

impl CreateSkill {
    pub fn new(agent_id: AgentId, deps: VizierDependencies, indexer: Option<VizierIndexer>) -> Self {
        let workspace = deps.config.workspace.clone();
        Self(
            agent_id.clone(),
            SkillManager::for_agent(&workspace, &agent_id),
            indexer,
        )
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SkillResource {
    #[schemars(
        description = "relative path of the resource file (e.g., \"scripts/do_something.py\")"
    )]
    pub path: String,

    #[schemars(description = "content of the resource file")]
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateSkillArgs {
    #[schemars(description = "name of the skill, in snake_case format")]
    pub name: String,

    #[schemars(description = "short description of the skill")]
    pub description: String,

    #[schemars(description = "content/instruction of the skill")]
    pub instruction: String,

    #[schemars(description = "keywords for matching (e.g., [\"review\", \"quality\"])")]
    #[serde(default)]
    pub keywords: Vec<String>,

    #[schemars(description = "additional resource files to create with this skill")]
    #[serde(default)]
    pub resources: Vec<SkillResource>,
}

#[async_trait::async_trait]
impl VizierTool for CreateSkill {
    type Input = CreateSkillArgs;
    type Output = String;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Skills)
    }

    fn name() -> String {
        "create_skill".to_string()
    }

    fn description(&self) -> String {
        "create a new skill you have learn, to be reusable".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let slug = slugify::slugify!(&args.name);

        let resource_paths: Vec<String> = args.resources.iter().map(|r| r.path.clone()).collect();

        let skill = Skill {
            author: self.0.clone(),
            agent_id: Some(self.0.clone()),
            description: args.description,
            name: slug.clone(),
            content: args.instruction,
            keywords: args.keywords,
            version: Default::default(),
            resources: resource_paths,
            user_invocable: true,
            disable_model_invocation: false,
            argument_hint: None,
        };

        self.1
            .save_skill(&skill)
            .map_err(|err| VizierError(err.to_string()))?;

        let skill_dir = self.1.skill_dir(&slug);
        write_resource_files(&skill_dir, &args.resources)?;

        if let Some(indexer) = &self.2 {
            let scope = scope_for_agent(&self.0);
            let _ = index_skill(indexer, &scope, &skill).await;
        }

        Ok(format!("Skill '{}' created successfully", slug))
    }
}
