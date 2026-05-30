use anyhow::Result;

use crate::config::provider::ProviderVariant;
use crate::schema::ProviderEntry;

#[async_trait::async_trait]
pub trait ProviderStorage {
    async fn list_providers(&self) -> Result<Vec<ProviderEntry>>;
    async fn get_provider(&self, variant: &ProviderVariant) -> Result<Option<ProviderEntry>>;
    async fn upsert_provider(&self, entry: &ProviderEntry) -> Result<()>;
    async fn delete_provider(&self, variant: &ProviderVariant) -> Result<()>;
}
