use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use axum_extra::extract::Multipart;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    schema::{
        BundleSummary, ImportReport, Memory, MemoryGraph, MemoryQueryParams, VizierAttachment,
        default_bundle,
    },
    storage::agent::AgentStorage,
};

use super::user_can_view_agent;

pub fn memory() -> Router<HTTPState> {
    Router::new()
        .route("/", get(get_all_memories))
        .route("/", post(create_memory))
        .route("/query", get(query_memories))
        .route("/bundles", get(list_bundles))
        .route("/bundles/graph", get(get_bundle_level_graph))
        .route("/bundles/import", post(import_bundle_handler))
        .route("/bundles/{bundle}", delete(delete_bundle_handler))
        .route("/bundles/{bundle}/export", get(export_bundle_handler))
        .route("/{bundle}/graph", get(get_bundle_graph))
        .route("/{slug}", get(get_memory_detail))
        .route("/{slug}", put(update_memory))
        .route("/{slug}", delete(delete_memory))
        .route("/{slug}/related", get(get_related_memories))
        .route("/doc/{bundle}/{*path}", get(get_memory_detail_scoped))
        .route("/doc/{bundle}/{*path}", put(update_memory_scoped))
        .route("/doc/{bundle}/{*path}", delete(delete_memory_scoped))
        .route("/related/{bundle}/{*path}", get(get_related_memories_scoped))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateMemoryRequest {
    title: String,
    content: String,
    #[serde(default)]
    bundle: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    attachments: Option<Vec<VizierAttachment>>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateMemoryRequest {
    title: String,
    content: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    attachments: Option<Vec<VizierAttachment>>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct QueryMemoryRequest {
    query: String,
    #[serde(default)]
    bundle: Option<String>,
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
    pub bundle: Option<String>,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default = "default_list_offset")]
    pub offset: usize,
    #[serde(default = "default_list_limit")]
    pub limit: usize,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
}

#[derive(Debug, Deserialize, Default, utoipa::ToSchema)]
pub struct GraphQueryParams {
    #[serde(default)]
    pub search: Option<String>,
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
    pub bundle: String,
    pub path: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
    pub attachment_count: usize,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MemoryDetail {
    pub agent_id: String,
    pub bundle: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
    pub attachments: Vec<VizierAttachment>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CreateMemoryResponse {
    pub agent_id: String,
    pub bundle: String,
    pub title: String,
    pub path: String,
    pub message: String,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct UpdateMemoryResponse {
    pub agent_id: String,
    pub bundle: String,
    pub path: String,
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

fn summarize_memory(memory: &Memory) -> MemorySummary {
    MemorySummary {
        agent_id: memory.agent_id.clone(),
        bundle: memory.bundle.clone(),
        path: memory.slug.clone(),
        title: memory.title.clone(),
        created_at: memory.created_at,
        updated_at: memory.updated_at,
        tags: memory.tags.clone(),
        relations: memory.relations.clone(),
        attachment_count: memory.attachment_count,
    }
}

fn detail_from_memory(memory: &Memory) -> MemoryDetail {
    MemoryDetail {
        agent_id: memory.agent_id.clone(),
        bundle: memory.bundle.clone(),
        path: memory.slug.clone(),
        title: memory.title.clone(),
        content: memory.content.clone(),
        created_at: memory.created_at,
        updated_at: memory.updated_at,
        tags: memory.tags.clone(),
        relations: memory.relations.clone(),
        attachments: memory.attachments.clone(),
    }
}

async fn require_agent(
    state: &HTTPState,
    agent_id: &str,
    user: &crate::channels::http::auth::AuthenticatedUser,
) -> Result<crate::schema::AgentConfig, (StatusCode, String)> {
    let config = match state.storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                format!("agent {agent_id} not found"),
            ))
        }
        Err(e) => {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    };
    if !super::user_can_view_agent(user, &config) {
        return Err((StatusCode::FORBIDDEN, "Access denied".into()));
    }
    Ok(config)
}

fn error_status_for(message: &str) -> StatusCode {
    let lower = message.to_lowercase();
    if lower.contains("already exists") {
        StatusCode::CONFLICT
    } else if lower.contains("malformed") || lower.contains("archive") {
        StatusCode::BAD_REQUEST
    } else if lower.contains("does not exist") || lower.contains("not found") {
        StatusCode::NOT_FOUND
    } else if lower.contains("linked in the knowledge graph") || lower.contains("still has") {
        StatusCode::CONFLICT
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("bundle" = Option<String>, Query, description = "Narrow to one bundle; omitted lists across all bundles")
    ),
    responses(
        (status = 200, description = "List of memories", body = APIResponse<PaginatedMemoryResponse>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn get_all_memories(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Query(params): Query<ListMemoryParams>,
) -> models::response::Response<PaginatedMemoryResponse> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }

    let tags = params.tags.map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    });

    let query_params = MemoryQueryParams {
        agent_id: agent_id.clone(),
        bundle: params.bundle,
        tags,
        offset: params.offset,
        limit: params.limit,
        sort_by: params.sort_by,
        sort_order: params.sort_order,
    };

    match state
        .transport
        .send_memory_op(&agent_id, crate::schema::MemoryOpRequest::List { params: query_params })
        .await
    {
        Ok(crate::schema::MemoryOpResponse::Paginated(result)) => {
            let memories: Vec<MemorySummary> =
                result.memories.iter().map(summarize_memory).collect();

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
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
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
        (status = 409, description = "A memory already exists at that bundle/path", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn create_memory(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<CreateMemoryRequest>,
) -> models::response::Response<CreateMemoryResponse> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }

    match state
        .transport
        .send_memory_op(
            &agent_id,
            crate::schema::MemoryOpRequest::Write {
                bundle: body.bundle.clone(),
                path: body.path,
                create_only: true,
                title: body.title.clone(),
                content: body.content,
                tags: body.tags.clone(),
                attachments: body.attachments.unwrap_or_default(),
            },
        )
        .await
    {
        Ok(crate::schema::MemoryOpResponse::Memory(memory)) => api_response(
            StatusCode::CREATED,
            CreateMemoryResponse {
                agent_id,
                bundle: memory.bundle,
                title: body.title,
                path: memory.slug,
                message: "memory created successfully".to_string(),
                tags: memory.tags,
                relations: memory.relations,
            },
        ),
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => {
            let msg = e.to_string();
            err_response(error_status_for(&msg), msg)
        }
    }
}

async fn do_update_memory(
    state: &HTTPState,
    agent_id: String,
    bundle: Option<String>,
    path: String,
    body: UpdateMemoryRequest,
) -> models::response::Response<UpdateMemoryResponse> {
    match state
        .transport
        .send_memory_op(
            &agent_id,
            crate::schema::MemoryOpRequest::Write {
                bundle,
                path: Some(path.clone()),
                create_only: false,
                title: body.title,
                content: body.content,
                tags: body.tags.clone(),
                attachments: body.attachments.unwrap_or_default(),
            },
        )
        .await
    {
        Ok(crate::schema::MemoryOpResponse::Memory(memory)) => api_response(
            StatusCode::OK,
            UpdateMemoryResponse {
                agent_id,
                bundle: memory.bundle,
                path: memory.slug,
                message: "memory updated successfully".to_string(),
                tags: memory.tags,
                relations: memory.relations,
            },
        ),
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => {
            let msg = e.to_string();
            err_response(error_status_for(&msg), msg)
        }
    }
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/memory/{slug}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Memory path within the default bundle")
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
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    do_update_memory(&state, agent_id, None, slug, body).await
}

pub async fn update_memory_scoped(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Json(body): Json<UpdateMemoryRequest>,
) -> models::response::Response<UpdateMemoryResponse> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    do_update_memory(&state, agent_id, Some(bundle), path, body).await
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
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }

    match state
        .transport
        .send_memory_op(
            &agent_id,
            crate::schema::MemoryOpRequest::Query {
                bundle: params.bundle,
                query: params.query,
                limit: params.limit,
                threshold: params.threshold,
            },
        )
        .await
    {
        Ok(crate::schema::MemoryOpResponse::MemoryList(memories)) => {
            let response: Vec<MemoryDetail> = memories.iter().map(detail_from_memory).collect();
            api_response(StatusCode::OK, response)
        }
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn do_get_graph(
    state: &HTTPState,
    agent_id: String,
    bundle: Option<String>,
    search: Option<String>,
) -> models::response::Response<MemoryGraph> {
    match state
        .transport
        .send_memory_op(&agent_id, crate::schema::MemoryOpRequest::GetGraph { bundle, search })
        .await
    {
        Ok(crate::schema::MemoryOpResponse::Graph(graph)) => api_response(StatusCode::OK, graph),
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => {
            let msg = e.to_string();
            err_response(error_status_for(&msg), msg)
        }
    }
}

pub async fn get_bundle_level_graph(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Query(params): Query<GraphQueryParams>,
) -> models::response::Response<MemoryGraph> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    let search = params.search.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    do_get_graph(&state, agent_id, None, search).await
}

pub async fn get_bundle_graph(
    Path((agent_id, bundle)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    Query(params): Query<GraphQueryParams>,
) -> models::response::Response<MemoryGraph> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    let search = params.search.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    do_get_graph(&state, agent_id, Some(bundle), search).await
}

async fn do_get_memory_detail(
    state: &HTTPState,
    agent_id: String,
    bundle: Option<String>,
    path: String,
) -> models::response::Response<MemoryDetail> {
    match state
        .transport
        .send_memory_op(&agent_id, crate::schema::MemoryOpRequest::GetById { bundle, path })
        .await
    {
        Ok(crate::schema::MemoryOpResponse::MemoryOption(Some(memory))) => {
            api_response(StatusCode::OK, detail_from_memory(&memory))
        }
        Ok(crate::schema::MemoryOpResponse::MemoryOption(None)) => {
            err_response(StatusCode::NOT_FOUND, "Not Found".into())
        }
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory/{slug}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Memory path within the default bundle")
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
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    do_get_memory_detail(&state, agent_id, None, slug).await
}

pub async fn get_memory_detail_scoped(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<MemoryDetail> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    do_get_memory_detail(&state, agent_id, Some(bundle), path).await
}

async fn do_get_related_memories(
    state: &HTTPState,
    agent_id: String,
    bundle: Option<String>,
    path: String,
) -> models::response::Response<Vec<MemoryDetail>> {
    match state
        .transport
        .send_memory_op(&agent_id, crate::schema::MemoryOpRequest::GetRelated { bundle, path })
        .await
    {
        Ok(crate::schema::MemoryOpResponse::MemoryList(memories)) => {
            let response: Vec<MemoryDetail> = memories.iter().map(detail_from_memory).collect();
            api_response(StatusCode::OK, response)
        }
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn get_related_memories(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<MemoryDetail>> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    do_get_related_memories(&state, agent_id, None, slug).await
}

pub async fn get_related_memories_scoped(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<MemoryDetail>> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    do_get_related_memories(&state, agent_id, Some(bundle), path).await
}

async fn do_delete_memory(
    state: &HTTPState,
    agent_id: String,
    bundle: Option<String>,
    path: String,
) -> models::response::Response<String> {
    match state
        .transport
        .send_memory_op(&agent_id, crate::schema::MemoryOpRequest::Delete { bundle, path: path.clone() })
        .await
    {
        Ok(crate::schema::MemoryOpResponse::Unit) => {
            api_response(StatusCode::OK, format!("{path} deleted"))
        }
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("linked in the knowledge graph") {
                err_response(StatusCode::CONFLICT, msg)
            } else {
                err_response(StatusCode::NOT_FOUND, msg)
            }
        }
    }
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/memory/{slug}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("slug" = String, Path, description = "Memory path within the default bundle")
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
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    do_delete_memory(&state, agent_id, None, slug).await
}

pub async fn delete_memory_scoped(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<String> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    do_delete_memory(&state, agent_id, Some(bundle), path).await
}

pub async fn list_bundles(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<BundleSummary>> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }

    match state
        .transport
        .send_memory_op(&agent_id, crate::schema::MemoryOpRequest::ListBundles)
        .await
    {
        Ok(crate::schema::MemoryOpResponse::Bundles(bundles)) => api_response(StatusCode::OK, bundles),
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn delete_bundle_handler(
    Path((agent_id, bundle)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<String> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }

    match state
        .transport
        .send_memory_op(
            &agent_id,
            crate::schema::MemoryOpRequest::DeleteBundle { bundle: bundle.clone() },
        )
        .await
    {
        Ok(crate::schema::MemoryOpResponse::Unit) => {
            api_response(StatusCode::OK, format!("bundle '{bundle}' deleted"))
        }
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => {
            let msg = e.to_string();
            err_response(error_status_for(&msg), msg)
        }
    }
}

pub async fn export_bundle_handler(
    Path((agent_id, bundle)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> impl IntoResponse {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return (status, message).into_response();
    }

    match state
        .transport
        .send_memory_op(&agent_id, crate::schema::MemoryOpRequest::ExportBundle { bundle: bundle.clone() })
        .await
    {
        Ok(crate::schema::MemoryOpResponse::Export(bytes)) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/zip".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{bundle}.zip\""),
                ),
            ],
            bytes,
        )
            .into_response(),
        Ok(_) => (StatusCode::INTERNAL_SERVER_ERROR, "unexpected response").into_response(),
        Err(e) => {
            let msg = e.to_string();
            (error_status_for(&msg), msg).into_response()
        }
    }
}

pub async fn import_bundle_handler(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    mut multipart: Multipart,
) -> models::response::Response<ImportReport> {
    if let Err((status, message)) = require_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }

    let mut bundle_name: Option<String> = None;
    let mut zip_bytes: Option<Vec<u8>> = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return err_response(StatusCode::BAD_REQUEST, e.to_string()),
        };
        match field.name().map(|s| s.to_string()) {
            Some(name) if name == "bundle" => {
                bundle_name = field.text().await.ok();
            }
            Some(name) if name == "file" || name == "zip" => {
                zip_bytes = field.bytes().await.ok().map(|b| b.to_vec());
            }
            _ => {}
        }
    }

    let bundle = bundle_name.unwrap_or_else(default_bundle);
    let Some(bytes) = zip_bytes else {
        return err_response(
            StatusCode::BAD_REQUEST,
            "multipart body must include a 'file' field with the .zip archive".into(),
        );
    };

    match state
        .transport
        .send_memory_op(&agent_id, crate::schema::MemoryOpRequest::ImportBundle { bundle, zip_bytes: bytes })
        .await
    {
        Ok(crate::schema::MemoryOpResponse::Import(report)) => api_response(StatusCode::OK, report),
        Ok(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected response".into(),
        ),
        Err(e) => {
            let msg = e.to_string();
            err_response(error_status_for(&msg), msg)
        }
    }
}
