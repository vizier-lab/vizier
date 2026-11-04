//! The published views: what a gateway reads when it cannot ask the owner
//! (`specs/048-agent-service-seam` contracts/shared-state-views.md).
//!
//! A gateway never learns which node owns an agent (FR-007), so it cannot assemble a
//! fleet-wide view by asking each owner. Shared state is the one place both sides meet: the
//! owner writes, any gateway reads.
//!
//! **The deadline is in the value, not in the backend.** `SharedState` has no TTL - it is
//! `get`/`put`/`cas`/`delete`/`watch` over bytes with a revision - so the entry carries the
//! moment it stops being true. A missing or expired entry reads as *unknown*, never as a
//! stale value (FR-010).

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::schema::AgentHealthStatus;
use crate::setup::SetupState;

/// How long a published view is good for, in milliseconds (FR-010, SC-008).
pub const VIEW_TTL_MS: i64 = 10_000;

/// The envelope a view is published under.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentView<T> {
    /// Epoch milliseconds. The reader treats the entry as absent once the clock passes it.
    pub expires_at_ms: i64,
    /// The view's payload.
    pub data: T,
}

impl<T> AgentView<T> {
    /// A view that is good until the deadline, from now.
    pub fn new(data: T, now_ms: i64) -> Self {
        Self { expires_at_ms: now_ms + VIEW_TTL_MS, data }
    }

    /// Whether the view has expired.
    pub fn is_expired(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }
}

/// The key an agent's health view is published under. The id is one encoded token, so it
/// cannot widen the prefix a reader watches.
pub fn health_key(agent_id: &str) -> String {
    format!("agent.{}.health", vizier_schema::Subject::encode_token(agent_id))
}

/// The key an agent's setup view is published under.
pub fn setup_key(agent_id: &str) -> String {
    format!("agent.{}.setup", vizier_schema::Subject::encode_token(agent_id))
}

/// The clock the views are read against.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The payload of a published view, if the bytes are a view that has not expired.
pub fn read_view<T: serde::de::DeserializeOwned>(bytes: &[u8], now_ms: i64) -> Option<T> {
    let view: AgentView<T> = serde_json::from_slice(bytes).ok()?;
    if view.is_expired(now_ms) {
        return None;
    }
    Some(view.data)
}

/// Where the views are published.
///
/// The write is its own task: the caller is usually a synchronous state change, and a
/// shared-state write must not block it.
#[derive(Clone)]
pub struct Publisher {
    state: Arc<dyn vizier_schema::SharedState>,
}

impl Publisher {
    /// The publisher for this process's bus.
    pub fn new(transport: &crate::transport::VizierTransport) -> Self {
        Self { state: transport.bus().state.clone() }
    }

    /// Publish a view under `key`.
    pub fn publish<T: Serialize>(&self, key: String, data: T) {
        let Ok(bytes) = serde_json::to_vec(&AgentView::new(data, now_ms())) else {
            tracing::warn!(key, "could not encode a view");
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            // No runtime: there is no bus to write to (a test that never starts one).
            return;
        };
        let state = self.state.clone();
        handle.spawn(async move {
            if let Err(e) = state.put(&key, bytes).await {
                tracing::warn!(key, "could not publish a view: {e}");
            }
        });
    }

    /// Publish an agent's health view.
    pub fn publish_health(&self, agent_id: &str, status: &AgentHealthStatus) {
        self.publish(health_key(agent_id), status.clone());
    }

    /// Publish an agent's setup view.
    pub fn publish_setup(&self, agent_id: &str, setup: &SetupState) {
        self.publish(setup_key(agent_id), setup.clone());
    }
}

/// The health view of `agent_id`, if it is published and live.
pub async fn read_health(
    deps: &crate::dependencies::VizierDependencies,
    agent_id: &str,
) -> Option<AgentHealthStatus> {
    read_health_from(&deps.transport.bus().state, agent_id).await
}

/// The health view of `agent_id`, if it is published and live.
pub async fn read_health_from(
    state: &Arc<dyn vizier_schema::SharedState>,
    agent_id: &str,
) -> Option<AgentHealthStatus> {
    let entry = state.get(&health_key(agent_id)).await.ok()??;
    read_view(&entry.value, now_ms())
}

