use anyhow::Result;

use crate::storage::VizierStorage;

#[async_trait::async_trait]
pub trait StateStorage {
    async fn save_state(&self, key: String, value: serde_json::Value) -> Result<()>;
    async fn get_state(&self, key: String) -> Result<Option<serde_json::Value>>;
    /// Removes `key`; a key that is not there is not an error.
    async fn delete_state(&self, key: String) -> Result<()>;
}

#[async_trait::async_trait]
impl StateStorage for VizierStorage {
    async fn save_state(&self, key: String, value: serde_json::Value) -> Result<()> {
        self.0.save_state(key, value).await
    }

    async fn get_state(&self, key: String) -> Result<Option<serde_json::Value>> {
        self.0.get_state(key).await
    }

    async fn delete_state(&self, key: String) -> Result<()> {
        self.0.delete_state(key).await
    }
}
