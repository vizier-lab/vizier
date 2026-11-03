//! The `gateway-grpc` role (#170): the same operations as the HTTP gateway, over gRPC, for
//! other software.
//!
//! It scales out like the HTTP gateway, and is built only when the `gateway-grpc` feature is on.

use std::sync::Arc;

use vizier_core::config::VizierConfig;
use vizier_core::storage::{VizierStorage, blob::BlobStore};

/// One gRPC gateway node.
#[allow(dead_code, reason = "a skeleton until #170 wires it into `vizier run`")]
pub struct GatewayGrpc {
    /// The engine's config, as this node was started with. A role is told; it never reads the
    /// environment or the home directory for one.
    config: Arc<VizierConfig>,
    /// The shared storage seams: every role talks to Postgres + S3 directly (#157).
    storage: VizierStorage,
    blobs: Arc<dyn BlobStore>,
}

impl GatewayGrpc {
    /// Built from the seams only: the config and the two storage traits. The bus
    /// (`Messaging`/`Leases`, #162) becomes the first parameter here once `vizier-schema` lands.
    pub fn new(config: Arc<VizierConfig>, storage: VizierStorage, blobs: Arc<dyn BlobStore>) -> Self {
        Self { config, storage, blobs }
    }
}
