//! Background delivery (spec 058, generalised by spec 059 research D9): work that finishes
//! beside a conversation — detached scripts, subagent workers — reaches the agent exactly once,
//! as one user message, either at its next round boundary or as a turn of its own.
//!
//! Exactly once is each source's database claim. This module only asks every registered source
//! in turn and joins what they hand over, so the three claim sites (the round boundary in
//! `prompt_with`, the idle poke in `agent_process` and the queued poke in `handle_request`)
//! never name a source.

use std::{sync::Arc, time::Duration};

use crate::{
    schema::{AgentId, VizierRequest, VizierRequestContent, VizierSession},
    transport::VizierTransport,
};

/// Waits between delivery pokes while the agent is not registered (being respawned).
const POKE_BACKOFF_MS: [u64; 6] = [500, 1_000, 2_000, 4_000, 8_000, 14_000];

/// Something that finishes beside a conversation and must be handed to its agent once.
#[async_trait::async_trait]
pub trait DeliverySource: Send + Sync {
    /// Take this conversation's deliverable outcomes and render each as a section of the
    /// delivery message. Whoever calls first gets them; an empty result means nothing to hand
    /// over. Failures are logged and read as nothing.
    async fn claim(&self, session: &VizierSession) -> Vec<String>;

    /// The agent's conversations with something deliverable (recovery pokes each once).
    async fn sessions_with_deliveries(&self, agent_id: &AgentId) -> Vec<VizierSession>;
}

/// Every delivery source of the process, in a fixed order (detached scripts first).
#[derive(Clone)]
pub struct Deliveries {
    sources: Arc<Vec<Arc<dyn DeliverySource>>>,
}

impl Deliveries {
    pub fn new(sources: Vec<Arc<dyn DeliverySource>>) -> Self {
        Self {
            sources: Arc::new(sources),
        }
    }

    /// Claim from every source; `None` when none had anything. Sections ready at the same
    /// moment share one message, in source order.
    pub async fn claim(&self, session: &VizierSession) -> Option<String> {
        let mut sections = vec![];
        for source in self.sources.iter() {
            sections.extend(source.claim(session).await);
        }
        (!sections.is_empty()).then(|| sections.join("\n\n"))
    }

    /// The agent's conversations with something to deliver, each once.
    pub async fn sessions_with_deliveries(&self, agent_id: &AgentId) -> Vec<VizierSession> {
        let mut sessions: Vec<VizierSession> = vec![];
        for source in self.sources.iter() {
            for session in source.sessions_with_deliveries(agent_id).await {
                if !sessions.contains(&session) {
                    sessions.push(session);
                }
            }
        }
        sessions
    }
}

/// Tell a conversation something may be ready: an empty `DetachedDelivery` request. Its handler
/// claims, and an empty claim runs no turn, so a poke that loses to a round-boundary claim
/// costs nothing. Retried while the agent is between instances.
pub async fn poke(transport: &VizierTransport, session: VizierSession, depth: u8) {
    let request = VizierRequest {
        timestamp: crate::time::now(),
        user: session.0.clone(),
        content: VizierRequestContent::DetachedDelivery(String::new()),
        metadata: serde_json::json!({}),
        background_depth: depth.saturating_add(1),
        ..Default::default()
    };
    let mut last_err = None;
    for attempt in 0..=POKE_BACKOFF_MS.len() {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(POKE_BACKOFF_MS[attempt - 1])).await;
        }
        let mut request = request.clone();
        request.timestamp = crate::time::now();
        let replies = transport.broadcast_sender(session.clone()).await.ok();
        match transport.send_request(session.clone(), request, replies).await {
            Ok(()) => return,
            Err(e) => last_err = Some(e.to_string()),
        }
    }
    // The outcome stays deliverable: the next round boundary or the next recovery takes it.
    tracing::warn!(
        session = %session.to_slug(),
        "could not poke a conversation about finished background work: {}",
        last_err.unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::VizierChannelId;
    use parking_lot::Mutex;

    struct Fake {
        ready: Mutex<Vec<String>>,
        sessions: Vec<VizierSession>,
    }

    #[async_trait::async_trait]
    impl DeliverySource for Fake {
        async fn claim(&self, _session: &VizierSession) -> Vec<String> {
            std::mem::take(&mut *self.ready.lock())
        }
        async fn sessions_with_deliveries(&self, _agent: &AgentId) -> Vec<VizierSession> {
            self.sessions.clone()
        }
    }

    fn session(topic: &str) -> VizierSession {
        VizierSession("a".into(), VizierChannelId::HTTP("u".into(), "c".into()), Some(topic.into()))
    }

    fn fake(ready: &[&str], sessions: &[&str]) -> Arc<dyn DeliverySource> {
        Arc::new(Fake {
            ready: Mutex::new(ready.iter().map(|s| s.to_string()).collect()),
            sessions: sessions.iter().map(|t| session(t)).collect(),
        })
    }

    #[tokio::test]
    async fn sections_from_every_source_share_one_message_in_source_order() {
        let deliveries = Deliveries::new(vec![fake(&["script"], &[]), fake(&["w1", "w2"], &[])]);
        assert_eq!(deliveries.claim(&session("t")).await.as_deref(), Some("script\n\nw1\n\nw2"));
        assert_eq!(deliveries.claim(&session("t")).await, None, "each claimed once");
    }

    #[tokio::test]
    async fn sessions_are_listed_once_across_sources() {
        let deliveries = Deliveries::new(vec![fake(&[], &["t", "u"]), fake(&[], &["u", "v"])]);
        assert_eq!(
            deliveries.sessions_with_deliveries(&"a".into()).await,
            vec![session("t"), session("u"), session("v")]
        );
    }
}
