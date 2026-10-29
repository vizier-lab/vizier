//! The artifact gallery API (`specs/035-add-artifacts`, contracts/artifacts-api.md).
//!
//! Reads (list/detail/content/activity) need artifact **view**; delete needs **manage**. Both
//! come from `crate::artifacts::access`, never from agent visibility.

use axum::{
    Extension, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    access::Caller,
    artifacts::access as ArtifactsAccess,
    channels::http::{
        api::v1::range::{RangeRequest, parse_range},
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    schema::{
        Artifact, ArtifactCursor, ArtifactDeleteResult, ArtifactEvent, ArtifactKind, ArtifactPage,
        ArtifactReference, Requester,
    },
    storage::agent::AgentStorage,
};

use super::user_can_view_agent;

pub fn artifacts() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_artifacts))
        .route("/activity", get(activity))
        .route("/{artifact_id}", get(artifact_detail))
        .route("/{artifact_id}", delete(delete_artifact))
        .route("/{artifact_id}/content", get(artifact_content))
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ArtifactItemView {
    pub id: String,
    pub agent_id: String,
    pub name: String,
    pub kind: ArtifactKind,
    pub mime_type: String,
    pub size: u64,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub available: bool,
}

impl ArtifactItemView {
    fn new(artifact: &Artifact, available: bool) -> Self {
        Self {
            id: artifact.id.clone(),
            agent_id: artifact.agent_id.clone(),
            name: artifact.name.clone(),
            kind: artifact.kind,
            mime_type: artifact.mime_type.clone(),
            size: artifact.size,
            version: artifact.version,
            created_at: artifact.created_at,
            updated_at: artifact.updated_at,
            available,
        }
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ArtifactListView {
    pub items: Vec<ArtifactItemView>,
    pub next: Option<String>,
    pub total: usize,
    pub total_bytes: u64,
    pub agent_total: usize,
    pub agent_total_bytes: u64,
}

fn list_view(page: ArtifactPage, artifacts: &crate::artifacts::Artifacts) -> ArtifactListView {
    let items = page
        .items
        .iter()
        .map(|artifact| {
            ArtifactItemView::new(
                artifact,
                artifacts.is_available(&artifact.agent_id, artifact),
            )
        })
        .collect();
    ArtifactListView {
        items,
        next: page.next,
        total: page.total,
        total_bytes: page.total_bytes,
        agent_total: page.agent_total,
        agent_total_bytes: page.agent_total_bytes,
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ArtifactReferenceView {
    pub kind: String,
    pub label: String,
    pub href: Option<String>,
    pub created_at: DateTime<Utc>,
}

fn reference_view(reference: &ArtifactReference) -> ArtifactReferenceView {
    let href = match reference.kind.as_str() {
        "message" => reference
            .topic
            .as_ref()
            .map(|topic| format!("/{}/chat/{}", reference.agent_id, topic)),
        "memory" => Some(format!(
            "/{}/memory/{}/{}",
            reference.agent_id,
            reference.bundle.as_deref().unwrap_or("default"),
            reference.path.as_deref().unwrap_or("")
        )),
        _ => None,
    };
    ArtifactReferenceView {
        kind: reference.kind.clone(),
        label: reference.label.clone(),
        href,
        created_at: reference.created_at,
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ArtifactEventView {
    pub id: String,
    pub artifact_id: String,
    pub artifact_name: String,
    pub action: crate::schema::ArtifactEventAction,
    pub actor: Requester,
    pub actor_name: String,
    pub source_channel: Option<String>,
    pub source_topic: Option<String>,
    pub source_filename: Option<String>,
    pub created_at: DateTime<Utc>,
}

async fn event_view(state: &HTTPState, event: &ArtifactEvent) -> ArtifactEventView {
    ArtifactEventView {
        id: event.id.clone(),
        artifact_id: event.artifact_id.clone(),
        artifact_name: event.artifact_name.clone(),
        action: event.action,
        actor: event.actor.clone(),
        actor_name: crate::utils::actor_display_name(&state.storage, &event.actor).await,
        source_channel: event.source_channel.clone(),
        source_topic: event.source_topic.clone(),
        source_filename: event.source_filename.clone(),
        created_at: event.created_at,
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ArtifactDetailView {
    pub artifact: ArtifactItemView,
    pub origin: crate::schema::ArtifactOrigin,
    pub available: bool,
    pub references: Vec<ArtifactReferenceView>,
    pub events: Vec<ArtifactEventView>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ArtifactActivityView {
    pub items: Vec<ArtifactEventView>,
    pub next: Option<String>,
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

/// Resolve the agent and check artifact view permission. Returns the agent config on success,
/// or the response to send.
async fn require_view(
    state: &HTTPState,
    user: &crate::channels::http::auth::AuthenticatedUser,
    agent_id: &str,
) -> Result<crate::schema::AgentConfig, (StatusCode, String)> {
    let config = match state.storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err((StatusCode::NOT_FOUND, format!("agent {agent_id} not found"))),
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    if !user_can_view_agent(user, &config) {
        return Err((StatusCode::FORBIDDEN, "Access denied".into()));
    }
    if !ArtifactsAccess::can_view(&Caller::from(user), &config) {
        return Err((
            StatusCode::FORBIDDEN,
            "You do not have permission to view this agent's artifacts".into(),
        ));
    }
    Ok(config)
}

async fn require_manage(
    state: &HTTPState,
    user: &crate::channels::http::auth::AuthenticatedUser,
    agent_id: &str,
) -> Result<crate::schema::AgentConfig, (StatusCode, String)> {
    let config = require_view(state, user, agent_id).await?;
    if !ArtifactsAccess::can_manage(&Caller::from(user), &config) {
        return Err((
            StatusCode::FORBIDDEN,
            "You do not have permission to manage this agent's artifacts".into(),
        ));
    }
    Ok(config)
}

fn parse_day(raw: &str, end: bool) -> Option<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()?;
    let time = if end {
        date.and_hms_milli_opt(23, 59, 59, 999)?
    } else {
        date.and_hms_opt(0, 0, 0)?
    };
    Some(Utc.from_utc_datetime(&time))
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
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<ArtifactListView> {
    if let Err((status, message)) = require_view(&state, &user, &agent_id).await {
        return err_response(status, message);
    }
    let kind = match params.kind.as_deref().filter(|k| !k.is_empty()) {
        Some(kind) => match ArtifactKind::parse(kind) {
            Some(kind) => Some(kind),
            None => return err_response(StatusCode::BAD_REQUEST, format!("unknown kind '{kind}'")),
        },
        None => None,
    };
    let cursor = match params.cursor.as_deref().filter(|c| !c.is_empty()) {
        Some(raw) => match ArtifactCursor::decode(raw) {
            Some(cursor) => Some(cursor),
            None => return err_response(StatusCode::BAD_REQUEST, "malformed cursor".into()),
        },
        None => None,
    };
    let from = match params.from.as_deref().filter(|d| !d.is_empty()) {
        Some(raw) => match parse_day(raw, false) {
            Some(at) => Some(at),
            None => return err_response(StatusCode::BAD_REQUEST, "malformed from date".into()),
        },
        None => None,
    };
    let to = match params.to.as_deref().filter(|d| !d.is_empty()) {
        Some(raw) => match parse_day(raw, true) {
            Some(at) => Some(at),
            None => return err_response(StatusCode::BAD_REQUEST, "malformed to date".into()),
        },
        None => None,
    };
    let filters = crate::schema::ArtifactFilters {
        agent_id,
        q: params.q,
        kind,
        from,
        to,
        cursor,
        limit: params.limit.unwrap_or(50).clamp(1, 100),
    };
    match state.deps.artifacts.list(filters).await {
        Ok(page) => api_response(StatusCode::OK, list_view(page, &state.deps.artifacts)),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<ArtifactActivityView> {
    if let Err((status, message)) = require_view(&state, &user, &agent_id).await {
        return err_response(status, message);
    }
    let cursor = match params.cursor.as_deref().filter(|c| !c.is_empty()) {
        Some(raw) => match ArtifactCursor::decode(raw) {
            Some(cursor) => Some(cursor),
            None => return err_response(StatusCode::BAD_REQUEST, "malformed cursor".into()),
        },
        None => None,
    };
    let limit = params.limit.unwrap_or(50).clamp(1, 100);
    match state
        .deps
        .artifacts
        .activity(&agent_id, cursor, limit)
        .await
    {
        Ok(events) => {
            let next = if events.len() == limit {
                events
                    .last()
                    .map(|event| ArtifactCursor::new(event.created_at, &event.id).encode())
            } else {
                None
            };
            let mut items = Vec::with_capacity(events.len());
            for event in &events {
                items.push(event_view(&state, event).await);
            }
            api_response(StatusCode::OK, ArtifactActivityView { items, next })
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<ArtifactDetailView> {
    if let Err((status, message)) = require_view(&state, &user, &agent_id).await {
        return err_response(status, message);
    }
    let detail = match state.deps.artifacts.detail(&agent_id, &artifact_id).await {
        Ok(Some(detail)) => detail,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "artifact not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let mut events = Vec::with_capacity(detail.events.len());
    for event in &detail.events {
        events.push(event_view(&state, event).await);
    }
    api_response(
        StatusCode::OK,
        ArtifactDetailView {
            artifact: ArtifactItemView::new(&detail.artifact, detail.available),
            origin: detail.artifact.origin(),
            available: detail.available,
            references: detail.references.iter().map(reference_view).collect(),
            events,
        },
    )
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
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
    headers: axum::http::HeaderMap,
) -> Response {
    if let Err((status, message)) = require_view(&state, &user, &agent_id).await {
        return err_response::<String>(status, message).into_response();
    }
    let detail = match state.deps.artifacts.detail(&agent_id, &artifact_id).await {
        Ok(Some(detail)) => detail,
        Ok(None) => {
            return err_response::<String>(StatusCode::NOT_FOUND, "artifact not found".into())
                .into_response();
        }
        Err(e) => {
            return err_response::<String>(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                .into_response();
        }
    };
    let bytes = match state
        .deps
        .artifacts
        .read_content(&agent_id, &artifact_id)
        .await
    {
        Ok(bytes) => bytes,
        Err(_) => {
            return err_response::<String>(
                StatusCode::NOT_FOUND,
                "artifact content is unavailable".into(),
            )
            .into_response();
        }
    };
    let len = bytes.len() as u64;
    let disposition = format!(
        "inline; filename*=UTF-8''{}",
        percent_encode(&detail.artifact.name)
    );
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
        .header(header::CONTENT_TYPE, detail.artifact.mime_type)
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
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<ArtifactDeleteResult> {
    if let Err((status, message)) = require_manage(&state, &user, &agent_id).await {
        return err_response(status, message);
    }
    match state
        .deps
        .artifacts
        .delete(
            &agent_id,
            &artifact_id,
            Requester::User(user.user_id.clone()),
        )
        .await
    {
        Ok(result) => api_response(StatusCode::OK, result),
        Err(e) => {
            let message = e.to_string();
            let status = if message.contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            err_response(status, message)
        }
    }
}
