use std::sync::Arc;

use anyhow::Result;

use crate::{
    config::{VizierConfig, storage::StorageConfig},
    storage::{VizierStorage, fs::FileSystemStorage, surreal::SurrealStorage},
    transport::VizierTransport,
};

#[derive(Clone)]
pub struct VizierDependencies {
    pub config: Arc<VizierConfig>,
    pub embedder: Option<Arc<crate::embedding::EmbeddingModel>>,
    pub transport: VizierTransport,
    pub storage: Arc<VizierStorage>,
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

        let storage = match config.storage {
            StorageConfig::Filesystem => {
                let fs = FileSystemStorage::new(config.workspace.clone(), embedder.clone()).await?;
                VizierStorage::new(fs)
            }
            StorageConfig::Surreal => {
                let surreal =
                    SurrealStorage::new(config.workspace.clone(), embedder.clone()).await?;
                VizierStorage::new(surreal)
            }
        };

        Ok(Self {
            config: Arc::new(config.clone()),
            storage: Arc::new(VizierStorage::new(storage)),
            transport: VizierTransport::new(),
            embedder,
        })
    }

    pub async fn run(&self) -> Result<()> {
        self.transport.run().await?;

        Ok(())
    }
}
