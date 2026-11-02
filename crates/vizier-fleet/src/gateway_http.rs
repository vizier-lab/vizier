//! The `gateway-http` role (#169): REST, WebSocket, the WebUI, and authentication and
//! authorization through `core::service`.
//!
//! It scales out: any number of nodes may serve it, and none of them runs an agent.

use std::sync::Arc;

use vizier_core::config::VizierConfig;
use vizier_core::storage::{VizierStorage, blob::BlobStore};

/// One HTTP gateway node.
#[allow(dead_code, reason = "a skeleton until #169 wires it into `vizier run`")]
pub struct GatewayHttp {
    /// The engine's config, as this node was started with. A role is told; it never reads the
    /// environment or the home directory for one.
    config: Arc<VizierConfig>,
    /// The shared storage seams: every role talks to Postgres + S3 directly (#157).
    storage: VizierStorage,
    blobs: Arc<dyn BlobStore>,
}

impl GatewayHttp {
    /// Built from the seams only: the config and the two storage traits. The bus
    /// (`Messaging`/`Leases`, #162) becomes the first parameter here once `vizier-schema` lands.
    pub fn new(config: Arc<VizierConfig>, storage: VizierStorage, blobs: Arc<dyn BlobStore>) -> Self {
        Self { config, storage, blobs }
    }
}
