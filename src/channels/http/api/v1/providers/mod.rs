//! Provider credentials (contract `http-api.md` § Providers): any selectable provider can be
//! connected with a saved key, and keys found in the environment are shown alongside.

use axum::{
    Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    catalogue::{ProviderOrigin, builtin::CredentialField, data::ProviderKind},
    channels::http::{
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    config::provider::ProviderId,
    schema::{ProviderCredentials, ProviderEntry},
    storage::provider::ProviderStorage,
};

pub mod custom;
pub mod usage;

use usage::{AffectedAgent, TestResult, UsedBy};

pub fn providers() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_providers))
        .nest("/custom", custom::custom())
        .route(
            "/{id}",
            get(get_provider)
                .put(upsert_provider)
                .delete(delete_provider),
        )
        .route("/{id}/test", post(test_provider))
}

/// A connected provider: saved credentials or a key in the environment, never the secrets.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ProviderResponse {
    pub id: ProviderId,
    /// The same id, under the field name pre-015 clients read.
    pub variant: ProviderId,
    pub name: String,
    pub origin: ProviderOrigin,
    pub kind: ProviderKind,
    pub logo: Option<String>,
    pub model_count: usize,
    pub fields: Vec<CredentialField>,
    /// `saved`, `env:NAME`, or `null` for a local provider on its default address.
    pub source: Option<String>,
    pub has_api_key: bool,
    pub endpoint: Option<String>,
    pub base_url: Option<String>,
    pub has_access_token: bool,
    pub account_id: Option<String>,
    /// The pre-015 name this provider's saved row was migrated from.
    pub migrated_from: Option<String>,
    pub used_by: UsedBy,
    /// Local and custom providers only; checked in the background.
    pub reachable: Option<bool>,
}

fn set(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.is_empty())
}

async fn connected(state: &HTTPState) -> Vec<ProviderResponse> {
    let summaries = crate::channels::http::api::v1::catalogue::provider_summaries(state).await;
    let mut used = usage::used_by_all(state).await;
    let migrated = usage::migrated_from(state).await;
    let mut out = vec![];
    for p in summaries {
        if p.origin == ProviderOrigin::Custom || p.id == ProviderId::DUMMYPLUG {
            continue;
        }
        let used_by = used.remove(&p.id).map(|(u, _)| u).unwrap_or_default();
        let local = p.kind == ProviderKind::Local;
        // A local provider on its default address has no source, but is listed once used.
        let listed = p.connection.source.is_some() || (local && used_by.total() > 0);
        if !listed {
            continue;
        }
        let env = crate::llm::resolve::provider_env(&state.catalogue, &p.id);
        let creds = crate::provider_keys::resolve_credentials(&state.storage, &p.id, &env)
            .await
            .map(|(c, _)| c)
            .unwrap_or_default();
        out.push(ProviderResponse {
            variant: p.id.clone(),
            migrated_from: migrated.get(p.id.as_str()).cloned(),
            reachable: local.then(|| usage::reachable(state, &p.id)).flatten(),
            name: p.name,
            origin: p.origin,
            kind: p.kind,
            logo: p.logo,
            model_count: p.model_count,
            fields: p.fields,
            source: p.connection.source,
            has_api_key: set(&creds.api_key),
            endpoint: creds.endpoint,
            base_url: creds.base_url,
            has_access_token: set(&creds.access_token),
            account_id: creds.account_id,
            used_by,
            id: p.id,
        });
    }
    out
}

#[utoipa::path(
    get,
    path = "/providers",
    responses(
        (status = 200, description = "Connected providers: saved or from the environment", body = APIResponse<Vec<ProviderResponse>>)
    )
)]
pub(crate) async fn list_providers(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<ProviderResponse>> {
    crate::catalogue::refresh::maybe_refresh(&state.deps);
    api_response(StatusCode::OK, connected(&state).await)
}

#[utoipa::path(
    get,
    path = "/providers/{id}",
    params(
        ("id" = String, Path, description = "Provider id (legacy names are accepted)")
    ),
    responses(
        (status = 200, description = "Provider details", body = APIResponse<ProviderResponse>),
        (status = 404, description = "Provider not connected", body = APIResponse<String>)
    )
)]
pub(crate) async fn get_provider(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<ProviderResponse> {
    let Ok(id) = ProviderId::parse(&id) else {
        return err_response(StatusCode::BAD_REQUEST, "invalid provider id".into());
    };
    if id == ProviderId::DUMMYPLUG {
        return err_response(StatusCode::NOT_FOUND, "provider not found".into());
    }
    match connected(&state).await.into_iter().find(|p| p.id == id) {
        Some(p) => api_response(StatusCode::OK, p),
        None => err_response(StatusCode::NOT_FOUND, "provider not found".into()),
    }
}

/// Credentials to save. An omitted field keeps the stored value; an empty string clears it.
#[derive(Debug, Default, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpsertProviderRequest {
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub base_url: Option<String>,
    pub access_token: Option<String>,
    pub account_id: Option<String>,
}

