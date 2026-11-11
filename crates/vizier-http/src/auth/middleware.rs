use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION, Uri},
    middleware::Next,
    response::Response,
};

use crate::{auth::{AuthMethod, AuthService, Principal}, state::{AuthMode, HTTPState}};
use vizier_core::storage::user::{Role, UserStorage};

/// When the bearer token that authenticated a request expires, as Unix seconds. Present only
/// for a token (an API key or the desktop principal does not expire this way). A handler that
/// keeps a connection open ends it at this moment, so a signed-out session cannot keep
/// receiving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenExpiry(pub u64);

impl TokenExpiry {
    /// A credential that does not expire this way (an API key, the desktop principal).
    pub const NEVER: Self = Self(u64::MAX);

    /// How long from `now_secs` until it expires; `None` when it never does.
    pub fn remaining(self, now_secs: u64) -> Option<std::time::Duration> {
        (self != Self::NEVER)
            .then(|| std::time::Duration::from_secs(self.0.saturating_sub(now_secs)))
    }
}

#[derive(Debug, Clone)]
pub enum AuthError {
    MissingCredentials,
    InvalidCredentials,
    InvalidToken,
    Forbidden,
    InternalError,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingCredentials => write!(f, "Missing authentication credentials"),
            AuthError::InvalidCredentials => write!(f, "Invalid authentication credentials"),
            AuthError::InvalidToken => write!(f, "Invalid or expired token"),
            AuthError::Forbidden => write!(f, "Forbidden: insufficient permissions"),
            AuthError::InternalError => write!(f, "Internal authentication error"),
        }
    }
}

impl std::error::Error for AuthError {}

impl axum::response::IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AuthError::MissingCredentials => (StatusCode::UNAUTHORIZED, "Missing authentication credentials"),
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid authentication credentials"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
            AuthError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden: insufficient permissions"),
            AuthError::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal authentication error"),
        };

        let body = axum::Json(serde_json::json!({
            "status": status.as_u16(),
            "message": message,
            "data": null
        }));

        (status, body).into_response()
    }
}

async fn resolve_user_role(state: &HTTPState, user_id: &str) -> Result<(Role, Vec<String>), AuthError> {
    let user = state
        .storage
        .get_user_by_id(user_id)
        .await
        .map_err(|_| AuthError::InternalError)?
        .ok_or(AuthError::InvalidCredentials)?;

    let role = state
        .storage
        .get_role(&user.role_id)
        .await
        .map_err(|_| AuthError::InternalError)?
        .ok_or(AuthError::InternalError)?;

    Ok((role.clone(), role.permissions.clone()))
}

/// The one person the desktop app signed in as, with role and permissions re-read.
async fn desktop_principal(state: &HTTPState, user_id: &str) -> Result<Principal, AuthError> {
    let (role, permissions) = resolve_user_role(state, user_id).await?;
    let user = state
        .storage
        .get_user_by_id(user_id)
        .await
        .map_err(|_| AuthError::InternalError)?
        .ok_or(AuthError::InvalidCredentials)?;
    Ok(Principal {
        user_id: user.user_id,
        username: user.username,
        role,
        permissions,
        method: AuthMethod::Desktop,
    })
}

