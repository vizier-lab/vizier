use std::{collections::HashMap, path::PathBuf, str::FromStr};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    schema::{AgentId, Skill},
    storage::{fs::FileSystemStorage, skill::SkillStorage},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SkillFrontMatter {
    pub name: String,
    pub author: String,
    pub description: String,
}

impl From<Skill> for SkillFrontMatter {
    fn from(value: Skill) -> Self {
        Self {
            name: value.name,
            author: value.author,
            description: value.description,
        }
    }
}

#[async_trait::async_trait]
impl SkillStorage for FileSystemStorage {
    async fn save_skill(&self, agent_id: Option<AgentId>, skill: Skill) -> Result<()> {
        let mut path = format!("{}/skills/{}/SKILL.md", self.workspace, skill.name);
        if let Some(agent_id) = agent_id {
            path = format!(
                "{}/agents/{}/skills/{}/SKILL.md",
                self.workspace, agent_id, skill.name
            );
        }

        let frontmatter = SkillFrontMatter::from(skill.clone());
        crate::utils::markdown::write_markdown(
            &frontmatter,
            skill.content.clone(),
            PathBuf::from_str(&path)?,
        )?;

        Ok(())
    }

    async fn list_skill(&self, agent_id: Option<AgentId>) -> Result<Vec<Skill>> {
        let mut skills = HashMap::<String, Skill>::new();

        let path = format!("{}/skills/*/SKILL.md", self.workspace);
        for entry in glob::glob(&path)? {
            let entry = entry?;

            let (frontmatter, content) =
                crate::utils::markdown::read_markdown::<SkillFrontMatter>(entry)?;

            let skill = Skill {
                name: frontmatter.name,
                author: frontmatter.author,
                description: frontmatter.description,
                agent_id: None,
                content,
            };

            skills.insert(skill.name.clone(), skill);
        }

        if let Some(agent_id) = agent_id {
            let path = format!("{}/agents/{}/skills/*/SKILL.md", self.workspace, agent_id);

            for entry in glob::glob(&path)? {
                let entry = entry?;

                let (frontmatter, content) =
                    crate::utils::markdown::read_markdown::<SkillFrontMatter>(entry)?;

                let skill = Skill {
                    name: frontmatter.name,
                    author: frontmatter.author,
                    description: frontmatter.description,
                    agent_id: Some(agent_id.clone()),
                    content,
                };

                skills.insert(skill.name.clone(), skill);
            }
        }

        Ok(skills.iter().map(|(_, skill)| skill.clone()).collect())
    }
    async fn get_skill(&self, agent_id: Option<AgentId>, slug: String) -> Result<Option<Skill>> {
        if let Some(agent_id) = agent_id {
            let path = format!(
                "{}/agents/{}/skills/{}/SKILL.md",
                self.workspace, agent_id, slug
            );

            let (frontmatter, content) = crate::utils::markdown::read_markdown::<SkillFrontMatter>(
                PathBuf::from_str(&path)?,
            )?;

            let skill = Skill {
                name: frontmatter.name,
                author: frontmatter.author,
                description: frontmatter.description,
                agent_id: Some(agent_id.clone()),
                content,
            };

            return Ok(Some(skill));
        }

        let path = format!("{}/skills/{}/SKILL.md", self.workspace, slug);

        let (frontmatter, content) =
            crate::utils::markdown::read_markdown::<SkillFrontMatter>(PathBuf::from_str(&path)?)?;

        let skill = Skill {
            name: frontmatter.name,
            author: frontmatter.author,
            description: frontmatter.description,
            agent_id: None,
            content,
        };

        Ok(Some(skill))
    }
}
