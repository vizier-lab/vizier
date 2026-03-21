use std::sync::Arc;

use crate::{config::VizierConfig, storage::VizierStorage, transport::VizierTransport};

#[derive(Clone)]
pub struct HTTPState {
    pub config: Arc<VizierConfig>,
    pub transport: VizierTransport,
    pub storage: VizierStorage,
}