/// Establishes who a network request is from its credentials (the `Authorization` header, or
/// `?token=`). Shared by [`require_auth`], which rejects when this fails, and [`optional_auth`],
/// which treats a failure as "no session".
async fn principal_from_credentials(
    state: &HTTPState,
    headers: &HeaderMap,
    uri: &Uri,
) -> Result<(Principal, TokenExpiry), AuthError> {
    // Try to get credentials from Authorization header first
    let credentials_from_header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|auth_header| {
            auth_header
                .split_once(' ')
                .map(|(auth_type, creds)| (auth_type.to_string(), creds.to_string()))
        });

    // Try to get token from query parameter if header is not present
    let credentials_from_query = if credentials_from_header.is_none() {
        extract_token_from_query(uri)
            .map(|token| ("bearer".to_string(), token))
    } else {
        None
    };

    // Use credentials from header or query parameter
    let (auth_type, credentials) = credentials_from_header
        .or(credentials_from_query)
        .ok_or(AuthError::MissingCredentials)?;

    let http_config = state
        .config
        .channels
        .http
        .as_ref()
        .ok_or(AuthError::InternalError)?;

    let auth_service = AuthService::new(http_config);

    // When the credential expires, for a long-lived connection that must end with it.
    let mut token_expiry = TokenExpiry::NEVER;

    let user = match auth_type.to_lowercase().as_str() {
        "bearer" => {
            // JWT token authentication
            let claims = auth_service
                .validate_token(&credentials)
                .map_err(|_| AuthError::InvalidToken)?;
            token_expiry = TokenExpiry(claims.exp as u64);

            let (role, permissions) = resolve_user_role(state, &claims.sub).await?;

            // A person using the WebUI is "here"; a script with an API key is not. A failed write
            // is logged and never fails the request.
            if state.visit_throttle.check(&claims.sub) {
                if let Err(e) = state.storage.record_visit(&claims.sub, vizier_core::time::now()).await {
                    tracing::warn!("couldn't record a visit for {}: {e}", claims.sub);
                }
            }

            Principal {
                user_id: claims.sub,
                username: claims.username,
                role,
                permissions,
                method: AuthMethod::Jwt,
            }
        }
        "apikey" => {
            // API key authentication
            let key_hash = auth_service.hash_api_key(&credentials);
            let api_key = state
                .storage
                .get_api_key_by_hash(&key_hash)
                .await
                .map_err(|_| AuthError::InternalError)?
                .ok_or(AuthError::InvalidCredentials)?;

            // Update last used timestamp
            let _ = state
                .storage
                .update_api_key_last_used(&api_key.id)
                .await;

            // Get the user associated with this API key
            let (role, permissions) = resolve_user_role(state, &api_key.user_id).await?;

            Principal {
                user_id: api_key.user_id,
                username: String::new(),
                role,
                permissions,
                method: AuthMethod::ApiKey,
            }
        }
        _ => return Err(AuthError::MissingCredentials),
    };
    Ok((user, token_expiry))
}

pub async fn require_auth(
    State(state): State<HTTPState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    if let AuthMode::Desktop { user_id } = &state.auth {
        let principal = desktop_principal(&state, user_id).await?;
        request.extensions_mut().insert(principal);
        request.extensions_mut().insert(TokenExpiry::NEVER);
        return Ok(next.run(request).await);
    }
    if request
        .extensions()
        .get::<Principal>()
        .is_some_and(|p| p.method == AuthMethod::Desktop)
    {
        return Err(AuthError::InvalidCredentials);
    }

    let (user, token_expiry) = principal_from_credentials(&state, &headers, request.uri()).await?;

    // Add the authenticated user to request extensions
    request.extensions_mut().insert(user);
    request.extensions_mut().insert(token_expiry);

    Ok(next.run(request).await)
}

/// For a route anyone may open but whose answer depends on who asks (`specs/055-shared-chat`):
/// inserts an `Option<Principal>` and never rejects. No credentials, a bad or expired token and
/// an unknown key are all "no session"; the desktop app is always its one person.
pub async fn optional_auth(
    State(state): State<HTTPState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    let who = match &state.auth {
        AuthMode::Desktop { user_id } => desktop_principal(&state, user_id).await.ok(),
        AuthMode::Network => {
            match principal_from_credentials(&state, &headers, request.uri()).await {
                Ok((principal, _)) => Some(principal),
                Err(AuthError::InternalError) => {
                    tracing::warn!("optional sign-in could not be checked; treated as no session");
                    None
                }
                Err(_) => None,
            }
        }
    };
    request.extensions_mut().insert(who);
    next.run(request).await
}

/// Middleware state for permission checking
#[derive(Clone)]
pub struct PermissionState {
    pub permission: String,
}

/// Middleware function that checks if the authenticated user has a specific permission
pub async fn require_permission(
    State(state): State<PermissionState>,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let user = request
        .extensions()
        .get::<Principal>()
        .ok_or(AuthError::InvalidCredentials)?;

    vizier_core::access::require(user, &state.permission).map_err(|_| AuthError::Forbidden)?;
    Ok(next.run(request).await)
}

/// Extract token from query parameter
/// Supports: ?token=<jwt_token>
fn extract_token_from_query(uri: &Uri) -> Option<String> {
    uri.query()
        .and_then(|query| {
            query
                .split('&')
                .find_map(|param| {
                    let mut parts = param.split('=');
                    match (parts.next(), parts.next()) {
                        (Some("token"), Some(token)) => Some(token.to_string()),
                        _ => None,
                    }
                })
        })
}