impl UpsertProviderRequest {
    /// The fields present in the body.
    fn sent(&self) -> Vec<CredentialField> {
        use CredentialField::*;
        [
            (ApiKey, &self.api_key),
            (Endpoint, &self.endpoint),
            (BaseUrl, &self.base_url),
            (AccessToken, &self.access_token),
            (AccountId, &self.account_id),
        ]
        .into_iter()
        .filter(|(_, v)| v.is_some())
        .map(|(f, _)| f)
        .collect()
    }

    fn into_credentials(self) -> ProviderCredentials {
        ProviderCredentials {
            api_key: self.api_key,
            endpoint: self.endpoint,
            base_url: self.base_url,
            access_token: self.access_token,
            account_id: self.account_id,
        }
    }
}

/// Apply `update` over `stored`: `None` keeps, `""` clears, a value replaces.
fn apply(stored: Option<String>, update: Option<String>) -> Option<String> {
    match update {
        None => stored,
        Some(v) if v.is_empty() => None,
        Some(v) => Some(v),
    }
}

fn field_name(f: CredentialField) -> &'static str {
    match f {
        CredentialField::ApiKey => "api_key",
        CredentialField::Endpoint => "endpoint",
        CredentialField::BaseUrl => "base_url",
        CredentialField::AccessToken => "access_token",
        CredentialField::AccountId => "account_id",
    }
}

/// The provider's connect fields, or why credentials can't be saved for it.
fn check_fields(
    state: &HTTPState,
    id: &ProviderId,
    sent: &[CredentialField],
    custom: &[crate::schema::CustomProvider],
) -> Result<(), (StatusCode, String)> {
    if *id == ProviderId::DUMMYPLUG {
        return Err((
            StatusCode::BAD_REQUEST,
            "dummyplug requires no configuration".into(),
        ));
    }
    let Some(listing) = state.catalogue.providers(custom).into_iter().find(|p| p.id == *id) else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("provider \"{id}\" is not available"),
        ));
    };
    if listing.origin == ProviderOrigin::Custom {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("\"{id}\" is a custom provider; edit it under /providers/custom/{id}"),
        ));
    }
    for f in sent {
        if listing.fields.contains(f) {
            continue;
        }
        let message = if *f == CredentialField::BaseUrl {
            // FR-027: a cloud provider's address comes from the catalogue.
            format!(
                "\"{id}\" does not take a base_url; to use another address, add a custom provider"
            )
        } else {
            format!("\"{id}\" does not use {}", field_name(*f))
        };
        return Err((StatusCode::BAD_REQUEST, message));
    }
    Ok(())
}

#[utoipa::path(
    put,
    path = "/providers/{id}",
    params(
        ("id" = String, Path, description = "Provider id (legacy names are accepted)")
    ),
    request_body = UpsertProviderRequest,
    responses(
        (status = 200, description = "Provider saved", body = APIResponse<ProviderResponse>),
        (status = 400, description = "A field the provider does not use", body = APIResponse<String>)
    )
)]
pub(crate) async fn upsert_provider(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    axum::Json(body): axum::Json<UpsertProviderRequest>,
) -> models::response::Response<ProviderResponse> {
    use crate::storage::provider::CustomProviderStorage;
    let Ok(id) = ProviderId::parse(&id) else {
        return err_response(StatusCode::BAD_REQUEST, "invalid provider id".into());
    };
    let custom = state.storage.list_custom_providers().await.unwrap_or_default();
    if let Err((status, message)) = check_fields(&state, &id, &body.sent(), &custom) {
        return err_response(status, message.into());
    }

    let stored = match state.storage.get_provider(&id).await {
        Ok(entry) => entry.map(|e| e.credentials).unwrap_or_default(),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    };
    let entry = ProviderEntry {
        id: id.clone(),
        credentials: ProviderCredentials {
            api_key: apply(stored.api_key, body.api_key),
            endpoint: apply(stored.endpoint, body.endpoint),
            base_url: apply(stored.base_url, body.base_url),
            access_token: apply(stored.access_token, body.access_token),
            account_id: apply(stored.account_id, body.account_id),
        },
    };
    let saved = if entry.credentials.is_empty() {
        state.storage.delete_provider(&id).await
    } else {
        state.storage.upsert_provider(&entry).await
    };
    if let Err(e) = saved {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into());
    }
    restart_users(&state, &id).await;
    match connected(&state).await.into_iter().find(|p| p.id == id) {
        Some(p) => api_response(StatusCode::OK, p),
        None => err_response(StatusCode::OK, "saved; no credentials remain".into()),
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct DeleteQuery {
    /// Delete even though agents use the provider.
    #[serde(default)]
    pub confirm: bool,
}

/// The agents a delete would affect, for the WebUI's warning.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct InUse {
    pub agents: Vec<AffectedAgent>,
}

