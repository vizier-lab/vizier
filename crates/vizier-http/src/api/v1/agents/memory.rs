use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use axum_extra::extract::Multipart;
use reqwest::StatusCode;

use crate::{
    error::ApiError,
    models::{
        page::{self, ListQuery, Listing, SortKey, SortOrder},
        response::{APIResponse, api_response, err_response},
    },
    state::HTTPState,
};
use vizier_core::{
    schema::{
        BundleSummary, ImportReport, MemoryGraph, MemoryPassageResult, RollbackResponse,
    },
    service::{self, Services},
};

pub use vizier_core::service::memory::{
    AttachArtifactRequest, CreateMemoryRequest, CreateMemoryResponse, DeleteBundleParams,
    DetachAttachmentQuery, GraphQueryParams, ListMemoryParams, MemoryDetail, MemoryHistoryQuery,
    MemoryHistoryResponse, MemoryRollbackRequest, MemorySummary, PaginatedMemoryResponse,
    QueryMemoryRequest, UpdateMemoryRequest, UpdateMemoryResponse,
};

type Reply<T> = Result<(StatusCode, Json<APIResponse<T>>), ApiError>;

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
        .route(
            "/history/{bundle}/{*path}",
            get(get_memory_history).post(rollback_memory),
        )
        .route("/{bundle}/graph", get(get_bundle_graph))
        .route("/{slug}", get(get_memory_detail))
        .route("/{slug}", put(update_memory))
        .route("/{slug}", delete(delete_memory))
        .route("/{slug}/related", get(get_related_memories))
        .route("/doc/{bundle}/{*path}", get(get_memory_detail_scoped))
        .route("/doc/{bundle}/{*path}", put(update_memory_scoped))
        .route("/doc/{bundle}/{*path}", delete(delete_memory_scoped))
        // Attach/detach live on their own prefix because axum requires a catch-all to be the
        // last segment, so `/doc/{bundle}/{*path}/attachments` cannot be a route.
        .route("/attachments/{bundle}/{*path}", post(attach_artifact))
        .route("/attachments/{bundle}/{*path}", delete(detach_attachment))
        .route(
            "/related/{bundle}/{*path}",
            get(get_related_memories_scoped),
        )
}

