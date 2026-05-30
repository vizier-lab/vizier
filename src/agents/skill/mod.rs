use std::sync::Arc;

use anyhow::Result;
use rig_core::completion::ToolDefinition;
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::{
    dependencies::VizierDependencies,
    schema::{AgentId, Skill},
    storage::{VizierStorage, skill::SkillStorage},
};

#[derive(Clone)]
pub struct VizierSkills {
    agent_id: String,
    storage: Arc<VizierStorage>,
}

impl VizierSkills {
    pub async fn new(agent_id: AgentId, deps: VizierDependencies) -> Result<Self> {
        Ok(Self {
            agent_id,
            storage: deps.storage,
        })
    }

    pub async fn get_skills(&self) -> Result<Vec<ToolDefinition>> {
        Ok(self
            .storage
            .list_skill(Some(self.agent_id.clone()))
            .await?
            .iter()
            .map(|skill| skill.to_definition())
            .collect())
    }

    pub async fn get_skill_content(&self, slug: String) -> Result<Option<String>> {
        Ok(self
            .storage
            .get_skill(Some(self.agent_id.clone()), slug.clone())
            .await?
            .map(|item| item.content.clone()))
    }
}

#[allow(unused)]
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SkillArgs {}

impl Skill {
    pub fn to_definition(&self) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(SkillArgs)).unwrap();

        ToolDefinition {
            name: format!("SKILL__{}", self.name.clone()),
            description: self.description.clone(),
            parameters,
        }
    }
}
