use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;

use crate::skill::{SkillManager, SkillMeta, SkillSource};

const DEFAULT_REGISTRY: &str = "https://github.com/vizier-lab/vizier.git";

pub fn check_git_available() -> crate::Result<()> {
    match Command::new("git").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                Err(crate::VizierError(
                    "git is not working properly. Please install git: https://git-scm.com/downloads".into(),
                ))
            }
        }
        Err(_) => Err(crate::VizierError(
            "git is required for skill installation. Install git: https://git-scm.com/downloads".into(),
        )),
    }
}

pub fn detect_source(source: &str) -> (SkillSource, String, Option<String>) {
    if source.starts_with("http://") || source.starts_with("https://") || source.ends_with(".git") {
        (SkillSource::Git, source.to_string(), None)
    } else if source.contains('/') && !source.starts_with('.') {
        let url = if source.starts_with("https://") || source.starts_with("http://") {
            source.to_string()
        } else {
            format!("https://github.com/{}.git", source)
        };
        (SkillSource::Git, url, Some(source.to_string()))
    } else if source.starts_with('.') || source.starts_with('/') {
        (SkillSource::Local, source.to_string(), None)
    } else {
        (
            SkillSource::Registry,
            DEFAULT_REGISTRY.to_string(),
            Some(source.to_string()),
        )
    }
}

pub fn install_skill(
    manager: &SkillManager,
    source: &str,
    target_dir: &Path,
) -> crate::Result<Vec<String>> {
    check_git_available()?;

    let (source_type, url, slug) = detect_source(source);

    match source_type {
        SkillSource::Registry => {
            let slug = slug.ok_or_else(|| crate::VizierError("Registry source requires a skill slug".into()))?;
            install_from_registry(&url, &slug, target_dir, manager)
        }
        SkillSource::Git => {
            install_from_git(&url, slug.as_deref(), target_dir, manager)
        }
        SkillSource::Local => {
            install_from_local(&url, target_dir, manager)
        }
        SkillSource::Created => {
            Err(crate::VizierError("Cannot install created skills".into()))
        }
    }
}

