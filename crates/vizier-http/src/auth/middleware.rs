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

/// Whether a **scoped** API key may make this request (`specs/055-live-voice` FR-062).
///
/// A scoped key acts as its owner, so every agent and permission check reads the owner's role
/// exactly as for an unscoped key. What the scope limits is *where* the key may go: a key scoped to
/// `live_voice` reaches the chat socket and nothing else, and once there it may send live voice
/// frames only (`TopicConnection::handle_inbound`). A key whose scope names nothing this release
/// knows reaches nothing. This is the one place the rule is decided.
pub fn scoped_key_may_reach(capabilities: &[String], path: &str) -> bool {
    capabilities
        .iter()
        .any(|c| c == vizier_core::service::api_keys::CAPABILITY_LIVE_VOICE)
        && is_chat_socket(path)
}

/// `/api/v1/agents/{agent}/channel/{channel}/topic/{topic}/chat`, the topic socket.
fn is_chat_socket(path: &str) -> bool {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    matches!(
        parts.as_slice(),
        ["", "api", "v1", "agents", agent, "channel", channel, "topic", topic, "chat"]
            if [agent, channel, topic].iter().all(|part| !part.is_empty())
    )
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

pub async fn require_auth(
    State(state): State<HTTPState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    if let AuthMode::Desktop { user_id } = &state.auth {
        let (role, permissions) = resolve_user_role(&state, user_id).await?;
        let user = state
            .storage
            .get_user_by_id(user_id)
            .await
            .map_err(|_| AuthError::InternalError)?
            .ok_or(AuthError::InvalidCredentials)?;
        request.extensions_mut().insert(Principal {
            user_id: user.user_id,
            username: user.username,
            role,
            permissions,
            method: AuthMethod::Desktop,
        });
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
        extract_token_from_query(request.uri())
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

            let (role, permissions) = resolve_user_role(&state, &claims.sub).await?;

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
            let (role, permissions) = resolve_user_role(&state, &api_key.user_id).await?;
            // T109/FR-062: a scoped key reaches only what its scope names. The path is the full
            // one: this middleware runs inside nested routers, which see a shortened URI.
            let scoped = api_key.capabilities.is_some();
            if let Some(capabilities) = &api_key.capabilities {
                let path = request
                    .extensions()
                    .get::<axum::extract::OriginalUri>()
                    .map(|uri| uri.0.path().to_string())
                    .unwrap_or_else(|| request.uri().path().to_string());
                if !scoped_key_may_reach(capabilities, &path) {
                    return Err(AuthError::Forbidden);
                }
            }

            Principal {
                user_id: api_key.user_id,
                username: String::new(),
                role,
                permissions,
                method: AuthMethod::ApiKey {
                    scoped,
                    // T110: the session this key opens must be findable when the key is
                    // revoked or lapses, and both facts are known here and nowhere cheaper.
                    key_id: Some(api_key.id),
                    expires_at: api_key.expires_at,
                },
            }
        }
        _ => return Err(AuthError::MissingCredentials),
    };

    // Add the authenticated user to request extensions
    request.extensions_mut().insert(user);
    request.extensions_mut().insert(token_expiry);

    Ok(next.run(request).await)
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
mod tests {
    //! T109/FR-062 - where a scoped key may go.

    use super::scoped_key_may_reach;

    const SOCKET: &str = "/api/v1/agents/helper/channel/web/topic/t1/chat";

    fn caps(list: &[&str]) -> Vec<String> {
        list.iter().map(|c| c.to_string()).collect()
    }

    #[test]
    fn a_live_voice_key_reaches_the_chat_socket() {
        assert!(scoped_key_may_reach(&caps(&["live_voice"]), SOCKET));
        assert!(scoped_key_may_reach(&caps(&["live_voice"]), &format!("{SOCKET}/")));
    }

    #[test]
    fn and_nothing_else() {
        for path in [
            "/api/v1/agents",
            "/api/v1/agents/helper",
            "/api/v1/agents/helper/channel/web/topics",
            "/api/v1/agents/helper/channel/web/topic/t1/subagents",
            "/api/v1/agents/helper/channel/web/topic/t1/chat/extra",
            "/api/v1/agents//channel/web/topic/t1/chat",
            "/api/v1/auth/api-keys",
            "/api/v1/users",
            "/api/v1/inbox/stream",
        ] {
            assert!(!scoped_key_may_reach(&caps(&["live_voice"]), path), "{path}");
        }
    }

    #[test]
    fn a_scope_that_names_nothing_known_reaches_nothing() {
        assert!(!scoped_key_may_reach(&caps(&[]), SOCKET));
        assert!(!scoped_key_may_reach(&caps(&["users:manage"]), SOCKET));
    }
}