fn services(state: &HTTPState) -> Services {
    Services::new(state.deps.clone())
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/memory/attachments/{bundle}/{*path}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("bundle" = String, Path, description = "Bundle name"),
        ("path" = String, Path, description = "Memory path")
    ),
    request_body = AttachArtifactRequest,
    responses(
        (status = 200, description = "The memory with its updated attachment list", body = APIResponse<MemoryDetail>),
        (status = 403, description = "Memory or artifact visibility denied", body = APIResponse<String>),
        (status = 404, description = "Memory or artifact not found", body = APIResponse<String>)
    )
)]
pub async fn attach_artifact(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<AttachArtifactRequest>,
) -> Reply<MemoryDetail> {
    let detail =
        service::memory::attach_artifact(&services(&state), &user, &agent_id, bundle, &path, body).await?;
    Ok(api_response(StatusCode::OK, detail))
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/memory/attachments/{bundle}/{*path}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("bundle" = String, Path, description = "Bundle name"),
        ("path" = String, Path, description = "Memory path"),
        DetachAttachmentQuery
    ),
    responses(
        (status = 200, description = "The memory with its updated attachment list", body = APIResponse<MemoryDetail>),
        (status = 403, description = "Memory visibility denied", body = APIResponse<String>),
        (status = 404, description = "Memory not found", body = APIResponse<String>)
    )
)]
pub async fn detach_attachment(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    Query(query): Query<DetachAttachmentQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Reply<MemoryDetail> {
    let detail =
        service::memory::detach_attachment(&services(&state), &user, &agent_id, bundle, &path, query)
            .await?;
    Ok(api_response(StatusCode::OK, detail))
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
    Extension(user): Extension<crate::auth::Principal>,
    Query(params): Query<ListMemoryParams>,
) -> Reply<PaginatedMemoryResponse> {
    let list = service::memory::list(&services(&state), &user, &agent_id, params).await?;
    Ok(api_response(StatusCode::OK, list))
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
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<CreateMemoryRequest>,
) -> Reply<CreateMemoryResponse> {
    let created = service::memory::create(&services(&state), &user, &agent_id, body).await?;
    Ok(api_response(StatusCode::CREATED, created))
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
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<UpdateMemoryRequest>,
) -> Reply<UpdateMemoryResponse> {
    let updated = service::memory::update(&services(&state), &user, &agent_id, None, slug, body).await?;
    Ok(api_response(StatusCode::OK, updated))
}

pub async fn update_memory_scoped(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<UpdateMemoryRequest>,
) -> Reply<UpdateMemoryResponse> {
    let updated =
        service::memory::update(&services(&state), &user, &agent_id, Some(bundle), path, body).await?;
    Ok(api_response(StatusCode::OK, updated))
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory/query",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    request_body = QueryMemoryRequest,
    responses(
        (status = 200, description = "Matching passages", body = APIResponse<Vec<MemoryPassageResult>>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn query_memories(
    Path(agent_id): Path<String>,
    Query(params): Query<QueryMemoryRequest>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Reply<Vec<MemoryPassageResult>> {
    let passages = service::memory::query(&services(&state), &user, &agent_id, params).await?;
    Ok(api_response(StatusCode::OK, passages))
}

pub async fn get_bundle_level_graph(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Query(params): Query<GraphQueryParams>,
) -> Reply<MemoryGraph> {
    let graph = service::memory::graph(&services(&state), &user, &agent_id, None, params).await?;
    Ok(api_response(StatusCode::OK, graph))
}

pub async fn get_bundle_graph(
    Path((agent_id, bundle)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Query(params): Query<GraphQueryParams>,
) -> Reply<MemoryGraph> {
    let graph = service::memory::graph(&services(&state), &user, &agent_id, Some(bundle), params).await?;
    Ok(api_response(StatusCode::OK, graph))
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
    Extension(user): Extension<crate::auth::Principal>,
) -> Reply<MemoryDetail> {
    let detail = service::memory::detail(&services(&state), &user, &agent_id, None, slug).await?;
    Ok(api_response(StatusCode::OK, detail))
}

pub async fn get_memory_detail_scoped(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Reply<MemoryDetail> {
    let detail =
        service::memory::detail(&services(&state), &user, &agent_id, Some(bundle), path).await?;
    Ok(api_response(StatusCode::OK, detail))
}

pub async fn get_related_memories(
    Path((agent_id, slug)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Reply<Vec<MemoryDetail>> {
    let related = service::memory::related(&services(&state), &user, &agent_id, None, slug).await?;
    Ok(api_response(StatusCode::OK, related))
}

pub async fn get_related_memories_scoped(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Reply<Vec<MemoryDetail>> {
    let related =
        service::memory::related(&services(&state), &user, &agent_id, Some(bundle), path).await?;
    Ok(api_response(StatusCode::OK, related))
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
    Extension(user): Extension<crate::auth::Principal>,
) -> Reply<String> {
    let message = service::memory::delete(&services(&state), &user, &agent_id, None, slug).await?;
    Ok(api_response(StatusCode::OK, message))
}

pub async fn delete_memory_scoped(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Reply<String> {
    let message =
        service::memory::delete(&services(&state), &user, &agent_id, Some(bundle), path).await?;
    Ok(api_response(StatusCode::OK, message))
}

/// Search by name; sorts `documents` (most first), `updated` (newest first), `links` (most
/// first) and `name`. No filters.
struct BundleListing;

impl Listing<BundleSummary> for BundleListing {
    fn matches(&self, item: &BundleSummary, q: &str) -> bool {
        item.name.to_lowercase().contains(q)
    }

    fn filter_keys(&self, _: &[BundleSummary]) -> Vec<String> {
        Vec::new()
    }

    fn in_filter(&self, _: &str, _: &BundleSummary) -> bool {
        false
    }

    fn sort_key(&self, item: &BundleSummary, sort: &str) -> SortKey {
        match sort {
            "updated" => SortKey::Num(item.updated_at.map_or(0, |t| t.timestamp_millis())),
            "links" => SortKey::Num(item.link_count as i64),
            "name" => SortKey::text(&item.name),
            _ => SortKey::Num(item.concept_count as i64),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["documents", "updated", "links", "name"]
    }

    fn default_order(&self, sort: &str) -> SortOrder {
        if sort == "name" {
            SortOrder::Asc
        } else {
            SortOrder::Desc
        }
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory/bundles",
    params(("agent_id" = String, Path, description = "Agent ID"), ListQuery),
    responses(
        (status = 200, description = "Bundles; with `limit`, a page `{items, total, counts}` (sorts `documents`, `updated`, `links`, `name`)", body = APIResponse<Vec<BundleSummary>>)
    )
)]
pub async fn list_bundles(
    Path(agent_id): Path<String>,
    Query(query): Query<ListQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> Result<crate::models::response::Response<serde_json::Value>, ApiError> {
    let bundles = service::memory::list_bundles(&services(&state), &user, &agent_id).await?;
    Ok(page::respond(bundles, &BundleListing, &query))
}

pub async fn delete_bundle_handler(
    Path((agent_id, bundle)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Query(params): Query<DeleteBundleParams>,
) -> Reply<String> {
    let message =
        service::memory::delete_bundle(&services(&state), &user, &agent_id, bundle, params).await?;
    Ok(api_response(StatusCode::OK, message))
}

pub async fn export_bundle_handler(
    Path((agent_id, bundle)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> impl IntoResponse {
    match service::memory::export_bundle(&services(&state), &user, &agent_id, bundle).await {
        Ok(export) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/zip".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", export.filename),
                ),
            ],
            export.bytes,
        )
            .into_response(),
        // The export route has always answered failures as plain text, not the JSON envelope.
        Err(e) => (
            StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            e.message().to_string(),
        )
            .into_response(),
    }
}

pub async fn import_bundle_handler(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    mut multipart: Multipart,
) -> Reply<ImportReport> {
    let s = services(&state);
    // Refused before the upload is read.
    service::memory::authorize(&s, &user, &agent_id).await?;

    let mut bundle_name: Option<String> = None;
    let mut zip_bytes: Option<Vec<u8>> = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return Ok(err_response(StatusCode::BAD_REQUEST, e.to_string())),
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

    let Some(bytes) = zip_bytes else {
        return Ok(err_response(
            StatusCode::BAD_REQUEST,
            "multipart body must include a 'file' field with the .zip archive".into(),
        ));
    };

    let report = service::memory::import_bundle(&s, &user, &agent_id, bundle_name, bytes).await?;
    Ok(api_response(StatusCode::OK, report))
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/memory/history/{bundle}/{path}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("bundle" = String, Path, description = "Bundle name"),
        ("path" = String, Path, description = "Concept path within the bundle (may contain `/`)"),
        MemoryHistoryQuery
    ),
    responses(
        (status = 200, description = "Paginated list (no `seq`/`to`), one version (`seq`), or a line diff (`to`, optional `from`; `from` defaults to `to - 1`)", body = APIResponse<MemoryHistoryResponse>),
        (status = 404, description = "Agent, memory, or version not found", body = APIResponse<String>)
    )
)]
pub async fn get_memory_history(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Query(q): Query<MemoryHistoryQuery>,
) -> Reply<MemoryHistoryResponse> {
    let history =
        service::memory::history(&services(&state), &user, &agent_id, bundle, &path, q).await?;
    Ok(api_response(StatusCode::OK, history))
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/memory/history/{bundle}/{path}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("bundle" = String, Path, description = "Bundle name"),
        ("path" = String, Path, description = "Concept path within the bundle (may contain `/`)")
    ),
    request_body = MemoryRollbackRequest,
    responses(
        (status = 200, description = "Version restored as a new revision (or `no_change` when already identical); restoring a content version of a deleted memory recreates it", body = APIResponse<RollbackResponse>),
        (status = 400, description = "The target version is a deletion entry", body = APIResponse<String>),
        (status = 404, description = "Agent, memory, or version not found", body = APIResponse<String>)
    )
)]
pub async fn rollback_memory(
    Path((agent_id, bundle, path)): Path<(String, String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<MemoryRollbackRequest>,
) -> Reply<RollbackResponse> {
    let res =
        service::memory::rollback(&services(&state), &user, &agent_id, bundle, &path, body).await?;
    Ok(api_response(StatusCode::OK, res))
}