fn install_from_registry(
    registry_url: &str,
    slug: &str,
    target_dir: &Path,
    manager: &SkillManager,
) -> crate::Result<Vec<String>> {
    let tmp_dir = tempfile::TempDir::new().map_err(|e| crate::VizierError(format!("Failed to create temp dir: {}", e)))?;

    // Sparse clone
    let output = Command::new("git")
        .args(["clone", "--depth", "1", "--filter=blob:none", "--sparse", registry_url])
        .arg(tmp_dir.path())
        .output()
        .map_err(|e| crate::VizierError(format!("Failed to run git clone: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::VizierError(format!("git clone failed: {}", stderr)));
    }

    // Set sparse checkout
    let sparse_pattern = format!("skills/{}", slug);
    let output = Command::new("git")
        .args(["sparse-checkout", "set", &sparse_pattern])
        .current_dir(tmp_dir.path())
        .output()
        .map_err(|e| crate::VizierError(format!("Failed to run git sparse-checkout: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::VizierError(format!("git sparse-checkout failed: {}", stderr)));
    }

    let source_dir = tmp_dir.path().join("skills").join(slug);
    if !source_dir.exists() {
        return Err(crate::VizierError(format!("Skill '{}' not found in registry", slug)));
    }

    // Read skill to get version
    let skill_md = source_dir.join("SKILL.md");
    if !skill_md.exists() {
        return Err(crate::VizierError(format!("SKILL.md not found for skill '{}'", slug)));
    }

    let (frontmatter, _) = crate::utils::markdown::read_markdown::<crate::skill::SkillFrontMatter>(skill_md)
        .map_err(|e| crate::VizierError(format!("Failed to read SKILL.md: {}", e)))?;

    // Copy to target
    let dest = target_dir.join(slug);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| crate::VizierError(format!("Failed to remove existing skill: {}", e)))?;
    }
    copy_dir_all(&source_dir, &dest)?;

    // Save meta
    let meta = SkillMeta {
        source: SkillSource::Registry,
        registry_url: Some(registry_url.to_string()),
        slug: Some(slug.to_string()),
        installed_version: frontmatter.version,
        installed_at: Utc::now(),
    };
    manager.save_meta(slug, &meta)?;

    Ok(vec![slug.to_string()])
}

fn install_from_git(
    repo_url: &str,
    skill_slug: Option<&str>,
    target_dir: &Path,
    manager: &SkillManager,
) -> crate::Result<Vec<String>> {
    let tmp_dir = tempfile::TempDir::new().map_err(|e| crate::VizierError(format!("Failed to create temp dir: {}", e)))?;

    // Clone repo
    let output = Command::new("git")
        .args(["clone", "--depth", "1", repo_url])
        .arg(tmp_dir.path())
        .output()
        .map_err(|e| crate::VizierError(format!("Failed to run git clone: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::VizierError(format!("git clone failed: {}", stderr)));
    }

    // Find skills in the cloned repo
    let skills_dir = tmp_dir.path().join("skills");
    let mut installed = Vec::new();

    if let Some(slug) = skill_slug {
        // Install specific skill
        let source_dir = skills_dir.join(slug);
        if !source_dir.exists() {
            return Err(crate::VizierError(format!("Skill '{}' not found in repository", slug)));
        }

        let dest = target_dir.join(slug);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)
                .map_err(|e| crate::VizierError(format!("Failed to remove existing skill: {}", e)))?;
        }
        copy_dir_all(&source_dir, &dest)?;

        // Read version
        let skill_md = source_dir.join("SKILL.md");
        let version = if skill_md.exists() {
            let (frontmatter, _) = crate::utils::markdown::read_markdown::<crate::skill::SkillFrontMatter>(skill_md)
                .map_err(|e| crate::VizierError(format!("Failed to read SKILL.md: {}", e)))?;
            frontmatter.version
        } else {
            1
        };

        let meta = SkillMeta {
            source: SkillSource::Git,
            registry_url: Some(repo_url.to_string()),
            slug: Some(slug.to_string()),
            installed_version: version,
            installed_at: Utc::now(),
        };
        manager.save_meta(slug, &meta)?;

        installed.push(slug.to_string());
    } else {
        // Install all skills found
        if skills_dir.exists() {
            for entry in std::fs::read_dir(&skills_dir)
                .map_err(|e| crate::VizierError(format!("Failed to read skills directory: {}", e)))?
            {
                let entry = entry.map_err(|e| crate::VizierError(format!("Failed to read entry: {}", e)))?;
                let path = entry.path();
                if path.is_dir() {
                    let slug = path.file_name().unwrap().to_str().unwrap();
                    let dest = target_dir.join(slug);
                    if dest.exists() {
                        std::fs::remove_dir_all(&dest)
                            .map_err(|e| crate::VizierError(format!("Failed to remove existing skill: {}", e)))?;
                    }
                    copy_dir_all(&path, &dest)?;

                    let skill_md = path.join("SKILL.md");
                    let version = if skill_md.exists() {
                        let (frontmatter, _) = crate::utils::markdown::read_markdown::<crate::skill::SkillFrontMatter>(skill_md)
                            .map_err(|e| crate::VizierError(format!("Failed to read SKILL.md: {}", e)))?;
                        frontmatter.version
                    } else {
                        1
                    };

                    let meta = SkillMeta {
                        source: SkillSource::Git,
                        registry_url: Some(repo_url.to_string()),
                        slug: Some(slug.to_string()),
                        installed_version: version,
                        installed_at: Utc::now(),
                    };
                    manager.save_meta(slug, &meta)?;

                    installed.push(slug.to_string());
                }
            }
        }
    }

    if installed.is_empty() {
        return Err(crate::VizierError("No skills found in repository".into()));
    }

    Ok(installed)
}

fn install_from_local(
    local_path: &str,
    target_dir: &Path,
    manager: &SkillManager,
) -> crate::Result<Vec<String>> {
    let source_dir = PathBuf::from(local_path);
    if !source_dir.exists() {
        return Err(crate::VizierError(format!("Source path '{}' does not exist", local_path)));
    }

    let mut installed = Vec::new();

    // Check if it's a single skill directory (has SKILL.md)
    if source_dir.join("SKILL.md").exists() {
        let slug = source_dir.file_name().unwrap().to_str().unwrap();
        let dest = target_dir.join(slug);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)
                .map_err(|e| crate::VizierError(format!("Failed to remove existing skill: {}", e)))?;
        }
        copy_dir_all(&source_dir, &dest)?;

        let (frontmatter, _) = crate::utils::markdown::read_markdown::<crate::skill::SkillFrontMatter>(source_dir.join("SKILL.md"))
            .map_err(|e| crate::VizierError(format!("Failed to read SKILL.md: {}", e)))?;

        let meta = SkillMeta {
            source: SkillSource::Local,
            registry_url: None,
            slug: Some(slug.to_string()),
            installed_version: frontmatter.version,
            installed_at: Utc::now(),
        };
        manager.save_meta(slug, &meta)?;

        installed.push(slug.to_string());
    } else {
        // Check if it contains skill directories
        for entry in std::fs::read_dir(&source_dir)
            .map_err(|e| crate::VizierError(format!("Failed to read directory: {}", e)))?
        {
            let entry = entry.map_err(|e| crate::VizierError(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();
            if path.is_dir() && path.join("SKILL.md").exists() {
                let slug = path.file_name().unwrap().to_str().unwrap();
                let dest = target_dir.join(slug);
                if dest.exists() {
                    std::fs::remove_dir_all(&dest)
                        .map_err(|e| crate::VizierError(format!("Failed to remove existing skill: {}", e)))?;
                }
                copy_dir_all(&path, &dest)?;

                let (frontmatter, _) = crate::utils::markdown::read_markdown::<crate::skill::SkillFrontMatter>(path.join("SKILL.md"))
                    .map_err(|e| crate::VizierError(format!("Failed to read SKILL.md: {}", e)))?;

                let meta = SkillMeta {
                    source: SkillSource::Local,
                    registry_url: None,
                    slug: Some(slug.to_string()),
                    installed_version: frontmatter.version,
                    installed_at: Utc::now(),
                };
                manager.save_meta(slug, &meta)?;

                installed.push(slug.to_string());
            }
        }
    }

    if installed.is_empty() {
        return Err(crate::VizierError("No skills found at the specified path".into()));
    }

    Ok(installed)
}

fn copy_dir_all(src: &Path, dst: &Path) -> crate::Result<()> {
    std::fs::create_dir_all(dst)
        .map_err(|e| crate::VizierError(format!("Failed to create directory: {}", e)))?;

    for entry in std::fs::read_dir(src)
        .map_err(|e| crate::VizierError(format!("Failed to read directory: {}", e)))?
    {
        let entry = entry.map_err(|e| crate::VizierError(format!("Failed to read entry: {}", e)))?;
        let ty = entry.file_type().map_err(|e| crate::VizierError(format!("Failed to get file type: {}", e)))?;
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path)
                .map_err(|e| crate::VizierError(format!("Failed to copy file: {}", e)))?;
        }
    }

    Ok(())
}
