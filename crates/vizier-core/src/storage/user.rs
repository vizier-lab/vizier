use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const AVAILABLE_PERMISSIONS: &[&str] = &[
    "all_agents:view",
    "owned_agents:view",
    "all_agents:create",
    "all_agents:edit",
    "owned_agents:edit",
    "all_agents:delete",
    "owned_agents:delete",
    "settings:providers",
    "all_agents:integrations:manage",
    "owned_agents:integrations:manage",
    "all_agents:secret",
    "owned_agents:secret",
    "all_agents:view_artifacts",
    "owned_agents:view_artifacts",
    "all_agents:manage_artifacts",
    "owned_agents:manage_artifacts",
    "all_agents:show_discord_chats",
    "owned_agents:show_discord_chats",
    "all_agents:show_telegram_chats",
    "owned_agents:show_telegram_chats",
    "agents:shell_config",
    "settings:password",
    "settings:api_keys",
    "users:manage",
    "roles:manage",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub role_id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub password_hash: String,
    pub role_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: String,
    pub discord_id: Option<String>,
    pub discord_username: Option<String>,
    pub telegram_id: Option<String>,
    pub telegram_username: Option<String>,
    #[serde(default)]
    pub alias: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub key_hash: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    /// The first characters of the key, kept to tell keys apart on the API keys page. Keys made
    /// before it was kept have none.
    #[serde(default)]
    pub prefix: Option<String>,
}

/// When a person was last seen, and when they were here before that (spec 024: "Since you were
/// away" starts from `previous_visit_at`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visit {
    pub last_seen_at: Option<DateTime<Utc>>,
    pub previous_visit_at: Option<DateTime<Utc>>,
}

/// A gap longer than this between two requests starts a new visit.
pub const VISIT_GAP_MINUTES: i64 = 30;

impl Visit {
    /// The visit after a request at `now`: within [`VISIT_GAP_MINUTES`] of the last one it is the
    /// same visit and only `last_seen_at` moves; after a longer gap `last_seen_at` becomes the
    /// previous visit first.
    pub fn seen_at(&self, now: DateTime<Utc>) -> Visit {
        let new_visit = self
            .last_seen_at
            .is_some_and(|last| now - last > chrono::Duration::minutes(VISIT_GAP_MINUTES));
        Visit {
            last_seen_at: Some(now),
            previous_visit_at: if new_visit {
                self.last_seen_at
            } else {
                self.previous_visit_at
            },
        }
    }
}

#[async_trait::async_trait]
pub trait UserStorage {
    async fn get_user(&self, username: &str) -> Result<Option<User>>;
    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>>;
    async fn create_user(&self, username: &str, password_hash: &str, role_id: &str)
    -> Result<User>;
    async fn update_user(
        &self,
        user_id: &str,
        username: Option<&str>,
        role_id: Option<&str>,
    ) -> Result<()>;
    async fn delete_user(&self, user_id: &str) -> Result<()>;
    async fn list_users(&self) -> Result<Vec<User>>;
    async fn update_password(&self, user_id: &str, password_hash: &str) -> Result<()>;
    async fn user_exists(&self) -> Result<bool>;

    /// Notes that `user_id` made a request at `now` and returns the visit as it now stands.
    async fn record_visit(&self, user_id: &str, now: DateTime<Utc>) -> Result<Visit>;
    async fn get_visit(&self, user_id: &str) -> Result<Visit>;

    async fn create_role(
        &self,
        name: &str,
        permissions: Vec<String>,
        is_system: bool,
    ) -> Result<Role>;
    async fn get_role(&self, role_id: &str) -> Result<Option<Role>>;
    async fn list_roles(&self) -> Result<Vec<Role>>;
    async fn update_role(&self, role_id: &str, name: &str, permissions: Vec<String>) -> Result<()>;
    async fn delete_role(&self, role_id: &str) -> Result<()>;
    async fn get_system_role(&self) -> Result<Option<Role>>;

    async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>>;
    async fn upsert_user_profile(&self, user_id: &str, profile: &UserProfile) -> Result<()>;

    async fn create_api_key(
        &self,
        user_id: &str,
        name: &str,
        key_hash: &str,
        prefix: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiKey>;
    async fn get_api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>>;
    async fn list_api_keys(&self, user_id: &str) -> Result<Vec<ApiKey>>;
    async fn delete_api_key(&self, key_id: &str) -> Result<()>;
    async fn update_api_key_last_used(&self, key_id: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minutes: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-23T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + chrono::Duration::minutes(minutes)
    }

    #[test]
    fn a_first_request_has_no_previous_visit() {
        let v = Visit::default().seen_at(at(0));
        assert_eq!(
            v,
            Visit {
                last_seen_at: Some(at(0)),
                previous_visit_at: None
            }
        );
    }

    #[test]
    fn requests_within_the_gap_are_one_visit() {
        let v = Visit::default()
            .seen_at(at(0))
            .seen_at(at(20))
            .seen_at(at(49));
        assert_eq!(v.last_seen_at, Some(at(49)));
        assert_eq!(v.previous_visit_at, None);
    }

    #[test]
    fn a_long_gap_starts_a_new_visit_and_keeps_where_the_last_one_ended() {
        let v = Visit::default()
            .seen_at(at(0))
            .seen_at(at(10))
            .seen_at(at(200));
        assert_eq!(v.previous_visit_at, Some(at(10)));
        assert_eq!(v.last_seen_at, Some(at(200)));
        // And the next request in the same visit leaves the previous one alone.
        assert_eq!(v.seen_at(at(210)).previous_visit_at, Some(at(10)));
    }
}
