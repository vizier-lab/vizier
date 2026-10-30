//! Skills, global and per agent (spec 038 US2). The skill files live under the workspace; an
//! agent's own skills need the person to be able to view the agent.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ServiceError, Services};
use crate::{
    access::can_view_agent,
    auth::Principal,
    schema::{AgentConfig, Skill},
    skill::{SkillManager, invocation},
    storage::agent::AgentStorage,
};

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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

#[derive(Debug, Deserialize)]
pub struct CreateSkill {
    pub name: String,
    pub description: String,
    pub content: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSkill {
    pub description: Option<String>,
    pub content: Option<String>,
    pub keywords: Option<Vec<String>>,
    /// `base_hash` of the content this edit started from; a mismatch is `409 stale_content`.
    pub base_hash: Option<String>,
}

/// A skill a person can invoke by name on the agent (spec 021): no content.
#[derive(Debug, Serialize, Clone)]
pub struct AvailableSkill {
    pub name: String,
    pub description: String,
}

/// Why a skill update did not apply: an ordinary refusal, or the skill changed since the editor
/// loaded it (`409 stale_content`, carrying its current state).
#[derive(Debug)]
pub enum SkillError {
    Service(ServiceError),
    Stale(Box<SkillResponse>),
}

impl From<ServiceError> for SkillError {
    fn from(e: ServiceError) -> Self {
        Self::Service(e)
    }
}

fn failed(message: &str) -> ServiceError {
    ServiceError::Internal(message.into())
}

fn global(s: &Services) -> SkillManager {
    SkillManager::new(&s.deps().config.workspace)
}

fn for_agent(s: &Services, agent_id: &str) -> SkillManager {
    SkillManager::for_agent(&s.deps().config.workspace, agent_id)
}

/// The agent, when the person may view it.
async fn viewable(s: &Services, who: &Principal, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    let config = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound("Agent not found".into())),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    Ok(config)
}

fn get_with_resources(manager: &SkillManager, slug: &str) -> Result<SkillResponse, ServiceError> {
    let skill = manager.get_skill(slug).map_err(|_| failed("Failed to get skill"))?;
    match skill {
        Some(skill) => {
            let mut response = SkillResponse::from(skill);
            if let Ok(resources) = manager.list_resources(slug) {
                response.resources = resources.into_iter().map(|p| p.to_string_lossy().to_string()).collect();
            }
            Ok(response)
        }
        None => Err(ServiceError::NotFound("Skill not found".into())),
    }
}

fn create_in(manager: &SkillManager, agent_id: Option<String>, request: CreateSkill) -> Result<SkillResponse, ServiceError> {
    let skill = Skill {
        name: request.name,
        agent_id,
        author: "api".to_string(),
        description: request.description,
        content: request.content,
        keywords: request.keywords,
        version: 1,
        resources: Vec::new(),
    };
    manager.save_skill(&skill).map_err(|_| failed("Failed to save skill"))?;
    Ok(SkillResponse::from(skill))
}

fn update_in(manager: &SkillManager, slug: &str, request: UpdateSkill) -> Result<SkillResponse, SkillError> {
    let skill = manager.get_skill(slug).map_err(|_| failed("Failed to get skill"))?;
    let Some(mut skill) = skill else {
        return Err(ServiceError::NotFound("Skill not found".into()).into());
    };
    if is_stale(request.base_hash.as_deref(), &skill.content) {
        return Err(SkillError::Stale(Box::new(SkillResponse::from(skill))));
    }
    if let Some(description) = request.description {
        skill.description = description;
    }
    if let Some(content) = request.content {
        skill.content = content;
    }
    if let Some(keywords) = request.keywords {
        skill.keywords = keywords;
    }
    skill.version += 1;
    manager.save_skill(&skill).map_err(|_| failed("Failed to save skill"))?;
    Ok(SkillResponse::from(skill))
}

