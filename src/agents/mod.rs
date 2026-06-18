use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;

use crate::agents::process::agent_process;
use crate::config::provider::ProviderVariant;
use crate::dependencies::VizierDependencies;
use crate::embedding::VizierEmbedder;
use crate::indexer::VizierIndexer;
use crate::indexer::sqlite::SqliteIndexer;
use crate::schema::{
    AgentCommand, AgentCommandResult, AgentConfig, AgentHealthStatus, AgentId, AgentSummary,
    ProviderEntryConfig,
};
use crate::storage::agent::AgentStorage;
use crate::storage::provider::ProviderStorage;
use crate::storage::user::UserStorage;
use crate::utils::agent_workspace;

pub mod agent;
pub mod hook;
pub mod mcp;
pub mod memory_ops;
pub mod process;
pub mod shell;
pub mod skill;
pub mod tools;

struct AgentProcess {
    handle: JoinHandle<()>,
    config: AgentConfig,
    shutdown: watch::Sender<bool>,
    memory_op_handle: Option<JoinHandle<()>>,
}

pub struct VizierAgents {
    deps: VizierDependencies,
    processes: HashMap<AgentId, AgentProcess>,
}

impl VizierAgents {
    pub async fn new(deps: VizierDependencies) -> Result<Self> {
        let mut processes = HashMap::new();

        let stored_agents = deps.storage.list_agents().await?;
        let mut join_set = JoinSet::new();

        for (agent_id, config) in stored_agents {
            tracing::info!("starting agent: {}", agent_id);
            let deps = deps.clone();
            join_set.spawn(async move {
                let result = Self::spawn_agent(&deps, &agent_id, &config).await;
                (agent_id, result)
            });
        }

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok((agent_id, Ok(process))) => {
                    processes.insert(agent_id, process);
                }
                Ok((agent_id, Err(e))) => {
                    tracing::error!("failed to start agent '{}': {}", agent_id, e);
                }
                Err(e) => {
                    tracing::error!("agent spawn task failed: {}", e);
                }
            }
        }

        Ok(Self { deps, processes })
    }

    async fn build_indexer(
        deps: &VizierDependencies,
        config: &AgentConfig,
    ) -> Result<Option<VizierIndexer>> {
        let (emb_settings, _idx_cfg) = match (&config.embedding, &config.indexer) {
            (Some(e), Some(i)) => (e, i),
            _ => return Ok(None),
        };

        let conn = match &deps.sqlite_conn {
            Some(conn) => conn.clone(),
            None => return Ok(None),
        };

        let embedder = Arc::new(
            VizierEmbedder::from_agent_settings(
                emb_settings,
                &deps.storage,
                &deps.config.workspace,
            )
            .await?,
        );
        let sqlite_idx = SqliteIndexer::new(conn, embedder).await?;
        Ok(Some(VizierIndexer::build(sqlite_idx)))
    }

    async fn spawn_agent(
        deps: &VizierDependencies,
        agent_id: &str,
        config: &AgentConfig,
    ) -> Result<AgentProcess> {
        if config.provider == ProviderVariant::ollama {
            if let Ok(Some(provider)) = deps.storage.get_provider(&ProviderVariant::ollama).await {
                if let ProviderEntryConfig::Ollama { base_url } = &provider.config {
                    crate::utils::ollama::ollama_pull_model(base_url, &config.model).await?;
                }
            }
        }

        if config.provider == ProviderVariant::mistralrs {
            crate::utils::mistralrs::mistralrs_prefetch_model(
                &deps.config.workspace,
                &config.model,
            )
            .await?;
        }

        if config.tools.stt.enabled
            && config.tools.stt.settings.provider == crate::schema::agent::SttProvider::SenseVoice
        {
            let model = config
                .tools
                .stt
                .settings
                .model
                .as_deref()
                .unwrap_or("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17");
            crate::utils::sense_voice::sense_voice_prefetch_model(&deps.config.workspace, model)
                .await?;
        }

        let indexer = Self::build_indexer(deps, config).await?;
        let memory_op_handle = if let Some(idx) = indexer.clone() {
            let rx = deps.transport.register_memory_op(agent_id.to_string()).await;
            let storage = (*deps.storage).clone();
            let agent_id_owned = agent_id.to_string();
            Some(tokio::spawn(async move {
                if let Err(e) =
                    memory_ops::handle_memory_ops(rx, idx, agent_id_owned, storage).await
                {
                    tracing::error!("memory_ops handler exited with error: {}", e);
                }
            }))
        } else {
            None
        };

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let deps_clone = deps.clone();
        let agent_id_clone = agent_id.to_string();
        let config_clone = config.clone();
        let indexer_clone = indexer.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = agent_process(
                agent_id_clone.clone(),
                deps_clone,
                config_clone,
                indexer_clone,
                shutdown_rx,
            )
            .await
            {
                tracing::error!("agent '{}' exited with error: {}", agent_id_clone, e);
            }
        });

        Ok(AgentProcess {
            handle,
            config: config.clone(),
            shutdown: shutdown_tx,
            memory_op_handle,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let cmd = self.deps.transport.recv_agent_command().await?;
            match cmd {
                AgentCommand::Create {
                    agent_id,
                    config,
                    resp,
                } => {
                    let result = self.handle_create(&agent_id, config).await;
                    let _ = resp.send(result);
                }
                AgentCommand::Update {
                    agent_id,
                    config,
                    resp,
                } => {
                    let result = self.handle_update(&agent_id, config).await;
                    let _ = resp.send(result);
                }
                AgentCommand::Delete {
                    agent_id,
                    delete_workspace,
                    resp,
                } => {
                    let result = self.handle_delete(&agent_id, delete_workspace).await;
                    let _ = resp.send(result);
                }
                AgentCommand::HealthCheck { resp } => {
                    let statuses: Vec<AgentHealthStatus> = self
                        .processes
                        .iter()
                        .map(|(id, process)| AgentHealthStatus {
                            agent_id: id.clone(),
                            alive: !process.handle.is_finished(),
                        })
                        .collect();
                    let _ = resp.send(statuses);
                }
            }
        }
    }

    async fn handle_create(&mut self, agent_id: &str, config: AgentConfig) -> AgentCommandResult {
        if self.processes.contains_key(agent_id) {
            return AgentCommandResult::Error(format!("agent '{}' already exists", agent_id));
        }

        if let Err(e) = self.deps.storage.create_agent(agent_id, &config).await {
            return AgentCommandResult::Error(format!("failed to persist agent: {}", e));
        }

        let workspace = agent_workspace(&self.deps.config.workspace, agent_id)
            .to_string_lossy()
            .to_string();
        agent::system_prompt::init_workspace(workspace);

        match Self::spawn_agent(&self.deps, agent_id, &config).await {
            Ok(process) => {
                let owner_username = if let Some(ref owner_id) = config.owner_id {
                    self.deps.storage.get_user_by_id(owner_id).await.ok().flatten().map(|u| u.username)
                } else {
                    None
                };
                let summary = AgentSummary {
                    agent_id: agent_id.to_string(),
                    name: config.name.clone(),
                    description: config.description.clone(),
                    avatar_url: config.avatar_url.clone(),
                    owner_username,
                    owner_id: config.owner_id.clone(),
                    shared_to: config.shared_to.clone(),
                };
                self.processes.insert(agent_id.to_string(), process);
                AgentCommandResult::Ok(summary)
            }
            Err(e) => AgentCommandResult::Error(format!("failed to start agent: {}", e))
        }
    }

    async fn handle_update(&mut self, agent_id: &str, config: AgentConfig) -> AgentCommandResult {
        if !self.processes.contains_key(agent_id)
            && self.deps.storage.get_agent(agent_id).await.ok().flatten().is_none()
        {
            return AgentCommandResult::Error(format!("agent '{}' not found", agent_id));
        }

        if let Err(e) = self.deps.storage.update_agent(agent_id, &config).await {
            return AgentCommandResult::Error(format!("failed to persist agent: {}", e));
        }

        if let Some(old) = self.processes.remove(agent_id) {
            let _ = old.shutdown.send(true);
            old.handle.abort();
            if let Some(mh) = old.memory_op_handle {
                mh.abort();
            }
        }

        self.deps.transport.unregister_agent(&agent_id.to_string()).await;
        self.deps
            .transport
            .unregister_memory_op(&agent_id.to_string())
            .await;

        match Self::spawn_agent(&self.deps, agent_id, &config).await {
            Ok(process) => {
                let owner_username = if let Some(ref owner_id) = config.owner_id {
                    self.deps.storage.get_user_by_id(owner_id).await.ok().flatten().map(|u| u.username)
                } else {
                    None
                };
                let summary = AgentSummary {
                    agent_id: agent_id.to_string(),
                    name: config.name.clone(),
                    description: config.description.clone(),
                    avatar_url: config.avatar_url.clone(),
                    owner_username,
                    owner_id: config.owner_id.clone(),
                    shared_to: config.shared_to.clone(),
                };
                self.processes.insert(agent_id.to_string(), process);
                AgentCommandResult::Ok(summary)
            }
            Err(e) => AgentCommandResult::Error(format!("failed to restart agent: {}", e)),
        }
    }

    async fn handle_delete(
        &mut self,
        agent_id: &str,
        delete_workspace: bool,
    ) -> AgentCommandResult {
        if !self.processes.contains_key(agent_id) {
            return AgentCommandResult::Error(format!("agent '{}' not found", agent_id));
        }

        if let Err(e) = self.deps.storage.delete_agent(agent_id).await {
            return AgentCommandResult::Error(format!("failed to delete agent: {}", e));
        }

        if let Some(process) = self.processes.remove(agent_id) {
            let _ = process.shutdown.send(true);
            process.handle.abort();
            if let Some(mh) = process.memory_op_handle {
                mh.abort();
            }
        }

        self.deps.transport.unregister_agent(&agent_id.to_string()).await;
        self.deps
            .transport
            .unregister_memory_op(&agent_id.to_string())
            .await;

        if delete_workspace {
            let workspace = agent_workspace(&self.deps.config.workspace, agent_id);
            if workspace.exists() {
                if let Err(e) = std::fs::remove_dir_all(&workspace) {
                    tracing::warn!("failed to delete workspace for '{}': {}", agent_id, e);
                }
            }
        }

        AgentCommandResult::Ok(AgentSummary {
            agent_id: agent_id.to_string(),
            name: String::new(),
            description: None,
            avatar_url: None,
            owner_username: None,
            owner_id: None,
            shared_to: Vec::new(),
        })
    }
}
