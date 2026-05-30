use anyhow::Result;

use crate::schema::GlobalConfigEntry;
use crate::storage::{global_config::GlobalConfigStorage, surreal::SurrealStorage};

#[async_trait::async_trait]
impl GlobalConfigStorage for SurrealStorage {
    async fn list_global_configs(&self) -> Result<Vec<GlobalConfigEntry>> {
        let mut result = self
            .conn
            .query("SELECT key, value FROM global_config")
            .await?;

        let rows: Vec<serde_json::Value> = result.take(0)?;
        let mut entries = Vec::new();
        for row in rows {
            if let Ok(entry) = serde_json::from_value::<GlobalConfigEntry>(row) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    async fn get_global_config(&self, key: &str) -> Result<Option<GlobalConfigEntry>> {
        let mut result = self
            .conn
            .query("SELECT key, value FROM global_config WHERE key = $k")
            .bind(("k", key.to_string()))
            .await?;

        let rows: Vec<serde_json::Value> = result.take(0)?;
        Ok(rows
            .into_iter()
            .next()
            .and_then(|r| serde_json::from_value::<GlobalConfigEntry>(r).ok()))
    }

    async fn upsert_global_config(&self, entry: &GlobalConfigEntry) -> Result<()> {
        let record = serde_json::to_value(entry)?;

        let _: Option<serde_json::Value> = self
            .conn
            .upsert(("global_config", entry.key.clone()))
            .content(record)
            .await?;

        Ok(())
    }

    async fn delete_global_config(&self, key: &str) -> Result<()> {
        let _: Option<serde_json::Value> = self
            .conn
            .delete(("global_config", key.to_string()))
            .await?;

        Ok(())
    }
}
