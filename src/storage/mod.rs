use std::sync::Arc;

use anyhow::Result;

use crate::{
    config::provider::ProviderVariant,
    schema::{AgentConfig, DocumentIndex, GlobalConfigEntry, ProviderEntry},
    storage::{
        agent::AgentStorage, global_config::GlobalConfigStorage, history::HistoryStorage,
        indexer::DocumentIndexer, memory::MemoryStorage, provider::ProviderStorage,
        session::SessionStorage, shared_document::SharedDocumentStorage, skill::SkillStorage,
        state::StateStorage, task::TaskStorage, user::UserStorage,
    },
};

pub mod agent;
pub mod global_config;
pub mod history;
pub mod indexer;
pub mod memory;
pub mod provider;
pub mod session;
pub mod shared_document;
pub mod skill;
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
        + SkillStorage
        + SessionStorage
        + StateStorage
        + DocumentIndexer
        + UserStorage
        + SharedDocumentStorage
        + AgentStorage
        + ProviderStorage
        + GlobalConfigStorage,
{
}

#[derive(Clone)]
pub struct VizierStorage(Arc<Box<dyn VizierStorageProvider + Sync + Send + 'static>>);

impl VizierStorage {
    pub fn new<Storage: VizierStorageProvider + Sync + Send + 'static>(storage: Storage) -> Self {
        Self(Arc::new(Box::new(storage)))
    }
}

#[async_trait::async_trait]
impl DocumentIndexer for VizierStorage {
    async fn add_document_index(&self, context: String, path: String) -> Result<DocumentIndex> {
        self.0.add_document_index(context, path).await
    }
    async fn search_document_index(
        &self,
        context: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<DocumentIndex>> {
        self.0
            .search_document_index(context, query, limit, threshold)
            .await
    }

    async fn delete_index(&self, context: String, path: String) -> Result<()> {
        self.0.delete_index(context, path).await
    }
}

impl VizierStorageProvider for VizierStorage {}

#[async_trait::async_trait]
impl UserStorage for VizierStorage {
    async fn get_user(&self, username: &str) -> Result<Option<crate::storage::user::User>> {
        self.0.get_user(username).await
    }

    async fn create_user(&self, username: &str, password_hash: &str) -> Result<crate::storage::user::User> {
        self.0.create_user(username, password_hash).await
    }

    async fn update_password(&self, user_id: &str, password_hash: &str) -> Result<()> {
        self.0.update_password(user_id, password_hash).await
    }

    async fn user_exists(&self) -> Result<bool> {
        self.0.user_exists().await
    }

    async fn create_api_key(
        &self,
        user_id: &str,
        name: &str,
        key_hash: &str,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<crate::storage::user::ApiKey> {
        self.0.create_api_key(user_id, name, key_hash, expires_at).await
    }

    async fn get_api_key_by_hash(&self, key_hash: &str) -> Result<Option<crate::storage::user::ApiKey>> {
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
