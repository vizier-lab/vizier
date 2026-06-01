use std::sync::Arc;

use anyhow::Result;
use rig_core::completion::ToolDefinition;
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::{
    dependencies::VizierDependencies,
    schema::{AgentId, Skill, SkillActivation},
    skill::{SkillManager, context},
};

#[derive(Clone)]
pub struct VizierSkills {
    agent_id: String,
    global_manager: SkillManager,
    agent_manager: SkillManager,
}

impl VizierSkills {
    pub async fn new(agent_id: AgentId, deps: VizierDependencies) -> Result<Self> {
        let workspace = deps.config.workspace.clone();
        Ok(Self {
            agent_id: agent_id.clone(),
            global_manager: SkillManager::new(&workspace),
            agent_manager: SkillManager::for_agent(&workspace, &agent_id),
        })
    }

    fn all_skills(&self) -> crate::Result<Vec<Skill>> {
        let mut skills = self.global_manager.list_skills()?;
        let agent_skills = self.agent_manager.list_skills()?;

        // Agent skills override global skills with same name
        for agent_skill in agent_skills {
            if !skills.iter().any(|s| s.name == agent_skill.name) {
                skills.push(agent_skill);
            }
        }

        Ok(skills)
    }

    pub async fn get_always_skills(&self) -> Result<Vec<String>> {
        let skills = self.all_skills()?;
        Ok(skills
            .iter()
            .filter(|s| s.activation == SkillActivation::Always)
            .map(|s| format!("# Skill: {}\n\n{}", s.name, s.content))
            .collect())
    }

    pub async fn get_ondemand_skills(&self) -> Result<Vec<ToolDefinition>> {
        let skills = self.all_skills()?;
        Ok(skills
            .iter()
            .filter(|s| s.activation == SkillActivation::OnDemand)
            .map(|s| s.to_definition())
            .collect())
    }

    pub async fn get_contextual_skills(&self, task: &str) -> Result<Vec<ToolDefinition>> {
        let skills = self.all_skills()?;
        let contextual_skills: Vec<Skill> = skills
            .into_iter()
            .filter(|s| s.activation == SkillActivation::Contextual)
            .collect();

        // Try keyword matching first
        let matched = context::match_skills_by_keywords(&contextual_skills, task);

        if !matched.is_empty() {
            return Ok(matched.into_iter().map(|s| s.to_definition()).collect());
        }

        // Fall back to description matching
        let matched = context::match_skills_by_description(&contextual_skills, task);
        Ok(matched.into_iter().map(|s| s.to_definition()).collect())
    }

    pub async fn get_skills(&self) -> Result<Vec<ToolDefinition>> {
        let mut tools = self.get_ondemand_skills().await?;
        // Note: Always skills are injected into system prompt, not as tools
        // Contextual skills are matched against task and added separately
        Ok(tools)
    }

    pub async fn get_skill_content(&self, slug: String) -> Result<Option<String>> {
        // Agent skill takes priority
        if let Some(skill) = self.agent_manager.get_skill(&slug)? {
            return Ok(Some(skill.content));
        }
        if let Some(skill) = self.global_manager.get_skill(&slug)? {
            return Ok(Some(skill.content));
        }
        Ok(None)
    }

    pub fn get_skill_manager(&self) -> &SkillManager {
        &self.global_manager
    }

    pub fn get_agent_skill_manager(&self) -> &SkillManager {
        &self.agent_manager
    }
}

#[allow(unused)]
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SkillArgs {}

impl Skill {
    pub fn to_definition(&self) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(SkillArgs)).unwrap();

        let mut description = self.description.clone();
        if !self.resources.is_empty() {
            description.push_str(&format!(
                "\n\nAvailable resources: {}",
                self.resources.join(", ")
            ));
        }

        ToolDefinition {
            name: format!("SKILL__{}", self.name.clone()),
            description,
            parameters,
        }
    }
}
