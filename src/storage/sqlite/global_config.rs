use anyhow::Result;

use crate::schema::GlobalConfigEntry;
use crate::storage::{global_config::GlobalConfigStorage, sqlite::SqliteStorage};

#[async_trait::async_trait]
impl GlobalConfigStorage for SqliteStorage {
    async fn list_global_configs(&self) -> Result<Vec<GlobalConfigEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM global_config")?;
        let entries = stmt
            .query_map([], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<GlobalConfigEntry>(&data).ok())
            .collect();
        Ok(entries)
    }

    async fn get_global_config(&self, key: &str) -> Result<Option<GlobalConfigEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM global_config WHERE key = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![key], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn upsert_global_config(&self, entry: &GlobalConfigEntry) -> Result<()> {
        let data = serde_json::to_string(entry)?;
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
