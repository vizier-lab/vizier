use std::sync::Arc;

use anyhow::Result;

use crate::{
    config::provider::ProviderVariant,
    schema::{AgentConfig, GlobalConfigEntry, ProviderEntry, SessionFileRecord, VizierSession},
    storage::{
        agent::AgentStorage, dream::DreamStorage, dream_journal::DreamJournalStorage,
        global_config::GlobalConfigStorage, history::HistoryStorage, memory::MemoryStorage,
        provider::ProviderStorage, session::SessionStorage, session_file::SessionFileStorage,
        state::StateStorage, task::TaskStorage, user::UserStorage,
    },
};

pub mod agent;
pub mod dream;
pub mod dream_journal;
pub mod global_config;
pub mod history;
pub mod memory;
pub mod provider;
pub mod session;
pub mod session_file;
pub mod state;
pub mod task;
pub mod user;

pub mod fs;
pub mod surreal;

pub trait VizierStorageProvider
where
    Self: MemoryStorage
        + TaskStorage
        + HistoryStorage
        + SessionStorage
        + StateStorage
        + UserStorage
        + AgentStorage
        + ProviderStorage
        + GlobalConfigStorage
        + DreamJournalStorage
        + DreamStorage
        + SessionFileStorage,
{
}

#[derive(Clone)]
pub struct VizierStorage(Arc<Box<dyn VizierStorageProvider + Sync + Send + 'static>>);

impl VizierStorage {
    pub fn new<Storage: VizierStorageProvider + Sync + Send + 'static>(storage: Storage) -> Self {
        Self(Arc::new(Box::new(storage)))
    }
}

impl VizierStorageProvider for VizierStorage {}

#[async_trait::async_trait]
impl UserStorage for VizierStorage {
    async fn get_user(&self, username: &str) -> Result<Option<crate::storage::user::User>> {
        self.0.get_user(username).await
    }

    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<crate::storage::user::User>> {
        self.0.get_user_by_id(user_id).await
    }

    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        role_id: &str,
    ) -> Result<crate::storage::user::User> {
        self.0.create_user(username, password_hash, role_id).await
    }

    async fn update_user(
        &self,
        user_id: &str,
        username: Option<&str>,
        role_id: Option<&str>,
    ) -> Result<()> {
        self.0.update_user(user_id, username, role_id).await
    }

    async fn delete_user(&self, user_id: &str) -> Result<()> {
        self.0.delete_user(user_id).await
    }

    async fn list_users(&self) -> Result<Vec<crate::storage::user::User>> {
        self.0.list_users().await
    }

    async fn update_password(&self, user_id: &str, password_hash: &str) -> Result<()> {
        self.0.update_password(user_id, password_hash).await
    }

    async fn user_exists(&self) -> Result<bool> {
        self.0.user_exists().await
    }

    async fn create_role(
        &self,
        name: &str,
        permissions: Vec<String>,
        is_system: bool,
    ) -> Result<crate::storage::user::Role> {
        self.0.create_role(name, permissions, is_system).await
    }

    async fn get_role(&self, role_id: &str) -> Result<Option<crate::storage::user::Role>> {
        self.0.get_role(role_id).await
    }

    async fn list_roles(&self) -> Result<Vec<crate::storage::user::Role>> {
        self.0.list_roles().await
    }

    async fn update_role(&self, role_id: &str, name: &str, permissions: Vec<String>) -> Result<()> {
        self.0.update_role(role_id, name, permissions).await
    }

    async fn delete_role(&self, role_id: &str) -> Result<()> {
        self.0.delete_role(role_id).await
    }

    async fn get_system_role(&self) -> Result<Option<crate::storage::user::Role>> {
        self.0.get_system_role().await
    }

    async fn get_user_profile(
        &self,
        user_id: &str,
    ) -> Result<Option<crate::storage::user::UserProfile>> {
        self.0.get_user_profile(user_id).await
    }

    async fn upsert_user_profile(
        &self,
        user_id: &str,
        profile: &crate::storage::user::UserProfile,
    ) -> Result<()> {
        self.0.upsert_user_profile(user_id, profile).await
    }

    async fn create_api_key(
        &self,
        user_id: &str,
        name: &str,
        key_hash: &str,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<crate::storage::user::ApiKey> {
        self.0
            .create_api_key(user_id, name, key_hash, expires_at)
            .await
    }

    async fn get_api_key_by_hash(
        &self,
        key_hash: &str,
    ) -> Result<Option<crate::storage::user::ApiKey>> {
        self.0.get_api_key_by_hash(key_hash).await
    }

    async fn list_api_keys(&self, user_id: &str) -> Result<Vec<crate::storage::user::ApiKey>> {
        self.0.list_api_keys(user_id).await
    }

    async fn delete_api_key(&self, key_id: &str) -> Result<()> {
        self.0.delete_api_key(key_id).await
    }

    async fn update_api_key_last_used(&self, key_id: &str) -> Result<()> {
        self.0.update_api_key_last_used(key_id).await
    }
}

#[async_trait::async_trait]
impl AgentStorage for VizierStorage {
    async fn list_agents(&self) -> Result<Vec<(String, AgentConfig)>> {
        self.0.list_agents().await
    }

    async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentConfig>> {
        self.0.get_agent(agent_id).await
    }

    async fn create_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        self.0.create_agent(agent_id, config).await
    }

    async fn update_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        self.0.update_agent(agent_id, config).await
    }

    async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        self.0.delete_agent(agent_id).await
    }
}

#[async_trait::async_trait]
impl ProviderStorage for VizierStorage {
    async fn list_providers(&self) -> Result<Vec<ProviderEntry>> {
        self.0.list_providers().await
    }

    async fn get_provider(&self, variant: &ProviderVariant) -> Result<Option<ProviderEntry>> {
        self.0.get_provider(variant).await
    }

    async fn upsert_provider(&self, entry: &ProviderEntry) -> Result<()> {
        self.0.upsert_provider(entry).await
    }

    async fn delete_provider(&self, variant: &ProviderVariant) -> Result<()> {
        self.0.delete_provider(variant).await
    }
}

#[async_trait::async_trait]
impl GlobalConfigStorage for VizierStorage {
    async fn list_global_configs(&self) -> Result<Vec<GlobalConfigEntry>> {
        self.0.list_global_configs().await
    }

    async fn get_global_config(&self, key: &str) -> Result<Option<GlobalConfigEntry>> {
        self.0.get_global_config(key).await
    }

    async fn upsert_global_config(&self, entry: &GlobalConfigEntry) -> Result<()> {
        self.0.upsert_global_config(entry).await
    }

    async fn delete_global_config(&self, key: &str) -> Result<()> {
        self.0.delete_global_config(key).await
    }
}

#[async_trait::async_trait]
impl SessionFileStorage for VizierStorage {
    async fn save_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<SessionFileRecord> {
        self.0
            .save_session_file(session, filename, mime_type, size, file_id)
            .await
    }

    async fn list_session_files(&self, session: &VizierSession) -> Result<Vec<SessionFileRecord>> {
        self.0.list_session_files(session).await
    }

    async fn get_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<Option<SessionFileRecord>> {
        self.0.get_session_file(session, filename).await
    }

    async fn delete_session_file(&self, session: &VizierSession, filename: &str) -> Result<()> {
        self.0.delete_session_file(session, filename).await
    }
}
