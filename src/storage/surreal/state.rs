use anyhow::Result;

use crate::storage::{state::StateStorage, surreal::SurrealStorage};

#[async_trait::async_trait]
impl StateStorage for SurrealStorage {
    async fn save_state(&self, key: String, value: serde_json::Value) -> Result<()> {
        let _: Option<serde_json::Value> = self.conn.upsert(("state", key)).content(value).await?;

        Ok(())
    }

    async fn get_state(&self, key: String) -> Result<Option<serde_json::Value>> {
        let mut response = self
            .conn
            .query("SELECT * FROM state WHERE id = $key")
            .bind(("key", key))
            .await?;

        let value: Option<serde_json::Value> = response.take(0)?;

        Ok(value)
    }
}
