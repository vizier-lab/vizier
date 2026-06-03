use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json,
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response, APIResponse},
        },
        state::HTTPState,
    },
    schema::{MemoryGraph, MemoryQueryParams, MemoryVisibility},
    storage::{agent::AgentStorage, memory::MemoryStorage},
};

use super::user_can_view_agent;

pub fn memory() -> Router<HTTPState> {
    Router::new()
        .route("/", get(get_all_memories))
        .route("/", post(create_memory))
        .route("/query", get(query_memories))
        .route("/graph", get(get_memory_graph))
        .route("/{slug}", get(get_memory_detail))
        .route("/{slug}", put(update_memory))
        .route("/{slug}", delete(delete_memory))
        .route("/{slug}/related", get(get_related_memories))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateMemoryRequest {
    title: String,
    content: String,
    slug: Option<String>,
    #[serde(default = "default_visibility")]
    visibility: String,
    #[serde(default)]
    shared_to: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

fn default_visibility() -> String {
    "private".to_string()
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateMemoryRequest {
    title: String,
    content: String,
    #[serde(default = "default_visibility")]
    visibility: String,
    #[serde(default)]
    shared_to: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct QueryMemoryRequest {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_threshold")]
    threshold: f64,
}

fn default_limit() -> usize {
    10
}

fn default_threshold() -> f64 {
    0.5
}

#[derive(Debug, Deserialize)]
pub struct ListMemoryParams {
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default = "default_list_offset")]
    pub offset: usize,
    #[serde(default = "default_list_limit")]
    pub limit: usize,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
}

fn default_list_offset() -> usize {
    0
}

fn default_list_limit() -> usize {
    50
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MemorySummary {
    pub agent_id: String,
    pub slug: String,
    pub title: String,
    pub timestamp: DateTime<Utc>,
    pub visibility: String,
    pub shared_to: Vec<String>,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MemoryDetail {
    pub agent_id: String,
    pub slug: String,
    pub title: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub visibility: String,
    pub shared_to: Vec<String>,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CreateMemoryResponse {
    pub agent_id: String,
    pub title: String,
    pub slug: String,
    pub message: String,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct UpdateMemoryResponse {
    pub agent_id: String,
    pub slug: String,
    pub message: String,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct PaginatedMemoryResponse {
    pub memories: Vec<MemorySummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "List of memories", body = APIResponse<Vec<MemorySummary>>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn get_all_memories(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Query(params): Query<ListMemoryParams>,
) -> models::response::Response<PaginatedMemoryResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let tags = params.tags.map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    });

    let visibility = params.visibility.and_then(|v| v.parse().ok());

    let query_params = MemoryQueryParams {
        agent_id: agent_id.clone(),
        tags,
        visibility,
        offset: params.offset,
        limit: params.limit,
        sort_by: params.sort_by,
        sort_order: params.sort_order,
    };

    match state.storage.get_filtered_memories(query_params).await {
        Ok(result) => {
            let memories: Vec<MemorySummary> = result
                .memories
                .iter()
                .map(|memory| MemorySummary {
                    agent_id: memory.agent_id.clone(),
                    slug: memory.slug.clone(),
                    title: memory.title.clone(),
                    timestamp: memory.timestamp,
                    visibility: memory.visibility.to_string(),
                    shared_to: memory.shared_to.clone(),
                    tags: memory.tags.clone(),
                    relations: memory.relations.clone(),
                })
                .collect();

            api_response(
                StatusCode::OK,
                PaginatedMemoryResponse {
                    memories,
                    total: result.total,
                    offset: result.offset,
                    limit: result.limit,
                },
            )
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/memory",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = CreateMemoryRequest,
    responses(
        (status = 201, description = "Memory created", body = APIResponse<CreateMemoryResponse>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn create_memory(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<CreateMemoryRequest>,
) -> models::response::Response<CreateMemoryResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let visibility: MemoryVisibility = match body.visibility.parse() {
        Ok(v) => v,
        Err(e) => return err_response(StatusCode::BAD_REQUEST, e),
    };

    match state
        .storage
        .write_memory(
            agent_id.clone(),
            body.slug,
            body.title.clone(),
            body.content,
            visibility,
            body.shared_to,
            body.tags.clone(),
        )
        .await
    {
        Ok(memory) => api_response(
            StatusCode::CREATED,
            CreateMemoryResponse {
                agent_id,
                title: body.title,
                slug: memory.slug,
                message: "memory created successfully".to_string(),
                tags: memory.tags,
                relations: memory.relations,
            },
        ),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/memory/{slug}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Memory slug")
    ),
    request_body = UpdateMemoryRequest,
    responses(
        (status = 200, description = "Memory updated", body = APIResponse<UpdateMemoryResponse>),
        (status = 404, description = "Agent or memory not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn update_memory(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<UpdateMemoryRequest>,
) -> models::response::Response<UpdateMemoryResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state.storage.get_memory_detail(agent_id.clone(), slug.clone()).await {
        Ok(None) => {
            return err_response(StatusCode::NOT_FOUND, format!("memory {slug} not found"));
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        _ => {}
    }

    let visibility: MemoryVisibility = match body.visibility.parse() {
        Ok(v) => v,
        Err(e) => return err_response(StatusCode::BAD_REQUEST, e),
    };

    match state
        .storage
        .write_memory(
            agent_id.clone(),
            Some(slug.clone()),
            body.title,
            body.content,
            visibility,
            body.shared_to,
            body.tags.clone(),
        )
        .await
    {
        Ok(memory) => api_response(
            StatusCode::OK,
            UpdateMemoryResponse {
                agent_id,
                slug,
                message: "memory updated successfully".to_string(),
                tags: memory.tags,
                relations: memory.relations,
            },
        ),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory/query",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = QueryMemoryRequest,
    responses(
        (status = 200, description = "Query results", body = APIResponse<Vec<MemoryDetail>>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn query_memories(
    Path(agent_id): Path<String>,
    Query(params): Query<QueryMemoryRequest>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<MemoryDetail>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state
        .storage
        .query_memory(agent_id, params.query, params.limit, params.threshold)
        .await
    {
        Ok(memories) => {
            let response: Vec<MemoryDetail> = memories
                .iter()
                .map(|memory| MemoryDetail {
                    agent_id: memory.agent_id.clone(),
                    slug: memory.slug.clone(),
                    title: memory.title.clone(),
                    content: memory.content.clone(),
                    timestamp: memory.timestamp,
                    visibility: memory.visibility.to_string(),
                    shared_to: memory.shared_to.clone(),
                    tags: memory.tags.clone(),
                    relations: memory.relations.clone(),
                })
                .collect();

            api_response(StatusCode::OK, response)
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory/graph",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Memory graph", body = APIResponse<MemoryGraph>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn get_memory_graph(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<MemoryGraph> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state.storage.get_memory_graph(agent_id).await {
        Ok(graph) => api_response(StatusCode::OK, graph),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory/{slug}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Memory slug")
    ),
    responses(
        (status = 200, description = "Memory details", body = APIResponse<MemoryDetail>),
        (status = 404, description = "Agent or memory not found", body = APIResponse<String>)
    )
)]
pub async fn get_memory_detail(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<MemoryDetail> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state.storage.get_memory_detail(agent_id, slug).await {
        Ok(Some(memory)) => api_response(
            StatusCode::OK,
            MemoryDetail {
                agent_id: memory.agent_id,
                slug: memory.slug,
                title: memory.title,
                content: memory.content,
                timestamp: memory.timestamp,
                visibility: memory.visibility.to_string(),
                shared_to: memory.shared_to,
                tags: memory.tags,
                relations: memory.relations,
            },
        ),
        _ => err_response(StatusCode::NOT_FOUND, "Not Found".into()),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory/{slug}/related",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Memory slug")
    ),
    responses(
        (status = 200, description = "Related memories", body = APIResponse<Vec<MemoryDetail>>),
        (status = 404, description = "Agent or memory not found", body = APIResponse<String>)
    )
)]
pub async fn get_related_memories(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<MemoryDetail>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state.storage.get_related_memories(agent_id, slug).await {
        Ok(memories) => {
            let response: Vec<MemoryDetail> = memories
                .iter()
                .map(|memory| MemoryDetail {
                    agent_id: memory.agent_id.clone(),
                    slug: memory.slug.clone(),
                    title: memory.title.clone(),
                    content: memory.content.clone(),
                    timestamp: memory.timestamp,
                    visibility: memory.visibility.to_string(),
                    shared_to: memory.shared_to.clone(),
                    tags: memory.tags.clone(),
                    relations: memory.relations.clone(),
                })
                .collect();

            api_response(StatusCode::OK, response)
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/memory/{slug}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Memory slug")
    ),
    responses(
        (status = 200, description = "Memory deleted", body = APIResponse<String>),
        (status = 404, description = "Agent or memory not found", body = APIResponse<String>)
    )
)]
pub async fn delete_memory(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<String> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state.storage.delete_memory(agent_id, slug.clone()).await {
        Ok(_) => api_response(StatusCode::OK, format!("{slug} deleted")),
        _ => err_response(StatusCode::NOT_FOUND, "Not Found".into()),
    }
}
