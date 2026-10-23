//! What the global and the per-agent skill routes share: the response, its listing rules
//! (paging, `agent`/`global` filters, `name`/`updated` sorts) and the stale-save check.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    channels::http::models::page::{Listing, SortKey, SortOrder},
    schema::Skill,
    skill::SkillManager,
};

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct SkillResponse {
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub version: u32,
    pub resources: Vec<String>,
    pub content: String,
    pub agent_id: Option<String>,
    /// `agent` or `global`.
    pub scope: &'static str,
    /// sha256 (hex) of `content`: send it back as `base_hash` to refuse overwriting a newer save.
    pub base_hash: String,
    /// When SKILL.md was last written. Only set by the list routes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<DateTime<Utc>>,
}

impl From<Skill> for SkillResponse {
    fn from(skill: Skill) -> Self {
        Self {
            base_hash: content_hash(&skill.content),
            scope: if skill.agent_id.is_some() { "agent" } else { "global" },
            name: skill.name,
            description: skill.description,
            keywords: skill.keywords,
            version: skill.version,
            resources: skill.resources,
            content: skill.content,
            agent_id: skill.agent_id,
            updated: None,
        }
    }
}

pub fn content_hash(content: &str) -> String {
    Sha256::digest(content.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether a save based on `base_hash` would overwrite something newer.
pub fn is_stale(base_hash: Option<&str>, current_content: &str) -> bool {
    base_hash.is_some_and(|h| h != content_hash(current_content))
}

/// A skill response with when its SKILL.md was last written, for the list routes.
pub fn listed(manager: &SkillManager, skill: Skill) -> SkillResponse {
    let updated = std::fs::metadata(manager.skill_dir(&skill.name).join("SKILL.md"))
        .and_then(|m| m.modified())
        .ok()
        .map(DateTime::<Utc>::from);
    SkillResponse { updated, ..SkillResponse::from(skill) }
}

/// Filters `agent` and `global`; sorts `name` and `updated` (newest first).
pub struct SkillListing;

impl Listing<SkillResponse> for SkillListing {
    fn matches(&self, item: &SkillResponse, q: &str) -> bool {
        item.name.to_lowercase().contains(q)
            || item.description.to_lowercase().contains(q)
            || item.keywords.iter().any(|k| k.to_lowercase().contains(q))
    }

    fn filter_keys(&self, _: &[SkillResponse]) -> Vec<String> {
        vec!["agent".into(), "global".into()]
    }

    fn in_filter(&self, filter: &str, item: &SkillResponse) -> bool {
        item.scope == filter
    }

    fn sort_key(&self, item: &SkillResponse, sort: &str) -> SortKey {
        match sort {
            "updated" => SortKey::Num(item.updated.map_or(0, |t| t.timestamp_millis())),
            _ => SortKey::text(&item.name),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["name", "updated"]
    }

    fn default_order(&self, sort: &str) -> SortOrder {
        if sort == "updated" { SortOrder::Desc } else { SortOrder::Asc }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_based_on_an_older_hash_is_stale() {
        let h = content_hash("one");
        assert!(!is_stale(Some(&h), "one"));
        assert!(is_stale(Some(&h), "two"));
        assert!(!is_stale(None, "anything"));
        assert_eq!(h.len(), 64);
    }
}
