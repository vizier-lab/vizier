use anyhow::{Context, Result};

use crate::config::provider::ProviderId;
use crate::schema::{CustomProvider, ProviderEntry};
use crate::storage::{
    provider::{CustomProviderStorage, ProviderStorage},
    sqlite::SqliteStorage,
};

#[async_trait::async_trait]
impl ProviderStorage for SqliteStorage {
    /// Rows still in the pre-015 format (before `migrate_provider_ids_v1` runs) don't parse and
    /// are left out. A row that parses but cannot be decrypted is an error naming the provider:
    /// silently dropping it would hide a wrong key or a tampered value.
    async fn list_providers(&self) -> Result<Vec<ProviderEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM provider_config ORDER BY variant")?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        drop(conn);

        let mut providers = Vec::with_capacity(rows.len());
        for data in rows {
            let Ok(entry) = serde_json::from_str::<ProviderEntry>(&data) else {
                continue;
            };
            let entry = crate::encryption::fields::decrypt_provider_entry(&self.cipher, &entry)
                .with_context(|| format!("provider '{}'", entry.id))?;
            providers.push(entry);
        }
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
        drop(conn);

        let Some(entry) = data.and_then(|d| serde_json::from_str::<ProviderEntry>(&d).ok()) else {
            return Ok(None);
        };
        let entry = crate::encryption::fields::decrypt_provider_entry(&self.cipher, &entry)
            .with_context(|| format!("provider '{}'", entry.id))?;
        Ok(Some(entry))
    }

    async fn upsert_provider(&self, entry: &ProviderEntry) -> Result<()> {
        let (protected, _) = crate::encryption::fields::protect_provider_entry(
            &self.cipher,
            entry,
            crate::encryption::ProtectMode::Write,
        )?;
        let data = serde_json::to_string(&protected)?;
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
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        drop(conn);

        let mut providers = Vec::with_capacity(rows.len());
        for (id, data) in rows {
            let provider = match serde_json::from_str::<CustomProvider>(&data) {
                Ok(provider) => provider,
                Err(e) => {
                    tracing::warn!("skipping unreadable custom provider '{id}': {e}");
                    continue;
                }
            };
            let provider =
                crate::encryption::fields::decrypt_custom_provider(&self.cipher, &provider)
                    .with_context(|| format!("custom provider '{id}'"))?;
            providers.push(provider);
        }
        Ok(providers)
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
        drop(conn);

        let Some(provider) = data.map(|d| serde_json::from_str::<CustomProvider>(&d)).transpose()?
        else {
            return Ok(None);
        };
        let provider = crate::encryption::fields::decrypt_custom_provider(&self.cipher, &provider)
            .with_context(|| format!("custom provider '{}'", provider.id))?;
        Ok(Some(provider))
    }

    async fn upsert_custom_provider(&self, provider: &CustomProvider) -> Result<()> {
        let (protected, _) = crate::encryption::fields::protect_custom_provider(
            &self.cipher,
            provider,
            crate::encryption::ProtectMode::Write,
        )?;
        let data = serde_json::to_string(&protected)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encryption::storage_tests::{file_backed, provider, raw_value};

    #[tokio::test]
    async fn provider_secrets_are_envelopes_and_siblings_are_readable() {
        let h = file_backed();
        let s = &h.storage;
        s.upsert_provider(&provider("provider-secret-abc123")).await.unwrap();

        let raw = raw_value(s, "SELECT data FROM provider_config WHERE variant = 'openai'", &[]);
        assert!(raw.contains("enc:v1:"), "{raw}");
        assert!(!raw.contains("provider-secret-abc123"));
        assert!(raw.contains("https://base.example/v1"));
        assert!(raw.contains("https://endpoint.example"));

        let got = s.get_provider(&"openai".parse().unwrap()).await.unwrap().unwrap();
        assert_eq!(got.credentials.api_key.as_deref(), Some("provider-secret-abc123"));
        assert_eq!(got.credentials.endpoint.as_deref(), Some("https://endpoint.example"));

        s.conn
            .lock()
            .execute("UPDATE provider_config SET data = replace(data, 'enc:v1:', 'enc:v1:x')", [])
            .unwrap();
        assert!(s.get_provider(&"openai".parse().unwrap()).await.is_err());
        assert!(s.list_providers().await.is_err());
    }

    #[tokio::test]
    async fn custom_provider_headers_and_key_are_envelopes() {
        let h = file_backed();
        let s = &h.storage;
        let mut custom = custom();
        custom.id = "acme".parse().unwrap();
        s.upsert_custom_provider(&custom).await.unwrap();

        let raw = raw_value(s, "SELECT data FROM custom_provider WHERE id = 'acme'", &[]);
        assert!(raw.contains("enc:v1:"), "{raw}");
        assert!(!raw.contains("custom-secret"));
        assert!(!raw.contains("header-secret"));
        assert!(raw.contains("Acme"));
        assert!(raw.contains("https://acme.invalid/v1"));
        assert!(raw.contains("\"Authorization\""));

        let got = s.get_custom_provider(&"acme".parse().unwrap()).await.unwrap().unwrap();
        assert_eq!(got.api_key.as_deref(), Some("custom-secret"));
        assert_eq!(got.headers["Authorization"], "Bearer header-secret");
    }

    fn custom() -> crate::schema::CustomProvider {
        let mut headers = std::collections::BTreeMap::new();
        headers.insert("Authorization".into(), "Bearer header-secret".into());
        crate::schema::CustomProvider {
            id: "acme".parse().unwrap(),
            name: "Acme".into(),
            protocol: crate::schema::CustomProtocol::OpenaiCompatible,
            base_url: "https://acme.invalid/v1".into(),
            api_key: Some("custom-secret".into()),
            headers,
            models: vec![],
            models_fetched_at: None,
        }
    }
}
