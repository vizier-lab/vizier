//! The signed-in person's own profile (their other identities and aliases).

use crate::{
    auth::Principal,
    service::{ServiceError, Services, users::fail},
    storage::user::{UserProfile, UserStorage},
};

fn blank(user_id: &str) -> UserProfile {
    UserProfile {
        user_id: user_id.to_string(),
        discord_id: None,
        discord_username: None,
        telegram_id: None,
        telegram_username: None,
        alias: Vec::new(),
    }
}

pub async fn get(services: &Services, principal: &Principal) -> Result<UserProfile, ServiceError> {
    let profile = services
        .deps()
        .storage
        .get_user_profile(&principal.user_id)
        .await
        .map_err(|e| fail("Failed to get profile", e))?;
    Ok(profile.unwrap_or_else(|| blank(&principal.user_id)))
}

/// What to change: `None` keeps the stored value.
#[derive(Default)]
pub struct ProfileChanges {
    pub discord_id: Option<String>,
    pub discord_username: Option<String>,
    pub telegram_id: Option<String>,
    pub telegram_username: Option<String>,
    pub alias: Option<Vec<String>>,
}

pub async fn update(
    services: &Services,
    principal: &Principal,
    changes: ProfileChanges,
) -> Result<UserProfile, ServiceError> {
    let existing = get(services, principal).await?;
    let updated = UserProfile {
        user_id: principal.user_id.clone(),
        discord_id: changes.discord_id.or(existing.discord_id),
        discord_username: changes.discord_username.or(existing.discord_username),
        telegram_id: changes.telegram_id.or(existing.telegram_id),
        telegram_username: changes.telegram_username.or(existing.telegram_username),
        alias: changes.alias.unwrap_or(existing.alias),
    };
    services
        .deps()
        .storage
        .upsert_user_profile(&principal.user_id, &updated)
        .await
        .map_err(|e| fail("Failed to update profile", e))?;
    Ok(updated)
}
