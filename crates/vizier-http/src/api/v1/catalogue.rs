//! The read-only catalogue API (contract `http-api.md` § Catalogue): providers and models for
//! the pickers, and logos. Any authenticated user can read it; logos need no auth, since
//! `<img>` tags send no token. No secret is ever returned.

use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    http::{HeaderValue, header},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use reqwest::StatusCode;
use serde::Deserialize;

use vizier_core::{config::provider::ProviderId, service::catalogue as service};
use crate::{
    auth::{
        Principal,
        middleware::{PermissionState, require_auth, require_permission},
    },
    models::{
        self,
        response::{APIResponse, api_response},
    },
    state::HTTPState,
};

pub use service::{
    CatalogueStatus, Connection, MediaModels, ModelCaps, ModelSummary, ProviderSummary, RefreshStatus,
};

pub fn catalogue(state: HTTPState) -> Router<HTTPState> {
    let authed = Router::new()
        .route("/", get(catalogue_status))
        .route("/providers", get(list_providers))
        .route("/providers/{id}/models", get(list_models))
        .route("/providers/{id}/models/{*model}", get(get_model))
        .route("/media-models", get(media_models))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));
    let admin = Router::new()
        .route("/refresh", axum::routing::post(refresh))
        .route("/media-models/{capability}/refresh", axum::routing::post(refresh_media))
        .route_layer(middleware::from_fn_with_state(
            PermissionState {
                permission: "settings:providers".to_string(),
            },
            require_permission,
        ))
        .route_layer(middleware::from_fn_with_state(state, require_auth));
    let public = Router::new().route("/logos/{file}", get(logo));
    authed.merge(admin).merge(public)
}

fn fail<T: serde::Serialize + Clone>(e: vizier_core::service::ServiceError) -> models::response::Response<T> {
    crate::api::v1::auth::err(e)
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct MediaModelsQuery {
    /// `speech`, `transcription` or `image`.
    pub capability: Option<String>,
}

#[utoipa::path(
    get,
    path = "/catalogue/media-models",
    params(MediaModelsQuery),
    responses(
        (status = 200, description = "One capability's models, default marked", body = APIResponse<MediaModels>),
        (status = 400, description = "Missing or unknown capability", body = APIResponse<String>)
    )
)]
pub async fn media_models(
    State(state): State<HTTPState>,
    Query(query): Query<MediaModelsQuery>,
) -> models::response::Response<MediaModels> {
    match service::media_models(&state.deps, query.capability.as_deref()) {
        Ok(models) => api_response(StatusCode::OK, models),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    post,
    path = "/catalogue/media-models/{capability}/refresh",
    params(("capability" = String, Path, description = "`speech`, `transcription` or `image`")),
    responses(
        (status = 200, description = "Refreshed (or already current)", body = APIResponse<MediaModels>),
        (status = 400, description = "Unknown capability", body = APIResponse<String>),
        (status = 409, description = "Refresh is disabled in the config", body = APIResponse<String>),
        (status = 502, description = "The check failed; the previous/seed list is still in use", body = APIResponse<String>)
    )
)]
pub async fn refresh_media(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(capability): Path<String>,
) -> models::response::Response<MediaModels> {
    match service::refresh_media(&state.deps, &user, &capability).await {
        Ok(models) => api_response(StatusCode::OK, models),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/catalogue",
    responses((status = 200, description = "Catalogue status", body = APIResponse<CatalogueStatus>))
)]
pub async fn catalogue_status(State(state): State<HTTPState>) -> models::response::Response<CatalogueStatus> {
    api_response(StatusCode::OK, service::catalogue_status(&state.deps))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ProvidersQuery {
    pub q: Option<String>,
    /// `chat`, `media`, `local` or `custom`.
    pub kind: Option<String>,
    pub connected: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/catalogue/providers",
    params(ProvidersQuery),
    responses((status = 200, description = "Selectable providers", body = APIResponse<Vec<ProviderSummary>>))
)]
pub async fn list_providers(
    State(state): State<HTTPState>,
    Query(query): Query<ProvidersQuery>,
) -> models::response::Response<Vec<ProviderSummary>> {
    api_response(
        StatusCode::OK,
        service::list_providers(&state.deps, query.q.as_deref(), query.kind.as_deref(), query.connected).await,
    )
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ModelsQuery {
    /// Every model, deprecated and not tool-capable ones included.
    #[serde(default)]
    pub all: bool,
}

#[utoipa::path(
    get,
    path = "/catalogue/providers/{id}/models",
    params(("id" = String, Path, description = "Provider id"), ModelsQuery),
    responses(
        (status = 200, description = "The provider's models, newest first", body = APIResponse<Vec<ModelSummary>>),
        (status = 404, description = "Unknown provider", body = APIResponse<String>)
    )
)]
pub async fn list_models(
    State(state): State<HTTPState>,
    Path(id): Path<String>,
    Query(query): Query<ModelsQuery>,
) -> models::response::Response<Vec<ModelSummary>> {
    match service::list_models(&state.deps, &id, query.all).await {
        Ok(models) => api_response(StatusCode::OK, models),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/catalogue/providers/{id}/models/{model}",
    params(("id" = String, Path, description = "Provider id"), ("model" = String, Path, description = "Model id")),
    responses(
        (status = 200, description = "One model", body = APIResponse<ModelSummary>),
        (status = 404, description = "Not in catalogue", body = APIResponse<String>)
    )
)]
pub async fn get_model(
    State(state): State<HTTPState>,
    Path((id, model)): Path<(String, String)>,
) -> models::response::Response<ModelSummary> {
    match service::get_model(&state.deps, &id, &model).await {
        Ok(m) => api_response(StatusCode::OK, m),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    post,
    path = "/catalogue/refresh",
    responses(
        (status = 200, description = "Refreshed (or already current)", body = APIResponse<CatalogueStatus>),
        (status = 409, description = "Refresh is disabled in the config", body = APIResponse<String>),
        (status = 502, description = "The check failed; the previous catalogue stays in use", body = APIResponse<String>)
    )
)]
pub async fn refresh(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<CatalogueStatus> {
    match service::refresh(&state.deps, &user).await {
        Ok(status) => api_response(StatusCode::OK, status),
        Err(e) => fail(e),
    }
}

fn svg_response(svg: Vec<u8>) -> Response {
    let mut response = (StatusCode::OK, svg).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'"),
    );
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    response
}

