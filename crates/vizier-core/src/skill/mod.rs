pub mod builtin;
pub mod install;
pub mod invocation;
pub mod scopes;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    schema::{Skill, SkillFrontMatter},
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

/// A resource path inside a skill folder: relative, no `..`, no leading `/`. The one rule every
/// resource read and write shares, because a skill folder is not a place a tool may leave.
pub fn validate_resource_path(path: &str) -> crate::Result<()> {
    if path.is_empty() {
        return Err(crate::VizierError("resource path cannot be empty".into()));
    }
    if path.starts_with('/') {
        return Err(crate::VizierError(
            "resource path must be relative (no leading /)".into(),
        ));
    }
    for component in Path::new(path).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(crate::VizierError("resource path must not contain '..'".into()));
        }
    }
    Ok(())
}

/// What one read of a skills folder found.
#[derive(Debug, Clone, Default)]
pub struct SkillScan {
    pub skills: Vec<Skill>,
    /// Folders that could not be read, each already logged.
    pub skipped: Vec<SkippedSkill>,
}

/// A role folder that holds no readable `SKILL.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedSkill {
    pub slug: String,
    pub reason: String,
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

    /// A project's own skills: the folder holds them where it lies, in the project, and nothing
    /// here writes to it (`specs/041-project-coding-workspace` FR-037).
    pub fn for_project(skills_dir: &Path) -> Self {
        Self { skills_dir: skills_dir.to_path_buf(), agent_id: None }
    }

    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }

    pub fn list_skills(&self) -> crate::Result<Vec<Skill>> {
        Ok(self.scan()?.skills)
    }

    /// [`Self::list_skills`], and the folders it could not read. A skill that fails to parse used
    /// to be dropped in silence; a bad `SKILL.md` is a person's typo, so it is named instead
    /// (`specs/041-project-coding-workspace` FR-039).
    pub fn scan(&self) -> crate::Result<SkillScan> {
        let mut scan = SkillScan::default();

        if !self.skills_dir.exists() {
            return Ok(scan);
        }

        let entries = std::fs::read_dir(&self.skills_dir)
            .map_err(|e| crate::VizierError(format!("Failed to read skills directory: {}", e)))?;

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(slug) = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
            else {
                continue;
            };
            if !path.is_dir() {
                continue;
            }
            match self.read_skill_from_dir(&path) {
                Ok(Some(skill)) => scan.skills.push(skill),
                Ok(None) => scan.skipped.push(SkippedSkill {
                    slug,
                    reason: "no SKILL.md".to_string(),
                }),
                Err(err) => scan.skipped.push(SkippedSkill { slug, reason: err.to_string() }),
            }
        }

        for skipped in &scan.skipped {
            tracing::warn!(
                skills_dir = %self.skills_dir.display(),
                skill = %skipped.slug,
                "skill folder could not be read: {}",
                skipped.reason
            );
        }

        scan.skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(scan)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        schema::{SkillFrontMatter, SkillVersion},
        utils::markdown::parse_markdown_str,
    };

    /// The skills this repository itself carries, in the Agent Skills layout. They are the reason
    /// the frontmatter is as wide as it is (`specs/041-project-coding-workspace` FR-038): a real
    /// corpus written by other harnesses is the only honest fixture.
    fn repo_skills() -> Option<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agents/skills");
        dir.is_dir().then_some(dir)
    }

    fn frontmatter(path: &Path) -> SkillFrontMatter {
        let raw = std::fs::read_to_string(path).unwrap();
        parse_markdown_str::<SkillFrontMatter>(&raw)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .0
    }

    #[test]
    fn every_skill_the_repo_carries_parses() {
        let Some(dir) = repo_skills() else {
            eprintln!("no .agents/skills in this checkout; nothing to check");
            return;
        };
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let skill_md = entry.path().join("SKILL.md");
            if !skill_md.is_file() {
                continue;
            }
            seen += 1;
            let fm = frontmatter(&skill_md);
            assert!(!fm.name.is_empty(), "{}", skill_md.display());
            assert!(!fm.description.is_empty(), "{}", skill_md.display());
        }
        assert!(seen >= 15, "expected the repo's skills, found {seen}");
    }

    /// `speckit-plan` carries no `author` and both invocation flags: the shape that made all 15 of
    /// these skills unreadable before the field became optional.
    #[test]
    fn a_spec_kit_skill_without_author_parses_with_its_flags() {
        let Some(dir) = repo_skills() else { return };
        let fm = frontmatter(&dir.join("speckit-plan/SKILL.md"));
        assert_eq!(fm.name, "speckit-plan");
        assert_eq!(fm.author, "", "an absent author is an empty string, not a failure");
        assert!(fm.user_invocable, "user-invocable: true");
        assert!(!fm.disable_model_invocation);
        assert!(fm.argument_hint.is_some());
        assert_eq!(fm.version, SkillVersion::Number(1), "no version field defaults to 1");
    }

    /// `impeccable` writes a semantic version as a string: `version: 4.7.0`.
    #[test]
    fn a_string_version_parses_and_keeps_its_shape() {
        let Some(dir) = repo_skills() else { return };
        let fm = frontmatter(&dir.join("impeccable/SKILL.md"));
        assert_eq!(fm.version, SkillVersion::Text("4.7.0".into()));
        assert_eq!(fm.version.leading_number(), 4);
        assert_eq!(fm.version.to_string(), "4.7.0");
        assert!(fm.user_invocable);
        assert!(!fm.disable_model_invocation);
    }

    /// A save writes back the version it read, so a skill is never rewritten into Vizier's number
    /// shape, and the new fields survive the round trip.
    #[test]
    fn a_save_round_trips_a_text_version_and_the_flags() {
        let dir = tempfile::tempdir().unwrap();
        let manager = SkillManager::new(dir.path().to_str().unwrap());
        let skill = Skill {
            name: "impeccable".into(),
            agent_id: None,
            author: String::new(),
            description: "design craft".into(),
            content: "## Body\nfollow the craft floor".into(),
            keywords: vec!["design".into()],
            version: SkillVersion::Text("4.7.0".into()),
            resources: vec![],
            user_invocable: true,
            disable_model_invocation: true,
            argument_hint: Some("[audit|critique] [target]".into()),
        };
        manager.save_skill(&skill).unwrap();

        let written =
            std::fs::read_to_string(dir.path().join("skills/impeccable/SKILL.md")).unwrap();
        assert!(written.contains("user-invocable: true"), "{written}");
        assert!(written.contains("disable-model-invocation: true"), "{written}");
        assert!(written.contains("argument-hint:"), "{written}");
        assert!(written.contains("4.7.0"), "{written}");

        let read = manager.get_skill("impeccable").unwrap().expect("saved skill");
        assert_eq!(read.version, SkillVersion::Text("4.7.0".into()));
        assert!(read.user_invocable);
        assert!(read.disable_model_invocation);
        assert_eq!(read.argument_hint.as_deref(), Some("[audit|critique] [target]"));
        assert_eq!(read.content, "## Body\nfollow the craft floor");
    }

    #[test]
    fn a_numeric_version_counts_up_and_a_text_one_does_not() {
        assert_eq!(SkillVersion::Number(2).bumped(), SkillVersion::Number(3));
        assert_eq!(
            SkillVersion::Text("4.7.0".into()).bumped(),
            SkillVersion::Text("4.7.0".into())
        );
    }
}

