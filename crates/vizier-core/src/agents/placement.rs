//! Which host owns which agent (`specs/048-agent-service-seam`).
//!
//! Placement is the control side of the split: it decides, and the agent host obeys. The
//! host never reads the agent list itself - it is told what to hold
//! (`specs/048-agent-service-seam` contracts/agent-host-placement.md).

use async_trait::async_trait;

use crate::Result;
use crate::schema::AgentId;

/// The host that owns an agent.
///
/// The node is **never** exposed to a gateway: the seam addresses the agent and nothing
/// else (FR-007). The gateway never learns which node holds an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerRef {
    node: String,
}

impl OwnerRef {
    /// The owner of the one local host, in a single node.
    pub fn local(node: impl Into<String>) -> Self {
        Self { node: node.into() }
    }

    /// The node the owner runs on. Only the control role reads this.
    pub fn node(&self) -> &str {
        &self.node
    }
}

/// One agent and the host that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedAgent {
    pub agent_id: AgentId,
    pub owner: OwnerRef,
}

/// Decides which host owns which agent.
///
/// The only component that may answer "which node owns this agent", and it answers only to
/// the control role - never to a gateway (FR-007).
#[async_trait]
pub trait Placement: Send + Sync {
    /// The agents this node's host should hold, and who holds each.
    async fn assign(&self) -> Result<Vec<PlacedAgent>>;
}

/// The one node's placement: every stored agent belongs to the one local host.
///
/// This is what a single node does today, so a single node behaves exactly as before
/// (FR-005). The lease-backed placement arrives with the worker and control roles
/// (#166, #168).
pub struct SingleNodePlacement {
    storage: std::sync::Arc<crate::storage::VizierStorage>,
    node: String,
}

impl SingleNodePlacement {
    pub fn new(storage: std::sync::Arc<crate::storage::VizierStorage>) -> Self {
        Self { storage, node: "local".to_string() }
    }

    /// The name of the one local host.
    pub fn node(&self) -> &str {
        &self.node
    }
}

#[async_trait]
impl Placement for SingleNodePlacement {
    async fn assign(&self) -> Result<Vec<PlacedAgent>> {
        use crate::storage::agent::AgentStorage;
        let owner = OwnerRef::local(self.node.clone());
        Ok(self
            .storage
            .list_agents()
            .await
            .map_err(|e| crate::error::VizierError(format!("could not list the agents: {e}")))?
            .into_iter()
            .map(|(agent_id, _)| PlacedAgent { agent_id, owner: owner.clone() })
            .collect())
    }
}
