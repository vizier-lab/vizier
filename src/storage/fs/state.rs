use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::DreamStatus,
    storage::{
        dream::DreamStorage,
        fs::{FileSystemStorage, STATE_PATH},
        state::StateStorage,
    },
    utils::build_path,
};

#[async_trait::async_trait]
impl StateStorage for FileSystemStorage {
    async fn save_state(&self, key: String, value: serde_json::Value) -> Result<()> {
        let mut path = build_path(&self.workspace, &[STATE_PATH]);
        let _ = std::fs::create_dir_all(&path)?;
        path.push(format!("{}.json", key));
        std::fs::write(path, serde_json::to_string_pretty(&value)?)?;

        Ok(())
    }

    async fn get_state(&self, key: String) -> Result<Option<serde_json::Value>> {
        let path = build_path(&self.workspace, &[STATE_PATH, &format!("{}.json", key)]);

        if let Ok(raw) = std::fs::read_to_string(&path) {
            let res = serde_json::from_str(&raw)?;

            return Ok(Some(res));
        }

        Ok(None)
    }
}

#[async_trait::async_trait]
impl DreamStorage for FileSystemStorage {
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
