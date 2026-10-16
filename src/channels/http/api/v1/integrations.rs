//! Integration-level routes (`specs/016-integrations-section/contracts/http-api.md`): edit and
//! delete an integration independent of any agent, and the kinds catalogue. The agent-scoped
//! routes the pages use are in `agents/integrations.rs`; both share the helpers here.

use std::collections::HashMap;

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
    channels::http::{
        auth::{AuthenticatedUser, middleware::require_auth},
        models::response::APIResponse,
        state::HTTPState,
    },
    integrations::{
        access::{Caller, integration_access},
        kind::{IntegrationKind, KindField},
        service::{self, Actor, IntegrationPatch, ServiceError},
    },
    schema::{AgentConfig, AgentId, Integration, IntegrationFamily, Requester},
    storage::{
        agent::AgentStorage,
        integration::{IntegrationConflict, IntegrationStorage},
        user::UserStorage,
    },
};

use super::agents::user_can_view_agent;

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

/// A storage error as a response: a rule conflict is `409` with its code and the integration
/// already holding what was asked for, anything else `500`.
pub(crate) fn storage_error(e: anyhow::Error) -> Response {
    match e.downcast_ref::<IntegrationConflict>() {
        Some(conflict) => {
            let (id, name) = conflict.conflicting();
            fail(
                StatusCode::CONFLICT,
                conflict.to_string(),
                serde_json::json!({
                    "error": conflict.code(),
                    "integration": { "id": id, "name": name },
                }),
            )
        }
        None => plain(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// A service refusal as a response: 404/403/422, a rule conflict `409` with its code and the
/// integration already holding what was asked for, anything else `500`.
pub(crate) fn service_error(e: ServiceError) -> Response {
    match e {
        ServiceError::NotFound => plain(StatusCode::NOT_FOUND, "integration not found"),
        ServiceError::Forbidden(message) => fail(
            StatusCode::FORBIDDEN,
            message,
            serde_json::json!({
                "error": "forbidden",
                "required": [
                    crate::integrations::access::MANAGE_ALL,
                    format!("{} (own agents)", crate::integrations::access::MANAGE_OWNED),
                ],
            }),
        ),
        ServiceError::Invalid(message) => plain(StatusCode::UNPROCESSABLE_ENTITY, message),
        ServiceError::Conflict(conflict) => storage_error(conflict.into()),
        ServiceError::Storage(e) => storage_error(e),
    }
}

/// An integration as every response shows it: settings redacted by its kind.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationView {
    pub id: String,
    pub kind: String,
    pub family: IntegrationFamily,
    pub name: String,
    pub enabled: bool,
    /// Who made it: `{"user": …}` or `{"agent": …}`. Informational only.
    pub creator: Option<Requester>,
    /// The agent's name, or the username.
    pub creator_name: Option<String>,
    #[schema(value_type = Object)]
    pub settings: serde_json::Value,
    /// Required settings that are missing; the integration contributes nothing until set.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
}

/// Who a `Requester` is, as a person reads it: the agent's name, or the username.
pub(crate) fn requester_name(
    requester: &Requester,
    agents: &HashMap<AgentId, AgentConfig>,
    users: &HashMap<String, String>,
) -> String {
    match requester {
        Requester::User(id) => users.get(id).cloned().unwrap_or_else(|| id.clone()),
        Requester::Agent(id) => agents.get(id).map(|a| a.name.clone()).unwrap_or_else(|| id.clone()),
    }
}

/// Usernames by user id.
pub(crate) async fn usernames(state: &HTTPState) -> HashMap<String, String> {
    state
        .storage
        .list_users()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|user| (user.user_id, user.username))
        .collect()
}

pub(crate) fn view(
    kind: Option<&dyn IntegrationKind>,
    integration: &Integration,
    agents: &HashMap<AgentId, AgentConfig>,
    users: &HashMap<String, String>,
) -> IntegrationView {
    IntegrationView {
        id: integration.id.clone(),
        kind: integration.kind.clone(),
        family: integration.family,
        name: integration.name.clone(),
        enabled: integration.enabled,
        creator: integration.creator.clone(),
        creator_name: integration.creator.as_ref().map(|creator| requester_name(creator, agents, users)),
        // An unknown kind's settings could hold anything; show none of them.
        settings: kind
            .map(|k| k.redact(&integration.settings))
            .unwrap_or(serde_json::json!({})),
        missing: kind.map(|k| k.missing(&integration.settings)).unwrap_or_default(),
    }
}

/// Every stored agent, by id.
pub(crate) async fn agents_by_id(state: &HTTPState) -> HashMap<AgentId, AgentConfig> {
    state.storage.list_agents().await.unwrap_or_default().into_iter().collect()
}

// ---------------------------------------------------------------------------------------------
// Integration-level routes
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationDetail {
    #[serde(flatten)]
    pub integration: IntegrationView,
    /// The agents it is attached to that the caller can view.
    pub agents: Vec<AgentId>,
}

async fn load(state: &HTTPState, id: &str) -> Result<Integration, Response> {
    match state.storage.get_integration(id).await {
        Ok(Some(integration)) => Ok(integration),
        Ok(None) => Err(plain(StatusCode::NOT_FOUND, "integration not found")),
        Err(e) => Err(storage_error(e)),
    }
}

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
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> Response {
    let integration = match load(&state, &id).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    let caller = Caller::from(&user);
    let agents = agents_by_id(&state).await;
    if !integration_access(caller, &integration, &agents, |a| user_can_view_agent(&user, a)).see {
        return plain(StatusCode::NOT_FOUND, "integration not found");
    }
    let kind = state.deps.integration_kinds.get(&integration.kind);
    let visible = integration
        .agents
        .iter()
        .filter(|a| {
            caller.manages_all()
                || agents.get(*a).is_some_and(|config| user_can_view_agent(&user, config))
        })
        .cloned()
        .collect();
    ok(
        StatusCode::OK,
        IntegrationDetail {
            integration: view(kind.as_deref(), &integration, &agents, &usernames(&state).await),
            agents: visible,
        },
    )
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateIntegration {
    pub name: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub settings: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationSaved {
    #[serde(flatten)]
    pub integration: IntegrationView,
    pub affected_agents: Vec<AgentId>,
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
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateIntegration>,
) -> Response {
    let actor = Actor::Person(Caller::from(&user));
    let patch = IntegrationPatch { name: body.name, settings: body.settings, enabled: body.enabled };
    let can_view = |a: &AgentConfig| user_can_view_agent(&user, a);
    match service::update(&state.deps, &actor, &id, patch, &can_view).await {
        Ok((integration, affected)) => {
            let kind = state.deps.integration_kinds.get(&integration.kind);
            let agents = agents_by_id(&state).await;
            ok(
                StatusCode::OK,
                IntegrationSaved {
                    integration: view(kind.as_deref(), &integration, &agents, &usernames(&state).await),
                    affected_agents: affected,
                },
            )
        }
        Err(e) => service_error(e),
    }
}

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
pub struct DeleteQuery {
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct IntegrationDeleted {
    pub affected_agents: Vec<AgentId>,
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
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
    Query(query): Query<DeleteQuery>,
) -> Response {
    let integration = match load(&state, &id).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    let agents = agents_by_id(&state).await;
    let access = integration_access(Caller::from(&user), &integration, &agents, |a| {
        user_can_view_agent(&user, a)
    });
    if !access.see {
        return plain(StatusCode::NOT_FOUND, "integration not found");
    }
    if !access.edit {
        return service_error(ServiceError::Forbidden("you may not delete this integration".into()));
    }
    if !integration.agents.is_empty() && !query.confirm {
        return fail(
            StatusCode::CONFLICT,
            format!(
                "in use by {} agent(s); resend with ?confirm=true to delete anyway",
                integration.agents.len()
            ),
            serde_json::json!({ "error": "attached", "agents": integration.agents }),
        );
    }
    let actor = Actor::Person(Caller::from(&user));
    let can_view = |a: &AgentConfig| user_can_view_agent(&user, a);
    match service::delete(&state.deps, &actor, &id, None, &can_view).await {
        Ok(affected) => ok(StatusCode::OK, IntegrationDeleted { affected_agents: affected }),
        Err(e) => service_error(e),
    }
}

// ---------------------------------------------------------------------------------------------
// Catalogue
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct KindView {
    pub id: &'static str,
    pub family: IntegrationFamily,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub fields: Vec<KindField>,
}

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
    let kinds: Vec<KindView> = state
        .deps
        .integration_kinds
        .by_family(query.family)
        .iter()
        .map(|kind| {
            let listing = kind.listing();
            KindView {
                id: kind.id(),
                family: kind.family(),
                name: listing.name,
                description: listing.description,
                icon: listing.icon,
                fields: listing.fields,
            }
        })
        .collect();
    ok(StatusCode::OK, kinds)
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
    let Some(kind) = state.deps.integration_kinds.get(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match Icons::get(&format!("{}.svg", kind.listing().icon)) {
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
        for kind in crate::integrations::IntegrationKinds::builtin().by_family(None) {
            let icon = Icons::get(&format!("{}.svg", kind.listing().icon));
            assert!(icon.is_some_and(|f| f.data.starts_with(b"<svg")), "{}", kind.id());
        }
    }
}
