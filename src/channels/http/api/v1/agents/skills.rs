use axum::{
    Extension, Router, Json,
    extract::{Path, Query, State},
    routing::{get, post, put, delete},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::response::{api_response, APIResponse, Response, err_response},
        state::HTTPState,
    },
    schema::Skill,
    skill::{SkillManager, invocation},
    storage::agent::AgentStorage,
};

use super::user_can_view_agent;
use super::super::skill_support::{self, SkillListing};
use crate::channels::http::models::page::{self, ListQuery};

#[derive(Debug, Deserialize, Default)]
pub struct ScopeQuery {
    /// `all` merges the global skills into the list.
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
    pub content: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSkillRequest {
    pub description: Option<String>,
    pub content: Option<String>,
    pub keywords: Option<Vec<String>>,
    /// `base_hash` of the content this edit started from; a mismatch is `409 stale_content`.
    pub base_hash: Option<String>,
}

pub use super::super::skill_support::SkillResponse;

/// A skill a person can invoke by name on the agent (spec 021): no content.
#[derive(Debug, Serialize, Clone)]
pub struct AvailableSkill {
    pub name: String,
    pub description: String,
}

pub fn agent_skills() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_agent_skills).post(create_agent_skill))
        .route("/available", get(list_available_skills))
        .route("/{slug}", get(get_agent_skill).put(update_agent_skill).delete(delete_agent_skill))
}

async fn list_agent_skills(
    State(state): State<HTTPState>,
    Path(agent_id): Path<String>,
    Query(scope): Query<ScopeQuery>,
    Query(query): Query<ListQuery>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> Response<serde_json::Value> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let workspace = &state.config.workspace;
    let manager = SkillManager::for_agent(workspace, &agent_id);

    let skills = match manager.list_skills() {
        Ok(skills) => skills,
        Err(_) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list skills".into()),
    };
    let mut responses: Vec<SkillResponse> = skills.into_iter().map(|s| skill_support::listed(&manager, s)).collect();
    if scope.scope.as_deref() == Some("all") {
        let global = SkillManager::new(workspace);
        if let Ok(skills) = global.list_skills() {
            responses.extend(skills.into_iter().map(|s| skill_support::listed(&global, s)));
        }
    }

    page::respond(responses, &SkillListing, &query)
}

/// Every skill the agent can use, its own and the global ones, for the composer's `/`
/// suggestions. Unlike `GET /`, which lists only the agent's own skills.
async fn list_available_skills(
    State(state): State<HTTPState>,
    Path(agent_id): Path<String>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> Response<Vec<AvailableSkill>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let workspace = &state.config.workspace;
    let skills = match invocation::available(
        &SkillManager::for_agent(workspace, &agent_id),
        &SkillManager::new(workspace),
    ) {
        Ok(skills) => skills,
        Err(_) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list skills".into()),
    };
    let available = skills
        .into_iter()
        .map(|skill| AvailableSkill { name: skill.name, description: skill.description })
        .collect();

    api_response(StatusCode::OK, available)
}

async fn create_agent_skill(
    State(state): State<HTTPState>,
    Path(agent_id): Path<String>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(request): Json<CreateSkillRequest>,
) -> Response<SkillResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let workspace = &state.config.workspace;
    let manager = SkillManager::for_agent(workspace, &agent_id);

    let skill = Skill {
        name: request.name,
        agent_id: Some(agent_id),
        author: "api".to_string(),
        description: request.description,
        content: request.content,
        keywords: request.keywords,
        version: 1,
        resources: Vec::new(),
    };

    if let Err(_) = manager.save_skill(&skill) {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to save skill".into());
    }

    let response = SkillResponse::from(skill);
    api_response(StatusCode::CREATED, response)
}

async fn get_agent_skill(
    State(state): State<HTTPState>,
    Path((agent_id, slug)): Path<(String, String)>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> Response<SkillResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let workspace = &state.config.workspace;
    let manager = SkillManager::for_agent(workspace, &agent_id);

    let skill = match manager.get_skill(&slug) {
        Ok(skill) => skill,
        Err(_) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get skill".into()),
    };

    match skill {
        Some(skill) => {
            let mut response = SkillResponse::from(skill.clone());
            if let Ok(resources) = manager.list_resources(&slug) {
                response.resources = resources.into_iter().map(|p| p.to_string_lossy().to_string()).collect();
            }
            api_response(StatusCode::OK, response)
        }
        None => err_response(StatusCode::NOT_FOUND, "Skill not found".into()),
    }
}

async fn update_agent_skill(
    State(state): State<HTTPState>,
    Path((agent_id, slug)): Path<(String, String)>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(request): Json<UpdateSkillRequest>,
) -> Response<SkillResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let workspace = &state.config.workspace;
    let manager = SkillManager::for_agent(workspace, &agent_id);

    let mut skill = match manager.get_skill(&slug) {
        Ok(skill) => skill,
        Err(_) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get skill".into()),
    };

    if let Some(mut skill) = skill.take() {
        if skill_support::is_stale(request.base_hash.as_deref(), &skill.content) {
            return super::super::skills::stale_content(skill);
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

        if let Err(_) = manager.save_skill(&skill) {
            return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to save skill".into());
        }

        let response = SkillResponse::from(skill);
        api_response(StatusCode::OK, response)
    } else {
        err_response(StatusCode::NOT_FOUND, "Skill not found".into())
    }
}

async fn delete_agent_skill(
    State(state): State<HTTPState>,
    Path((agent_id, slug)): Path<(String, String)>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> Response<String> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "Agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let workspace = &state.config.workspace;
    let manager = SkillManager::for_agent(workspace, &agent_id);

    let deleted = match manager.delete_skill(&slug) {
        Ok(deleted) => deleted,
        Err(_) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete skill".into()),
    };

    if deleted {
        api_response(StatusCode::OK, format!("Skill '{}' deleted", slug))
    } else {
        err_response(StatusCode::NOT_FOUND, "Skill not found".into())
    }
}
