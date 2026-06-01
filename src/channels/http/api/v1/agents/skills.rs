use axum::{
    Router, Json,
    extract::{Path, State},
    routing::{get, post, put, delete},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::response::{api_response, APIResponse, Response, err_response},
        state::HTTPState,
    },
    schema::{Skill, SkillActivation},
    skill::SkillManager,
};

#[derive(Debug, Deserialize)]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
    pub content: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default = "default_activation")]
    pub activation: SkillActivation,
}

fn default_activation() -> SkillActivation {
    SkillActivation::OnDemand
}

#[derive(Debug, Deserialize)]
pub struct UpdateSkillRequest {
    pub description: Option<String>,
    pub content: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub activation: Option<SkillActivation>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SkillResponse {
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub activation: String,
    pub version: u32,
    pub resources: Vec<String>,
    pub content: String,
    pub agent_id: Option<String>,
}

impl From<Skill> for SkillResponse {
    fn from(skill: Skill) -> Self {
        Self {
            name: skill.name,
            description: skill.description,
            keywords: skill.keywords,
            activation: format!("{:?}", skill.activation),
            version: skill.version,
            resources: skill.resources,
            content: skill.content,
            agent_id: skill.agent_id,
        }
    }
}

pub fn agent_skills() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_agent_skills).post(create_agent_skill))
        .route("/{slug}", get(get_agent_skill).put(update_agent_skill).delete(delete_agent_skill))
}

async fn list_agent_skills(
    State(state): State<HTTPState>,
    Path(agent_id): Path<String>,
) -> Response<Vec<SkillResponse>> {
    let workspace = &state.config.workspace;
    let manager = SkillManager::for_agent(workspace, &agent_id);

    let skills = match manager.list_skills() {
        Ok(skills) => skills,
        Err(_) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list skills".into()),
    };
    let responses: Vec<SkillResponse> = skills.into_iter().map(SkillResponse::from).collect();

    api_response(StatusCode::OK, responses)
}

async fn create_agent_skill(
    State(state): State<HTTPState>,
    Path(agent_id): Path<String>,
    Json(request): Json<CreateSkillRequest>,
) -> Response<SkillResponse> {
    let workspace = &state.config.workspace;
    let manager = SkillManager::for_agent(workspace, &agent_id);

    let skill = Skill {
        name: request.name,
        agent_id: Some(agent_id),
        author: "api".to_string(),
        description: request.description,
        content: request.content,
        keywords: request.keywords,
        activation: request.activation,
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
) -> Response<SkillResponse> {
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
    Json(request): Json<UpdateSkillRequest>,
) -> Response<SkillResponse> {
    let workspace = &state.config.workspace;
    let manager = SkillManager::for_agent(workspace, &agent_id);

    let mut skill = match manager.get_skill(&slug) {
        Ok(skill) => skill,
        Err(_) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get skill".into()),
    };

    if let Some(mut skill) = skill.take() {
        if let Some(description) = request.description {
            skill.description = description;
        }
        if let Some(content) = request.content {
            skill.content = content;
        }
        if let Some(keywords) = request.keywords {
            skill.keywords = keywords;
        }
        if let Some(activation) = request.activation {
            skill.activation = activation;
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
) -> Response<String> {
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
