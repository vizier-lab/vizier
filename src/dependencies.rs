use std::sync::Arc;

use anyhow::Result;
use arc_swap::ArcSwap;

use crate::{
    config::{
        VizierConfig,
        provider::ProviderVariant,
        storage::{DocumentIndexerConfig, StorageConfig},
    },
    embedding::VizierEmbedder,
    mcp::VizierMcpClients,
    schema::{GlobalConfigEntry, GlobalConfigValue, ProviderEntry, ProviderEntryConfig},
    shell::VizierShell,
    storage::{
        VizierStorage,
        agent::AgentStorage,
        fs::FileSystemStorage,
        global_config::GlobalConfigStorage,
        indexer::{VizierIndexer, inmem::InMemIndexer},
        provider::ProviderStorage,
        surreal::SurrealStorage,
        user::{AVAILABLE_PERMISSIONS, UserStorage},
    },
    transport::VizierTransport,
};

#[derive(Clone)]
pub struct VizierDependencies {
    pub config: Arc<VizierConfig>,
    pub embedder: Option<Arc<VizierEmbedder>>,
    pub transport: VizierTransport,
    pub storage: Arc<VizierStorage>,
    pub mcp_clients: Arc<ArcSwap<VizierMcpClients>>,
    pub shell: Arc<ArcSwap<VizierShell>>,
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

        let shell = Arc::new(ArcSwap::new(Arc::new(
            VizierShell::new(&config.shell).await?,
        )));

        let mcp_clients = Arc::new(ArcSwap::new(Arc::new(
            VizierMcpClients::new(config.clone()).await?,
        )));

        // Migrate existing users to have roles
        Self::migrate_users(&storage).await?;

        // Auto-migrate providers from YAML config if storage is empty
        Self::migrate_providers(&config, &storage).await?;

        // Auto-migrate global config (mcp_servers, shell) from YAML if storage is empty
        Self::migrate_global_config(&config, &storage).await?;

        // Auto-migrate channel tokens into agent configs
        Self::migrate_channel_tokens(&config, &storage).await?;

        Ok(Self {
            config: Arc::new(config.clone()),
            storage: Arc::new(VizierStorage::new(storage)),
            transport: VizierTransport::new(),
            embedder,
            mcp_clients,
            shell,
        })
    }

    async fn migrate_users(storage: &VizierStorage) -> Result<()> {
        // Create system role if it doesn't exist
        let system_role = match storage.get_system_role().await? {
            Some(role) => role,
            None => {
                tracing::info!("Creating system role (superadmin)");
                storage
                    .create_role(
                        "superadmin",
                        AVAILABLE_PERMISSIONS.to_vec().into_iter().map(String::from).collect(),
                        true,
                    )
                    .await?
            }
        };

        // Check if any users exist
        if storage.user_exists().await? {
            // Migrate existing users without role_id to superadmin role
            let users = storage.list_users().await?;
            for user in users {
                // Check if user has a valid role_id
                if storage.get_role(&user.role_id).await?.is_none() {
                    tracing::info!(
                        "Migrating user '{}' to superadmin role",
                        user.username
                    );
                    storage
                        .update_user(&user.user_id, None, Some(&system_role.role_id))
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn migrate_providers(config: &VizierConfig, storage: &VizierStorage) -> Result<()> {
        if !storage.list_providers().await?.is_empty() {
            return Ok(());
        }

        tracing::info!("migrating providers from YAML config to storage");

        let providers = &config.providers;
        let entries: Vec<ProviderEntry> = [
            providers.ollama.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::ollama,
                config: ProviderEntryConfig::Ollama {
                    base_url: c.base_url.clone(),
                },
            }),
            providers.openai.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::openai,
                config: ProviderEntryConfig::Openai {
                    api_key: c.api_key.clone(),
                    base_url: c.base_url.clone(),
                },
            }),
            providers.anthropic.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::anthropic,
                config: ProviderEntryConfig::Anthropic {
                    api_key: c.api_key.clone(),
                    base_url: c.base_url.clone(),
                },
            }),
            providers.deepseek.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::deepseek,
                config: ProviderEntryConfig::Deepseek {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.openrouter.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::openrouter,
                config: ProviderEntryConfig::Openrouter {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.gemini.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::gemini,
                config: ProviderEntryConfig::Gemini {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.mimo.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::mimo,
                config: ProviderEntryConfig::Mimo {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.llama_cpp.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::llama_cpp,
                config: ProviderEntryConfig::LlamaCpp {
                    base_url: c.base_url.clone(),
                },
            }),
        ]
        .into_iter()
        .flatten()
        .collect();

        for entry in entries {
            if let Err(e) = storage.upsert_provider(&entry).await {
                tracing::warn!("failed to migrate provider {:?}: {}", entry.variant, e);
            }
        }

        Ok(())
    }

    async fn migrate_channel_tokens(config: &VizierConfig, storage: &VizierStorage) -> Result<()> {
        let agents = storage.list_agents().await?;
        if agents.is_empty() {
            return Ok(());
        }

        let mut needs_update = false;
        for (_, agent_config) in &agents {
            if agent_config.discord_token.is_some() || agent_config.telegram_token.is_some() {
                needs_update = true;
                break;
            }
        }

        if needs_update {
            return Ok(());
        }

        // Check if there are channel tokens in YAML config to migrate
        let has_discord = config
            .channels
            .discord
            .as_ref()
            .map_or(false, |d| !d.is_empty());
        let has_telegram = config
            .channels
            .telegram
            .as_ref()
            .map_or(false, |t| !t.is_empty());

        if !has_discord && !has_telegram {
            return Ok(());
        }

        tracing::info!("migrating channel tokens from YAML config to agent configs");

        for (agent_id, mut agent_config) in agents {
            let mut changed = false;

            if let Some(discord_configs) = &config.channels.discord {
                if let Some(discord_config) = discord_configs.get(&agent_id) {
                    agent_config.discord_token = Some(discord_config.token.clone());
                    changed = true;
                }
            }

            if let Some(telegram_configs) = &config.channels.telegram {
                if let Some(telegram_config) = telegram_configs.get(&agent_id) {
                    agent_config.telegram_token = Some(telegram_config.token.clone());
                    changed = true;
                }
            }

            if changed {
                if let Err(e) = storage.update_agent(&agent_id, &agent_config).await {
                    tracing::warn!(
                        "failed to migrate channel tokens for agent '{}': {}",
                        agent_id,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        self.transport.run().await?;

        Ok(())
    }

    async fn migrate_global_config(config: &VizierConfig, storage: &VizierStorage) -> Result<()> {
        if !storage.list_global_configs().await?.is_empty() {
            return Ok(());
        }

        tracing::info!("migrating global config (mcp_servers, shell) from YAML to storage");

        if !config.tools.mcp_servers.is_empty() {
            let entry = GlobalConfigEntry {
                key: "mcp_servers".to_string(),
                value: GlobalConfigValue::McpServers(config.tools.mcp_servers.clone()),
            };
            if let Err(e) = storage.upsert_global_config(&entry).await {
                tracing::warn!("failed to migrate mcp_servers config: {}", e);
            }
        }

        let entry = GlobalConfigEntry {
            key: "shell".to_string(),
            value: GlobalConfigValue::Shell(config.shell.clone()),
        };
        if let Err(e) = storage.upsert_global_config(&entry).await {
            tracing::warn!("failed to migrate shell config: {}", e);
        }

        Ok(())
    }
}
