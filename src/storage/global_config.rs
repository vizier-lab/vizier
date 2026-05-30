use anyhow::Result;

use crate::schema::GlobalConfigEntry;

#[async_trait::async_trait]
pub trait GlobalConfigStorage {
    async fn list_global_configs(&self) -> Result<Vec<GlobalConfigEntry>>;
    async fn get_global_config(&self, key: &str) -> Result<Option<GlobalConfigEntry>>;
    async fn upsert_global_config(&self, entry: &GlobalConfigEntry) -> Result<()>;
    async fn delete_global_config(&self, key: &str) -> Result<()>;
}
