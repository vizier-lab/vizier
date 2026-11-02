//! Provider credentials (contract `http-api.md` § Providers): any selectable provider can be
//! connected with a saved key, and keys found in the environment are shown alongside.

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    auth::Principal,
    models::{
        self,
        response::{APIResponse, api_response, err_response},
    },
    state::HTTPState,
};
use vizier_core::service::{ServiceError, providers as service, providers_sso};

pub mod custom;

pub use service::{ProviderResponse, UpsertProviderRequest};
pub use vizier_core::service::usage::{self, InUse};

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
        // Amazon Bedrock over IAM Identity Center (`contracts/provider-credentials-api.md` §3):
        // the sign-in, the pickers that follow it, and signing out inside Vizier.
        .route("/{id}/sso/sign-in", post(sso_sign_in))
        .route("/{id}/sso/accounts", get(sso_accounts))
        .route(
            "/{id}/sso/accounts/{account_id}/roles",
            get(sso_roles),
        )
        .route("/{id}/sso/sign-out", post(sso_sign_out))
}

/// A service error in the API's envelope. A `409` that names the agents using a provider keeps
/// them in `data`, for the WebUI's warning.
pub(crate) fn fail<T: serde::Serialize + Clone>(e: ServiceError) -> models::response::Response<T> {
    crate::api::v1::auth::err(e)
}

/// Like [`fail`], for the responses whose `data` is free-form JSON.
pub(crate) fn fail_data(e: ServiceError) -> models::response::Response<serde_json::Value> {
    if let ServiceError::Conflict {
        message,
        data: Some(data),
        ..
    } = &e
    {
        return (
            StatusCode::CONFLICT,
            Json(APIResponse {
                status: StatusCode::CONFLICT.as_u16(),
                message: Some(message.clone()),
                data: Some(data.clone()),
            }),
        );
    }
    crate::api::v1::auth::err(e)
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
    Extension(user): Extension<Principal>,
) -> models::response::Response<Vec<ProviderResponse>> {
    match service::list(&state.deps, &user).await {
        Ok(providers) => api_response(StatusCode::OK, providers),
        Err(e) => fail(e),
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
        (status = 404, description = "Provider not connected", body = APIResponse<String>)
    )
)]
pub(crate) async fn get_provider(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<ProviderResponse> {
    match service::get(&state.deps, &user, &id).await {
        Ok(p) => api_response(StatusCode::OK, p),
        Err(e) => fail(e),
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
        (status = 200, description = "Provider saved", body = APIResponse<ProviderResponse>),
        (status = 400, description = "A field the provider does not use", body = APIResponse<String>)
    )
)]
pub(crate) async fn upsert_provider(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    axum::Json(body): axum::Json<UpsertProviderRequest>,
) -> models::response::Response<ProviderResponse> {
    match service::upsert(&state.deps, &user, &id, body).await {
        Ok(Some(p)) => api_response(StatusCode::OK, p),
        Ok(None) => err_response(StatusCode::OK, "saved; no credentials remain".into()),
        Err(e) => fail(e),
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct DeleteQuery {
    /// Delete even though agents use the provider.
    #[serde(default)]
    pub confirm: bool,
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
    Extension(user): Extension<Principal>,
) -> models::response::Response<serde_json::Value> {
    match service::delete(&state.deps, &user, &id, query.confirm).await {
        Ok(message) => api_response(StatusCode::OK, serde_json::Value::String(message)),
        Err(e) => fail_data(e),
    }
}

#[utoipa::path(
    post,
    path = "/providers/{id}/test",
    params(("id" = String, Path, description = "Provider id")),
    request_body(content = UpsertProviderRequest, description = "Optional credentials to try before saving"),
    responses(
        (status = 200, description = "`ok` says whether the provider answered", body = APIResponse<usage::TestResult>)
    )
)]
pub(crate) async fn test_provider(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    body: axum::body::Bytes,
) -> models::response::Response<usage::TestResult> {
    match service::test(&state.deps, &user, &id, &body).await {
        Ok(result) => api_response(StatusCode::OK, result),
        Err(e) => fail(e),
    }
}

// ---------------------------------------------------------------- Bedrock over IAM Identity Center

