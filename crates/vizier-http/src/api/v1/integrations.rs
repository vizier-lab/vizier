//! Integration-level routes (`specs/016-integrations-section/contracts/http-api.md`): edit and
//! delete an integration independent of any agent, and the kinds catalogue. The agent-scoped
//! routes the pages use are in `agents/integrations.rs`; both share the response helpers here.
//! The rules and views are in `vizier_core::service::integrations`.

use rust_embed::RustEmbed;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderValue, header},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{Principal, middleware::require_auth},
    models::response::APIResponse,
    state::HTTPState,
};
use vizier_core::{
    integrations::service::IntegrationPatch,
    schema::IntegrationFamily,
    service::{ServiceError, Services, integrations as service},
};

pub use service::{IntegrationDeleted, IntegrationDetail, IntegrationSaved, IntegrationView, KindView};

/// The service layer over this server's engine.
pub(crate) fn svc(state: &HTTPState) -> Services {
    Services::new(state.deps.clone())
}

#[derive(RustEmbed)]
#[folder = "assets/integrations/"]
struct Icons;

pub fn integrations(state: HTTPState) -> Router<HTTPState> {
    let authed = Router::new()
        .route("/kinds", get(list_kinds))
        .route(
            "/{id}",
            get(get_integration).put(update_integration).delete(delete_integration),
        )
        .route_layer(middleware::from_fn_with_state(state, require_auth));
    let public = Router::new().route("/kinds/{id}/icon", get(kind_icon));
    authed.merge(public)
}

// ---------------------------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------------------------

pub(crate) fn ok<T: Serialize>(status: StatusCode, data: T) -> Response {
    (
        status,
        Json(APIResponse {
            status: status.as_u16(),
            message: None,
            data: serde_json::to_value(data).ok(),
        }),
    )
        .into_response()
}

/// The existing error shape, with `data` carrying `{"error": code, ...details}`.
pub(crate) fn fail(status: StatusCode, message: impl Into<String>, details: serde_json::Value) -> Response {
    (
        status,
        Json(APIResponse::<serde_json::Value> {
            status: status.as_u16(),
            message: Some(message.into()),
            data: Some(details),
        }),
    )
        .into_response()
}

pub(crate) fn plain(status: StatusCode, message: impl Into<String>) -> Response {
    let message = message.into();
    let code = match status {
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::UNPROCESSABLE_ENTITY => "invalid",
        _ => "error",
    };
    fail(status, message, serde_json::json!({ "error": code }))
}

/// A service refusal as this API's response: `data` carries `{"error": code, ...details}`.
/// A rule conflict keeps the integration already holding what was asked for in its `data`;
/// `required` is what a `403` lists as the permissions that would have allowed it.
pub(crate) fn failure(e: ServiceError, required: Option<serde_json::Value>) -> Response {
    let status = StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    match e {
        ServiceError::Conflict { message, data: Some(data), .. } => fail(status, message, data),
        ServiceError::Gone { code, message } => fail(status, message, serde_json::json!({ "error": code })),
        ServiceError::Forbidden(message) if required.is_some() => fail(
            status,
            message,
            serde_json::json!({ "error": "forbidden", "required": required }),
        ),
        other => plain(status, other.message().to_string()),
    }
}

/// The permissions that manage an agent's integrations, for a `403`'s `required`.
pub(crate) fn integrations_required() -> serde_json::Value {
    serde_json::json!([
        service::MANAGE_ALL,
        format!("{} (own agents)", service::MANAGE_OWNED),
    ])
}

fn fail_integrations(e: ServiceError) -> Response {
    failure(e, Some(integrations_required()))
}

