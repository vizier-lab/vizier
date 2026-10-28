use anyhow::{Context, Result};

use crate::schema::GlobalConfigEntry;
use crate::storage::{global_config::GlobalConfigStorage, sqlite::SqliteStorage};

#[async_trait::async_trait]
impl GlobalConfigStorage for SqliteStorage {
    async fn list_global_configs(&self) -> Result<Vec<GlobalConfigEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT key, data FROM global_config")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        drop(conn);

        let mut entries = Vec::with_capacity(rows.len());
        for (key, data) in rows {
            let Ok(entry) = serde_json::from_str::<GlobalConfigEntry>(&data) else {
                continue;
            };
            let entry = crate::encryption::fields::decrypt_global_config(&self.cipher, &entry)
                .with_context(|| format!("global config '{key}'"))?;
            entries.push(entry);
        }
        Ok(entries)
    }

    async fn get_global_config(&self, key: &str) -> Result<Option<GlobalConfigEntry>> {
        let conn = self.conn.lock();
        let data: Option<String> = conn
            .query_row(
                "SELECT data FROM global_config WHERE key = ?1",
                rusqlite::params![key],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })?;
        drop(conn);

        let Some(entry) = data
            .map(|d| serde_json::from_str::<GlobalConfigEntry>(&d))
            .transpose()?
        else {
            return Ok(None);
        };
        let entry = crate::encryption::fields::decrypt_global_config(&self.cipher, &entry)
            .with_context(|| format!("global config '{key}'"))?;
        Ok(Some(entry))
    }

    async fn upsert_global_config(&self, entry: &GlobalConfigEntry) -> Result<()> {
        let (protected, _) = crate::encryption::fields::protect_global_config(
            &self.cipher,
            entry,
            crate::encryption::ProtectMode::Write,
        )?;
        let data = serde_json::to_string(&protected)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO global_config (key, data) VALUES (?1, ?2)",
            rusqlite::params![entry.key, data],
        )?;
        Ok(())
    }

    async fn delete_global_config(&self, key: &str) -> Result<()> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM global_config WHERE key = ?1",
            rusqlite::params![key],
        )?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("Global config '{}' not found", key));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encryption::storage_tests::{file_backed, global_config, raw_value};

    #[tokio::test]
    async fn shell_env_values_are_protected_and_the_rest_is_readable() {
        let h = file_backed();
        let s = &h.storage;
        s.upsert_global_config(&global_config()).await.unwrap();

        let raw = raw_value(s, "SELECT data FROM global_config WHERE key = 'default'", &[]);
        assert!(raw.contains("enc:v1:"), "{raw}");
        assert!(!raw.contains("global-secret-abc123"));
        assert!(raw.contains("/bin/sh"));

        let entry = s.get_global_config("default").await.unwrap().unwrap();
        let value = serde_json::to_string(&entry).unwrap();
        assert!(value.contains("global-secret-abc123"));

        s.conn
            .lock()
            .execute("UPDATE global_config SET data = replace(data, 'enc:v1:', 'enc:v1:x')", [])
            .unwrap();
        assert!(s.get_global_config("default").await.is_err());
        assert!(s.list_global_configs().await.is_err());
    }
}
