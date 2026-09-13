use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::agents::skill::{index_skill, scope_for_agent};
use crate::agents::tools::{ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::indexer::VizierIndexer;
use crate::schema::{AgentId, Skill};
use crate::skill::SkillManager;

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

fn validate_resource_path(path: &str) -> Result<(), VizierError> {
    if path.is_empty() {
        return Err(VizierError("resource path cannot be empty".into()));
    }
    if path.starts_with('/') {
        return Err(VizierError(
            "resource path must be relative (no leading /)".into(),
        ));
    }
    for component in Path::new(path).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(VizierError("resource path must not contain '..'".into()));
        }
    }
    Ok(())
}

fn write_resource_files(skill_dir: &Path, resources: &[SkillResource]) -> Result<(), VizierError> {
    for resource in resources {
        validate_resource_path(&resource.path)?;
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
            version: 1,
            resources: resource_paths,
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
