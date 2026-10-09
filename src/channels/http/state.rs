use std::sync::Arc;

use crate::{
    agents::background::BackgroundJobs,
    config::VizierConfig,
    storage::{VizierStorage, agent::AgentStorage},
    transport::VizierTransport,
};

#[derive(Clone)]
pub struct HTTPState {
    pub config: Arc<VizierConfig>,
    pub transport: VizierTransport,
    pub storage: Arc<VizierStorage>,
    pub background_jobs: BackgroundJobs,
}

impl HTTPState {
    pub async fn is_agent_exists(&self, agent_id: &str) -> bool {
        self.storage
            .get_agent(agent_id)
            .await
            .ok()
            .flatten()
            .is_some()
    }
}
