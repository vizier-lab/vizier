use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::agents::supervisor::{AgentStatus, BuiltAgent, SharedStatus, Supervisor};
use crate::constant::CORE_MD;
use crate::dependencies::VizierDependencies;
use crate::schema::{
    AgentCommand, AgentCommandResult, AgentConfig, AgentHealthStatus, AgentId, AgentSummary,
    RevisionOrigin, RevisionTrigger,
};
use crate::storage::agent::AgentStorage;
use crate::storage::user::UserStorage;
use crate::transport::VizierRequestEnvelope;
use crate::utils::agent_workspace;

pub mod agent;
pub mod background;
pub mod hook;
pub mod mcp;
pub mod memory_ops;
pub mod process;
pub mod shell;
pub mod skill;
pub mod supervisor;
pub mod tools;

/// How long an update waits for the old agent to stop before aborting it.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// One agent's supervisor. Every agent in storage has one, whether it is running, starting
/// or failed, so update and delete always find it (#42).
struct AgentProcess {
    handle: JoinHandle<()>,
    shutdown: watch::Sender<bool>,
    status: SharedStatus,
    /// The agent's request channel, registered once and handed from one supervisor to the
    /// next across updates, so nothing sent during a swap is lost (#43).
    recv: flume::Receiver<VizierRequestEnvelope>,
}

impl AgentProcess {
    /// Ask the supervisor to stop, and wait for it to, up to `STOP_TIMEOUT`.
    async fn stop(self, agent_id: &str) {
        let _ = self.shutdown.send(true);
        let abort = self.handle.abort_handle();
        if tokio::time::timeout(STOP_TIMEOUT, self.handle).await.is_err() {
            tracing::warn!(agent_id, "agent did not stop in time, aborting it");
            abort.abort();
        }
    }
}

pub struct VizierAgents {
    deps: VizierDependencies,
    processes: HashMap<AgentId, AgentProcess>,
}

impl VizierAgents {
    /// Register and start every stored agent. Returns once all are registered, without
    /// waiting for any to finish starting: requests sent meanwhile queue (#26, #43).
    pub async fn new(deps: VizierDependencies) -> Result<Self> {
        let mut agents = Self {
            deps,
            processes: HashMap::new(),
        };

        for (agent_id, config) in agents.deps.storage.list_agents().await? {
            tracing::info!("starting agent: {}", agent_id);
            let recv = agents.deps.transport.register_agent(agent_id.clone()).await;
            agents.start(agent_id, config, recv, None);
        }

        Ok(agents)
    }