/// The setup view of `agent_id`, if it is published and live.
pub async fn read_setup(
    deps: &crate::dependencies::VizierDependencies,
    agent_id: &str,
) -> Option<SetupState> {
    read_setup_from(&deps.transport.bus().state, agent_id).await
}

/// The setup view of `agent_id`, if it is published and live.
pub async fn read_setup_from(
    state: &Arc<dyn vizier_schema::SharedState>,
    agent_id: &str,
) -> Option<SetupState> {
    let entry = state.get(&setup_key(agent_id)).await.ok()??;
    read_view(&entry.value, now_ms())
}

/// Publish an agent's health view.
pub fn publish_health(
    deps: &crate::dependencies::VizierDependencies,
    agent_id: &str,
    status: &AgentHealthStatus,
) {
    Publisher::new(&deps.transport).publish_health(agent_id, status);
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{AgentRunState, ChannelHealth};

    fn status(agent_id: &str) -> AgentHealthStatus {
        AgentHealthStatus {
            agent_id: agent_id.into(),
            alive: true,
            live: false,
            state: AgentRunState::Running,
            last_error: None,
            failures: 0,
            next_retry_at: None,
            channels: Vec::new(),
            model: None,
            warnings: Vec::new(),
            session_failure: None,
        }
    }

    fn bus() -> (crate::transport::VizierTransport, Arc<dyn vizier_schema::SharedState>) {
        let transport = crate::transport::VizierTransport::new();
        let state = transport.bus().state.clone();
        (transport, state)
    }

    /// A missing entry reads as unknown.
    #[tokio::test]
    async fn a_missing_entry_reads_as_unknown() {
        let (_transport, state) = bus();
        assert!(read_health_from(&state, "ghost").await.is_none());
        assert!(read_setup_from(&state, "ghost").await.is_none());
    }

    /// A live entry reads as its payload.
    #[tokio::test]
    async fn a_live_entry_reads_as_its_payload() {
        let (transport, state) = bus();
        let publisher = Publisher::new(&transport);
        let mut health = status("alpha");
        health.channels = vec![ChannelHealth {
            platform: crate::schema::ChannelPlatform::Discord,
            state: crate::schema::ChannelState::Running,
            last_error: None,
            failures: 0,
            next_retry_at: None,
        }];
        publisher.publish_health("alpha", &health);
        // The write is its own task; give it a turn to land.
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let read = read_health_from(&state, "alpha").await.expect("a live view");
        assert_eq!(read.agent_id, "alpha");
        assert!(read.alive);
        assert_eq!(read.state, AgentRunState::Running);
        // The view carries everything the reader used to get from the agent's memory.
        assert_eq!(read.channels.len(), 1);
    }

    /// An expired entry reads as unknown, never as a stale value.
    #[tokio::test]
    async fn an_expired_entry_reads_as_unknown() {
        let (transport, state) = bus();
        let publisher = Publisher::new(&transport);
        publisher.publish_health("alpha", &status("alpha"));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Back-date the deadline past the clock.
        let key = health_key("alpha");
        let mut entry = state.get(&key).await.unwrap().unwrap();
        let mut view: AgentView<AgentHealthStatus> = serde_json::from_slice(&entry.value).unwrap();
        view.expires_at_ms = now_ms() - 1;
        entry.value = serde_json::to_vec(&view).unwrap();
        state.put(&key, entry.value).await.unwrap();

        assert!(
            read_health_from(&state, "alpha").await.is_none(),
            "an expired view is unknown, not a stale value"
        );
    }

    /// The deadline is at most ten seconds after the publish (FR-010, SC-008).
    #[test]
    fn a_view_is_good_for_at_most_ten_seconds() {
        let view = AgentView::new(status("alpha"), 1_000);
        assert_eq!(view.expires_at_ms, 1_000 + VIEW_TTL_MS);
        assert_eq!(VIEW_TTL_MS, 10_000);
        assert!(!view.is_expired(1_000 + VIEW_TTL_MS - 1));
        assert!(view.is_expired(1_000 + VIEW_TTL_MS));
    }

    /// The id is one encoded token, so a key cannot widen a prefix.
    #[test]
    fn a_key_carries_one_encoded_token() {
        assert_eq!(health_key("my.agent"), "agent.my%2Eagent.health");
        assert_eq!(setup_key("my.agent"), "agent.my%2Eagent.setup");
        assert_eq!(health_key("my.agent").split('.').count(), 3);
    }
}
