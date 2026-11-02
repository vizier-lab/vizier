//! The `control` role (#166): the scheduler (cron, one-shot, dreams), retention purge,
//! `project_replies`, migrations, catalogue refresh and agent placement.
//!
//! One leader at a time (`lease.leader`) with standbys: a node that fails to renew stops the
//! leased work immediately, before the lease can expire, so a paused node cannot fire cron jobs
//! after another one took over.

use std::sync::Arc;

use vizier_core::config::VizierConfig;
use vizier_core::storage::{VizierStorage, blob::BlobStore};

/// One control node: the leader, or a standby waiting for the lease.
#[allow(dead_code, reason = "a skeleton until #166 wires it into `vizier run`")]
pub struct Control {
    /// The engine's config, as this node was started with. A role is told; it never reads the
    /// environment or the home directory for one.
    config: Arc<VizierConfig>,
    /// The shared storage seams: every role talks to Postgres + S3 directly (#157).
    storage: VizierStorage,
    blobs: Arc<dyn BlobStore>,
}

impl Control {
    /// Built from the seams only: the config and the two storage traits. The bus
    /// (`Messaging`/`Leases`, #162) becomes the first parameter here once `vizier-schema` lands.
    pub fn new(config: Arc<VizierConfig>, storage: VizierStorage, blobs: Arc<dyn BlobStore>) -> Self {
        Self { config, storage, blobs }
    }
}