    /// Spawn a supervisor for the agent on an already-registered request channel.
    fn start(
        &mut self,
        agent_id: AgentId,
        config: AgentConfig,
        recv: flume::Receiver<VizierRequestEnvelope>,
        prebuilt: Option<BuiltAgent>,
    ) {
        let (shutdown, shutdown_rx) = watch::channel(false);
        let status = SharedStatus::default();
        let supervisor = Supervisor {
            agent_id: agent_id.clone(),
            deps: self.deps.clone(),
            config,
            recv: recv.clone(),
            shutdown_rx,
            status: status.clone(),
        };
        let handle = tokio::spawn(supervisor.run(prebuilt));
        self.processes.insert(
            agent_id,
            AgentProcess {
                handle,
                shutdown,
                status,
                recv,
            },
        );
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
                        .map(|(id, process)| AgentStatus::health(&process.status, id))
                        .collect();
                    let _ = resp.send(statuses);
                }
            }
        }
    }

    async fn summary(&self, agent_id: &str, config: &AgentConfig) -> AgentSummary {
        let owner_username = if let Some(ref owner_id) = config.owner_id {
            self.deps
                .storage
                .get_user_by_id(owner_id)
                .await
                .ok()
                .flatten()
                .map(|u| u.username)
        } else {
            None
        };
        AgentSummary {
            agent_id: agent_id.to_string(),
            name: config.name.clone(),
            description: config.description.clone(),
            avatar_url: config.avatar_url.clone(),
            owner_username,
            owner_id: config.owner_id.clone(),
            shared_to: config.shared_to.clone(),
        }
    }

    async fn exists(&self, agent_id: &str) -> bool {
        self.processes.contains_key(agent_id)
            || self
                .deps
                .storage
                .get_agent(agent_id)
                .await
                .ok()
                .flatten()
                .is_some()
    }

    /// Built before it is persisted, so a config that can't start is refused rather than
    /// stored as an agent that never runs.
    async fn handle_create(&mut self, agent_id: &str, config: AgentConfig) -> AgentCommandResult {
        if let Err(msg) = validate_config(&config) {
            return AgentCommandResult::Error(msg);
        }

        if self.exists(agent_id).await {
            return AgentCommandResult::Error(format!("agent '{}' already exists", agent_id));
        }

        let agent_id = agent_id.to_string();
        let built = match BuiltAgent::build(&self.deps, &agent_id, &config).await {
            Ok(built) => built,
            Err(e) => return AgentCommandResult::Error(format!("failed to start agent: {e:#}")),
        };

        if let Err(e) = self.deps.storage.create_agent(&agent_id, &config).await {
            return AgentCommandResult::Error(format!("failed to persist agent: {}", e));
        }

        if let Err(e) = self
            .deps
            .storage
            .set_agent_core(
                &agent_id,
                CORE_MD,
                &RevisionOrigin::system(RevisionTrigger::Baseline),
            )
            .await
        {
            tracing::warn!(
                "failed to seed default CORE for agent '{}': {}",
                agent_id,
                e
            );
        }

        let recv = self.deps.transport.register_agent(agent_id.clone()).await;
        let summary = self.summary(&agent_id, &config).await;
        self.start(agent_id, config, recv, Some(built));
        AgentCommandResult::Ok(summary)
    }

    /// Build the new config first and swap only once it has started: a config that can't
    /// start is refused and the agent keeps running as it was (#43). Requests sent during
    /// the swap wait in the agent's channel, which the new supervisor inherits.
    async fn handle_update(&mut self, agent_id: &str, config: AgentConfig) -> AgentCommandResult {
        if let Err(msg) = validate_config(&config) {
            return AgentCommandResult::Error(msg);
        }

        if !self.exists(agent_id).await {
            return AgentCommandResult::Error(format!("agent '{}' not found", agent_id));
        }

        let agent_id = agent_id.to_string();
        let built = match BuiltAgent::build(&self.deps, &agent_id, &config).await {
            Ok(built) => built,
            Err(e) => {
                return AgentCommandResult::Error(format!(
                    "the new config failed to start, so the agent keeps its previous one: {e:#}"
                ));
            }
        };

        if let Err(e) = self.deps.storage.update_agent(&agent_id, &config).await {
            return AgentCommandResult::Error(format!("failed to persist agent: {}", e));
        }

        let recv = match self.processes.remove(&agent_id) {
            Some(old) => {
                let recv = old.recv.clone();
                old.stop(&agent_id).await;
                recv
            }
            None => self.deps.transport.register_agent(agent_id.clone()).await,
        };

        let summary = self.summary(&agent_id, &config).await;
        self.start(agent_id, config, recv, Some(built));
        AgentCommandResult::Ok(summary)
    }

    /// Works for an agent in any state, and for one that exists only in storage.
    async fn handle_delete(
        &mut self,
        agent_id: &str,
        delete_workspace: bool,
    ) -> AgentCommandResult {
        if !self.exists(agent_id).await {
            return AgentCommandResult::Error(format!("agent '{}' not found", agent_id));
        }

        if let Err(e) = self.deps.storage.delete_agent(agent_id).await {
            return AgentCommandResult::Error(format!("failed to delete agent: {}", e));
        }

        self.deps
            .transport
            .unregister_agent(&agent_id.to_string())
            .await;
        if let Some(process) = self.processes.remove(agent_id) {
            process.stop(agent_id).await;
        }
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

/// Invariants an agent config must satisfy before it is persisted.
fn validate_config(config: &AgentConfig) -> Result<(), String> {
    if config.tools.python.code_mode && !config.tools.python.enabled {
        return Err("tools.python.code_mode requires tools.python.enabled".into());
    }
    Ok(())
}
