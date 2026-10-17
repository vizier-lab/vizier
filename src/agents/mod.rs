use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::{oneshot, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::agents::supervisor::{AgentStatus, BuiltAgent, CurrentAgent, SharedStatus, Supervisor};
use crate::constant::CORE_MD;
use crate::dependencies::VizierDependencies;
use crate::schema::{
    AgentCommand, AgentCommandResult, AgentConfig, AgentHealthStatus, AgentId, AgentSummary,
    IntegrationChanged, RevisionOrigin, RevisionTrigger,
};
use crate::storage::agent::AgentStorage;
use crate::storage::integration::IntegrationStorage;
use crate::storage::user::UserStorage;
use crate::transport::VizierRequestEnvelope;
use crate::utils::agent_workspace;

pub mod agent;
pub mod client;
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
    current: CurrentAgent,
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

/// A create or update whose new config is being built, off the command loop.
struct PendingBuild {
    agent_id: AgentId,
    config: AgentConfig,
    kind: BuildKind,
    built: Result<BuiltAgent>,
    resp: oneshot::Sender<AgentCommandResult>,
}

#[derive(Clone, Copy)]
enum BuildKind {
    Create,
    Update,
}

pub struct VizierAgents {
    deps: VizierDependencies,
    processes: HashMap<AgentId, AgentProcess>,
    /// Builds run as their own tasks, so one slow build (an Ollama pull, an embedder) does
    /// not hold up every other agent's commands and health checks (#52).
    builds: JoinSet<PendingBuild>,
    /// Agents with a build in flight. Other creates, updates and deletes of them are
    /// refused until it finishes, so two swaps of one agent never race.
    building: HashSet<AgentId>,
}

impl VizierAgents {
    /// Register and start every stored agent. Returns once all are registered, without
    /// waiting for any to finish starting: requests sent meanwhile queue (#26, #43).
    pub async fn new(deps: VizierDependencies) -> Result<Self> {
        let mut agents = Self {
            deps,
            processes: HashMap::new(),
            builds: JoinSet::new(),
            building: HashSet::new(),
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
        let current = CurrentAgent::default();
        let supervisor = Supervisor {
            current: current.clone(),
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
                current,
                recv,
            },
        );
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut integration_changes = self.deps.transport.subscribe_integration_changes();
        loop {
            tokio::select! {
                cmd = self.deps.transport.recv_agent_command() => self.handle_command(cmd?).await,
                change = integration_changes.recv() => match change {
                    Ok(change) => {
                        for agent_id in &change.agents {
                            self.refresh_integrations(agent_id);
                        }
                    }
                    // Missed some: every running agent re-reads its integrations.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let ids: Vec<AgentId> = self.processes.keys().cloned().collect();
                        for agent_id in &ids {
                            self.refresh_integrations(agent_id);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        integration_changes = self.deps.transport.subscribe_integration_changes();
                    }
                },
                Some(done) = self.builds.join_next(), if !self.builds.is_empty() => match done {
                    Ok(done) => self.finish_build(done).await,
                    // `start_build` catches a panicking build, so only an abort lands here.
                    Err(e) => tracing::error!("agent build task ended: {e}"),
                },
            }
        }
    }

    async fn handle_command(&mut self, cmd: AgentCommand) {
        match cmd {
            AgentCommand::Create {
                agent_id,
                config,
                resp,
            } => self.handle_create(agent_id, config, resp).await,
            AgentCommand::Update {
                agent_id,
                config,
                resp,
            } => self.handle_update(agent_id, config, resp).await,
            AgentCommand::Delete {
                agent_id,
                delete_workspace,
                resp,
            } => {
                let result = self.handle_delete(&agent_id, delete_workspace).await;
                let _ = resp.send(result);
            }
            AgentCommand::Restart { agent_id, resp } => {
                let result = self.handle_restart(agent_id).await;
                let _ = resp.send(result);
            }
            AgentCommand::HealthCheck { resp } => {
                let statuses: Vec<AgentHealthStatus> = self
                    .processes
                    .iter()
                    .map(|(id, process)| {
                        AgentStatus::health(&process.status, &self.deps.channel_board, id)
                    })
                    .collect();
                let _ = resp.send(statuses);
            }
            AgentCommand::RefreshIntegrations { agent_id } => self.refresh_integrations(&agent_id),
            AgentCommand::IntegrationStatus { agent_id, resp } => match self.current(&agent_id) {
                // Off the command loop: it reads storage.
                Some(agent) => {
                    tokio::spawn(async move {
                        let _ = resp.send(agent.integration_status().await);
                    });
                }
                None => {
                    let _ = resp.send(Default::default());
                }
            },
        }
    }

    /// The agent's running instance, if it has one.
    fn current(&self, agent_id: &str) -> Option<std::sync::Arc<agent::VizierAgent>> {
        self.processes
            .get(agent_id)?
            .current
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Rebuild a running agent's integration tools, as its own task so the command loop
    /// never waits on it. An agent that isn't running reads storage when it is next built.
    fn refresh_integrations(&self, agent_id: &str) {
        if let Some(agent) = self.current(agent_id) {
            tokio::spawn(async move { agent.refresh_integrations().await });
        }
    }

    fn busy(agent_id: &str) -> AgentCommandResult {
        AgentCommandResult::Error(format!(
            "agent '{agent_id}' is still being created or updated; try again once that finishes"
        ))
    }

    /// Build the config as its own task; `finish_build` picks up the result.
    fn start_build(
        &mut self,
        agent_id: AgentId,
        config: AgentConfig,
        kind: BuildKind,
        resp: oneshot::Sender<AgentCommandResult>,
    ) {
        self.building.insert(agent_id.clone());
        let deps = self.deps.clone();
        self.builds.spawn(async move {
            // Its own task, so a panicking build is an error for this one command.
            let build = tokio::spawn({
                let (deps, agent_id, config) = (deps.clone(), agent_id.clone(), config.clone());
                async move { BuiltAgent::build(&deps, &agent_id, &config).await }
            });
            let built = match build.await {
                Ok(built) => built,
                Err(e) => Err(anyhow::anyhow!("building the agent panicked: {e}")),
            };
            PendingBuild {
                agent_id,
                config,
                kind,
                built,
                resp,
            }
        });
    }

    async fn finish_build(&mut self, done: PendingBuild) {
        let PendingBuild {
            agent_id,
            config,
            kind,
            built,
            resp,
        } = done;
        self.building.remove(&agent_id);
        let result = match (kind, built) {
            (BuildKind::Create, Err(e)) => {
                AgentCommandResult::Error(format!("failed to start agent: {e:#}"))
            }
            (BuildKind::Update, Err(e)) => AgentCommandResult::Error(format!(
                "the new config failed to start, so the agent keeps its previous one: {e:#}"
            )),
            (BuildKind::Create, Ok(built)) => self.finish_create(agent_id, config, built).await,
            (BuildKind::Update, Ok(built)) => self.finish_update(agent_id, config, built).await,
        };
        let _ = resp.send(result);
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
    async fn handle_create(
        &mut self,
        agent_id: AgentId,
        config: AgentConfig,
        resp: oneshot::Sender<AgentCommandResult>,
    ) {
        if self.building.contains(&agent_id) {
            let _ = resp.send(Self::busy(&agent_id));
            return;
        }
        if self.exists(&agent_id).await {
            let _ = resp.send(AgentCommandResult::Error(format!(
                "agent '{}' already exists",
                agent_id
            )));
            return;
        }
        self.start_build(agent_id, config, BuildKind::Create, resp);
    }

    async fn finish_create(
        &mut self,
        agent_id: AgentId,
        config: AgentConfig,
        built: BuiltAgent,
    ) -> AgentCommandResult {
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
    async fn handle_update(
        &mut self,
        agent_id: AgentId,
        config: AgentConfig,
        resp: oneshot::Sender<AgentCommandResult>,
    ) {
        if self.building.contains(&agent_id) {
            let _ = resp.send(Self::busy(&agent_id));
            return;
        }
        if !self.exists(&agent_id).await {
            let _ = resp.send(AgentCommandResult::Error(format!(
                "agent '{}' not found",
                agent_id
            )));
            return;
        }
        self.start_build(agent_id, config, BuildKind::Update, resp);
    }

    async fn finish_update(
        &mut self,
        agent_id: AgentId,
        config: AgentConfig,
        built: BuiltAgent,
    ) -> AgentCommandResult {
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

    /// The supervisor builds from the stored config; a build that fails is retried with
    /// backoff and shows as `Failed`, like at boot.
    async fn handle_restart(&mut self, agent_id: AgentId) -> AgentCommandResult {
        if self.building.contains(&agent_id) {
            return Self::busy(&agent_id);
        }
        let config = match self.deps.storage.get_agent(&agent_id).await {
            Ok(Some(config)) => config,
            Ok(None) => return AgentCommandResult::Error(format!("agent '{agent_id}' not found")),
            Err(e) => return AgentCommandResult::Error(e.to_string()),
        };
        let recv = match self.processes.remove(&agent_id) {
            Some(old) => {
                let recv = old.recv.clone();
                old.stop(&agent_id).await;
                recv
            }
            None => self.deps.transport.register_agent(agent_id.clone()).await,
        };
        let summary = self.summary(&agent_id, &config).await;
        self.start(agent_id, config, recv, None);
        AgentCommandResult::Ok(summary)
    }

    /// Works for an agent in any state, and for one that exists only in storage.
    async fn handle_delete(
        &mut self,
        agent_id: &str,
        delete_workspace: bool,
    ) -> AgentCommandResult {
        if self.building.contains(agent_id) {
            return Self::busy(agent_id);
        }
        if !self.exists(agent_id).await {
            return AgentCommandResult::Error(format!("agent '{}' not found", agent_id));
        }

        // Read before the delete drops the attachments, so the bots it had can be stopped.
        let attached = self
            .deps
            .storage
            .list_agent_integrations(agent_id)
            .await
            .unwrap_or_default();
        if let Err(e) = self.deps.storage.delete_agent(agent_id).await {
            return AgentCommandResult::Error(format!("failed to delete agent: {}", e));
        }
        for integration in attached {
            self.deps.transport.publish_integration_change(IntegrationChanged {
                integration_id: integration.id,
                family: integration.family,
                agents: vec![agent_id.to_string()],
            });
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
