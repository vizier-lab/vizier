use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::DreamStatus,
    storage::{dream::DreamStorage, state::StateStorage, surreal::SurrealStorage},
};

#[async_trait::async_trait]
impl StateStorage for SurrealStorage {
    async fn save_state(&self, key: String, value: serde_json::Value) -> Result<()> {
        let _: Option<serde_json::Value> = self.conn.upsert(("state", key)).content(value).await?;

        Ok(())
    }

    async fn get_state(&self, key: String) -> Result<Option<serde_json::Value>> {
        let mut response = self
            .conn
            .query("SELECT * FROM state WHERE id = $key")
            .bind(("key", key))
            .await?;

        let value: Option<serde_json::Value> = response.take(0)?;

        Ok(value)
    }
}

#[async_trait::async_trait]
impl DreamStorage for SurrealStorage {
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
