use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::VizierTool;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::skill::SkillManager;

pub struct ListSkills(SkillManager);

impl ListSkills {
    pub fn new(deps: VizierDependencies) -> Self {
        let workspace = deps.config.workspace.clone();
        Self(SkillManager::new(&workspace))
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
    pub keywords: Vec<String>,
    pub activation: String,
    pub version: u32,
}

#[async_trait::async_trait]
impl VizierTool for ListSkills {
    type Input = ListSkillsArgs;
    type Output = Vec<SkillInfo>;

    fn name() -> String {
        "list_skills".to_string()
    }

    fn description(&self) -> String {
        "list available skills, optionally filtered by keyword".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let skills = self.0.list_skills()
            .map_err(|e| VizierError(e.to_string()))?;

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
                keywords: skill.keywords.clone(),
                activation: format!("{:?}", skill.activation),
                version: skill.version,
            })
            .collect();

        Ok(filtered)
    }
}
