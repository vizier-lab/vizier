//! The application-service layer: every operation a person can ask of the engine, with its
//! authorization, independent of the adapter (HTTP, desktop IPC) that carries it.

pub mod error;
pub mod topics;
pub mod agents;
pub mod core;
pub mod memory;
pub mod documents;
pub mod tasks;
pub mod dream;
pub mod jobs;
pub mod conversations;
pub mod artifacts;
pub mod secrets;
pub mod integrations;
pub mod skills;
pub mod users;
pub mod roles;
pub mod api_keys;
pub mod profile;
pub mod setup;
pub mod providers;
pub mod custom_providers;
pub mod usage;
pub mod catalogue;
pub mod embedding;
pub mod notifications;
pub mod overview;
pub mod files;
pub mod setup_state;
pub mod page;

pub use error::ServiceError;

use crate::{
    auth::{AuthMethod, Principal},
    dependencies::VizierDependencies,
    storage::user::UserStorage,
};

#[derive(Clone)]
pub struct Services {
    deps: VizierDependencies,
}

impl Services {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }

    pub fn deps(&self) -> &VizierDependencies {
        &self.deps
    }

    /// Resolves a user's current role and permissions into a [`Principal`]. A desktop principal
    /// is minted only by the embedded dispatcher, so asking for one here is refused.
    pub async fn principal_for(&self, user_id: &str, method: AuthMethod) -> Result<Principal, ServiceError> {
        if method == AuthMethod::Desktop {
            return Err(ServiceError::invalid("a desktop principal cannot be requested here"));
        }
        self.resolve(user_id, method).await
    }

    #[doc(hidden)]
    pub async fn resolve(&self, user_id: &str, method: AuthMethod) -> Result<Principal, ServiceError> {
        let user = self
            .deps
            .storage
            .get_user_by_id(user_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or_else(|| ServiceError::Unauthorized("Invalid authentication credentials".into()))?;
        let role = self
            .deps
            .storage
            .get_role(&user.role_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or_else(|| ServiceError::Internal("Internal authentication error".into()))?;
        Ok(Principal {
            user_id: user.user_id,
            username: user.username,
            permissions: role.permissions.clone(),
            role: role.clone(),
            method,
        })
    }
}
