use anyhow::Result;

use crate::storage::VizierStorage;

#[async_trait::async_trait]
pub trait StateStorage {
    async fn save_state(&self, key: String, value: serde_json::Value) -> Result<()>;
    async fn get_state(&self, key: String) -> Result<Option<serde_json::Value>>;
}

#[async_trait::async_trait]
impl StateStorage for VizierStorage {
    async fn save_state(&self, key: String, value: serde_json::Value) -> Result<()> {
        self.0.save_state(key, value).await
    }

    async fn get_state(&self, key: String) -> Result<Option<serde_json::Value>> {
        self.0.get_state(key).await
    }
}