#[utoipa::path(
    get,
    path = "/catalogue/logos/{file}",
    params(("file" = String, Path, description = "`<provider or lab id>.svg`")),
    responses(
        (status = 200, description = "The logo", content_type = "image/svg+xml"),
        (status = 404, description = "No logo")
    )
)]
pub async fn logo(State(state): State<HTTPState>, Path(file): Path<String>) -> Response {
    let Some(id) = file.strip_suffix(".svg").filter(|id| ProviderId::is_valid(id)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match vizier_core::catalogue::refresh::logo(&state.deps, id).await {
        Some(svg) => svg_response(svg),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vizier_core::llm::DialectKind;

    async fn test_state() -> (HTTPState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = vizier_core::config::VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        let deps = vizier_core::dependencies::VizierDependencies::new(config).await.unwrap();
        let state = HTTPState {
            auth: crate::state::AuthMode::Network,
            config: deps.config.clone(),
            transport: deps.transport.clone(),
            storage: deps.storage.clone(),
            background_jobs: deps.background_jobs.clone(),
            catalogue: deps.catalogue.clone(),
            deps,
            client_events: std::sync::Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
                10,
                std::time::Duration::from_secs(60),
            )),
            client_state_saves: std::sync::Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
                2,
                std::time::Duration::from_secs(1),
            )),
            visit_throttle: std::sync::Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
                1,
                std::time::Duration::from_secs(60),
            )),
            deferred_client_calls: Default::default(),
        };
        (state, dir)
    }

    /// FR-001/FR-011a: Bedrock is listed with the four credential fields, the two extra env
    /// names and its guide; every other provider has no guide.
    #[tokio::test]
    async fn amazon_bedrock_lists_with_four_fields_and_its_guide() {
        use vizier_core::catalogue::builtin::CredentialField;
        let (state, _dir) = test_state().await;
        let summaries = vizier_core::service::catalogue::provider_summaries(&state.deps).await;
        let bedrock = summaries
            .iter()
            .find(|p| p.id == ProviderId::AMAZON_BEDROCK)
            .expect("Amazon Bedrock is listed");
        assert_eq!(
            bedrock.fields,
            vec![
                CredentialField::AccessKeyId,
                CredentialField::SecretAccessKey,
                CredentialField::SessionToken,
                CredentialField::Region,
            ]
        );
        assert!(!bedrock.fields.contains(&CredentialField::ApiKey));
        assert_eq!(bedrock.dialect, Some(DialectKind::Bedrock));
        assert!(bedrock.model_count > 100, "the catalogue's Bedrock models are listed");
        for name in ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_REGION", "AWS_SESSION_TOKEN", "AWS_DEFAULT_REGION"] {
            assert!(bedrock.env.iter().any(|e| e == name), "{name} missing from {:?}", bedrock.env);
        }
        let guide = bedrock.guide.as_ref().expect("Bedrock has a setup guide");
        assert_eq!(guide.steps.len(), 5);
        assert!(guide.policy.code.contains("bedrock:InvokeModel"));
        assert!(guide.policy.code.contains("bedrock:ListFoundationModels"));
        assert!(guide.links.iter().any(|l| l.url.ends_with("#amazon-bedrock")));
        assert!(
            summaries
                .iter()
                .filter(|p| p.id != ProviderId::AMAZON_BEDROCK)
                .all(|p| p.guide.is_none()),
            "no other provider has a guide"
        );
    }
}
