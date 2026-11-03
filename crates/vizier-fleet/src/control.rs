//! The `control` role (#166): the scheduler (cron, one-shot, dreams), retention purge,
//! `project_replies`, migrations, catalogue refresh and agent placement.
//!
//! One leader at a time (`lease.leader`) with standbys: a node that fails to renew stops the
//! leased work immediately, before the lease can expire, so a paused node cannot fire cron jobs
//! after another one took over.

use std::sync::Arc;

use vizier_core::config::VizierConfig;
use vizier_schema::{Bus, Capabilities};
use vizier_core::storage::{VizierStorage, blob::BlobStore};

/// One control node: the leader, or a standby waiting for the lease.
#[allow(dead_code, reason = "a skeleton until #166 wires it into `vizier run`")]
pub struct Control {
    /// The bus: messaging, leases and shared state, never the backend behind them.
    bus: Bus,
    /// The engine's config, as this node was started with. A role is told; it never reads the
    /// environment or the home directory for one.
    config: Arc<VizierConfig>,
    /// The shared storage seams: every role talks to Postgres + S3 directly (#157).
    storage: VizierStorage,
    blobs: Arc<dyn BlobStore>,
}

impl Control {
    /// Built from the seams only: the bus, the config and the two storage traits.
    pub fn new(bus: Bus, config: Arc<VizierConfig>, storage: VizierStorage, blobs: Arc<dyn BlobStore>) -> Self {
        Self { bus, config, storage, blobs }
    }
}

impl Control {
    /// What this role needs from the bus; startup refuses a backend that lacks any of it
    /// (`Bus::require`).
    pub fn needs() -> Capabilities {
        Capabilities { messaging: true, leases: true, shared_state: false }
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }
}
