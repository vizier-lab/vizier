//! The `worker` role (#168): runs agents (`VizierAgents`: supervisors and instances).
//!
//! It scales out, but an agent has exactly one owner worker (`lease.agent.{{id}}`), so a worker
//! takes an agent only when it holds that lease. Its local disk is cache and scratch: anything
//! an agent must see after a move lives in storage.

use std::sync::Arc;

use vizier_core::config::VizierConfig;
use vizier_core::storage::{VizierStorage, blob::BlobStore};

/// One worker node.
#[allow(dead_code, reason = "a skeleton until #168 wires it into `vizier run`")]
pub struct Worker {
    /// The engine's config, as this node was started with. A role is told; it never reads the
    /// environment or the home directory for one.
    config: Arc<VizierConfig>,
    /// The shared storage seams: every role talks to Postgres + S3 directly (#157).
    storage: VizierStorage,
    blobs: Arc<dyn BlobStore>,
}

impl Worker {
    /// Built from the seams only: the config and the two storage traits. The bus
    /// (`Messaging`/`Leases`, #162) becomes the first parameter here once `vizier-schema` lands.
    pub fn new(config: Arc<VizierConfig>, storage: VizierStorage, blobs: Arc<dyn BlobStore>) -> Self {
        Self { config, storage, blobs }
    }
}
