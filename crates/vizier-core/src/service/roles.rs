//! Roles and the permissions they grant. All of it needs `roles:manage`.

use std::collections::HashMap;

use crate::{
    access,
    auth::Principal,
    service::{ServiceError, Services, users::fail},
    storage::user::{AVAILABLE_PERMISSIONS, Role, UserStorage},
};

const MANAGE: &str = "roles:manage";

fn check_permissions(permissions: &[String]) -> Result<(), ServiceError> {
    for perm in permissions {
        if !AVAILABLE_PERMISSIONS.contains(&perm.as_str()) {
            return Err(ServiceError::invalid(format!("Invalid permission: {perm}")));
        }
    }
    Ok(())
}

pub fn available_permissions(principal: &Principal) -> Result<Vec<String>, ServiceError> {
    access::require(principal, MANAGE)?;
    Ok(AVAILABLE_PERMISSIONS.iter().map(|p| p.to_string()).collect())
}

/// Every role with how many people hold it.
pub async fn list(services: &Services, principal: &Principal) -> Result<Vec<(Role, usize)>, ServiceError> {
    access::require(principal, MANAGE)?;
    let storage = &services.deps().storage;
    let people = match storage.list_users().await {
        Ok(users) => users.into_iter().fold(HashMap::<String, usize>::new(), |mut acc, u| {
            *acc.entry(u.role_id).or_default() += 1;
            acc
        }),
        Err(e) => {
            tracing::error!("Failed to count people per role: {e}");
            Default::default()
        }
    };
    let roles = storage.list_roles().await.map_err(|e| fail("Failed to list roles", e))?;
    Ok(roles
        .into_iter()
        .map(|r| {
            let n = people.get(&r.role_id).copied().unwrap_or(0);
            (r, n)
        })
        .collect())
}

pub async fn get(services: &Services, principal: &Principal, role_id: &str) -> Result<(Role, usize), ServiceError> {
    access::require(principal, MANAGE)?;
    let storage = &services.deps().storage;
    let role = match storage.get_role(role_id).await {
        Ok(Some(role)) => role,
        Ok(None) => return Err(ServiceError::NotFound("Role not found".into())),
        Err(e) => return Err(fail("Failed to get role", e)),
    };
    let people = storage
        .list_users()
        .await
        .map(|users| users.iter().filter(|u| u.role_id == role.role_id).count())
        .unwrap_or(0);
    Ok((role, people))
}

pub async fn create(
    services: &Services,
    principal: &Principal,
    name: &str,
    permissions: Vec<String>,
) -> Result<Role, ServiceError> {
    access::require(principal, MANAGE)?;
    check_permissions(&permissions)?;
    services
        .deps()
        .storage
        .create_role(name, permissions, false)
        .await
        .map_err(|e| fail("Failed to create role", e))
}

async fn editable(services: &Services, role_id: &str, verb: &str) -> Result<Role, ServiceError> {
    let role = match services.deps().storage.get_role(role_id).await {
        Ok(Some(role)) => role,
        Ok(None) => return Err(ServiceError::NotFound("Role not found".into())),
        Err(e) => return Err(fail("Internal server error", e)),
    };
    if role.is_system {
        return Err(ServiceError::invalid(format!("Cannot {verb} system role")));
    }
    Ok(role)
}

pub async fn update(
    services: &Services,
    principal: &Principal,
    role_id: &str,
    name: &str,
    permissions: Vec<String>,
) -> Result<(), ServiceError> {
    access::require(principal, MANAGE)?;
    editable(services, role_id, "modify").await?;
    check_permissions(&permissions)?;
    services
        .deps()
        .storage
        .update_role(role_id, name, permissions)
        .await
        .map_err(|e| fail("Failed to update role", e))
}

pub async fn delete(services: &Services, principal: &Principal, role_id: &str) -> Result<(), ServiceError> {
    access::require(principal, MANAGE)?;
    editable(services, role_id, "delete").await?;
    let storage = &services.deps().storage;
    if let Ok(users) = storage.list_users().await {
        let n = users.iter().filter(|u| u.role_id == role_id).count();
        if n > 0 {
            return Err(ServiceError::invalid(format!(
                "Cannot delete role: {n} user(s) are assigned to it"
            )));
        }
    }
    storage
        .delete_role(role_id)
        .await
        .map_err(|e| fail("Failed to delete role", e))
}
