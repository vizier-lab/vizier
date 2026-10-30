use axum::{
    Router, Json,
    extract::{Path, Query, State},
    routing::get,
};
use reqwest::StatusCode;

use crate::{
    api::v1::integrations::svc,
    models::response::{Response, api_response},
    state::HTTPState,
};
use super::skill_support::{SkillListing, refuse, saved};
use crate::models::page::{self, ListQuery};
use vizier_core::service::skills as service;

pub use service::{CreateSkill as CreateSkillRequest, UpdateSkill as UpdateSkillRequest};
pub use super::skill_support::{SkillResponse, stale_content};

pub fn skills() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_skills).post(create_skill))
        .route("/{slug}", get(get_skill).put(update_skill).delete(delete_skill))
        .route("/{slug}/resources", get(list_resources))
        .route("/{slug}/resources/{path}", get(read_resource))
}

async fn list_skills(
    State(state): State<HTTPState>,
    Query(query): Query<ListQuery>,
) -> Response<serde_json::Value> {
    match service::list_global(&svc(&state)) {
        Ok(responses) => page::respond(responses, &SkillListing, &query),
        Err(e) => refuse(e),
    }
}

async fn create_skill(
    State(state): State<HTTPState>,
    Json(request): Json<CreateSkillRequest>,
) -> Response<SkillResponse> {
    match service::create_global(&svc(&state), request) {
        Ok(skill) => api_response(StatusCode::CREATED, skill),
        Err(e) => refuse(e),
    }
}

async fn get_skill(
    State(state): State<HTTPState>,
    Path(slug): Path<String>,
) -> Response<SkillResponse> {
    match service::get_global(&svc(&state), &slug) {
        Ok(skill) => api_response(StatusCode::OK, skill),
        Err(e) => refuse(e),
    }
}

async fn update_skill(
    State(state): State<HTTPState>,
    Path(slug): Path<String>,
    Json(request): Json<UpdateSkillRequest>,
) -> Response<SkillResponse> {
    saved(service::update_global(&svc(&state), &slug, request))
}

async fn delete_skill(
    State(state): State<HTTPState>,
    Path(slug): Path<String>,
) -> Response<String> {
    match service::delete_global(&svc(&state), &slug) {
        Ok(message) => api_response(StatusCode::OK, message),
        Err(e) => refuse(e),
    }
}

async fn list_resources(
    State(state): State<HTTPState>,
    Path(slug): Path<String>,
) -> Response<Vec<String>> {
    match service::list_resources(&svc(&state), &slug) {
        Ok(paths) => api_response(StatusCode::OK, paths),
        Err(e) => refuse(e),
    }
}

async fn read_resource(
    State(state): State<HTTPState>,
    Path((slug, path)): Path<(String, String)>,
) -> Response<String> {
    match service::read_resource(&svc(&state), &slug, &path) {
        Ok(content) => api_response(StatusCode::OK, content),
        Err(e) => refuse(e),
    }
}

