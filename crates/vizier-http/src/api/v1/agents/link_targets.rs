//! The entity wiki-link name index (spec 066, `contracts/link-targets-endpoint.md`): what
//! the WebUI's `[[kind:name]]` rendering checks live/broken against and what the composer
//! suggests from. One bounded index, one search, one exact resolve; the checks are
//! `vizier_core::service::link_targets`' — this module extracts and shapes responses.

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;

use vizier_core::service::{ServiceError, link_targets as service};

use crate::{
    api::v1::integrations::svc,
    models::{
        self,
        response::{APIResponse, api_response, err_response},
    },
    state::HTTPState,
};

pub use service::{DocTarget, LinkTargets, MemoryTargets, ResolveLink, ResolveRequest, SkillTarget, TaskTarget};

pub fn link_targets() -> Router<HTTPState> {
    Router::new()
        .route("/", get(index_or_search))
        .route("/resolve", post(resolve))
}

/// The service's refusal in this API's plain error shape.
fn refuse<T: serde::Serialize + Clone>(e: ServiceError) -> models::response::Response<T> {
    err_response(
        StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        e.message().to_string(),
    )
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct LinkTargetsQuery {
    /// With `q`: the search mode over this kind; absent, the bounded index.
    pub kind: Option<String>,
    pub q: Option<String>,
    /// Search page size, capped by the service.
    pub limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/link-targets",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        LinkTargetsQuery
    ),
    responses(
        (status = 200, description = "The bounded name index, or one kind's search page", body = APIResponse<LinkTargets>),
        (status = 400, description = "Unknown kind in the search mode", body = APIResponse<String>),
        (status = 403, description = "No agent view permission", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn index_or_search(
    Path(agent_id): Path<String>,
    Query(params): Query<LinkTargetsQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<LinkTargets> {
    let result = match params.kind {
        Some(kind) => {
            service::search(&svc(&state), &user, &agent_id, &kind, params.q.as_deref().unwrap_or(""), params.limit.unwrap_or(20))
                .await
        }
        None => service::index(&svc(&state), &user, &agent_id).await,
    };
    match result {
        Ok(targets) => api_response(StatusCode::OK, targets),
        Err(e) => refuse(e),
    }
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/link-targets/resolve",
    params(("agent_id" = String, Path, description = "Agent ID")),
    request_body = ResolveRequest,
    responses(
        (status = 200, description = "Exact existence per link, keyed `kind:name`", body = APIResponse<std::collections::BTreeMap<String, bool>>),
        (status = 403, description = "No agent view permission", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 422, description = "More than 100 links in the batch", body = APIResponse<String>)
    )
)]
pub async fn resolve(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    Json(body): Json<ResolveRequest>,
) -> models::response::Response<std::collections::BTreeMap<String, bool>> {
    match service::resolve(&svc(&state), &user, &agent_id, body).await {
        Ok(resolved) => api_response(StatusCode::OK, resolved),
        Err(e) => refuse(e),
    }
}
