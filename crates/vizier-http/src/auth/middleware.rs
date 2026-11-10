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

/// What a key carries, and whether it is scoped (T109, FR-062).
///
/// `None` is a key made before scopes existed, and every key the settings page makes today: it
/// carries its owner's permissions, and nothing about those keys changes. `Some(list)` carries the
/// list and nothing else — that is the whole of the scope, and it is why this decision lives in one
/// small function instead of inline in the middleware where it could not be tested.
pub fn key_permissions(
    capabilities: Option<Vec<String>>,
    owner: Vec<String>,
) -> (Vec<String>, bool) {
    match capabilities {
        Some(capabilities) => (capabilities, true),
        None => (owner, false),
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
            // T109/FR-062: a scoped key carries **only** its own capabilities and inherits nothing
            // from its owner. `role` stays as it is — it is a label and a few lookups read it — but
            // every permission check reads `permissions`, and this is the line that makes a key
            // handed to a third party unable to do anything else. `None` is a key made before
            // scopes existed: it keeps its owner's permissions exactly as before.
            let (permissions, scoped) = key_permissions(api_key.capabilities, permissions);

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
    //! T109/FR-062 — what a scoped key carries, and what it is refused.
    //!
    //! **What is not here.** The live socket's refusal of a scoped key that lacks `live_voice` is
    //! asserted one layer down, on the predicate in `topic.rs` (`may_start_live_voice`): building a
    //! `TopicConnection` needs an `HTTPState` and a topic that passes `authorize_topic`, and that
    //! harness does not exist (the same `TODO(C5)` `live_voice_frames.rs` carries). The predicate is
    //! the decision; the socket arm only calls it.

    use super::key_permissions;
    use crate::auth::AuthMethod;
    use vizier_core::access;
    use vizier_core::auth::Principal;
    use vizier_core::storage::user::Role;

    fn owner_permissions() -> Vec<String> {
        vec![
            "settings:api_keys".into(),
            "users:manage".into(),
            "live_voice".into(),
        ]
    }

    fn principal(permissions: Vec<String>, scoped: bool, is_system: bool) -> Principal {
        Principal {
            user_id: "olive".into(),
            username: String::new(),
            role: Role {
                role_id: "r".into(),
                name: "role".into(),
                permissions: owner_permissions(),
                is_system,
                created_at: vizier_core::time::now(),
            },
            permissions,
            method: AuthMethod::ApiKey { scoped, key_id: Some("k-test".into()), expires_at: None },
        }
    }

    #[test]
    fn a_scoped_key_carries_only_its_capabilities() {
        let (permissions, scoped) =
            key_permissions(Some(vec!["live_voice".into()]), owner_permissions());
        assert!(scoped);
        assert_eq!(permissions, vec!["live_voice".to_string()]);
        // The point of the whole change: nothing of the owner's comes along.
        assert!(!permissions.iter().any(|p| p == "users:manage"));
        assert!(!permissions.iter().any(|p| p == "settings:api_keys"));
    }

    #[test]
    fn a_key_without_capabilities_behaves_exactly_as_before() {
        let (permissions, scoped) = key_permissions(None, owner_permissions());
        assert!(!scoped, "an unscoped key is not scoped");
        assert_eq!(permissions, owner_permissions(), "it still carries its owner's");
        // Which means the settings page keeps working with no visible change.
        assert!(permissions.iter().any(|p| p == "settings:api_keys"));
    }

    #[test]
    fn an_empty_scope_carries_nothing() {
        let (permissions, scoped) = key_permissions(Some(vec![]), owner_permissions());
        assert!(scoped);
        assert!(permissions.is_empty(), "an empty list is a key that may do nothing");
    }

    #[test]
    fn a_scoped_key_is_refused_where_its_owner_is_allowed() {
        // The owner may manage users; the scoped key may not, and `access::require` is where that
        // is decided for every endpoint at once.
        let scoped = principal(vec!["live_voice".into()], true, false);
        assert!(access::require(&scoped, "users:manage").is_err());
        assert!(access::require(&scoped, "settings:api_keys").is_err());
        // Its own capability is allowed, which is what makes it useful at all.
        assert!(access::require(&scoped, "live_voice").is_ok());

        // And the unscoped key of the same owner keeps its access — no regression for the keys
        // people already hold.
        let full = principal(owner_permissions(), false, false);
        assert!(access::require(&full, "users:manage").is_ok());
    }

    #[test]
    fn a_scoped_key_of_an_administrator_is_still_not_a_system_caller() {
        // Without this the scope would be a decoration: `require` short-circuits for system roles.
        let scoped = principal(vec!["live_voice".into()], true, true);
        assert!(access::require(&scoped, "users:manage").is_err());
        assert!(access::require(&scoped, "live_voice").is_ok());

        let full = principal(owner_permissions(), false, true);
        assert!(access::require(&full, "users:manage").is_ok(), "a system key keeps everything");
    }
}
