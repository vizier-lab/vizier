use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::channels::http::{
    auth::{AuthService, AuthenticatedUser},
    models::{self, response::{api_response, APIResponse}},
    state::HTTPState,
};
use crate::storage::user::{AVAILABLE_PERMISSIONS, UserProfile, UserStorage};

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
        None => {
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HTTP channel not configured".to_string(),
            );
        }
    };

    let auth_service = AuthService::new(http_config);

    // Get user from storage
    let user = match state.storage.get_user(&req.username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return models::response::err_response(
                StatusCode::UNAUTHORIZED,
                "Invalid username or password".to_string(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    };

    // Verify password
    match auth_service.verify_password(&req.password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return models::response::err_response(
                StatusCode::UNAUTHORIZED,
                "Invalid username or password".to_string(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to verify password: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    }

    // Get user's role
    let role = match state.storage.get_role(&user.role_id).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "User role not found".to_string(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to get role: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
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
    Extension(user): Extension<AuthenticatedUser>,
) -> models::response::Response<CurrentUserResponse> {
    let role_name = state
        .storage
        .get_role(&user.role.role_id)
        .await
        .ok()
        .flatten()
        .map(|r| r.name)
        .unwrap_or_default();

    api_response(
        StatusCode::OK,
        CurrentUserResponse {
            user_id: user.user_id.clone(),
            username: user.username.clone(),
            role_name,
            permissions: user.permissions.clone(),
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
    Extension(user): Extension<AuthenticatedUser>,
    Json(req): Json<ChangePasswordRequest>,
) -> models::response::Response<String> {
    let http_config = match &state.config.channels.http {
        Some(cfg) => cfg,
        None => {
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HTTP channel not configured".to_string(),
            );
        }
    };

    let auth_service = AuthService::new(http_config);

    // Get current user data to verify current password
    let current_user = match state.storage.get_user(&user.username).await {
        Ok(Some(u)) => u,
        _ => {
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get user data".to_string(),
            );
        }
    };

    // Verify current password
    match auth_service.verify_password(&req.current_password, &current_user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return models::response::err_response(
                StatusCode::UNAUTHORIZED,
                "Current password is incorrect".to_string(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to verify password: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    }

    // Hash new password
    let new_password_hash = match auth_service.hash_password(&req.new_password) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("Failed to hash new password: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to process new password".to_string(),
            );
        }
    };

    // Update password in storage
    match state.storage.update_password(&user.user_id, &new_password_hash).await {
        Ok(_) => api_response(StatusCode::NO_CONTENT, String::new()),
        Err(e) => {
            tracing::error!("Failed to update password: {}", e);
            models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update password".to_string(),
            )
        }
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
    Extension(user): Extension<AuthenticatedUser>,
    Json(req): Json<CreateApiKeyRequest>,
) -> models::response::Response<CreateApiKeyResponse> {
    let http_config = match &state.config.channels.http {
        Some(cfg) => cfg,
        None => {
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HTTP channel not configured".to_string(),
            );
        }
    };

    let auth_service = AuthService::new(http_config);

    // Generate new API key (raw key shown once, hash stored)
    let (raw_key, key_hash) = auth_service.generate_api_key();

    // Calculate expiration
    let expires_at = req.expires_in_days.map(|days| Utc::now() + Duration::days(days));

    // Store API key
    let api_key = match state
        .storage
        .create_api_key(&user.user_id, &req.name, &key_hash, expires_at)
        .await
    {
        Ok(key) => key,
        Err(e) => {
            tracing::error!("Failed to create API key: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create API key".to_string(),
            );
        }
    };

    api_response(
        StatusCode::CREATED,
        CreateApiKeyResponse {
            id: api_key.id,
            name: api_key.name,
            key: raw_key, // Only time this is shown
            expires_at: api_key.expires_at,
            created_at: api_key.created_at,
        },
    )
}

#[utoipa::path(
    get,
    path = "/auth/api-keys",
    responses(
        (status = 200, description = "List of API keys", body = APIResponse<Vec<ApiKeyResponse>>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn list_api_keys(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> models::response::Response<Vec<ApiKeyResponse>> {
    let keys = match state.storage.list_api_keys(&user.user_id).await {
        Ok(keys) => keys,
        Err(e) => {
            tracing::error!("Failed to list API keys: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to list API keys".to_string(),
            );
        }
    };

    let response: Vec<ApiKeyResponse> = keys
        .into_iter()
        .map(|k| ApiKeyResponse {
            id: k.id,
            name: k.name,
            expires_at: k.expires_at,
            created_at: k.created_at,
            last_used_at: k.last_used_at,
        })
        .collect();

    api_response(StatusCode::OK, response)
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
    Extension(user): Extension<AuthenticatedUser>,
    Path(key_id): Path<String>,
) -> models::response::Response<String> {
    // First verify the key belongs to the user
    let keys = match state.storage.list_api_keys(&user.user_id).await {
        Ok(keys) => keys,
        Err(e) => {
            tracing::error!("Failed to list API keys: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to verify API key ownership".to_string(),
            );
        }
    };

    if !keys.iter().any(|k| k.id == key_id) {
        return models::response::err_response(
            StatusCode::NOT_FOUND,
            "API key not found".to_string(),
        );
    }

    match state.storage.delete_api_key(&key_id).await {
        Ok(_) => api_response(StatusCode::NO_CONTENT, String::new()),
        Err(e) => {
            tracing::error!("Failed to delete API key: {}", e);
            models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete API key".to_string(),
            )
        }
    }
}

// ============================================================================
// SETUP ENDPOINTS (Public - no auth required)
// ============================================================================

pub async fn setup_status(
    State(state): State<HTTPState>,
) -> models::response::Response<SetupStatusResponse> {
    let needs_setup = match state.storage.user_exists().await {
        Ok(exists) => !exists,
        Err(e) => {
            tracing::error!("Failed to check user existence: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    };

    api_response(StatusCode::OK, SetupStatusResponse { needs_setup })
}

pub async fn setup_first_user(
    State(state): State<HTTPState>,
    Json(req): Json<SetupRequest>,
) -> models::response::Response<LoginResponse> {
    // Check if any users exist
    match state.storage.user_exists().await {
        Ok(true) => {
            return models::response::err_response(
                StatusCode::BAD_REQUEST,
                "Setup already completed".to_string(),
            );
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!("Failed to check user existence: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    }

    let http_config = match &state.config.channels.http {
        Some(cfg) => cfg,
        None => {
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HTTP channel not configured".to_string(),
            );
        }
    };

    let auth_service = AuthService::new(http_config);

    // Create system role (superadmin) if it doesn't exist
    let system_role = match state.storage.get_system_role().await {
        Ok(Some(role)) => role,
        Ok(None) => {
            match state
                .storage
                .create_role(
                    "superadmin",
                    AVAILABLE_PERMISSIONS.to_vec().into_iter().map(String::from).collect(),
                    true,
                )
                .await
            {
                Ok(role) => role,
                Err(e) => {
                    tracing::error!("Failed to create system role: {}", e);
                    return models::response::err_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to create system role".to_string(),
                    );
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get system role: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    };

    // Hash password
    let password_hash = match auth_service.hash_password(&req.password) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("Failed to hash password: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to process password".to_string(),
            );
        }
    };

    // Create user
    let user = match state
        .storage
        .create_user(&req.username, &password_hash, &system_role.role_id)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to create user: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create user".to_string(),
            );
        }
    };

    // Generate JWT token
    match auth_service.generate_token(&user.user_id, &user.username, system_role.permissions) {
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

pub async fn list_roles(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<RoleResponse>> {
    let roles = match state.storage.list_roles().await {
        Ok(roles) => roles,
        Err(e) => {
            tracing::error!("Failed to list roles: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to list roles".to_string(),
            );
        }
    };

    let response: Vec<RoleResponse> = roles
        .into_iter()
        .map(|r| RoleResponse {
            role_id: r.role_id,
            name: r.name,
            permissions: r.permissions,
            is_system: r.is_system,
            created_at: r.created_at,
        })
        .collect();

    api_response(StatusCode::OK, response)
}

pub async fn create_role(
    State(state): State<HTTPState>,
    Json(req): Json<CreateRoleRequest>,
) -> models::response::Response<RoleResponse> {
    // Validate permissions
    for perm in &req.permissions {
        if !AVAILABLE_PERMISSIONS.contains(&perm.as_str()) {
            return models::response::err_response(
                StatusCode::BAD_REQUEST,
                format!("Invalid permission: {}", perm),
            );
        }
    }

    let role = match state
        .storage
        .create_role(&req.name, req.permissions, false)
        .await
    {
        Ok(role) => role,
        Err(e) => {
            tracing::error!("Failed to create role: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create role".to_string(),
            );
        }
    };

    api_response(
        StatusCode::CREATED,
        RoleResponse {
            role_id: role.role_id,
            name: role.name,
            permissions: role.permissions,
            is_system: role.is_system,
            created_at: role.created_at,
        },
    )
}

pub async fn update_role(
    State(state): State<HTTPState>,
    Path(role_id): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> models::response::Response<()> {
    // Check if role exists and is not system role
    let role = match state.storage.get_role(&role_id).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return models::response::err_response(
                StatusCode::NOT_FOUND,
                "Role not found".to_string(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to get role: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    };

    if role.is_system {
        return models::response::err_response(
            StatusCode::BAD_REQUEST,
            "Cannot modify system role".to_string(),
        );
    }

    // Validate permissions
    for perm in &req.permissions {
        if !AVAILABLE_PERMISSIONS.contains(&perm.as_str()) {
            return models::response::err_response(
                StatusCode::BAD_REQUEST,
                format!("Invalid permission: {}", perm),
            );
        }
    }

    match state
        .storage
        .update_role(&role_id, &req.name, req.permissions)
        .await
    {
        Ok(_) => api_response(StatusCode::OK, ()),
        Err(e) => {
            tracing::error!("Failed to update role: {}", e);
            models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update role".to_string(),
            )
        }
    }
}

pub async fn delete_role(
    State(state): State<HTTPState>,
    Path(role_id): Path<String>,
) -> models::response::Response<()> {
    // Check if role exists and is not system role
    let role = match state.storage.get_role(&role_id).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return models::response::err_response(
                StatusCode::NOT_FOUND,
                "Role not found".to_string(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to get role: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    };

    if role.is_system {
        return models::response::err_response(
            StatusCode::BAD_REQUEST,
            "Cannot delete system role".to_string(),
        );
    }

    // Check if any users are assigned to this role
    if let Ok(users) = state.storage.list_users().await {
        let users_with_role = users.iter().filter(|u| u.role_id == role_id).count();
        if users_with_role > 0 {
            return models::response::err_response(
                StatusCode::BAD_REQUEST,
                format!("Cannot delete role: {} user(s) are assigned to it", users_with_role),
            );
        }
    }

    match state.storage.delete_role(&role_id).await {
        Ok(_) => api_response(StatusCode::NO_CONTENT, ()),
        Err(e) => {
            tracing::error!("Failed to delete role: {}", e);
            models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete role".to_string(),
            )
        }
    }
}

pub async fn available_permissions() -> models::response::Response<AvailablePermissionsResponse> {
    api_response(
        StatusCode::OK,
        AvailablePermissionsResponse {
            permissions: AVAILABLE_PERMISSIONS.to_vec().into_iter().map(String::from).collect(),
        },
    )
}

// ============================================================================
// USER MANAGEMENT ENDPOINTS
// ============================================================================

pub async fn list_users(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<UserResponse>> {
    let users = match state.storage.list_users().await {
        Ok(users) => users,
        Err(e) => {
            tracing::error!("Failed to list users: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to list users".to_string(),
            );
        }
    };

    let mut response = Vec::new();
    for user in users {
        let role_name = state
            .storage
            .get_role(&user.role_id)
            .await
            .ok()
            .flatten()
            .map(|r| r.name);

        response.push(UserResponse {
            user_id: user.user_id,
            username: user.username,
            role_id: user.role_id,
            role_name,
            created_at: user.created_at,
        });
    }

    api_response(StatusCode::OK, response)
}

pub async fn create_user(
    State(state): State<HTTPState>,
    Json(req): Json<CreateUserRequest>,
) -> models::response::Response<UserResponse> {
    let http_config = match &state.config.channels.http {
        Some(cfg) => cfg,
        None => {
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HTTP channel not configured".to_string(),
            );
        }
    };

    let auth_service = AuthService::new(http_config);

    // Determine role
    let role_id = match req.role_id {
        Some(role_id) => {
            // Verify role exists
            match state.storage.get_role(&role_id).await {
                Ok(Some(_)) => role_id,
                Ok(None) => {
                    return models::response::err_response(
                        StatusCode::BAD_REQUEST,
                        "Role not found".to_string(),
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to get role: {}", e);
                    return models::response::err_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal server error".to_string(),
                    );
                }
            }
        }
        None => {
            // Use default user role
            match state.storage.list_roles().await {
                Ok(roles) => {
                    let existing_role = roles
                        .into_iter()
                        .find(|r| !r.is_system && r.name == "user")
                        .map(|r| r.role_id);

                    match existing_role {
                        Some(role_id) => role_id,
                        None => {
                            // Create default user role
                            match state
                                .storage
                                .create_role("user", vec!["agents:view".to_string(), "settings:password".to_string(), "settings:api_keys".to_string()], false)
                                .await
                            {
                                Ok(r) => r.role_id,
                                Err(e) => {
                                    tracing::error!("Failed to create default role: {}", e);
                                    return models::response::err_response(
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        "Internal server error".to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to list roles: {}", e);
                    return models::response::err_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal server error".to_string(),
                    );
                }
            }
        }
    };

    // Hash password
    let password_hash = match auth_service.hash_password(&req.password) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("Failed to hash password: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to process password".to_string(),
            );
        }
    };

    // Create user
    let user = match state
        .storage
        .create_user(&req.username, &password_hash, &role_id)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to create user: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create user".to_string(),
            );
        }
    };

    let role_name = state
        .storage
        .get_role(&user.role_id)
        .await
        .ok()
        .flatten()
        .map(|r| r.name);

    api_response(
        StatusCode::CREATED,
        UserResponse {
            user_id: user.user_id,
            username: user.username,
            role_id: user.role_id,
            role_name,
            created_at: user.created_at,
        },
    )
}

pub async fn update_user(
    State(state): State<HTTPState>,
    Path(user_id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> models::response::Response<()> {
    let http_config = match &state.config.channels.http {
        Some(cfg) => cfg,
        None => {
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HTTP channel not configured".to_string(),
            );
        }
    };

    let auth_service = AuthService::new(http_config);

    // Check if user exists
    let existing_user = match state.storage.get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return models::response::err_response(
                StatusCode::NOT_FOUND,
                "User not found".to_string(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    };

    // Check if trying to modify system user
    let system_role = state.storage.get_system_role().await.ok().flatten();
    if let Some(ref system_role) = system_role {
        if existing_user.role_id == system_role.role_id {
            // System user can only change their own password
            if req.username.is_some() || req.role_id.is_some() {
                return models::response::err_response(
                    StatusCode::BAD_REQUEST,
                    "Cannot modify system user's username or role".to_string(),
                );
            }
        }
    }

    // Verify role exists if provided
    if let Some(ref role_id) = req.role_id {
        match state.storage.get_role(role_id).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                return models::response::err_response(
                    StatusCode::BAD_REQUEST,
                    "Role not found".to_string(),
                );
            }
            Err(e) => {
                tracing::error!("Failed to get role: {}", e);
                return models::response::err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                );
            }
        }
    }

    // Update user
    match state
        .storage
        .update_user(&user_id, req.username.as_deref(), req.role_id.as_deref())
        .await
    {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Failed to update user: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update user".to_string(),
            );
        }
    }

    // Update password if provided
    if let Some(password) = req.password {
        let password_hash = match auth_service.hash_password(&password) {
            Ok(hash) => hash,
            Err(e) => {
                tracing::error!("Failed to hash password: {}", e);
                return models::response::err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to process password".to_string(),
                );
            }
        };

        match state.storage.update_password(&user_id, &password_hash).await {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Failed to update password: {}", e);
                return models::response::err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update password".to_string(),
                );
            }
        }
    }

    api_response(StatusCode::OK, ())
}

pub async fn delete_user(
    State(state): State<HTTPState>,
    Path(user_id): Path<String>,
) -> models::response::Response<()> {
    // Check if user exists
    let user = match state.storage.get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return models::response::err_response(
                StatusCode::NOT_FOUND,
                "User not found".to_string(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            );
        }
    };

    // Check if trying to delete system user
    let system_role = state.storage.get_system_role().await.ok().flatten();
    if let Some(ref system_role) = system_role {
        if user.role_id == system_role.role_id {
            return models::response::err_response(
                StatusCode::BAD_REQUEST,
                "Cannot delete system user".to_string(),
            );
        }
    }

    match state.storage.delete_user(&user_id).await {
        Ok(_) => api_response(StatusCode::NO_CONTENT, ()),
        Err(e) => {
            tracing::error!("Failed to delete user: {}", e);
            models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete user".to_string(),
            )
        }
    }
}

pub async fn get_my_profile(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> models::response::Response<UserProfileResponse> {
    let profile = match state.storage.get_user_profile(&user.user_id).await {
        Ok(profile) => profile,
        Err(e) => {
            tracing::error!("Failed to get user profile: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get profile".to_string(),
            );
        }
    };

    let profile = profile.unwrap_or_else(|| UserProfile {
        user_id: user.user_id.clone(),
        discord_id: None,
        discord_username: None,
        telegram_id: None,
        telegram_username: None,
        alias: Vec::new(),
    });

    api_response(
        StatusCode::OK,
        UserProfileResponse {
            user_id: profile.user_id,
            discord_id: profile.discord_id,
            discord_username: profile.discord_username,
            telegram_id: profile.telegram_id,
            telegram_username: profile.telegram_username,
            alias: profile.alias,
        },
    )
}

pub async fn update_my_profile(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(req): Json<UpdateUserProfileRequest>,
) -> models::response::Response<UserProfileResponse> {
    // Get existing profile or create default
    let existing = match state.storage.get_user_profile(&user.user_id).await {
        Ok(profile) => profile,
        Err(e) => {
            tracing::error!("Failed to get user profile: {}", e);
            return models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get profile".to_string(),
            );
        }
    };

    let existing = existing.unwrap_or_else(|| UserProfile {
        user_id: user.user_id.clone(),
        discord_id: None,
        discord_username: None,
        telegram_id: None,
        telegram_username: None,
        alias: Vec::new(),
    });

    // Merge: use provided values or keep existing
    let updated = UserProfile {
        user_id: user.user_id.clone(),
        discord_id: req.discord_id.or(existing.discord_id),
        discord_username: req.discord_username.or(existing.discord_username),
        telegram_id: req.telegram_id.or(existing.telegram_id),
        telegram_username: req.telegram_username.or(existing.telegram_username),
        alias: req.alias.unwrap_or(existing.alias),
    };

    match state
        .storage
        .upsert_user_profile(&user.user_id, &updated)
        .await
    {
        Ok(_) => api_response(
            StatusCode::OK,
            UserProfileResponse {
                user_id: updated.user_id,
                discord_id: updated.discord_id,
                discord_username: updated.discord_username,
                telegram_id: updated.telegram_id,
                telegram_username: updated.telegram_username,
                alias: updated.alias,
            },
        ),
        Err(e) => {
            tracing::error!("Failed to update user profile: {}", e);
            models::response::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update profile".to_string(),
            )
        }
    }
}
