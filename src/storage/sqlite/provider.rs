use anyhow::Result;

use crate::config::provider::ProviderVariant;
use crate::schema::ProviderEntry;
use crate::storage::{provider::ProviderStorage, sqlite::SqliteStorage};

#[async_trait::async_trait]
impl ProviderStorage for SqliteStorage {
    async fn list_providers(&self) -> Result<Vec<ProviderEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM provider_config")?;
        let providers = stmt
            .query_map([], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<ProviderEntry>(&data).ok())
            .collect();
        Ok(providers)
    }

    async fn get_provider(&self, variant: &ProviderVariant) -> Result<Option<ProviderEntry>> {
        let variant_str = serde_json::to_string(variant)?.trim_matches('"').to_string();
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM provider_config WHERE variant = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![variant_str], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn upsert_provider(&self, entry: &ProviderEntry) -> Result<()> {
        let variant_str = serde_json::to_string(&entry.variant)?
            .trim_matches('"')
            .to_string();
        let data = serde_json::to_string(entry)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO provider_config (variant, data) VALUES (?1, ?2)",
            rusqlite::params![variant_str, data],
        )?;
        Ok(())
    }

    async fn delete_provider(&self, variant: &ProviderVariant) -> Result<()> {
        let variant_str = serde_json::to_string(variant)?.trim_matches('"').to_string();
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM provider_config WHERE variant = ?1",
            rusqlite::params![variant_str],
        )?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("Provider '{:?}' not found", variant));
        }
        Ok(())
    }
}
