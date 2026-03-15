use std::sync::Arc;

use anyhow::Result;

use crate::{config::VizierConfig, database::VizierDatabases, transport::VizierTransport};

#[derive(Clone)]
pub struct VizierDependencies {
    pub config: Arc<VizierConfig>,
    pub embedder: Option<Arc<crate::embedding::EmbeddingModel>>,
    pub transport: VizierTransport,
    pub database: VizierDatabases,
}

impl VizierDependencies {
    pub async fn new(config: VizierConfig) -> Result<Self> {
        let workspace = config.workspace.clone();
        let embedder = config.tools.vector_memory.clone().map(|config| {
            Arc::new(
                crate::embedding::Client::new()
                    .embedding_model(&config.model.to_fastembed(), Some(workspace)),
            )
        });
        Ok(Self {
            config: Arc::new(config.clone()),
            database: VizierDatabases::new(config.workspace.clone()).await?,
            transport: VizierTransport::new(),
            embedder,
        })
    }

    pub async fn run(&self) -> Result<()> {
        self.transport.run().await?;

        Ok(())
    }
}
