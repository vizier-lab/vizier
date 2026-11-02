//! The `channels` role (#167): the Discord and Telegram readers.
//!
//! One reader per integration (`lease.reader.{{integration}}`), held by a channels node, so a
//! bot's token is polled once however many nodes run. Readers do not sit on the control leader.

use std::sync::Arc;

use vizier_core::config::VizierConfig;
use vizier_core::storage::{VizierStorage, blob::BlobStore};

/// One channels node.
#[allow(dead_code, reason = "a skeleton until #167 wires it into `vizier run`")]
pub struct Channels {
    /// The engine's config, as this node was started with. A role is told; it never reads the
    /// environment or the home directory for one.
    config: Arc<VizierConfig>,
    /// The shared storage seams: every role talks to Postgres + S3 directly (#157).
    storage: VizierStorage,
    blobs: Arc<dyn BlobStore>,
}

impl Channels {
    /// Built from the seams only: the config and the two storage traits. The bus
    /// (`Messaging`/`Leases`, #162) becomes the first parameter here once `vizier-schema` lands.
    pub fn new(config: Arc<VizierConfig>, storage: VizierStorage, blobs: Arc<dyn BlobStore>) -> Self {
        Self { config, storage, blobs }
    }
}
