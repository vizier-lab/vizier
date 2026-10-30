use std::sync::Arc;

use vizier_core::{
    agents::background::BackgroundJobs,
    config::VizierConfig,
    storage::{VizierStorage, agent::AgentStorage},
    transport::VizierTransport,
};

/// How `require_auth` establishes who is calling.
#[derive(Debug, Clone)]
pub enum AuthMode {
    /// Credentials on the request: a JWT or an API key.
    Network,
    /// The desktop app's in-process dispatcher: every request is this person.
    Desktop { user_id: String },
}

#[derive(Clone)]
pub struct HTTPState {
    pub config: Arc<VizierConfig>,
    pub transport: VizierTransport,
    pub storage: Arc<VizierStorage>,
    pub auth: AuthMode,
    pub background_jobs: BackgroundJobs,
    /// The provider and model catalogue (spec 015).
    pub catalogue: vizier_core::catalogue::Catalogue,
    /// For the catalogue and providers APIs, which resolve models and reach providers.
    pub deps: vizier_core::dependencies::VizierDependencies,
    /// Client events per conversation (`specs/019-client-tools` R15): 10 a minute.
    pub client_events: Arc<vizier_core::utils::rate_limit::RateLimiter>,
    /// Saved state per client tool call: 2 a second.
    pub client_state_saves: Arc<vizier_core::utils::rate_limit::RateLimiter>,
    /// One visit write per person a minute. Visits are still recorded, but nothing reads
    /// them since spec 029 removed the overview feed; tracked for #82.
    pub visit_throttle: Arc<vizier_core::utils::rate_limit::RateLimiter>,
    /// Deferred client tool calls some socket is delivering now (research R18).
    pub deferred_client_calls: Arc<vizier_core::agents::client::DeferredClaims>,
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
