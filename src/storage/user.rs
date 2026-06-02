use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

pub const AVAILABLE_PERMISSIONS: &[&str] = &[
    "all_agents:view",
    "owned_agents:view",
    "all_agents:create",
    "all_agents:edit",
    "owned_agents:edit",
    "all_agents:delete",
    "owned_agents:delete",
    "settings:providers",
    "settings:mcp_servers",
    "settings:shell",
    "settings:password",
    "settings:api_keys",
    "users:manage",
    "roles:manage",
];

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Role {
    pub role_id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub password_hash: String,
    pub role_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct UserProfile {
    pub user_id: String,
    pub discord_id: Option<String>,
    pub discord_username: Option<String>,
    pub telegram_id: Option<String>,
    pub telegram_username: Option<String>,
    #[serde(default)]
    pub alias: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub key_hash: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[async_trait::async_trait]
pub trait UserStorage {
    async fn get_user(&self, username: &str) -> Result<Option<User>>;
    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>>;
    async fn create_user(&self, username: &str, password_hash: &str, role_id: &str) -> Result<User>;
    async fn update_user(&self, user_id: &str, username: Option<&str>, role_id: Option<&str>) -> Result<()>;
    async fn delete_user(&self, user_id: &str) -> Result<()>;
    async fn list_users(&self) -> Result<Vec<User>>;
    async fn update_password(&self, user_id: &str, password_hash: &str) -> Result<()>;
    async fn user_exists(&self) -> Result<bool>;

    async fn create_role(&self, name: &str, permissions: Vec<String>, is_system: bool) -> Result<Role>;
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
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiKey>;
    async fn get_api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>>;
    async fn list_api_keys(&self, user_id: &str) -> Result<Vec<ApiKey>>;
    async fn delete_api_key(&self, key_id: &str) -> Result<()>;
    async fn update_api_key_last_used(&self, key_id: &str) -> Result<()>;
}
