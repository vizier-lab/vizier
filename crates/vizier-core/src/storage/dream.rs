use crate::time::Timestamp;
use anyhow::Result;


use crate::{
    schema::DreamStatus,
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait DreamStorage {
    async fn get_last_dream_time(&self, agent_id: &str) -> Result<Option<Timestamp>>;
    async fn set_last_dream_time(&self, agent_id: &str, time: Timestamp) -> Result<()>;
    async fn get_dream_status(&self, agent_id: &str) -> Result<Option<DreamStatus>>;
    async fn set_dream_status(&self, agent_id: &str, status: DreamStatus) -> Result<()>;
}

#[async_trait::async_trait]
impl DreamStorage for VizierStorage {
    async fn get_last_dream_time(&self, agent_id: &str) -> Result<Option<Timestamp>> {
        self.0.get_last_dream_time(agent_id).await
    }

    async fn set_last_dream_time(&self, agent_id: &str, time: Timestamp) -> Result<()> {
        self.0.set_last_dream_time(agent_id, time).await
    }

    async fn get_dream_status(&self, agent_id: &str) -> Result<Option<DreamStatus>> {
        self.0.get_dream_status(agent_id).await
    }

    async fn set_dream_status(&self, agent_id: &str, status: DreamStatus) -> Result<()> {
        self.0.set_dream_status(agent_id, status).await
    }
}
