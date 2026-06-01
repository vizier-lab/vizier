pub mod context;
pub mod install;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    schema::{Skill, SkillActivation},
    utils::{build_path, markdown::read_markdown},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillMeta {
    pub source: SkillSource,
    pub registry_url: Option<String>,
    pub slug: Option<String>,
    pub installed_version: u32,
    pub installed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum SkillSource {
    #[serde(rename = "registry")]
    Registry,
    #[serde(rename = "git")]
    Git,
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "created")]
    Created,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SkillFrontMatter {
    pub name: String,
    pub author: String,
    pub description: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default = "default_activation")]
    pub activation: SkillActivation,
    #[serde(default = "default_version")]
    pub version: u32,
}

fn default_activation() -> SkillActivation {
    SkillActivation::OnDemand
}

fn default_version() -> u32 {
    1
}

impl From<Skill> for SkillFrontMatter {
    fn from(skill: Skill) -> Self {
        Self {
            name: skill.name,
            author: skill.author,
            description: skill.description,
            keywords: skill.keywords,
            activation: skill.activation,
            version: skill.version,
        }
    }
}

impl From<SkillFrontMatter> for Skill {
    fn from(fm: SkillFrontMatter) -> Self {
        Self {
            name: fm.name,
            agent_id: None,
            author: fm.author,
            description: fm.description,
            content: String::new(),
            keywords: fm.keywords,
            activation: fm.activation,
            version: fm.version,
            resources: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct SkillManager {
    skills_dir: PathBuf,
    agent_id: Option<String>,
}

impl SkillManager {
    pub fn skill_dir(&self, slug: &str) -> PathBuf {
        self.skills_dir.join(slug)
    }

    pub fn new(workspace: &str) -> Self {
        let skills_dir = build_path(workspace, &["skills"]);
        Self { skills_dir, agent_id: None }
    }

    pub fn for_agent(workspace: &str, agent_id: &str) -> Self {
        let skills_dir = build_path(workspace, &["agents", agent_id, "skills"]);
        Self { skills_dir, agent_id: Some(agent_id.to_string()) }
    }

    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }

    pub fn list_skills(&self) -> crate::Result<Vec<Skill>> {
        let mut skills = Vec::new();

        if !self.skills_dir.exists() {
            return Ok(skills);
        }

        let entries = std::fs::read_dir(&self.skills_dir)
            .map_err(|e| crate::VizierError(format!("Failed to read skills directory: {}", e)))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(Some(skill)) = self.read_skill_from_dir(&path) {
                    skills.push(skill);
                }
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    pub fn get_skill(&self, slug: &str) -> crate::Result<Option<Skill>> {
        let skill_dir = self.skills_dir.join(slug);
        if !skill_dir.exists() {
            return Ok(None);
        }
        self.read_skill_from_dir(&skill_dir)
    }

    pub fn save_skill(&self, skill: &Skill) -> crate::Result<()> {
        let skill_dir = self.skills_dir.join(&skill.name);
        std::fs::create_dir_all(&skill_dir)
            .map_err(|e| crate::VizierError(format!("Failed to create skill directory: {}", e)))?;

        let skill_md_path = skill_dir.join("SKILL.md");
        let frontmatter = SkillFrontMatter::from(skill.clone());
        crate::utils::markdown::write_markdown(&frontmatter, skill.content.clone(), skill_md_path)
            .map_err(|e| crate::VizierError(format!("Failed to write SKILL.md: {}", e)))?;

        Ok(())
    }

    pub fn delete_skill(&self, slug: &str) -> crate::Result<bool> {
        let skill_dir = self.skills_dir.join(slug);
        if !skill_dir.exists() {
            return Ok(false);
        }
        std::fs::remove_dir_all(&skill_dir)
            .map_err(|e| crate::VizierError(format!("Failed to delete skill: {}", e)))?;
        Ok(true)
    }

    pub fn list_resources(&self, slug: &str) -> crate::Result<Vec<PathBuf>> {
        let skill_dir = self.skills_dir.join(slug);
        if !skill_dir.exists() {
            return Ok(Vec::new());
        }

        let mut resources = Vec::new();
        self.collect_resources(&skill_dir, &skill_dir, &mut resources)?;
        Ok(resources)
    }

    fn collect_resources(
        &self,
        base: &Path,
        dir: &Path,
        resources: &mut Vec<PathBuf>,
    ) -> crate::Result<()> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| crate::VizierError(format!("Failed to read skill directory: {}", e)))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.collect_resources(base, &path, resources)?;
            } else {
                let filename = path.file_name().unwrap().to_str().unwrap();
                if filename != "SKILL.md" && filename != ".meta.json" {
                    let relative = path.strip_prefix(base).unwrap().to_path_buf();
                    resources.push(relative);
                }
            }
        }

        Ok(())
    }

    pub fn read_resource(&self, slug: &str, resource_path: &str) -> crate::Result<Option<String>> {
        let path = self.skills_dir.join(slug).join(resource_path);
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| crate::VizierError(format!("Failed to read resource: {}", e)))?;
        Ok(Some(content))
    }

    pub fn read_resource_bytes(&self, slug: &str, resource_path: &str) -> crate::Result<Option<Vec<u8>>> {
        let path = self.skills_dir.join(slug).join(resource_path);
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read(&path)
            .map_err(|e| crate::VizierError(format!("Failed to read resource: {}", e)))?;
        Ok(Some(content))
    }

    pub fn save_meta(&self, slug: &str, meta: &SkillMeta) -> crate::Result<()> {
        let skill_dir = self.skills_dir.join(slug);
        std::fs::create_dir_all(&skill_dir)
            .map_err(|e| crate::VizierError(format!("Failed to create skill directory: {}", e)))?;

        let meta_path = skill_dir.join(".meta.json");
        let json = serde_json::to_string_pretty(meta)
            .map_err(|e| crate::VizierError(format!("Failed to serialize meta: {}", e)))?;
        std::fs::write(&meta_path, json)
            .map_err(|e| crate::VizierError(format!("Failed to write meta: {}", e)))?;

        Ok(())
    }

    pub fn load_meta(&self, slug: &str) -> crate::Result<Option<SkillMeta>> {
        let meta_path = self.skills_dir.join(slug).join(".meta.json");
        if !meta_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&meta_path)
            .map_err(|e| crate::VizierError(format!("Failed to read meta: {}", e)))?;
        let meta: SkillMeta = serde_json::from_str(&content)
            .map_err(|e| crate::VizierError(format!("Failed to parse meta: {}", e)))?;
        Ok(Some(meta))
    }

    fn read_skill_from_dir(&self, skill_dir: &Path) -> crate::Result<Option<Skill>> {
        let skill_md_path = skill_dir.join("SKILL.md");
        if !skill_md_path.exists() {
            return Ok(None);
        }

        let (frontmatter, content) = read_markdown::<SkillFrontMatter>(skill_md_path)
            .map_err(|e| crate::VizierError(format!("Failed to read SKILL.md: {}", e)))?;

        let slug = skill_dir.file_name().unwrap().to_str().unwrap();

        let mut skill: Skill = frontmatter.into();
        skill.name = slug.to_string();
        skill.content = content;
        skill.agent_id = self.agent_id.clone();

        // Populate resources
        let mut resources = Vec::new();
        self.collect_resources(skill_dir, skill_dir, &mut resources)?;
        skill.resources = resources.into_iter().map(|p| p.to_string_lossy().to_string()).collect();

        Ok(Some(skill))
    }
}
