//! The `worker` role (#168): runs agents (`VizierAgents`: supervisors and instances).
//!
//! It scales out, but an agent has exactly one owner worker (`lease.agent.{{id}}`), so a worker
//! takes an agent only when it holds that lease. Its local disk is cache and scratch: anything
//! an agent must see after a move lives in storage.

use std::sync::Arc;

use vizier_core::config::VizierConfig;
use vizier_schema::{Bus, Capabilities};
use vizier_core::storage::{VizierStorage, blob::BlobStore};

/// One worker node.
#[allow(dead_code, reason = "a skeleton until #168 wires it into `vizier run`")]
pub struct Worker {
    /// The bus: messaging, leases and shared state, never the backend behind them.
    bus: Bus,
    /// The engine's config, as this node was started with. A role is told; it never reads the
    /// environment or the home directory for one.
    config: Arc<VizierConfig>,
    /// The shared storage seams: every role talks to Postgres + S3 directly (#157).
    storage: VizierStorage,
    blobs: Arc<dyn BlobStore>,
}

impl Worker {
    /// Built from the seams only: the bus, the config and the two storage traits.
    pub fn new(bus: Bus, config: Arc<VizierConfig>, storage: VizierStorage, blobs: Arc<dyn BlobStore>) -> Self {
        Self { bus, config, storage, blobs }
    }
}

impl Worker {
    /// What this role needs from the bus; startup refuses a backend that lacks any of it
    /// (`Bus::require`).
    pub fn needs() -> Capabilities {
        Capabilities { messaging: true, leases: true, shared_state: false }
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }
}
