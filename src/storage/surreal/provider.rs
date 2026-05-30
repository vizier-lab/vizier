use anyhow::Result;

use crate::config::provider::ProviderVariant;
use crate::schema::ProviderEntry;
use crate::storage::{provider::ProviderStorage, surreal::SurrealStorage};

#[async_trait::async_trait]
impl ProviderStorage for SurrealStorage {
    async fn list_providers(&self) -> Result<Vec<ProviderEntry>> {
        let mut result = self
            .conn
            .query("SELECT variant, config FROM provider_config")
            .await?;

        let rows: Vec<serde_json::Value> = result.take(0)?;
        let mut providers = Vec::new();
        for row in rows {
            if let Ok(entry) = serde_json::from_value::<ProviderEntry>(row) {
                providers.push(entry);
            }
        }
        Ok(providers)
    }

    async fn get_provider(&self, variant: &ProviderVariant) -> Result<Option<ProviderEntry>> {
        let variant_str = serde_json::to_string(variant)?.trim_matches('"').to_string();
        let mut result = self
            .conn
            .query("SELECT variant, config FROM provider_config WHERE variant = $v")
            .bind(("v", variant_str))
            .await?;

        let rows: Vec<serde_json::Value> = result.take(0)?;
        Ok(rows
            .into_iter()
            .next()
            .and_then(|r| serde_json::from_value::<ProviderEntry>(r).ok()))
    }

    async fn upsert_provider(&self, entry: &ProviderEntry) -> Result<()> {
        let variant_str = serde_json::to_string(&entry.variant)?.trim_matches('"').to_string();
        let record = serde_json::to_value(entry)?;

        let _: Option<serde_json::Value> = self
            .conn
            .upsert(("provider_config", variant_str))
            .content(record)
            .await?;

        Ok(())
    }

    async fn delete_provider(&self, variant: &ProviderVariant) -> Result<()> {
        let variant_str = serde_json::to_string(variant)?.trim_matches('"').to_string();
        let _: Option<serde_json::Value> = self
            .conn
            .delete(("provider_config", variant_str))
            .await?;

        Ok(())
    }
}