fn delete_in(manager: &SkillManager, slug: &str) -> Result<String, ServiceError> {
    let deleted = manager.delete_skill(slug).map_err(|_| failed("Failed to delete skill"))?;
    if deleted {
        Ok(format!("Skill '{}' deleted", slug))
    } else {
        Err(ServiceError::NotFound("Skill not found".into()))
    }
}

// Global skills -----------------------------------------------------------------------------

pub fn list_global(s: &Services) -> Result<Vec<SkillResponse>, ServiceError> {
    let manager = global(s);
    let skills = manager.list_skills().map_err(|_| failed("Failed to list skills"))?;
    Ok(skills.into_iter().map(|skill| listed(&manager, skill)).collect())
}

pub fn create_global(s: &Services, request: CreateSkill) -> Result<SkillResponse, ServiceError> {
    create_in(&global(s), None, request)
}

pub fn get_global(s: &Services, slug: &str) -> Result<SkillResponse, ServiceError> {
    get_with_resources(&global(s), slug)
}

pub fn update_global(s: &Services, slug: &str, request: UpdateSkill) -> Result<SkillResponse, SkillError> {
    update_in(&global(s), slug, request)
}

pub fn delete_global(s: &Services, slug: &str) -> Result<String, ServiceError> {
    delete_in(&global(s), slug)
}

pub fn list_resources(s: &Services, slug: &str) -> Result<Vec<String>, ServiceError> {
    let resources = global(s)
        .list_resources(slug)
        .map_err(|_| failed("Failed to list resources"))?;
    Ok(resources.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
}

pub fn read_resource(s: &Services, slug: &str, path: &str) -> Result<String, ServiceError> {
    let content = global(s)
        .read_resource(slug, path)
        .map_err(|_| failed("Failed to read resource"))?;
    content.ok_or_else(|| ServiceError::NotFound("Resource not found".into()))
}

// An agent's skills -------------------------------------------------------------------------

/// The agent's own skills, plus the global ones with `include_global`.
pub async fn list_agent(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    include_global: bool,
) -> Result<Vec<SkillResponse>, ServiceError> {
    viewable(s, who, agent_id).await?;
    let manager = for_agent(s, agent_id);
    let skills = manager.list_skills().map_err(|_| failed("Failed to list skills"))?;
    let mut responses: Vec<SkillResponse> = skills.into_iter().map(|skill| listed(&manager, skill)).collect();
    if include_global {
        let global = global(s);
        if let Ok(skills) = global.list_skills() {
            responses.extend(skills.into_iter().map(|skill| listed(&global, skill)));
        }
    }
    Ok(responses)
}

/// Every skill the agent can use, its own and the global ones, for the composer's `/` suggestions.
pub async fn available(s: &Services, who: &Principal, agent_id: &str) -> Result<Vec<AvailableSkill>, ServiceError> {
    viewable(s, who, agent_id).await?;
    let skills = invocation::available(&for_agent(s, agent_id), &global(s))
        .map_err(|_| failed("Failed to list skills"))?;
    Ok(skills
        .into_iter()
        .map(|skill| AvailableSkill { name: skill.name, description: skill.description })
        .collect())
}

pub async fn create_agent(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    request: CreateSkill,
) -> Result<SkillResponse, ServiceError> {
    viewable(s, who, agent_id).await?;
    create_in(&for_agent(s, agent_id), Some(agent_id.to_string()), request)
}

pub async fn get_agent(s: &Services, who: &Principal, agent_id: &str, slug: &str) -> Result<SkillResponse, ServiceError> {
    viewable(s, who, agent_id).await?;
    get_with_resources(&for_agent(s, agent_id), slug)
}

pub async fn update_agent(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
    request: UpdateSkill,
) -> Result<SkillResponse, SkillError> {
    viewable(s, who, agent_id).await?;
    update_in(&for_agent(s, agent_id), slug, request)
}

pub async fn delete_agent(s: &Services, who: &Principal, agent_id: &str, slug: &str) -> Result<String, ServiceError> {
    viewable(s, who, agent_id).await?;
    delete_in(&for_agent(s, agent_id), slug)
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
