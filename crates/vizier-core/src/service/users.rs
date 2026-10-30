//! People: sign-in checks, the current user, password changes, and user management
//! (`specs/038-desktop-app-edition`, US2). Messages are the HTTP API's, word for word.

use crate::{
    access,
    auth::{AuthMethod, Principal, password},
    service::{ServiceError, Services},
    storage::user::{Role, User, UserStorage},
};

/// Logs `e` and returns an internal error carrying only `message`.
pub(crate) fn fail(message: &str, e: impl std::fmt::Display) -> ServiceError {
    tracing::error!("{message}: {e}");
    ServiceError::Internal(message.to_string())
}

fn not_found(message: &str) -> ServiceError {
    ServiceError::NotFound(message.to_string())
}

fn bad(message: impl Into<String>) -> ServiceError {
    ServiceError::invalid(message)
}

/// Checks a username and password and returns the person with their role. A user without a
/// password (the desktop owner) never verifies.
pub async fn verify_credentials(
    services: &Services,
    username: &str,
    password_in: &str,
) -> Result<(User, Role), ServiceError> {
    let storage = &services.deps().storage;
    let user = match storage.get_user(username).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(ServiceError::Unauthorized("Invalid username or password".into())),
        Err(e) => return Err(fail("Internal server error", e)),
    };
    if !password::verify(password_in, &user.password_hash) {
        return Err(ServiceError::Unauthorized("Invalid username or password".into()));
    }
    let role = match storage.get_role(&user.role_id).await {
        Ok(Some(role)) => role,
        Ok(None) => return Err(ServiceError::Internal("User role not found".into())),
        Err(e) => return Err(fail("Internal server error", e)),
    };
    Ok((user, role))
}

pub struct CurrentUser {
    pub role_name: String,
    pub has_password: bool,
}

pub async fn current(services: &Services, principal: &Principal) -> CurrentUser {
    let storage = &services.deps().storage;
    let role_name = storage
        .get_role(&principal.role.role_id)
        .await
        .ok()
        .flatten()
        .map(|r| r.name)
        .unwrap_or_default();
    let has_password = storage
        .get_user_by_id(&principal.user_id)
        .await
        .ok()
        .flatten()
        .is_some_and(|u| password::has_password(&u));
    CurrentUser { role_name, has_password }
}

/// Changes the caller's own password. Needs `settings:password`. The desktop app's window is
/// already the owner of the workspace and its files, so it may leave `current_password` empty:
/// that is how a forgotten password is replaced (research R10). Any other caller, a browser
/// signed in over web access included, must give the current one.
pub async fn change_password(
    services: &Services,
    principal: &Principal,
    current_password: &str,
    new_password: &str,
) -> Result<(), ServiceError> {
    access::require(principal, "settings:password")?;
    let storage = &services.deps().storage;
    let current_user = match storage.get_user(&principal.username).await {
        Ok(Some(u)) => u,
        _ => return Err(ServiceError::Internal("Failed to get user data".into())),
    };
    let desktop_reset = principal.method == AuthMethod::Desktop && current_password.is_empty();
    if !desktop_reset && !password::verify(current_password, &current_user.password_hash) {
        return Err(ServiceError::Unauthorized("Current password is incorrect".into()));
    }
    let hash = password::hash(new_password).map_err(|e| fail("Failed to process new password", e.0))?;
    storage
        .update_password(&principal.user_id, &hash)
        .await
        .map_err(|e| fail("Failed to update password", e))
}

/// Every user with their role's name. Needs `users:manage`.
pub async fn list(services: &Services, principal: &Principal) -> Result<Vec<(User, Option<String>)>, ServiceError> {
    access::require(principal, "users:manage")?;
    let storage = &services.deps().storage;
    let users = storage.list_users().await.map_err(|e| fail("Failed to list users", e))?;
    let mut out = Vec::new();
    for user in users {
        let role_name = storage.get_role(&user.role_id).await.ok().flatten().map(|r| r.name);
        out.push((user, role_name));
    }
    Ok(out)
}

pub async fn create(
    services: &Services,
    principal: &Principal,
    username: &str,
    password_in: &str,
    role_id: Option<String>,
) -> Result<(User, Option<String>), ServiceError> {
    access::require(principal, "users:manage")?;
    let storage = &services.deps().storage;
    let role_id = match role_id {
        Some(role_id) => match storage.get_role(&role_id).await {
            Ok(Some(_)) => role_id,
            Ok(None) => return Err(bad("Role not found")),
            Err(e) => return Err(fail("Internal server error", e)),
        },
        None => {
            let roles = storage.list_roles().await.map_err(|e| fail("Internal server error", e))?;
            match roles.into_iter().find(|r| !r.is_system && r.name == "user") {
                Some(role) => role.role_id,
                None => {
                    storage
                        .create_role(
                            "user",
                            vec![
                                "agents:view".to_string(),
                                "settings:password".to_string(),
                                "settings:api_keys".to_string(),
                            ],
                            false,
                        )
                        .await
                        .map_err(|e| fail("Internal server error", e))?
                        .role_id
                }
            }
        }
    };
    let hash = password::hash(password_in).map_err(|e| fail("Failed to process password", e.0))?;
    let user = storage
        .create_user(username, &hash, &role_id)
        .await
        .map_err(|e| fail("Failed to create user", e))?;
    let role_name = storage.get_role(&user.role_id).await.ok().flatten().map(|r| r.name);
    Ok((user, role_name))
}

pub async fn update(
    services: &Services,
    principal: &Principal,
    user_id: &str,
    username: Option<&str>,
    role_id: Option<&str>,
    new_password: Option<&str>,
) -> Result<(), ServiceError> {
    access::require(principal, "users:manage")?;
    let storage = &services.deps().storage;
    let existing = match storage.get_user_by_id(user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(not_found("User not found")),
        Err(e) => return Err(fail("Internal server error", e)),
    };
    let system_role = storage.get_system_role().await.ok().flatten();
    if let Some(system_role) = &system_role
        && existing.role_id == system_role.role_id
        && (username.is_some() || role_id.is_some())
    {
        return Err(bad("Cannot modify system user's username or role"));
    }
    if let Some(role_id) = role_id {
        match storage.get_role(role_id).await {
            Ok(Some(_)) => {}
            Ok(None) => return Err(bad("Role not found")),
            Err(e) => return Err(fail("Internal server error", e)),
        }
    }
    storage
        .update_user(user_id, username, role_id)
        .await
        .map_err(|e| fail("Failed to update user", e))?;
    if let Some(new_password) = new_password {
        let hash = password::hash(new_password).map_err(|e| fail("Failed to process password", e.0))?;
        storage
            .update_password(user_id, &hash)
            .await
            .map_err(|e| fail("Failed to update password", e))?;
    }
    Ok(())
}

pub async fn delete(services: &Services, principal: &Principal, user_id: &str) -> Result<(), ServiceError> {
    access::require(principal, "users:manage")?;
    let storage = &services.deps().storage;
    let user = match storage.get_user_by_id(user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(not_found("User not found")),
        Err(e) => return Err(fail("Internal server error", e)),
    };
    let system_role = storage.get_system_role().await.ok().flatten();
    if let Some(system_role) = &system_role
        && user.role_id == system_role.role_id
    {
        return Err(bad("Cannot delete system user"));
    }
    storage
        .delete_user(user_id)
        .await
        .map_err(|e| fail("Failed to delete user", e))
}