// ---------------------------------------------------------------------------------------------
// Integration-level routes
// ---------------------------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/integrations/{id}",
    params(("id" = String, Path, description = "Integration id")),
    responses(
        (status = 200, description = "The integration", body = APIResponse<IntegrationDetail>),
        (status = 404, description = "Not found, or not visible to the caller")
    )
)]
pub async fn get_integration(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
) -> Response {
    match service::get(&svc(&state), &user, &id).await {
        Ok(detail) => ok(StatusCode::OK, detail),
        Err(e) => fail_integrations(e),
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateIntegration {
    pub name: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub settings: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[utoipa::path(
    put,
    path = "/integrations/{id}",
    params(("id" = String, Path, description = "Integration id")),
    request_body = UpdateIntegration,
    responses(
        (status = 200, description = "Saved; applies to the affected agents from their next turn", body = APIResponse<IntegrationSaved>),
        (status = 403, description = "The caller may not edit it"),
        (status = 409, description = "token_in_use or name_in_use_on_agent"),
        (status = 422, description = "Settings the kind refuses")
    )
)]
pub async fn update_integration(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
    Json(body): Json<UpdateIntegration>,
) -> Response {
    let patch = IntegrationPatch { name: body.name, settings: body.settings, enabled: body.enabled };
    match service::update(&svc(&state), &user, &id, patch).await {
        Ok(saved) => ok(StatusCode::OK, saved),
        Err(e) => fail_integrations(e),
    }
}

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
pub struct DeleteQuery {
    #[serde(default)]
    pub confirm: bool,
}

#[utoipa::path(
    delete,
    path = "/integrations/{id}",
    params(("id" = String, Path, description = "Integration id"), DeleteQuery),
    responses(
        (status = 200, description = "Deleted", body = APIResponse<IntegrationDeleted>),
        (status = 409, description = "Attached to agents; resend with ?confirm=true")
    )
)]
pub async fn delete_integration(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
    Query(query): Query<DeleteQuery>,
) -> Response {
    match service::delete(&svc(&state), &user, &id, query.confirm).await {
        Ok(deleted) => ok(StatusCode::OK, deleted),
        Err(e) => fail_integrations(e),
    }
}

// ---------------------------------------------------------------------------------------------
// Catalogue
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
pub struct FamilyQuery {
    pub family: Option<IntegrationFamily>,
}

#[utoipa::path(
    get,
    path = "/integrations/kinds",
    params(FamilyQuery),
    responses((status = 200, description = "The built-in kinds", body = APIResponse<Vec<KindView>>))
)]
pub async fn list_kinds(State(state): State<HTTPState>, Query(query): Query<FamilyQuery>) -> Response {
    ok(StatusCode::OK, service::kinds(&svc(&state), query.family))
}

#[utoipa::path(
    get,
    path = "/integrations/kinds/{id}/icon",
    params(("id" = String, Path, description = "Kind id")),
    responses(
        (status = 200, description = "The icon", content_type = "image/svg+xml"),
        (status = 404, description = "No such kind")
    )
)]
pub async fn kind_icon(State(state): State<HTTPState>, Path(id): Path<String>) -> Response {
    let Some(icon) = service::kind_icon(&svc(&state), &id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match Icons::get(&format!("{}.svg", icon)) {
        Some(file) => {
            let mut response = (StatusCode::OK, file.data.into_owned()).into_response();
            let headers = response.headers_mut();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
            headers.insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'"),
            );
            headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400"));
            response
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_an_embedded_icon() {
        for kind in vizier_core::integrations::IntegrationKinds::builtin().by_family(None) {
            let icon = Icons::get(&format!("{}.svg", kind.listing().icon));
            assert!(icon.is_some_and(|f| f.data.starts_with(b"<svg")), "{}", kind.id());
        }
    }

    #[test]
    fn a_conflict_keeps_its_code_and_the_holder_and_a_gone_its_code() {
        let conflict = ServiceError::Conflict {
            message: "taken".into(),
            code: Some("token_in_use".into()),
            data: Some(serde_json::json!({ "error": "token_in_use", "integration": { "id": "i", "name": "n" } })),
        };
        assert_eq!(failure(conflict, None).status(), StatusCode::CONFLICT);
        let gone = ServiceError::Gone { code: "attach_removed".into(), message: "m".into() };
        assert_eq!(failure(gone, None).status(), StatusCode::GONE);
        assert_eq!(failure(ServiceError::Forbidden("no".into()), None).status(), StatusCode::FORBIDDEN);
    }
}