/// `409` with the agents using `id`, unless the delete is confirmed.
pub(crate) async fn in_use(
    state: &HTTPState,
    id: &ProviderId,
    confirm: bool,
) -> Option<models::response::Response<serde_json::Value>> {
    if confirm {
        return None;
    }
    let agents = usage::used_by_all(state)
        .await
        .remove(id)
        .map(|(_, a)| a)
        .unwrap_or_default();
    if agents.is_empty() {
        return None;
    }
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    Some((
        StatusCode::CONFLICT,
        axum::Json(APIResponse {
            status: StatusCode::CONFLICT.as_u16(),
            message: Some(format!(
                "used by {}; resend with ?confirm=true to delete anyway",
                names.join(", ")
            )),
            data: serde_json::to_value(InUse { agents }).ok(),
        }),
    ))
}

/// Restart every agent using `id`, so a changed key or a removed provider takes effect now
/// rather than at the next restart. An agent that can no longer build shows as `Failed`.
pub(crate) async fn restart_users(state: &HTTPState, id: &ProviderId) {
    let agents = usage::used_by_all(state)
        .await
        .remove(id)
        .map(|(_, a)| a)
        .unwrap_or_default();
    for agent in agents {
        let (resp, rx) = tokio::sync::oneshot::channel();
        let cmd = crate::schema::AgentCommand::Restart {
            agent_id: agent.id.clone(),
            resp,
        };
        if state.transport.send_agent_command(cmd).await.is_err() {
            continue;
        }
        if let Ok(crate::schema::AgentCommandResult::Error(e)) = rx.await {
            tracing::warn!("restarting agent '{}' after a change to '{id}': {e}", agent.id);
        }
    }
}

#[utoipa::path(
    delete,
    path = "/providers/{id}",
    params(
        ("id" = String, Path, description = "Provider id (legacy names are accepted)"),
        DeleteQuery
    ),
    responses(
        (status = 200, description = "Saved credentials deleted", body = APIResponse<String>),
        (status = 409, description = "Agents use this provider; `data` lists them", body = APIResponse<InUse>)
    )
)]
pub(crate) async fn delete_provider(
    Path(id): Path<String>,
    Query(query): Query<DeleteQuery>,
    State(state): State<HTTPState>,
) -> models::response::Response<serde_json::Value> {
    let Ok(id) = ProviderId::parse(&id) else {
        return err_response(StatusCode::BAD_REQUEST, "invalid provider id".into());
    };
    if let Some(conflict) = in_use(&state, &id, query.confirm).await {
        return conflict;
    }
    if let Err(e) = state.storage.delete_provider(&id).await {
        return err_response(StatusCode::BAD_REQUEST, e.to_string().into());
    }
    restart_users(&state, &id).await;
    let env = crate::llm::resolve::provider_env(&state.catalogue, &id);
    match crate::provider_keys::credentials_from_env(&id, &env) {
        Some((_, origin)) => api_response(
            StatusCode::OK,
            serde_json::Value::String(format!(
                "deleted the saved credentials; the {} environment variable still connects this provider and can't be deleted here",
                origin.label().trim_start_matches("env:")
            )),
        ),
        None => api_response(StatusCode::OK, "deleted".into()),
    }
}

#[utoipa::path(
    post,
    path = "/providers/{id}/test",
    params(("id" = String, Path, description = "Provider id")),
    request_body(content = UpsertProviderRequest, description = "Optional credentials to try before saving"),
    responses(
        (status = 200, description = "`ok` says whether the provider answered", body = APIResponse<TestResult>)
    )
)]
pub(crate) async fn test_provider(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    body: axum::body::Bytes,
) -> models::response::Response<TestResult> {
    use crate::storage::provider::CustomProviderStorage;
    let Ok(id) = ProviderId::parse(&id) else {
        return err_response(StatusCode::BAD_REQUEST, "invalid provider id".into());
    };
    // The body is optional: nothing (or `null`) tests what is saved.
    let body: UpsertProviderRequest = if body.iter().all(u8::is_ascii_whitespace) {
        UpsertProviderRequest::default()
    } else {
        match serde_json::from_slice::<Option<UpsertProviderRequest>>(&body) {
            Ok(b) => b.unwrap_or_default(),
            Err(e) => return err_response(StatusCode::BAD_REQUEST, e.to_string()),
        }
    };
    let custom = state.storage.get_custom_provider(&id).await.ok().flatten();
    if custom.is_none() {
        let all = state.storage.list_custom_providers().await.unwrap_or_default();
        if let Err((status, message)) = check_fields(&state, &id, &body.sent(), &all) {
            return err_response(status, message.into());
        }
    }
    let credentials = Some(body.into_credentials()).filter(|c| !c.is_empty());
    match usage::resolve_for_test(&state, &id, credentials, custom.as_ref()).await {
        Ok(resolved) => api_response(StatusCode::OK, usage::check(&state, resolved).await),
        Err(e) => api_response(
            StatusCode::OK,
            TestResult {
                ok: false,
                model_count: None,
                status: None,
                error: Some(e.to_string()),
            },
        ),
    }
}
