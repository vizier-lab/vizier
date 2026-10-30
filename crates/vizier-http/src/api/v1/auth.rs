use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthService, Principal},
    models::{
        self,
        page::{self, ListQuery, Listing, SortKey, SortOrder},
        response::{APIResponse, api_response},
    },
    state::HTTPState,
};
use vizier_core::{
    service::{ServiceError, Services, api_keys, profile, roles, setup, users},
    storage::user::{Role, User, UserProfile},
};

pub(crate) fn services(state: &HTTPState) -> Services {
    Services::new(state.deps.clone())
}

/// A service error in the API's envelope.
pub(crate) fn err<T: Serialize + Clone>(e: ServiceError) -> models::response::Response<T> {
    models::response::err_response(
        StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        e.message().to_string(),
    )
}

fn role_response(r: Role) -> RoleResponse {
    RoleResponse {
        role_id: r.role_id,
        name: r.name,
        permissions: r.permissions,
        is_system: r.is_system,
        created_at: r.created_at,
    }
}

fn user_response(user: User, role_name: Option<String>) -> UserResponse {
    UserResponse {
        user_id: user.user_id,
        username: user.username,
        role_id: user.role_id,
        role_name,
        created_at: user.created_at,
    }
}

fn profile_response(p: UserProfile) -> UserProfileResponse {
    UserProfileResponse {
        user_id: p.user_id,
        discord_id: p.discord_id,
        discord_username: p.discord_username,
        telegram_id: p.telegram_id,
        telegram_username: p.telegram_username,
        alias: p.alias,
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct SetupStatusResponse {
    pub needs_setup: bool,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct RoleResponse {
    pub role_id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateRoleRequest {
    pub name: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateRoleRequest {
    pub name: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct UserResponse {
    pub user_id: String,
    pub username: String,
    pub role_id: String,
    pub role_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role_id: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub role_id: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct AvailablePermissionsResponse {
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub name: String,
    pub key: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct ApiKeyResponse {
    pub id: String,
    pub name: String,
    /// The first characters of the key; empty for a key made before they were kept.
    pub prefix: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct CurrentUserResponse {
    pub user_id: String,
    pub username: String,
    pub role_name: String,
    pub permissions: Vec<String>,
    /// Whether the user has a password (the desktop owner may not).
    pub has_password: bool,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct UserProfileResponse {
    pub user_id: String,
    pub discord_id: Option<String>,
    pub discord_username: Option<String>,
    pub telegram_id: Option<String>,
    pub telegram_username: Option<String>,
    pub alias: Vec<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateUserProfileRequest {
    pub discord_id: Option<String>,
    pub discord_username: Option<String>,
    pub telegram_id: Option<String>,
    pub telegram_username: Option<String>,
    pub alias: Option<Vec<String>>,
}

#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = APIResponse<LoginResponse>),
        (status = 401, description = "Invalid credentials", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn login(
    State(state): State<HTTPState>,
    Json(req): Json<LoginRequest>,
) -> models::response::Response<LoginResponse> {
    let http_config = match &state.config.channels.http {
        Some(cfg) => cfg,
        None => return err(ServiceError::Internal("HTTP channel not configured".into())),
    };
    let auth_service = AuthService::new(http_config);

    let (user, role) = match users::verify_credentials(&services(&state), &req.username, &req.password).await {
        Ok(found) => found,
        Err(e) => return err(e),
    };

    // Generate JWT token with permissions
    match auth_service.generate_token(&user.user_id, &user.username, role.permissions) {
        Ok(token) => api_response(StatusCode::OK, LoginResponse { token }),
        Err(e) => {
            tracing::error!("Failed to generate token: {}", e);
            models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate token".to_string(),
            )
        }
    }
}

pub async fn get_current_user(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<CurrentUserResponse> {
    let current = users::current(&services(&state), &user).await;
    api_response(
        StatusCode::OK,
        CurrentUserResponse {
            user_id: user.user_id.clone(),
            username: user.username.clone(),
            role_name: current.role_name,
            permissions: user.permissions.clone(),
            has_password: current.has_password,
        },
    )
}

#[utoipa::path(
    post,
    path = "/auth/change-password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 204, description = "Password changed successfully"),
        (status = 401, description = "Current password is incorrect", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn change_password(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Json(req): Json<ChangePasswordRequest>,
) -> models::response::Response<String> {
    if state.config.channels.http.is_none() {
        return err(ServiceError::Internal("HTTP channel not configured".into()));
    }
    match users::change_password(&services(&state), &user, &req.current_password, &req.new_password).await {
        Ok(()) => api_response(StatusCode::NO_CONTENT, String::new()),
        Err(e) => err(e),
    }
}

#[utoipa::path(
    post,
    path = "/auth/api-keys",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 201, description = "API key created successfully", body = APIResponse<CreateApiKeyResponse>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn create_api_key(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Json(req): Json<CreateApiKeyRequest>,
) -> models::response::Response<CreateApiKeyResponse> {
    if state.config.channels.http.is_none() {
        return err(ServiceError::Internal("HTTP channel not configured".into()));
    }
    match api_keys::create(&services(&state), &user, &req.name, req.expires_in_days).await {
        Ok((api_key, raw_key)) => api_response(
            StatusCode::CREATED,
            CreateApiKeyResponse {
                id: api_key.id,
                name: api_key.name,
                key: raw_key, // Only time this is shown
                expires_at: api_key.expires_at,
                created_at: api_key.created_at,
            },
        ),
        Err(e) => err(e),
    }
}

#[utoipa::path(
    get,
    path = "/auth/api-keys",
    params(ListQuery),
    responses(
        (status = 200, description = "List of API keys, or one page of them with `limit`", body = APIResponse<Vec<ApiKeyResponse>>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn list_api_keys(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Query(query): Query<ListQuery>,
) -> models::response::Response<serde_json::Value> {
    let keys = match api_keys::list(&services(&state), &user).await {
        Ok(keys) => keys,
        Err(e) => return err(e),
    };

    let response: Vec<ApiKeyResponse> = keys
        .into_iter()
        .map(|k| ApiKeyResponse {
            id: k.id,
            name: k.name,
            prefix: k.prefix,
            expires_at: k.expires_at,
            created_at: k.created_at,
            last_used_at: k.last_used_at,
        })
        .collect();

    page::respond(response, &ApiKeyListing, &query)
}

/// No filters; sorts `created` (newest first) and `last_used` (most recent first, never-used last).
pub struct ApiKeyListing;

impl Listing<ApiKeyResponse> for ApiKeyListing {
    fn matches(&self, item: &ApiKeyResponse, q: &str) -> bool {
        item.name.to_lowercase().contains(q)
    }

    fn filter_keys(&self, _: &[ApiKeyResponse]) -> Vec<String> {
        vec![]
    }

    fn in_filter(&self, _: &str, _: &ApiKeyResponse) -> bool {
        false
    }

    fn sort_key(&self, item: &ApiKeyResponse, sort: &str) -> SortKey {
        match sort {
            "last_used" => SortKey::Num(item.last_used_at.map_or(0, |t| t.timestamp_millis())),
            _ => SortKey::Num(item.created_at.timestamp_millis()),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["created", "last_used"]
    }

    fn default_order(&self, _: &str) -> SortOrder {
        SortOrder::Desc
    }
}

#[utoipa::path(
    delete,
    path = "/auth/api-keys/{key_id}",
    params(
        ("key_id" = String, Path, description = "API key ID to delete")
    ),
    responses(
        (status = 204, description = "API key deleted successfully"),
        (status = 404, description = "API key not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn delete_api_key(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(key_id): Path<String>,
) -> models::response::Response<String> {
    match api_keys::delete(&services(&state), &user, &key_id).await {
        Ok(()) => api_response(StatusCode::NO_CONTENT, String::new()),
        Err(e) => err(e),
    }
}

// ============================================================================
// SETUP ENDPOINTS (Public - no auth required)
// ============================================================================

pub async fn setup_status(
    State(state): State<HTTPState>,
) -> models::response::Response<SetupStatusResponse> {
    match setup::needs_setup(&services(&state)).await {
        Ok(needs_setup) => api_response(StatusCode::OK, SetupStatusResponse { needs_setup }),
        Err(e) => err(e),
    }
}

pub async fn setup_first_user(
    State(state): State<HTTPState>,
    Json(req): Json<SetupRequest>,
) -> models::response::Response<LoginResponse> {
    let services = services(&state);
    let http_config = match &state.config.channels.http {
        Some(cfg) => cfg,
        None => return err(ServiceError::Internal("HTTP channel not configured".into())),
    };
    let auth_service = AuthService::new(http_config);
    let (user, role) = match setup::first_user(&services, &req.username, &req.password).await {
        Ok(created) => created,
        Err(e) => return err(e),
    };

    // Generate JWT token
    match auth_service.generate_token(&user.user_id, &user.username, role.permissions) {
        Ok(token) => api_response(StatusCode::CREATED, LoginResponse { token }),
        Err(e) => {
            tracing::error!("Failed to generate token: {}", e);
            models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate token".to_string(),
            )
        }
    }
}

// ============================================================================
// ROLE MANAGEMENT ENDPOINTS
// ============================================================================

/// A role with how many people have it.
#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct RoleItem {
    #[serde(flatten)]
    pub role: RoleResponse,
    pub people: usize,
}

/// Filters `builtin` and `custom`; sorts `people` (most first) and `name`.
pub struct RoleListing;

impl Listing<RoleItem> for RoleListing {
    fn matches(&self, item: &RoleItem, q: &str) -> bool {
        item.role.name.to_lowercase().contains(q)
    }

    fn filter_keys(&self, _: &[RoleItem]) -> Vec<String> {
        vec!["builtin".into(), "custom".into()]
    }

    fn in_filter(&self, filter: &str, item: &RoleItem) -> bool {
        match filter {
            "builtin" => item.role.is_system,
            "custom" => !item.role.is_system,
            _ => false,
        }
    }

    fn sort_key(&self, item: &RoleItem, sort: &str) -> SortKey {
        match sort {
            "name" => SortKey::text(&item.role.name),
            _ => SortKey::Num(item.people as i64),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["people", "name"]
    }

    fn default_order(&self, sort: &str) -> SortOrder {
        if sort == "name" { SortOrder::Asc } else { SortOrder::Desc }
    }
}

pub async fn list_roles(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Query(query): Query<ListQuery>,
) -> models::response::Response<serde_json::Value> {
    let roles = match roles::list(&services(&state), &user).await {
        Ok(roles) => roles,
        Err(e) => return err(e),
    };
    let response: Vec<RoleItem> = roles
        .into_iter()
        .map(|(r, people)| RoleItem { people, role: role_response(r) })
        .collect();

    page::respond(response, &RoleListing, &query)
}

/// One role with how many people hold it: what the Roles page opens when the role is not on the
/// page of the list it is showing.
pub async fn get_role(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(role_id): Path<String>,
) -> models::response::Response<RoleItem> {
    match roles::get(&services(&state), &user, &role_id).await {
        Ok((role, people)) => api_response(StatusCode::OK, RoleItem { people, role: role_response(role) }),
        Err(e) => err(e),
    }
}

pub async fn create_role(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Json(req): Json<CreateRoleRequest>,
) -> models::response::Response<RoleResponse> {
    match roles::create(&services(&state), &user, &req.name, req.permissions).await {
        Ok(role) => api_response(StatusCode::CREATED, role_response(role)),
        Err(e) => err(e),
    }
}

pub async fn update_role(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(role_id): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> models::response::Response<()> {
    match roles::update(&services(&state), &user, &role_id, &req.name, req.permissions).await {
        Ok(()) => api_response(StatusCode::OK, ()),
        Err(e) => err(e),
    }
}

pub async fn delete_role(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(role_id): Path<String>,
) -> models::response::Response<()> {
    match roles::delete(&services(&state), &user, &role_id).await {
        Ok(()) => api_response(StatusCode::NO_CONTENT, ()),
        Err(e) => err(e),
    }
}

pub async fn available_permissions(
    Extension(user): Extension<Principal>,
) -> models::response::Response<AvailablePermissionsResponse> {
    match roles::available_permissions(&user) {
        Ok(permissions) => api_response(StatusCode::OK, AvailablePermissionsResponse { permissions }),
        Err(e) => err(e),
    }
}

// ============================================================================
// USER MANAGEMENT ENDPOINTS
// ============================================================================

/// Filters `role:<name>`, one per role someone has; sorts `created` (newest first) and `username`.
pub struct UserListing;

fn role_of(user: &UserResponse) -> &str {
    user.role_name.as_deref().unwrap_or(&user.role_id)
}

impl Listing<UserResponse> for UserListing {
    fn matches(&self, item: &UserResponse, q: &str) -> bool {
        item.username.to_lowercase().contains(q)
    }

    fn filter_keys(&self, items: &[UserResponse]) -> Vec<String> {
        let mut roles: Vec<String> = items.iter().map(|u| format!("role:{}", role_of(u))).collect();
        roles.sort();
        roles.dedup();
        roles
    }

    fn in_filter(&self, filter: &str, item: &UserResponse) -> bool {
        filter.strip_prefix("role:").is_some_and(|role| role == role_of(item))
    }

    fn sort_key(&self, item: &UserResponse, sort: &str) -> SortKey {
        match sort {
            "username" => SortKey::text(&item.username),
            _ => SortKey::Num(item.created_at.timestamp_millis()),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["created", "username"]
    }

    fn default_order(&self, sort: &str) -> SortOrder {
        if sort == "username" { SortOrder::Asc } else { SortOrder::Desc }
    }
}

pub async fn list_users(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Query(query): Query<ListQuery>,
) -> models::response::Response<serde_json::Value> {
    let found = match users::list(&services(&state), &user).await {
        Ok(found) => found,
        Err(e) => return err(e),
    };
    let response: Vec<UserResponse> = found
        .into_iter()
        .map(|(user, role_name)| user_response(user, role_name))
        .collect();

    page::respond(response, &UserListing, &query)
}

pub async fn create_user(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Json(req): Json<CreateUserRequest>,
) -> models::response::Response<UserResponse> {
    if state.config.channels.http.is_none() {
        return err(ServiceError::Internal("HTTP channel not configured".into()));
    }
    match users::create(&services(&state), &user, &req.username, &req.password, req.role_id).await {
        Ok((created, role_name)) => api_response(StatusCode::CREATED, user_response(created, role_name)),
        Err(e) => err(e),
    }
}

pub async fn update_user(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(user_id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> models::response::Response<()> {
    if state.config.channels.http.is_none() {
        return err(ServiceError::Internal("HTTP channel not configured".into()));
    }
    match users::update(
        &services(&state),
        &user,
        &user_id,
        req.username.as_deref(),
        req.role_id.as_deref(),
        req.password.as_deref(),
    )
    .await
    {
        Ok(()) => api_response(StatusCode::OK, ()),
        Err(e) => err(e),
    }
}

pub async fn delete_user(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(user_id): Path<String>,
) -> models::response::Response<()> {
    match users::delete(&services(&state), &user, &user_id).await {
        Ok(()) => api_response(StatusCode::NO_CONTENT, ()),
        Err(e) => err(e),
    }
}

pub async fn get_my_profile(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<UserProfileResponse> {
    match profile::get(&services(&state), &user).await {
        Ok(profile) => api_response(StatusCode::OK, profile_response(profile)),
        Err(e) => err(e),
    }
}

pub async fn update_my_profile(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Json(req): Json<UpdateUserProfileRequest>,
) -> models::response::Response<UserProfileResponse> {
    let changes = profile::ProfileChanges {
        discord_id: req.discord_id,
        discord_username: req.discord_username,
        telegram_id: req.telegram_id,
        telegram_username: req.telegram_username,
        alias: req.alias,
    };
    match profile::update(&services(&state), &user, changes).await {
        Ok(updated) => api_response(StatusCode::OK, profile_response(updated)),
        Err(e) => err(e),
    }
}
