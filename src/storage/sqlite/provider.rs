use anyhow::Result;

use crate::config::provider::ProviderId;
use crate::schema::{CustomProvider, ProviderEntry};
use crate::storage::{
    provider::{CustomProviderStorage, ProviderStorage},
    sqlite::SqliteStorage,
};

#[async_trait::async_trait]
impl ProviderStorage for SqliteStorage {
    /// Rows still in the pre-015 format (before `migrate_provider_ids_v1` runs) don't parse and
    /// are left out.
    async fn list_providers(&self) -> Result<Vec<ProviderEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM provider_config ORDER BY variant")?;
        let providers = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<ProviderEntry>(&data).ok())
            .collect();
        Ok(providers)
    }

    async fn get_provider(&self, id: &ProviderId) -> Result<Option<ProviderEntry>> {
        let conn = self.conn.lock();
        let data: Option<String> = conn
            .query_row(
                "SELECT data FROM provider_config WHERE variant = ?1",
                rusqlite::params![id.as_str()],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })?;
        Ok(data.and_then(|d| serde_json::from_str(&d).ok()))
    }

    async fn upsert_provider(&self, entry: &ProviderEntry) -> Result<()> {
        let data = serde_json::to_string(entry)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO provider_config (variant, data) VALUES (?1, ?2)",
            rusqlite::params![entry.id.as_str(), data],
        )?;
        Ok(())
    }

    async fn delete_provider(&self, id: &ProviderId) -> Result<()> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM provider_config WHERE variant = ?1",
            rusqlite::params![id.as_str()],
        )?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("Provider '{id}' not found"));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl CustomProviderStorage for SqliteStorage {
    async fn list_custom_providers(&self) -> Result<Vec<CustomProvider>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id, data FROM custom_provider ORDER BY id")?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .filter_map(|(id, data)| match serde_json::from_str::<CustomProvider>(&data) {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::warn!("skipping unreadable custom provider '{id}': {e}");
                    None
                }
            })
            .collect();
        Ok(rows)
    }

    async fn get_custom_provider(&self, id: &ProviderId) -> Result<Option<CustomProvider>> {
        let conn = self.conn.lock();
        let data: Option<String> = conn
            .query_row(
                "SELECT data FROM custom_provider WHERE id = ?1",
                rusqlite::params![id.as_str()],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })?;
        Ok(match data {
            Some(d) => Some(serde_json::from_str(&d)?),
            None => None,
        })
    }

    async fn upsert_custom_provider(&self, provider: &CustomProvider) -> Result<()> {
        let data = serde_json::to_string(provider)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO custom_provider (id, data) VALUES (?1, ?2)",
            rusqlite::params![provider.id.as_str(), data],
        )?;
        Ok(())
    }

    async fn delete_custom_provider(&self, id: &ProviderId) -> Result<()> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM custom_provider WHERE id = ?1",
            rusqlite::params![id.as_str()],
        )?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("Custom provider '{id}' not found"));
        }
        Ok(())
    }
}
