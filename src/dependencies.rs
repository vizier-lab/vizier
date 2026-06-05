use std::sync::Arc;

use anyhow::Result;

use crate::{
    config::{
        VizierConfig,
        provider::ProviderVariant,
        storage::{DocumentIndexerConfig, StorageConfig},
    },
    embedding::VizierEmbedder,
    schema::{AgentToolsConfig, ProviderEntry, ProviderEntryConfig},
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

        // Migrate existing users to have roles
        Self::migrate_users(&storage).await?;

        // Auto-migrate providers from YAML config if storage is empty
        Self::migrate_providers(&config, &storage).await?;

        // Auto-migrate channel tokens into agent configs
        Self::migrate_channel_tokens(&config, &storage).await?;

        // Migrate per-agent MCP/shell from global config to agent configs
        Self::migrate_agent_tools(&storage).await?;

        Ok(Self {
            config: Arc::new(config.clone()),
            storage: Arc::new(VizierStorage::new(storage)),
            transport: VizierTransport::new(),
            embedder,
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

    async fn migrate_agent_tools(storage: &VizierStorage) -> Result<()> {
        use std::collections::HashMap;

        let agents = storage.list_agents().await?;
        if agents.is_empty() {
            return Ok(());
        }

        // Load global MCP servers config from storage
        let global_mcp = match storage.get_global_config("mcp_servers").await {
            Ok(Some(entry)) => {
                if let crate::schema::GlobalConfigValue::McpServers(servers) = entry.value {
                    Some(servers)
                } else {
                    None
                }
            }
            _ => None,
        };

        // Load global shell config from storage
        let global_shell = match storage.get_global_config("shell").await {
            Ok(Some(entry)) => {
                if let crate::schema::GlobalConfigValue::Shell(shell) = entry.value {
                    Some(shell)
                } else {
                    None
                }
            }
            _ => None,
        };

        let mut migrated = 0;
        for (agent_id, mut agent_config) in agents {
            let mut changed = false;

            // Migrate MCP servers: if agent has empty mcp_servers but global has servers,
            // this is a legacy agent that referenced global servers by name.
            // Since we can't know which names it used, we copy all global servers.
            if agent_config.tools.mcp_servers.is_empty() {
                if let Some(ref global_servers) = global_mcp {
                    if !global_servers.is_empty() {
                        agent_config.tools.mcp_servers = global_servers.clone();
                        changed = true;
                    }
                }
            }

            // Migrate shell: if agent had shell_access=true (now gone), inherit global shell
            // The old schema had shell_access: bool. After deserialization with the new schema,
            // shell will be None. We check if the old field was true by looking at the raw data.
            // Since we can't access raw data here, we use a heuristic: if the agent config
            // doesn't have shell set but the global shell exists, we don't auto-migrate
            // because we can't distinguish between "shell_access was false" and "shell_access was true".
            // The migration of shell_access -> shell is handled by serde's default behavior.
            // Old agents with shell_access: true will have shell: None in the new schema.
            // We leave this for manual migration or skip it.

            if changed {
                if let Err(e) = storage.update_agent(&agent_id, &agent_config).await {
                    tracing::warn!(
                        "failed to migrate tools for agent '{}': {}",
                        agent_id,
                        e
                    );
                } else {
                    migrated += 1;
                }
            }
        }

        if migrated > 0 {
            tracing::info!("migrated tools config for {} agents", migrated);
        }

        // Clean up global config entries (they're no longer needed)
        let _ = storage.delete_global_config("mcp_servers").await;
        let _ = storage.delete_global_config("shell").await;

        Ok(())
    }
}