#[cfg(test)]
mod optional_auth_tests {
    use crate::auth::{AuthService, Claims};
    use super::*;
    use axum::{Extension, Router, body::Body, routing::get};
    use tower::ServiceExt;
    use vizier_core::{
        config::VizierConfig,
        runtime::{RuntimeOptions, VizierRuntime},
        storage::user::UserStorage,
    };

    async fn who(Extension(who): Extension<Option<Principal>>) -> String {
        who.map_or("anonymous".to_string(), |p| format!("{}:{:?}", p.username, p.method))
    }

    struct Fixture {
        runtime: VizierRuntime,
        user_id: String,
        token: String,
        _dir: tempfile::TempDir,
    }

    async fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
        config.channels.http.as_mut().unwrap().jwt_secret = "optional-secret".into();
        config.catalogue.refresh = false;
        let runtime = VizierRuntime::start(RuntimeOptions { config, ..Default::default() })
            .await
            .unwrap();
        let storage = runtime.deps().storage.clone();
        let role = storage.create_role("r", vec![], false).await.unwrap();
        let user = storage.create_user("ann", "", &role.role_id).await.unwrap();
        let http = runtime.deps().config.channels.http.clone().unwrap();
        let token = AuthService::new(&http).generate_token(&user.user_id, "ann", vec![]).unwrap();
        Fixture { runtime, user_id: user.user_id, token, _dir: dir }
    }

    async fn ask(runtime: &VizierRuntime, auth: AuthMode, authorization: Option<String>, uri: &str) -> (u16, String) {
        let state = crate::new_state(runtime, auth);
        let app = Router::new()
            .route("/who", get(who))
            .layer(axum::middleware::from_fn_with_state(state.clone(), optional_auth))
            .with_state(state);
        let mut req = axum::http::Request::builder().uri(uri);
        if let Some(value) = authorization {
            req = req.header("authorization", value);
        }
        let res = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
        let status = res.status().as_u16();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 16).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn it_never_rejects_and_only_a_valid_credential_is_a_person() {
        let f = fixture().await;
        let rt = &f.runtime;
        let anonymous = (200, "anonymous".to_string());
        // No header.
        assert_eq!(ask(rt, AuthMode::Network, None, "/who").await, anonymous);
        // A bad token, a malformed header, an unknown scheme.
        for bad in ["Bearer not-a-token", "Bearer", "Basic abc", "ApiKey nope"] {
            assert_eq!(ask(rt, AuthMode::Network, Some(bad.into()), "/who").await, anonymous, "{bad}");
        }
        // An expired token is anonymous too, not an error.
        let http = rt.deps().config.channels.http.clone().unwrap();
        let past = chrono::Utc::now().timestamp() as usize - 3600;
        let claims = Claims {
            sub: f.user_id.clone(),
            username: "ann".into(),
            permissions: vec![],
            iat: past - 60,
            exp: past,
        };
        let expired = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(http.jwt_secret.as_bytes()),
        )
        .unwrap();
        assert_eq!(ask(rt, AuthMode::Network, Some(format!("Bearer {expired}")), "/who").await, anonymous);
        // A valid token is the person.
        let valid = ask(rt, AuthMode::Network, Some(format!("Bearer {}", f.token)), "/who").await;
        assert_eq!(valid, (200, "ann:Jwt".to_string()));
        // So is `?token=`, as for `require_auth`.
        let by_query = ask(rt, AuthMode::Network, None, &format!("/who?token={}", f.token)).await;
        assert_eq!(by_query, (200, "ann:Jwt".to_string()));
    }

    #[tokio::test]
    async fn the_desktop_is_always_its_one_person() {
        let f = fixture().await;
        let mode = AuthMode::Desktop { user_id: f.user_id.clone() };
        assert_eq!(ask(&f.runtime, mode.clone(), None, "/who").await, (200, "ann:Desktop".to_string()));
        // Credentials on the request make no difference.
        let with_bad = ask(&f.runtime, mode, Some("Bearer junk".into()), "/who").await;
        assert_eq!(with_bad, (200, "ann:Desktop".to_string()));
        // A desktop person that no longer exists is no session rather than a failure.
        let gone = AuthMode::Desktop { user_id: "missing".into() };
        assert_eq!(ask(&f.runtime, gone, None, "/who").await, (200, "anonymous".to_string()));
    }
}
