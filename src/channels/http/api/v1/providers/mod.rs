use axum::{
    Router,
    extract::{Path, State},
    routing::get,
};
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
    config::provider::ProviderId,
    schema::{ProviderCredentials, ProviderEntry},
    storage::provider::ProviderStorage,
};

pub fn providers() -> Router<HTTPState> {
    Router::new().route("/", get(list_providers)).route(
        "/{id}",
        get(get_provider)
            .put(upsert_provider)
            .delete(delete_provider),
    )
}

/// A provider's saved credentials, without the secrets.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ProviderResponse {
    pub id: ProviderId,
    /// The same id, under the field name pre-015 clients read.
    pub variant: ProviderId,
    pub has_api_key: bool,
    pub endpoint: Option<String>,
    pub base_url: Option<String>,
    pub has_access_token: bool,
    pub account_id: Option<String>,
}

fn provider_to_response(entry: &ProviderEntry) -> ProviderResponse {
    let c = &entry.credentials;
    let set = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.is_empty());
    ProviderResponse {
        id: entry.id.clone(),
        variant: entry.id.clone(),
        has_api_key: set(&c.api_key),
        endpoint: c.endpoint.clone(),
        base_url: c.base_url.clone(),
        has_access_token: set(&c.access_token),
        account_id: c.account_id.clone(),
    }
}

#[utoipa::path(
    get,
    path = "/providers",
    responses(
        (status = 200, description = "Providers with saved credentials", body = APIResponse<Vec<ProviderResponse>>)
    )
)]
async fn list_providers(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<ProviderResponse>> {
    crate::catalogue::refresh::maybe_refresh(&state.deps);
    match state.storage.list_providers().await {
        Ok(entries) => {
            let res: Vec<ProviderResponse> = entries
                .iter()
                .filter(|e| e.id != ProviderId::DUMMYPLUG)
                .map(provider_to_response)
                .collect();
            api_response(StatusCode::OK, res)
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[utoipa::path(
    get,
    path = "/providers/{id}",
    params(
        ("id" = String, Path, description = "Provider id (legacy names are accepted)")
    ),
    responses(
        (status = 200, description = "Provider details", body = APIResponse<ProviderResponse>),
        (status = 404, description = "Provider not found", body = APIResponse<String>)
    )
)]
async fn get_provider(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<ProviderResponse> {
    let Ok(id) = ProviderId::parse(&id) else {
        return err_response(StatusCode::BAD_REQUEST, "invalid provider id".into());
    };
    if id == ProviderId::DUMMYPLUG {
        return err_response(StatusCode::NOT_FOUND, "provider not found".into());
    }
    match state.storage.get_provider(&id).await {
        Ok(Some(entry)) => api_response(StatusCode::OK, provider_to_response(&entry)),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "provider not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

/// Credentials to save. An omitted field keeps the stored value; an empty string clears it.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpsertProviderRequest {
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub base_url: Option<String>,
    pub access_token: Option<String>,
    pub account_id: Option<String>,
}

/// Apply `update` over `stored`: `None` keeps, `""` clears, a value replaces.
fn apply(stored: Option<String>, update: Option<String>) -> Option<String> {
    match update {
        None => stored,
        Some(v) if v.is_empty() => None,
        Some(v) => Some(v),
    }
}

#[utoipa::path(
    put,
    path = "/providers/{id}",
    params(
        ("id" = String, Path, description = "Provider id (legacy names are accepted)")
    ),
    request_body = UpsertProviderRequest,
    responses(
        (status = 200, description = "Provider upserted", body = APIResponse<ProviderResponse>),
        (status = 400, description = "Bad request", body = APIResponse<String>)
    )
)]
async fn upsert_provider(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    axum::Json(body): axum::Json<UpsertProviderRequest>,
) -> models::response::Response<ProviderResponse> {
    let Ok(id) = ProviderId::parse(&id) else {
        return err_response(StatusCode::BAD_REQUEST, "invalid provider id".into());
    };
    if id == ProviderId::DUMMYPLUG {
        return err_response(
            StatusCode::BAD_REQUEST,
            "dummyplug requires no configuration".into(),
        );
    }

    let stored = match state.storage.get_provider(&id).await {
        Ok(entry) => entry.map(|e| e.credentials).unwrap_or_default(),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    };
    let entry = ProviderEntry {
        id,
        credentials: ProviderCredentials {
            api_key: apply(stored.api_key, body.api_key),
            endpoint: apply(stored.endpoint, body.endpoint),
            base_url: apply(stored.base_url, body.base_url),
            access_token: apply(stored.access_token, body.access_token),
            account_id: apply(stored.account_id, body.account_id),
        },
    };

    match state.storage.upsert_provider(&entry).await {
        Ok(()) => api_response(StatusCode::OK, provider_to_response(&entry)),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[utoipa::path(
    delete,
    path = "/providers/{id}",
    params(
        ("id" = String, Path, description = "Provider id (legacy names are accepted)")
    ),
    responses(
        (status = 200, description = "Provider deleted", body = APIResponse<String>),
        (status = 404, description = "Provider not found", body = APIResponse<String>)
    )
)]
async fn delete_provider(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<String> {
    let Ok(id) = ProviderId::parse(&id) else {
        return err_response(StatusCode::BAD_REQUEST, "invalid provider id".into());
    };
    match state.storage.delete_provider(&id).await {
        Ok(()) => api_response(StatusCode::OK, "deleted".into()),
        Err(e) => err_response(StatusCode::BAD_REQUEST, e.to_string().into()),
    }
}
