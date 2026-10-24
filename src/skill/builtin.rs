//! Skills shipped in the binary and written into the workspace's global `skills/` at startup,
//! so every agent has them from its first turn (they are indexed with the other global skills
//! at agent boot).
//!
//! A shipped skill is the operator's to change once it is on disk: an edited or deleted copy is
//! left alone, and only a copy still exactly as vizier last wrote it is replaced by a newer
//! shipped version. What was last written is remembered as a hash in state storage.

use sha2::{Digest, Sha256};

use crate::{
    storage::{VizierStorage, state::StateStorage},
    utils::build_path,
};

/// `(slug, SKILL.md)` of every built-in skill.
///
/// Empty since spec 026: `interactive-ui` was replaced by the `ui_components` tool. A copy an
/// earlier version wrote stays in the workspace, the operator's to keep or delete. Add a
/// `("slug", include_str!("../../templates/skills/slug/SKILL.md"))` entry to ship another.
pub const BUILTIN_SKILLS: &[(&str, &str)] = &[];

fn state_key(slug: &str) -> String {
    format!("skill.builtin.{slug}")
}

fn hash(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, PartialEq, Eq)]
enum Seed {
    /// Write the shipped copy and remember it.
    Write,
    /// Leave the file as it is.
    Keep,
}

/// What to do with one built-in skill, from the copy on disk (if any), the hash of what vizier
/// last wrote (if it ever did) and the shipped text.
fn decide(on_disk: Option<&str>, last_written: Option<&str>, shipped: &str) -> Seed {
    match (on_disk, last_written) {
        // Never seeded, nothing there: a new install, or a skill new in this version.
        (None, None) => Seed::Write,
        // Seeded once, then deleted by the operator.
        (None, Some(_)) => Seed::Keep,
        // A skill of the operator's own by the same name.
        (Some(_), None) => Seed::Keep,
        (Some(text), Some(written)) => {
            if text != shipped && hash(text) == written {
                Seed::Write
            } else {
                Seed::Keep
            }
        }
    }
}

/// Writes or updates the built-in skills under `{workspace}/skills/`. Never fails startup: a
/// skill that cannot be written is logged and tried again on the next start.
pub async fn seed_builtin_skills(workspace: &str, storage: &VizierStorage) {
    for (slug, shipped) in BUILTIN_SKILLS {
        if let Err(e) = seed_one(workspace, storage, slug, shipped).await {
            tracing::warn!("failed to install the built-in skill '{slug}': {e}");
        }
    }
}

async fn seed_one(
    workspace: &str,
    storage: &VizierStorage,
    slug: &str,
    shipped: &str,
) -> anyhow::Result<()> {
    let dir = build_path(workspace, &["skills", slug]);
    let path = dir.join("SKILL.md");
    let on_disk = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => anyhow::bail!("failed to read {}: {e}", path.display()),
    };
    let last_written = storage
        .get_state(state_key(slug))
        .await?
        .and_then(|value| value.as_str().map(str::to_string));

    if decide(on_disk.as_deref(), last_written.as_deref(), shipped) == Seed::Keep {
        return Ok(());
    }

    std::fs::create_dir_all(&dir)
        .map_err(|e| anyhow::anyhow!("failed to create {}: {e}", dir.display()))?;
    std::fs::write(&path, shipped)
        .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))?;
    storage
        .save_state(state_key(slug), serde_json::Value::String(hash(shipped)))
        .await?;
    tracing::info!("installed the built-in skill '{slug}'");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::SkillFrontMatter;

    #[test]
    fn a_fresh_install_gets_the_skill() {
        assert_eq!(decide(None, None, "v1"), Seed::Write);
    }

    #[test]
    fn an_untouched_copy_is_updated() {
        assert_eq!(decide(Some("v1"), Some(&hash("v1")), "v2"), Seed::Write);
        assert_eq!(decide(Some("v2"), Some(&hash("v2")), "v2"), Seed::Keep);
    }

    #[test]
    fn the_operators_changes_are_kept() {
        assert_eq!(decide(Some("edited"), Some(&hash("v1")), "v2"), Seed::Keep, "edited");
        assert_eq!(decide(None, Some(&hash("v1")), "v2"), Seed::Keep, "deleted");
        assert_eq!(decide(Some("theirs"), None, "v1"), Seed::Keep, "their own skill");
    }

    #[test]
    fn every_builtin_skill_parses_and_is_named_after_its_slug() {
        for (slug, text) in BUILTIN_SKILLS {
            let (frontmatter, body) =
                crate::utils::markdown::parse_markdown_str::<SkillFrontMatter>(text).expect(slug);
            assert_eq!(frontmatter.name, *slug);
            assert!(!frontmatter.description.is_empty(), "{slug}");
            assert!(!body.trim().is_empty(), "{slug}");
        }
    }
}
