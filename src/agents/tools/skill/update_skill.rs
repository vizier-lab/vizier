use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{SkillResource, write_resource_files};
use crate::agents::tools::VizierTool;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::{Skill, SkillActivation};
use crate::skill::SkillManager;

pub struct UpdateSkill(SkillManager);

impl UpdateSkill {
    pub fn new(deps: VizierDependencies) -> Self {
        let workspace = deps.config.workspace.clone();
        Self(SkillManager::new(&workspace))
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

    #[schemars(description = "new activation mode (optional)")]
    pub activation: Option<SkillActivation>,

    #[schemars(description = "resource files to add or update (optional, replaces all resources)")]
    pub resources: Option<Vec<SkillResource>>,
}

#[async_trait::async_trait]
impl VizierTool for UpdateSkill {
    type Input = UpdateSkillArgs;
    type Output = String;

    fn name() -> String {
        "update_skill".to_string()
    }

    fn description(&self) -> String {
        "update an existing skill's content, description, keywords, activation mode, or resource files".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let mut skill = self.0.get_skill(&args.slug)
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError(format!("Skill '{}' not found", args.slug)))?;

        if let Some(content) = args.content {
            skill.content = content;
        }
        if let Some(description) = args.description {
            skill.description = description;
        }
        if let Some(keywords) = args.keywords {
            skill.keywords = keywords;
        }
        if let Some(activation) = args.activation {
            skill.activation = activation;
        }

        if let Some(resources) = &args.resources {
            let skill_dir = self.0.skill_dir(&args.slug);
            write_resource_files(&skill_dir, resources)?;
            skill.resources = resources.iter().map(|r| r.path.clone()).collect();
        }

        skill.version += 1;

        self.0.save_skill(&skill)
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(format!("Skill '{}' updated to version {}", skill.name, skill.version))
    }
}
