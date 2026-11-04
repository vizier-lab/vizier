use crate::storage::sql::params;
use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::DreamStatus,
    storage::{dream::DreamStorage, sqlite::SqlStorage, state::StateStorage},
};

#[async_trait::async_trait]
impl StateStorage for SqlStorage {
    async fn save_state(&self, key: String, value: serde_json::Value) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO state (key, value) VALUES (?1, ?2) \
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(&value)?],
        )?;
        Ok(())
    }

    async fn get_state(&self, key: String) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT value FROM state WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| {
            let val: String = row.get(0)?;
            Ok(val)
        })?;

        match rows.next() {
            Some(Ok(val)) => Ok(Some(serde_json::from_str(&val)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn delete_state(&self, key: String) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute("DELETE FROM state WHERE key = ?1", params![key])?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl DreamStorage for SqlStorage {
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
                let status: DreamStatus = serde_json::from_value(val).unwrap_or(DreamStatus::Idle);
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
