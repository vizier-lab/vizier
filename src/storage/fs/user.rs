use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    storage::{
        fs::FileSystemStorage,
        user::{ApiKey, Role, User, UserProfile, UserStorage},
    },
    utils::build_path,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UserStore {
    users: Vec<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RoleStore {
    roles: Vec<Role>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UserProfileStore {
    profiles: Vec<UserProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ApiKeyStore {
    keys: Vec<ApiKey>,
}

const USERS_PATH: &str = "users/users.json";
const ROLES_PATH: &str = "users/roles.json";
const USER_PROFILES_PATH: &str = "users/profiles.json";
const API_KEYS_PATH: &str = "users/api_keys.json";

#[async_trait::async_trait]
impl UserStorage for FileSystemStorage {
    async fn get_user(&self, username: &str) -> Result<Option<User>> {
        let path = build_path(&self.workspace, &[USERS_PATH]);

        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: UserStore = serde_json::from_str(&raw)?;

        Ok(store.users.into_iter().find(|u| u.username == username))
    }

    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>> {
        let path = build_path(&self.workspace, &[USERS_PATH]);

        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: UserStore = serde_json::from_str(&raw)?;

        Ok(store.users.into_iter().find(|u| u.user_id == user_id))
    }

    async fn create_user(&self, username: &str, password_hash: &str, role_id: &str) -> Result<User> {
        let path = build_path(&self.workspace, &[USERS_PATH]);
        let _ = std::fs::create_dir_all(path.parent().unwrap())?;

        let mut store = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            UserStore::default()
        };

        let user = User {
            user_id: Uuid::new_v4().to_string(),
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            role_id: role_id.to_string(),
            created_at: Utc::now(),
        };

        store.users.push(user.clone());

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(user)
    }

    async fn update_user(&self, user_id: &str, username: Option<&str>, role_id: Option<&str>) -> Result<()> {
        let path = build_path(&self.workspace, &[USERS_PATH]);

        if !path.exists() {
            return Err(anyhow::anyhow!("User store not found"));
        }

        let raw = std::fs::read_to_string(&path)?;
        let mut store: UserStore = serde_json::from_str(&raw)?;

        if let Some(user) = store.users.iter_mut().find(|u| u.user_id == user_id) {
            if let Some(username) = username {
                user.username = username.to_string();
            }
            if let Some(role_id) = role_id {
                user.role_id = role_id.to_string();
            }
        } else {
            return Err(anyhow::anyhow!("User not found"));
        }

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(())
    }

    async fn delete_user(&self, user_id: &str) -> Result<()> {
        let path = build_path(&self.workspace, &[USERS_PATH]);

        if !path.exists() {
            return Err(anyhow::anyhow!("User store not found"));
        }

        let raw = std::fs::read_to_string(&path)?;
        let mut store: UserStore = serde_json::from_str(&raw)?;

        store.users.retain(|u| u.user_id != user_id);

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(())
    }

    async fn list_users(&self) -> Result<Vec<User>> {
        let path = build_path(&self.workspace, &[USERS_PATH]);

        if !path.exists() {
            return Ok(vec![]);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: UserStore = serde_json::from_str(&raw)?;

        Ok(store.users)
    }

    async fn update_password(&self, user_id: &str, password_hash: &str) -> Result<()> {
        let path = build_path(&self.workspace, &[USERS_PATH]);

        if !path.exists() {
            return Err(anyhow::anyhow!("User store not found"));
        }

        let raw = std::fs::read_to_string(&path)?;
        let mut store: UserStore = serde_json::from_str(&raw)?;

        if let Some(user) = store.users.iter_mut().find(|u| u.user_id == user_id) {
            user.password_hash = password_hash.to_string();
        } else {
            return Err(anyhow::anyhow!("User not found"));
        }

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(())
    }

    async fn user_exists(&self) -> Result<bool> {
        let path = build_path(&self.workspace, &[USERS_PATH]);

        if !path.exists() {
            return Ok(false);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: UserStore = serde_json::from_str(&raw).unwrap_or_default();

        Ok(!store.users.is_empty())
    }

    async fn create_role(&self, name: &str, permissions: Vec<String>, is_system: bool) -> Result<Role> {
        let path = build_path(&self.workspace, &[ROLES_PATH]);
        let _ = std::fs::create_dir_all(path.parent().unwrap())?;

        let mut store = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            RoleStore::default()
        };

        let role = Role {
            role_id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            permissions,
            is_system,
            created_at: Utc::now(),
        };

        store.roles.push(role.clone());

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(role)
    }

    async fn get_role(&self, role_id: &str) -> Result<Option<Role>> {
        let path = build_path(&self.workspace, &[ROLES_PATH]);

        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: RoleStore = serde_json::from_str(&raw)?;

        Ok(store.roles.into_iter().find(|r| r.role_id == role_id))
    }

    async fn list_roles(&self) -> Result<Vec<Role>> {
        let path = build_path(&self.workspace, &[ROLES_PATH]);

        if !path.exists() {
            return Ok(vec![]);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: RoleStore = serde_json::from_str(&raw)?;

        Ok(store.roles)
    }

    async fn update_role(&self, role_id: &str, name: &str, permissions: Vec<String>) -> Result<()> {
        let path = build_path(&self.workspace, &[ROLES_PATH]);

        if !path.exists() {
            return Err(anyhow::anyhow!("Role store not found"));
        }

        let raw = std::fs::read_to_string(&path)?;
        let mut store: RoleStore = serde_json::from_str(&raw)?;

        if let Some(role) = store.roles.iter_mut().find(|r| r.role_id == role_id) {
            role.name = name.to_string();
            role.permissions = permissions;
        } else {
            return Err(anyhow::anyhow!("Role not found"));
        }

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(())
    }

    async fn delete_role(&self, role_id: &str) -> Result<()> {
        let path = build_path(&self.workspace, &[ROLES_PATH]);

        if !path.exists() {
            return Err(anyhow::anyhow!("Role store not found"));
        }

        let raw = std::fs::read_to_string(&path)?;
        let mut store: RoleStore = serde_json::from_str(&raw)?;

        store.roles.retain(|r| r.role_id != role_id);

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(())
    }

    async fn get_system_role(&self) -> Result<Option<Role>> {
        let path = build_path(&self.workspace, &[ROLES_PATH]);

        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: RoleStore = serde_json::from_str(&raw)?;

        Ok(store.roles.into_iter().find(|r| r.is_system))
    }

    async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        let path = build_path(&self.workspace, &[USER_PROFILES_PATH]);

        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: UserProfileStore = serde_json::from_str(&raw)?;

        Ok(store.profiles.into_iter().find(|p| p.user_id == user_id))
    }

    async fn upsert_user_profile(&self, user_id: &str, profile: &UserProfile) -> Result<()> {
        let path = build_path(&self.workspace, &[USER_PROFILES_PATH]);
        let _ = std::fs::create_dir_all(path.parent().unwrap())?;

        let mut store = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            UserProfileStore::default()
        };

        if let Some(existing) = store.profiles.iter_mut().find(|p| p.user_id == user_id) {
            *existing = profile.clone();
        } else {
            store.profiles.push(profile.clone());
        }

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(())
    }

    async fn create_api_key(
        &self,
        user_id: &str,
        name: &str,
        key_hash: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiKey> {
        let path = build_path(&self.workspace, &[API_KEYS_PATH]);
        let _ = std::fs::create_dir_all(path.parent().unwrap())?;

        let mut store = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            ApiKeyStore::default()
        };

        let api_key = ApiKey {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            name: name.to_string(),
            key_hash: key_hash.to_string(),
            expires_at,
            created_at: Utc::now(),
            last_used_at: None,
        };

        store.keys.push(api_key.clone());

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(api_key)
    }

    async fn get_api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>> {
        let path = build_path(&self.workspace, &[API_KEYS_PATH]);

        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: ApiKeyStore = serde_json::from_str(&raw)?;

        Ok(store
            .keys
            .into_iter()
            .find(|k| k.key_hash == key_hash && k.expires_at.map_or(true, |exp| exp > Utc::now())))
    }

    async fn list_api_keys(&self, user_id: &str) -> Result<Vec<ApiKey>> {
        let path = build_path(&self.workspace, &[API_KEYS_PATH]);

        if !path.exists() {
            return Ok(vec![]);
        }

        let raw = std::fs::read_to_string(&path)?;
        let store: ApiKeyStore = serde_json::from_str(&raw)?;

        Ok(store
            .keys
            .into_iter()
            .filter(|k| k.user_id == user_id)
            .collect())
    }

    async fn delete_api_key(&self, key_id: &str) -> Result<()> {
        let path = build_path(&self.workspace, &[API_KEYS_PATH]);

        if !path.exists() {
            return Err(anyhow::anyhow!("API key store not found"));
        }

        let raw = std::fs::read_to_string(&path)?;
        let mut store: ApiKeyStore = serde_json::from_str(&raw)?;

        store.keys.retain(|k| k.id != key_id);

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(())
    }

    async fn update_api_key_last_used(&self, key_id: &str) -> Result<()> {
        let path = build_path(&self.workspace, &[API_KEYS_PATH]);

        if !path.exists() {
            return Err(anyhow::anyhow!("API key store not found"));
        }

        let raw = std::fs::read_to_string(&path)?;
        let mut store: ApiKeyStore = serde_json::from_str(&raw)?;

        if let Some(key) = store.keys.iter_mut().find(|k| k.id == key_id) {
            key.last_used_at = Some(Utc::now());
        } else {
            return Err(anyhow::anyhow!("API key not found"));
        }

        std::fs::write(path, serde_json::to_string_pretty(&store)?)?;

        Ok(())
    }
}
