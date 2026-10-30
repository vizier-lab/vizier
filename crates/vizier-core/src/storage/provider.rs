use anyhow::Result;

use crate::config::provider::ProviderId;
use crate::schema::{CustomProvider, ProviderEntry};

/// Saved credentials for catalogue and built-in providers (`provider_config`).
#[async_trait::async_trait]
pub trait ProviderStorage {
    async fn list_providers(&self) -> Result<Vec<ProviderEntry>>;
    async fn get_provider(&self, id: &ProviderId) -> Result<Option<ProviderEntry>>;
    async fn upsert_provider(&self, entry: &ProviderEntry) -> Result<()>;
    async fn delete_provider(&self, id: &ProviderId) -> Result<()>;
}

/// Operator-defined providers (`custom_provider`, data-model §5).
#[async_trait::async_trait]
pub trait CustomProviderStorage {
    async fn list_custom_providers(&self) -> Result<Vec<CustomProvider>>;
    async fn get_custom_provider(&self, id: &ProviderId) -> Result<Option<CustomProvider>>;
    async fn upsert_custom_provider(&self, provider: &CustomProvider) -> Result<()>;
    async fn delete_custom_provider(&self, id: &ProviderId) -> Result<()>;
}