/// The `POST` bodies of this section are all-optional (contracts §3), so an empty body means "use
/// the saved connection" rather than a deserialisation failure.
fn optional_sign_in_body(body: &[u8]) -> Result<providers_sso::SignInRequest, ServiceError> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(providers_sso::SignInRequest::default());
    }
    match serde_json::from_slice::<Option<providers_sso::SignInRequest>>(body) {
        Ok(body) => Ok(body.unwrap_or_default()),
        Err(error) => Err(ServiceError::invalid(error.to_string())),
    }
}

/// The `200`/`202` the answer asks for: `202` only while the browser step is pending.
fn sign_in_status(answer: &providers_sso::SignInResponse) -> StatusCode {
    if answer.status() == 202 {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    }
}

#[utoipa::path(
    post,
    path = "/providers/{id}/sso/sign-in",
    params(("id" = String, Path, description = "Provider id; only `amazon-bedrock-sso` signs in")),
    request_body(content = providers_sso::SignInRequest, description = "Optional start URL, SSO region and profile name, for a connection not saved yet"),
    responses(
        (status = 200, description = "A code to confirm, or the completed sign-in", body = APIResponse<providers_sso::SignInResponse>),
        (status = 202, description = "The browser step is still pending", body = APIResponse<providers_sso::SignInResponse>),
        (status = 400, description = "The sign-in was denied or the code expired", body = APIResponse<String>),
        (status = 502, description = "AWS did not answer", body = APIResponse<String>)
    )
)]
pub(crate) async fn sso_sign_in(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    body: axum::body::Bytes,
) -> models::response::Response<providers_sso::SignInResponse> {
    let body = match optional_sign_in_body(&body) {
        Ok(body) => body,
        Err(e) => return fail(e),
    };
    match providers_sso::sign_in(&state.deps, &user, &id, body).await {
        Ok(answer) => api_response(sign_in_status(&answer), answer),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/providers/{id}/sso/accounts",
    params(
        ("id" = String, Path, description = "Provider id; only `amazon-bedrock-sso` has accounts"),
        providers_sso::SignInRequest
    ),
    responses(
        (status = 200, description = "The accounts the identity was assigned; `fallback` asks for free text", body = APIResponse<providers_sso::AccountList>),
        (status = 401, description = "No usable session: sign in again", body = APIResponse<String>)
    )
)]
pub(crate) async fn sso_accounts(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Query(query): Query<providers_sso::SignInRequest>,
) -> models::response::Response<providers_sso::AccountList> {
    match providers_sso::discover_accounts(&state.deps, &user, &id, query).await {
        Ok(list) => api_response(StatusCode::OK, list),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    get,
    path = "/providers/{id}/sso/accounts/{account_id}/roles",
    params(
        ("id" = String, Path, description = "Provider id; only `amazon-bedrock-sso` has roles"),
        ("account_id" = String, Path, description = "AWS account id, 12 digits"),
        providers_sso::SignInRequest
    ),
    responses(
        (status = 200, description = "The roles this identity may assume in the account", body = APIResponse<providers_sso::RoleList>),
        (status = 400, description = "The account id is not 12 digits", body = APIResponse<String>),
        (status = 401, description = "No usable session: sign in again", body = APIResponse<String>)
    )
)]
pub(crate) async fn sso_roles(
    Path((id, account_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Query(query): Query<providers_sso::SignInRequest>,
) -> models::response::Response<providers_sso::RoleList> {
    match providers_sso::discover_roles(&state.deps, &user, &id, &account_id, query).await {
        Ok(list) => api_response(StatusCode::OK, list),
        Err(e) => fail(e),
    }
}

#[utoipa::path(
    post,
    path = "/providers/{id}/sso/sign-out",
    params(("id" = String, Path, description = "Provider id; only `amazon-bedrock-sso` signs out")),
    request_body(content = providers_sso::SignInRequest, description = "Optional profile name, when the connection is not saved yet"),
    responses(
        (status = 200, description = "The session row is gone and the connection is configured; never an error if it was already signed out", body = APIResponse<providers_sso::SignOutResponse>),
        (status = 400, description = "The provider is not an SSO connection", body = APIResponse<String>)
    )
)]
pub(crate) async fn sso_sign_out(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    body: axum::body::Bytes,
) -> models::response::Response<providers_sso::SignOutResponse> {
    let body = match optional_sign_in_body(&body) {
        Ok(body) => body,
        Err(e) => return fail(e),
    };
    match providers_sso::sign_out(&state.deps, &user, &id, body).await {
        Ok(answer) => api_response(StatusCode::OK, answer),
        Err(e) => fail(e),
    }
}

