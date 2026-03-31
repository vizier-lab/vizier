use std::sync::Arc;

use anyhow::Result;

use crate::{
    config::{
        VizierConfig,
        storage::{DocumentIndexerConfig, StorageConfig},
    },
    embedding::VizierEmbedder,
    mcp::VizierMcpClients,
    shell::VizierShell,
    storage::{
        VizierStorage,
        fs::FileSystemStorage,
        indexer::{VizierIndexer, inmem::InMemIndexer},
        surreal::SurrealStorage,
    },
    transport::VizierTransport,
};

#[derive(Clone)]
pub struct VizierDependencies {
    pub config: Arc<VizierConfig>,
    pub embedder: Option<Arc<VizierEmbedder>>,
    pub transport: VizierTransport,
    pub storage: Arc<VizierStorage>,
    pub mcp_clients: Arc<VizierMcpClients>,
    pub shell: Arc<VizierShell>,
}

impl VizierDependencies {
    pub async fn new(config: VizierConfig) -> Result<Self> {
        let embedder = if config.embedding.is_some() {
            Some(Arc::new(VizierEmbedder::new(&config).await?))
        } else {
            None
        };

        let surreal = SurrealStorage::new(config.workspace.clone(), embedder.clone()).await?;

        let storage = match &config.storage {
            StorageConfig::Surreal => VizierStorage::new(surreal),
            StorageConfig::Filesystem(indexer_config) => {
                let surreal_indexer = VizierIndexer::build(surreal);

                let indexer = match indexer_config {
                    DocumentIndexerConfig::Surreal => VizierIndexer::build(surreal_indexer),
                    DocumentIndexerConfig::InMem => {
                        VizierIndexer::build(InMemIndexer::new(embedder.clone()))
                    }
                };

                let fs =
                    FileSystemStorage::new(config.workspace.clone(), Arc::new(indexer)).await?;
                VizierStorage::new(fs)
            }
        };

        let shell = Arc::new(VizierShell::new(&config.shell).await?);

        let mcp_clients = Arc::new(VizierMcpClients::new(config.clone()).await?);

        Ok(Self {
            config: Arc::new(config.clone()),
            storage: Arc::new(VizierStorage::new(storage)),
            transport: VizierTransport::new(),
            embedder,
            mcp_clients,
            shell,
        })
    }

    pub async fn run(&self) -> Result<()> {
        self.transport.run().await?;

        Ok(())
    }
}
