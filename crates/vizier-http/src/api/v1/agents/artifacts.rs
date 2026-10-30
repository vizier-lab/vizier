//! The artifact gallery API (`specs/035-add-artifacts`, contracts/artifacts-api.md).
//!
//! Reads (list/detail/content/activity) need artifact **view**; delete needs **manage**. The
//! checks are `vizier_core::service::artifacts`'; this module extracts, shapes responses and
//! answers `Range` requests (`range.rs`).

use axum::{
    Extension, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use serde::Deserialize;

use vizier_core::{
    schema::ArtifactDeleteResult,
    service::{ServiceError, artifacts as service},
};
use crate::{
    api::v1::{
        integrations::svc,
        range::{RangeRequest, parse_range},
    },
    models::{
        self,
        response::{APIResponse, api_response, err_response},
    },
    state::HTTPState,
};

pub use service::{
    ArtifactActivityView, ArtifactDetailView, ArtifactEventView, ArtifactItemView, ArtifactListView,
    ArtifactReferenceView,
};

pub fn artifacts() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_artifacts))
        .route("/activity", get(activity))
        .route("/{artifact_id}", get(artifact_detail))
        .route("/{artifact_id}", delete(delete_artifact))
        .route("/{artifact_id}/content", get(artifact_content))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ArtifactListQuery {
    pub q: Option<String>,
    pub kind: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ArtifactActivityQuery {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

/// The service's refusal in this API's plain error shape.
fn refuse<T: serde::Serialize + Clone>(e: ServiceError) -> models::response::Response<T> {
    err_response(
        StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        e.message().to_string(),
    )
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/artifacts",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ArtifactListQuery
    ),
    responses(
        (status = 200, description = "One page of artifacts, newest change first", body = APIResponse<ArtifactListView>),
        (status = 400, description = "Malformed cursor", body = APIResponse<String>),
        (status = 403, description = "No artifact view permission", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn list_artifacts(
    Path(agent_id): Path<String>,
    Query(params): Query<ArtifactListQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<ArtifactListView> {
    let query = service::ArtifactQuery {
        q: params.q,
        kind: params.kind,
        from: params.from,
        to: params.to,
        cursor: params.cursor,
        limit: params.limit,
    };
    match service::list(&svc(&state), &user, &agent_id, query).await {
        Ok(view) => api_response(StatusCode::OK, view),
        Err(e) => refuse(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/artifacts/activity",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ArtifactActivityQuery
    ),
    responses(
        (status = 200, description = "The agent's artifact change log, newest first", body = APIResponse<ArtifactActivityView>),
        (status = 400, description = "Malformed cursor", body = APIResponse<String>),
        (status = 403, description = "No artifact view permission", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn activity(
    Path(agent_id): Path<String>,
    Query(params): Query<ArtifactActivityQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<ArtifactActivityView> {
    match service::activity(&svc(&state), &user, &agent_id, params.cursor.as_deref(), params.limit).await {
        Ok(view) => api_response(StatusCode::OK, view),
        Err(e) => refuse(e),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/artifacts/{artifact_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("artifact_id" = String, Path, description = "Artifact ID")
    ),
    responses(
        (status = 200, description = "Artifact detail with references and change history", body = APIResponse<ArtifactDetailView>),
        (status = 403, description = "No artifact view permission", body = APIResponse<String>),
        (status = 404, description = "Agent or artifact not found", body = APIResponse<String>)
    )
)]
pub async fn artifact_detail(
    Path((agent_id, artifact_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<ArtifactDetailView> {
    match service::detail(&svc(&state), &user, &agent_id, &artifact_id).await {
        Ok(view) => api_response(StatusCode::OK, view),
        Err(e) => refuse(e),
    }
}

fn percent_encode(name: &str) -> String {
    let mut out = String::new();
    for byte in name.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/artifacts/{artifact_id}/content",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("artifact_id" = String, Path, description = "Artifact ID")
    ),
    responses(
        (status = 200, description = "Current artifact bytes"),
        (status = 206, description = "A byte range of the current content"),
        (status = 403, description = "No artifact view permission", body = APIResponse<String>),
        (status = 404, description = "Artifact not found or content unavailable", body = APIResponse<String>)
    )
)]
pub async fn artifact_content(
    Path((agent_id, artifact_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    headers: axum::http::HeaderMap,
) -> Response {
    let content = match service::content(&svc(&state), &user, &agent_id, &artifact_id).await {
        Ok(content) => content,
        Err(e) => return refuse::<String>(e).into_response(),
    };
    let bytes = content.bytes;
    let len = bytes.len() as u64;
    let disposition = format!("inline; filename*=UTF-8''{}", percent_encode(&content.name));
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let (status, body, content_range) = match range {
        Some(range) => match parse_range(range, len) {
            RangeRequest::Ignore => (StatusCode::OK, bytes, None),
            RangeRequest::Unsatisfiable => {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{len}"))
                    .body(Body::empty())
                    .unwrap();
            }
            RangeRequest::Bytes { start, end } => {
                let slice = bytes[start as usize..=end as usize].to_vec();
                (
                    StatusCode::PARTIAL_CONTENT,
                    slice,
                    Some(format!("bytes {start}-{end}/{len}")),
                )
            }
        },
        None => (StatusCode::OK, bytes, None),
    };
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content.mime_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_DISPOSITION, disposition);
    if let Some(value) = content_range {
        builder = builder.header(header::CONTENT_RANGE, value);
    }
    builder.body(Body::from(body)).unwrap()
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/artifacts/{artifact_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("artifact_id" = String, Path, description = "Artifact ID")
    ),
    responses(
        (status = 200, description = "Artifact deleted", body = APIResponse<ArtifactDeleteResult>),
        (status = 403, description = "No artifact manage permission", body = APIResponse<String>),
        (status = 404, description = "Agent or artifact not found", body = APIResponse<String>)
    )
)]
pub async fn delete_artifact(
    Path((agent_id, artifact_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<ArtifactDeleteResult> {
    match service::delete(&svc(&state), &user, &agent_id, &artifact_id).await {
        Ok(result) => api_response(StatusCode::OK, result),
        Err(e) => refuse(e),
    }
}
