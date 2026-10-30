use axum::{
    Extension, Router, Json,
    extract::{Path, Query, State},
    routing::get,
};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    api::v1::integrations::svc,
    models::response::{Response, api_response},
    state::HTTPState,
};
use vizier_core::service::skills as service;

use super::super::skill_support::{SkillListing, refuse, saved};
use crate::models::page::{self, ListQuery};

#[derive(Debug, Deserialize, Default)]
pub struct ScopeQuery {
    /// `all` merges the global skills into the list.
    pub scope: Option<String>,
}

pub use service::{AvailableSkill, CreateSkill as CreateSkillRequest, UpdateSkill as UpdateSkillRequest};
pub use super::super::skill_support::SkillResponse;

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
    Extension(user): Extension<crate::auth::Principal>,
) -> Response<serde_json::Value> {
    let include_global = scope.scope.as_deref() == Some("all");
    match service::list_agent(&svc(&state), &user, &agent_id, include_global).await {
        Ok(responses) => page::respond(responses, &SkillListing, &query),
        Err(e) => refuse(e),
    }
}

/// Every skill the agent can use, its own and the global ones, for the composer's `/`
/// suggestions. Unlike `GET /`, which lists only the agent's own skills.
async fn list_available_skills(
    State(state): State<HTTPState>,
    Path(agent_id): Path<String>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Response<Vec<AvailableSkill>> {
    match service::available(&svc(&state), &user, &agent_id).await {
        Ok(available) => api_response(StatusCode::OK, available),
        Err(e) => refuse(e),
    }
}

async fn create_agent_skill(
    State(state): State<HTTPState>,
    Path(agent_id): Path<String>,
    Extension(user): Extension<crate::auth::Principal>,
    Json(request): Json<CreateSkillRequest>,
) -> Response<SkillResponse> {
    match service::create_agent(&svc(&state), &user, &agent_id, request).await {
        Ok(skill) => api_response(StatusCode::CREATED, skill),
        Err(e) => refuse(e),
    }
}

async fn get_agent_skill(
    State(state): State<HTTPState>,
    Path((agent_id, slug)): Path<(String, String)>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Response<SkillResponse> {
    match service::get_agent(&svc(&state), &user, &agent_id, &slug).await {
        Ok(skill) => api_response(StatusCode::OK, skill),
        Err(e) => refuse(e),
    }
}

async fn update_agent_skill(
    State(state): State<HTTPState>,
    Path((agent_id, slug)): Path<(String, String)>,
    Extension(user): Extension<crate::auth::Principal>,
    Json(request): Json<UpdateSkillRequest>,
) -> Response<SkillResponse> {
    saved(service::update_agent(&svc(&state), &user, &agent_id, &slug, request).await)
}

async fn delete_agent_skill(
    State(state): State<HTTPState>,
    Path((agent_id, slug)): Path<(String, String)>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Response<String> {
    match service::delete_agent(&svc(&state), &user, &agent_id, &slug).await {
        Ok(message) => api_response(StatusCode::OK, message),
        Err(e) => refuse(e),
    }
}
