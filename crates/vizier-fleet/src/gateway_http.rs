//! The `gateway-http` role (#169): REST, WebSocket, the WebUI, and authentication and
//! authorization through `core::service`.
//!
//! It scales out: any number of nodes may serve it, and none of them runs an agent.

use std::sync::Arc;

use vizier_core::config::VizierConfig;
use vizier_schema::{Bus, Capabilities};
use vizier_core::storage::{VizierStorage, blob::BlobStore};

/// One HTTP gateway node.
#[allow(dead_code, reason = "a skeleton until #169 wires it into `vizier run`")]
pub struct GatewayHttp {
    /// The bus: messaging, leases and shared state, never the backend behind them.
    bus: Bus,
    /// The engine's config, as this node was started with. A role is told; it never reads the
    /// environment or the home directory for one.
    config: Arc<VizierConfig>,
    /// The shared storage seams: every role talks to Postgres + S3 directly (#157).
    storage: VizierStorage,
    blobs: Arc<dyn BlobStore>,
}

impl GatewayHttp {
    /// Built from the seams only: the bus, the config and the two storage traits.
    pub fn new(bus: Bus, config: Arc<VizierConfig>, storage: VizierStorage, blobs: Arc<dyn BlobStore>) -> Self {
        Self { bus, config, storage, blobs }
    }
}

impl GatewayHttp {
    /// What this role needs from the bus; startup refuses a backend that lacks any of it
    /// (`Bus::require`).
    pub fn needs() -> Capabilities {
        Capabilities { messaging: true, leases: false, shared_state: false }
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }
}