#[cfg(test)]
mod dummyplug_provider_tests {
    use super::*;
    use std::sync::Arc;
    use vizier_core::catalogue::builtin::CredentialField;
    use vizier_core::config::provider::ProviderId;
    use vizier_core::schema::SsoSessionState;
    use vizier_core::{
        config::VizierConfig, dependencies::VizierDependencies, storage::provider::ProviderStorage,
    };

    fn principal() -> Principal {
        Principal {
            user_id: "root".into(),
            username: "root".into(),
            role: vizier_core::auth::Role {
                role_id: "r".into(),
                name: "system".into(),
                permissions: vec![],
                is_system: true,
                created_at: chrono::Utc::now(),
            },
            permissions: vec![],
            method: crate::auth::AuthMethod::Jwt,
        }
    }

    async fn state() -> (HTTPState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        let deps = VizierDependencies::new(config).await.unwrap();
        let state = HTTPState {
            auth: crate::state::AuthMode::Network,
            config: deps.config.clone(),
            transport: deps.transport.clone(),
            storage: deps.storage.clone(),
            background_jobs: deps.background_jobs.clone(),
            catalogue: deps.catalogue.clone(),
            deps,
            client_events: Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
                10,
                std::time::Duration::from_secs(60),
            )),
            client_state_saves: Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
                2,
                std::time::Duration::from_secs(1),
            )),
            visit_throttle: Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
                1,
                std::time::Duration::from_secs(60),
            )),
            deferred_client_calls: Default::default(),
        };
        (state, dir)
    }

    #[tokio::test]
    async fn dummyplug_is_listed_only_while_saved_and_delete_removes_it() {
        let (state, _dir) = state().await;
        assert!(
            !service::connected(&state.deps)
                .await
                .iter()
                .any(|p| p.id == ProviderId::DUMMYPLUG),
            "the built-in test model alone is not a connected provider"
        );
        let put = upsert_provider(
            Path("dummyplug".into()),
            State(state.clone()),
            Extension(principal()),
            axum::Json(UpsertProviderRequest {
                base_url: Some("http://127.0.0.1:19998/v1".into()),
                ..Default::default()
            }),
        )
        .await;
        assert_eq!(put.0, StatusCode::OK);
        assert_eq!(
            put.1.0.data.as_ref().unwrap().fields,
            vec![CredentialField::BaseUrl]
        );
        assert_eq!(
            put.1.0.data.as_ref().unwrap().base_url.as_deref(),
            Some("http://127.0.0.1:19998/v1")
        );
        let get = get_provider(
            Path("dummyplug".into()),
            State(state.clone()),
            Extension(principal()),
        )
        .await;
        assert_eq!(get.0, StatusCode::OK);
        let saved = state
            .storage
            .get_provider(&ProviderId::DUMMYPLUG)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            saved.credentials.base_url.as_deref(),
            Some("http://127.0.0.1:19998/v1")
        );
        let deleted = delete_provider(
            Path("dummyplug".into()),
            Query(DeleteQuery { confirm: false }),
            State(state.clone()),
            Extension(principal()),
        )
        .await;
        assert_eq!(deleted.0, StatusCode::OK);
        assert!(
            !service::connected(&state.deps)
                .await
                .iter()
                .any(|p| p.id == ProviderId::DUMMYPLUG)
        );
        assert_eq!(
            get_provider(
                Path("dummyplug".into()),
                State(state),
                Extension(principal())
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }



    // ------------------------------------------------------------ Bedrock over IAM Identity Center

    const START_URL: &str = "https://acme.awsapps.com/start";

    /// The five fields a saved SSO connection needs, as the form sends them.
    fn sso_body() -> UpsertProviderRequest {
        UpsertProviderRequest {
            start_url: Some(START_URL.into()),
            sso_region: Some("us-east-1".into()),
            account_id: Some("123456789012".into()),
            role_name: Some("BedrockOperator".into()),
            sso_profile: Some("acme-bedrock".into()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn the_sso_sign_in_surface_is_for_the_sso_provider_only() {
        let (state, _dir) = state().await;
        for id in ["openai", "amazon-bedrock"] {
            let answer = sso_sign_in(
                Path(id.to_string()),
                State(state.clone()),
                Extension(principal()),
                axum::body::Bytes::new(),
            )
            .await;
            assert_eq!(answer.0, StatusCode::BAD_REQUEST, "{id}");
            assert!(
                answer
                    .1
                    .0
                    .message
                    .as_deref()
                    .unwrap_or_default()
                    .contains("IAM Identity Center"),
                "{:?}",
                answer.1.0
            );

            // The pickers and sign-out refuse the same way, so no other provider can be driven
            // through this surface.
            let accounts = sso_sign_out(
                Path(id.to_string()),
                State(state.clone()),
                Extension(principal()),
                axum::body::Bytes::new(),
            )
            .await;
            assert_eq!(accounts.0, StatusCode::BAD_REQUEST, "{id}");
        }
    }

    #[tokio::test]
    async fn signing_out_when_nothing_is_signed_in_is_not_an_error_and_echoes_no_secret() {
        let (state, _dir) = state().await;
        let answer = sso_sign_out(
            Path("amazon-bedrock-sso".into()),
            State(state.clone()),
            Extension(principal()),
            axum::body::Bytes::new(),
        )
        .await;
        assert_eq!(answer.0, StatusCode::OK, "{:?}", answer.1.0);
        let data = answer.1.0.data.expect("a body");
        assert_eq!(data.state, "configured");
        let json = serde_json::to_string(&data).unwrap();
        for forbidden in ["token", "secret", "device_code"] {
            assert!(!json.to_lowercase().contains(forbidden), "{json}");
        }
    }

    #[tokio::test]
    async fn a_saved_connection_reports_its_fields_and_a_configured_session() {
        let (state, dir) = state().await;
        let put = upsert_provider(
            Path("amazon-bedrock-sso".into()),
            State(state.clone()),
            Extension(principal()),
            axum::Json(sso_body()),
        )
        .await;
        assert_eq!(put.0, StatusCode::OK, "{:?}", put.1.0);
        let data = put.1.0.data.expect("a provider row");

        assert_eq!(data.start_url.as_deref(), Some(START_URL));
        assert_eq!(data.sso_region.as_deref(), Some("us-east-1"));
        assert_eq!(data.role_name.as_deref(), Some("BedrockOperator"));
        assert_eq!(data.sso_profile.as_deref(), Some("acme-bedrock"));
        // The Bedrock region defaults from the SSO region when only that one is given (FR-010).
        assert_eq!(data.region.as_deref(), Some("us-east-1"));
        assert_eq!(data.sso_state, Some(SsoSessionState::Configured));
        assert!(!data.has_sso_session);
        assert!(data.session_expires_at.is_none());
        assert!(data.profiles.is_empty());
        assert_eq!(
            data.fields,
            vec![
                CredentialField::StartUrl,
                CredentialField::SsoRegion,
                CredentialField::AccountId,
                CredentialField::RoleName,
                CredentialField::SsoProfile,
                CredentialField::Region,
            ]
        );

        // Nothing of it is a secret, and no session was written by saving the fields.
        let json = serde_json::to_string(&data).unwrap();
        // A presence flag such as `has_access_token` is a boolean, not material. Match the
        // key/value form so only a field that actually carries one trips this.
        for forbidden in ["access_token", "refresh_token", "client_secret", "device_code"] {
            let echoed = format!("\"{forbidden}\":");
            assert!(!json.contains(&echoed), "{json}");
        }
        let raw = vizier_core::storage::sqlite::SqliteStorage::open_connection(
            dir.path().to_str().unwrap(),
        )
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM bedrock_sso_session",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
        assert_eq!(raw, 0, "saving the five fields signs nobody in");

        // The picker endpoints answer the missing session with the re-sign-in wording and nothing
        // else: no token, no profile id, no session material (FR-009b, contracts §3).
        let accounts = sso_accounts(
            Path("amazon-bedrock-sso".into()),
            State(state.clone()),
            Extension(principal()),
            Query(providers_sso::SignInRequest::default()),
        )
        .await;
        assert_eq!(accounts.0, StatusCode::UNAUTHORIZED, "{:?}", accounts.1.0);
        let message = accounts.1.0.message.unwrap_or_default();
        assert!(message.contains("sign in again"), "{message}");
        assert!(!message.to_lowercase().contains("token"), "{message}");

        let roles = sso_roles(
            Path(("amazon-bedrock-sso".to_string(), "1234".to_string())),
            State(state.clone()),
            Extension(principal()),
            Query(providers_sso::SignInRequest::default()),
        )
        .await;
        assert_eq!(roles.0, StatusCode::BAD_REQUEST, "the account id is checked first");
    }

    #[tokio::test]
    async fn an_empty_body_is_read_as_use_the_saved_connection() {
        let body = optional_sign_in_body(b"  \n").expect("an empty body is not an error");
        assert!(body.profile.is_none() && body.sso_region.is_none() && body.start_url.is_none());
        assert!(optional_sign_in_body(b"null").unwrap().profile.is_none());
        let parsed = optional_sign_in_body(br#"{"profile":"acme-bedrock"}"#).unwrap();
        assert_eq!(parsed.profile.as_deref(), Some("acme-bedrock"));
        assert!(optional_sign_in_body(b"{\"nope\":1}").is_err());
    }

    #[tokio::test]
    async fn unconfigured_local_providers_are_not_connected_in_agent_picker() {
        let (state, _dir) = state().await;
        let summaries = vizier_core::service::catalogue::provider_summaries(&state.deps).await;
        for id in [ProviderId::OLLAMA, ProviderId::LLAMA_CPP] {
            let provider = summaries.iter().find(|p| p.id == id).unwrap();
            assert_eq!(provider.connection.source, None);
            assert_eq!(provider.connection.status, "missing");
        }
        let put = upsert_provider(
            Path("ollama".into()),
            State(state.clone()),
            Extension(principal()),
            axum::Json(UpsertProviderRequest {
                base_url: Some("http://127.0.0.1:11434".into()),
                ..Default::default()
            }),
        )
        .await;
        assert_eq!(put.0, StatusCode::OK);
        let summaries = vizier_core::service::catalogue::provider_summaries(&state.deps).await;
        let ollama = summaries
            .iter()
            .find(|p| p.id == ProviderId::OLLAMA)
            .unwrap();
        assert_eq!(ollama.connection.source.as_deref(), Some("saved"));
        assert_eq!(ollama.connection.status, "connected");
    }

    #[tokio::test]
    async fn put_rejects_fields_dummyplug_does_not_support() {
        let (state, _dir) = state().await;
        let put = upsert_provider(
            Path("dummyplug".into()),
            State(state),
            Extension(principal()),
            axum::Json(UpsertProviderRequest {
                api_key: Some("not-used".into()),
                ..Default::default()
            }),
        )
        .await;
        assert_eq!(put.0, StatusCode::BAD_REQUEST);
    }

    fn body(
        access: Option<&str>,
        secret: Option<&str>,
        region: Option<&str>,
    ) -> UpsertProviderRequest {
        UpsertProviderRequest {
            access_key_id: access.map(str::to_string),
            secret_access_key: secret.map(str::to_string),
            region: region.map(str::to_string),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn bedrock_saves_masks_and_refuses_an_incomplete_pair() {
        let (state, dir) = state().await;

        // Exactly one half of the pair is refused, naming what is missing.
        let put = upsert_provider(
            Path("amazon-bedrock".into()),
            State(state.clone()),
            Extension(principal()),
            axum::Json(body(Some("AKIAEXAMPLE"), None, Some("us-east-1"))),
        )
        .await;
        assert_eq!(put.0, StatusCode::BAD_REQUEST);
        assert!(
            put.1
                .0
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("secret access key"),
            "{:?}",
            put.1.0
        );
        assert!(
            state
                .storage
                .get_provider(&ProviderId::AMAZON_BEDROCK)
                .await
                .unwrap()
                .is_none()
        );

        // A malformed region is refused before any request.
        let put = upsert_provider(
            Path("amazon-bedrock".into()),
            State(state.clone()),
            Extension(principal()),
            axum::Json(body(
                Some("AKIAEXAMPLE"),
                Some("secret"),
                Some("not-a-region"),
            )),
        )
        .await;
        assert_eq!(put.0, StatusCode::BAD_REQUEST);
        assert!(
            put.1
                .0
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("invalid AWS region"),
            "{:?}",
            put.1.0
        );

        // The complete pair saves; the secret is write-only and encrypted at rest.
        let put = upsert_provider(
            Path("amazon-bedrock".into()),
            State(state.clone()),
            Extension(principal()),
            axum::Json(body(
                Some("AKIAEXAMPLE"),
                Some("secret-access"),
                Some("us-east-1"),
            )),
        )
        .await;
        assert_eq!(put.0, StatusCode::OK, "{:?}", put.1.0);
        let data = put.1.0.data.unwrap();
        assert_eq!(data.access_key_id.as_deref(), Some("AKIAEXAMPLE"));
        assert_eq!(data.region.as_deref(), Some("us-east-1"));
        assert!(data.has_secret_access_key);
        assert!(!data.has_session_token);
        assert!(!data.has_api_key);
        assert_eq!(
            data.fields,
            vec![
                CredentialField::AccessKeyId,
                CredentialField::SecretAccessKey,
                CredentialField::SessionToken,
                CredentialField::Region,
            ]
        );

        let stored = state
            .storage
            .get_provider(&ProviderId::AMAZON_BEDROCK)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            stored.credentials.access_key_id.as_deref(),
            Some("AKIAEXAMPLE")
        );
        assert_eq!(stored.credentials.region.as_deref(), Some("us-east-1"));
        // The secret is written protected: read the row without the storage decryptor.
        let raw =
            vizier_core::storage::sqlite::SqlStorage::open_connection(dir.path().to_str().unwrap())
                .unwrap()
                .query_row(
                    "SELECT data FROM provider_config WHERE variant = 'amazon-bedrock'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap();
        assert!(raw.contains("enc:v1:"), "{raw}");
        assert!(!raw.contains("secret-access"), "{raw}");
        assert!(
            raw.contains("AKIAEXAMPLE"),
            "the identifier stays readable: {raw}"
        );

        let get = get_provider(
            Path("amazon-bedrock".into()),
            State(state.clone()),
            Extension(principal()),
        )
        .await;
        let json = serde_json::to_value(get.1.0.data.unwrap()).unwrap();
        assert!(json.get("secret_access_key").is_none());
        assert!(!json.to_string().contains("secret-access"), "{json}");
        assert_eq!(json["has_secret_access_key"], serde_json::json!(true));

        // A Bedrock API key is refused by the provider's field list.
        let put = upsert_provider(
            Path("amazon-bedrock".into()),
            State(state.clone()),
            Extension(principal()),
            axum::Json(UpsertProviderRequest {
                api_key: Some("bearer".into()),
                ..Default::default()
            }),
        )
        .await;
        assert_eq!(put.0, StatusCode::BAD_REQUEST);
        assert!(
            put.1
                .0
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("does not use api_key"),
            "{:?}",
            put.1.0
        );

        // A base URL is refused: the regional endpoint is derived from the region.
        let put = upsert_provider(
            Path("amazon-bedrock".into()),
            State(state.clone()),
            Extension(principal()),
            axum::Json(UpsertProviderRequest {
                base_url: Some("https://elsewhere.example".into()),
                ..Default::default()
            }),
        )
        .await;
        assert_eq!(put.0, StatusCode::BAD_REQUEST);
        assert!(
            put.1
                .0
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("does not take a base_url"),
            "{:?}",
            put.1.0
        );

        // Deleting removes the connection; the provider is no longer connected.
        let deleted = delete_provider(
            Path("amazon-bedrock".into()),
            Query(DeleteQuery { confirm: true }),
            State(state.clone()),
            Extension(principal()),
        )
        .await;
        assert_eq!(deleted.0, StatusCode::OK);
        assert!(
            state
                .storage
                .get_provider(&ProviderId::AMAZON_BEDROCK)
                .await
                .unwrap()
                .is_none()
        );
        let get = get_provider(
            Path("amazon-bedrock".into()),
            State(state),
            Extension(principal()),
        )
        .await;
        assert_eq!(get.0, StatusCode::NOT_FOUND);
    }
}
