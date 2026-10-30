//! First-run setup: whether anyone exists yet, and creating the first (superadmin) user.

use crate::{
    auth::password,
    service::{ServiceError, Services, users::fail},
    storage::user::{AVAILABLE_PERMISSIONS, Role, User, UserStorage},
};

pub async fn needs_setup(services: &Services) -> Result<bool, ServiceError> {
    services
        .deps()
        .storage
        .user_exists()
        .await
        .map(|exists| !exists)
        .map_err(|e| fail("Internal server error", e))
}

/// Creates the superadmin role if needed and the first user with it. Refused once anyone exists.
pub async fn first_user(
    services: &Services,
    username: &str,
    password_in: &str,
) -> Result<(User, Role), ServiceError> {
    let storage = &services.deps().storage;
    match storage.user_exists().await {
        Ok(true) => return Err(ServiceError::invalid("Setup already completed")),
        Ok(false) => {}
        Err(e) => return Err(fail("Internal server error", e)),
    }
    let role = match storage.get_system_role().await {
        Ok(Some(role)) => role,
        Ok(None) => storage
            .create_role(
                "superadmin",
                AVAILABLE_PERMISSIONS.iter().map(|p| p.to_string()).collect(),
                true,
            )
            .await
            .map_err(|e| fail("Failed to create system role", e))?,
        Err(e) => return Err(fail("Internal server error", e)),
    };
    let hash = password::hash(password_in).map_err(|e| fail("Failed to process password", e.0))?;
    let user = storage
        .create_user(username, &hash, &role.role_id)
        .await
        .map_err(|e| fail("Failed to create user", e))?;
    Ok((user, role))
}
