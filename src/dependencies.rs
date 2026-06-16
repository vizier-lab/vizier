use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;

use crate::{
    config::{
        VizierConfig,
        provider::ProviderVariant,
        storage::StorageConfig,
    },
    file_manager::FileManager,
    schema::{AgentToolsConfig, ProviderEntry, ProviderEntryConfig},
    storage::{
        VizierStorage,
        agent::AgentStorage,
        fs::FileSystemStorage,
        global_config::GlobalConfigStorage,
        provider::ProviderStorage,
        sqlite::SqliteStorage,
        user::{AVAILABLE_PERMISSIONS, UserStorage},
    },
    transport::VizierTransport,
};

#[derive(Clone)]
pub struct VizierDependencies {
    pub config: Arc<VizierConfig>,
    pub storage: Arc<VizierStorage>,
    pub sqlite_conn: Option<Arc<Mutex<rusqlite::Connection>>>,
    pub transport: VizierTransport,
    pub file_manager: FileManager,
}

impl VizierDependencies {
    pub async fn new(config: VizierConfig) -> Result<Self> {
        let (storage, sqlite_conn) = match &config.storage {
            StorageConfig::Sqlite => {
                let conn = SqliteStorage::open_connection(&config.workspace)?;
                let conn = Arc::new(Mutex::new(conn));
                (
                    VizierStorage::new(SqliteStorage::new(conn.clone())),
                    Some(conn),
                )
            }
            StorageConfig::Filesystem => {
                let fs = FileSystemStorage::new(config.workspace.clone()).await?;
                (VizierStorage::new(fs), None)
            }
        };

        Self::migrate_users(&storage).await?;
        Self::migrate_providers(&config, &storage).await?;
        Self::migrate_agent_tools(&storage).await?;

        let transport = VizierTransport::new();
        let file_manager = FileManager::new(config.workspace.clone());

        let fm = file_manager.clone();
        let file_transport = transport.clone();
        tokio::spawn(async move {
            fm.run(file_transport).await;
        });

        Ok(Self {
            config: Arc::new(config.clone()),
            storage: Arc::new(storage),
            sqlite_conn,
            transport,
            file_manager,
        })
    }

    async fn migrate_users(storage: &VizierStorage) -> Result<()> {
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

        if storage.user_exists().await? {
            let users = storage.list_users().await?;
            for user in users {
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
                },
            }),
            providers.anthropic.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::anthropic,
                config: ProviderEntryConfig::Anthropic {
                    api_key: c.api_key.clone(),
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
            providers.mistralrs.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::mistralrs,
                config: ProviderEntryConfig::Mistralrs {
                    enabled: c.enabled,
                },
            }),
            providers.elevenlabs.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::elevenlabs,
                config: ProviderEntryConfig::Elevenlabs {
                    api_key: c.api_key.clone(),
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

            if agent_config.tools.mcp_servers.is_empty() {
                if let Some(ref global_servers) = global_mcp {
                    if !global_servers.is_empty() {
                        agent_config.tools.mcp_servers = global_servers.clone();
                        changed = true;
                    }
                }
            }

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

        let _ = storage.delete_global_config("mcp_servers").await;
        let _ = storage.delete_global_config("shell").await;

        Ok(())
    }
}
