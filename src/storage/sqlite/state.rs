use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::DreamStatus,
    storage::{dream::DreamStorage, state::StateStorage, sqlite::SqliteStorage},
};

#[async_trait::async_trait]
impl StateStorage for SqliteStorage {
    async fn save_state(&self, key: String, value: serde_json::Value) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO state (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, serde_json::to_string(&value)?],
        )?;
        Ok(())
    }

    async fn get_state(&self, key: String) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT value FROM state WHERE key = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![key], |row| {
            let val: String = row.get(0)?;
            Ok(val)
        })?;

        match rows.next() {
            Some(Ok(val)) => Ok(Some(serde_json::from_str(&val)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }
}

#[async_trait::async_trait]
impl DreamStorage for SqliteStorage {
    async fn get_last_dream_time(&self, agent_id: &str) -> Result<Option<DateTime<Utc>>> {
        let key = format!("dream_last_time:{}", agent_id);
        match self.get_state(key).await? {
            Some(val) => {
                let s: String = serde_json::from_value(val)?;
                Ok(Some(DateTime::parse_from_rfc3339(&s)?.with_timezone(&Utc)))
            }
            None => Ok(None),
        }
    }

    async fn set_last_dream_time(&self, agent_id: &str, time: DateTime<Utc>) -> Result<()> {
        let key = format!("dream_last_time:{}", agent_id);
        self.save_state(key, serde_json::json!(time.to_rfc3339()))
            .await
    }

    async fn get_dream_status(&self, agent_id: &str) -> Result<Option<DreamStatus>> {
        let key = format!("dream_status:{}", agent_id);
        match self.get_state(key).await? {
            Some(val) => {
                let status: DreamStatus =
                    serde_json::from_value(val).unwrap_or(DreamStatus::Idle);
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    async fn set_dream_status(&self, agent_id: &str, status: DreamStatus) -> Result<()> {
        let key = format!("dream_status:{}", agent_id);
        self.save_state(key, serde_json::to_value(status)?).await
    }
}
