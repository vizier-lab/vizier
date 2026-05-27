use std::sync::Arc;

use anyhow::Result;

use crate::{
    channels::http::auth::AuthService,
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
        user::UserStorage,
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

                let (reindex, indexer) = match indexer_config {
                    DocumentIndexerConfig::Surreal => {
                        (false, VizierIndexer::build(surreal_indexer))
                    }
                    DocumentIndexerConfig::InMem => (
                        true,
                        VizierIndexer::build(InMemIndexer::new(embedder.clone())),
                    ),
                };

                let fs =
                    FileSystemStorage::new(config.workspace.clone(), Arc::new(indexer), reindex)
                        .await?;
                VizierStorage::new(fs)
            }
        };

        let shell = Arc::new(VizierShell::new(&config.shell).await?);

        let mcp_clients = Arc::new(VizierMcpClients::new(config.clone()).await?);

        // Initialize default user if no users exist
        Self::initialize_default_user(&config, &storage).await?;

        Ok(Self {
            config: Arc::new(config.clone()),
            storage: Arc::new(VizierStorage::new(storage)),
            transport: VizierTransport::new(),
            embedder,
            mcp_clients,
            shell,
        })
    }

    async fn initialize_default_user(config: &VizierConfig, storage: &VizierStorage) -> Result<()> {
        // Check if any users exist
        if !storage.user_exists().await? {
            // Get the primary user name from config
            let username = &config.primary_user.username;

            // Create default password hash
            // We need to create a temporary AuthService for this
            // Since we don't have the HTTP config here, we'll use a default
            let default_http_config = crate::config::HTTPChannelConfig {
                port: 0,
                jwt_secret: "temp".to_string(),
                jwt_expiry_hours: 720,
            };
            let auth_service = AuthService::new(&default_http_config);

            let password_hash = auth_service.hash_password("admin")?;

            // Create the user
            storage.create_user(username, &password_hash).await?;

            tracing::warn!(
                "Default user '{}' created with password 'admin'. Please change immediately!",
                username
            );
        }

        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        self.transport.run().await?;

        Ok(())
    }
}
