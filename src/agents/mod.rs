use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::agents::process::agent_process;
use crate::config::provider::ProviderVariant;
use crate::dependencies::VizierDependencies;
use crate::schema::{
    AgentCommand, AgentCommandResult, AgentConfig, AgentId, AgentSummary, ProviderEntryConfig,
};
use crate::storage::agent::AgentStorage;
use crate::storage::provider::ProviderStorage;
use crate::utils::agent_workspace;

pub mod agent;
pub mod hook;
pub mod process;
pub mod skill;
pub mod tools;

struct AgentProcess {
    handle: JoinHandle<()>,
    config: AgentConfig,
    shutdown: watch::Sender<bool>,
}

pub struct VizierAgents {
    deps: VizierDependencies,
    processes: HashMap<AgentId, AgentProcess>,
}

impl VizierAgents {
    pub async fn new(deps: VizierDependencies) -> Result<Self> {
        let mut processes = HashMap::new();

        let stored_agents = deps.storage.list_agents().await?;
        for (agent_id, config) in stored_agents {
            tracing::info!("starting agent: {}", agent_id);
            match Self::spawn_agent(&deps, &agent_id, &config).await {
                Ok(process) => {
                    processes.insert(agent_id, process);
                }
                Err(e) => {
                    tracing::error!("failed to start agent '{}': {}", agent_id, e);
                }
            }
        }

        Ok(Self { deps, processes })
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

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let deps_clone = deps.clone();
        let agent_id_clone = agent_id.to_string();
        let config_clone = config.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = agent_process(
                agent_id_clone.clone(),
                deps_clone,
                config_clone,
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
                let summary = AgentSummary {
                    agent_id: agent_id.to_string(),
                    name: config.name.clone(),
                    description: config.description.clone(),
                };
                self.processes.insert(agent_id.to_string(), process);
                AgentCommandResult::Ok(summary)
            }
            Err(e) => {
                let _ = self.deps.storage.delete_agent(agent_id).await;
                AgentCommandResult::Error(format!("failed to start agent: {}", e))
            }
        }
    }

    async fn handle_update(&mut self, agent_id: &str, config: AgentConfig) -> AgentCommandResult {
        if !self.processes.contains_key(agent_id) {
            return AgentCommandResult::Error(format!("agent '{}' not found", agent_id));
        }

        if let Err(e) = self.deps.storage.update_agent(agent_id, &config).await {
            return AgentCommandResult::Error(format!("failed to persist agent: {}", e));
        }

        if let Some(old) = self.processes.remove(agent_id) {
            let _ = old.shutdown.send(true);
            old.handle.abort();
        }

        match Self::spawn_agent(&self.deps, agent_id, &config).await {
            Ok(process) => {
                let summary = AgentSummary {
                    agent_id: agent_id.to_string(),
                    name: config.name.clone(),
                    description: config.description.clone(),
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
        }

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
        })
    }
}
