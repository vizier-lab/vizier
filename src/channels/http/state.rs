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
    /// The provider and model catalogue (spec 015).
    pub catalogue: crate::catalogue::Catalogue,
    /// For the catalogue and providers APIs, which resolve models and reach providers.
    pub deps: crate::dependencies::VizierDependencies,
    /// Client events per conversation (`specs/019-client-tools` R15): 10 a minute.
    pub client_events: Arc<crate::channels::http::rate_limit::RateLimiter>,
    /// Saved state per client tool call: 2 a second.
    pub client_state_saves: Arc<crate::channels::http::rate_limit::RateLimiter>,
    /// Deferred client tool calls some socket is delivering now (research R18).
    pub deferred_client_calls: Arc<crate::agents::client::DeferredClaims>,
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
