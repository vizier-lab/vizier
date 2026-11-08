use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};
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
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub mod agent;
pub mod background;
pub mod client;
pub mod hook;
pub mod mcp;
pub mod memory_ops;
pub mod placement;
pub mod views;
pub mod process;
pub mod shell;
pub mod skill;
pub mod steps;
pub mod supervisor;
pub mod tools;

/// How long an update waits for the old agent to stop before aborting it.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the host refreshes the published health views. A view is good for ten
/// seconds, so it is refreshed well inside that.
const VIEW_REFRESH: Duration = Duration::from_secs(5);

/// Where a command came from: the fleet-wide subject, or one agent's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scoped {
    /// `agents.cmd`: the fleet-wide commands, the create among them.
    Fleet,
    /// `agent.{id}.cmd`: a command about one agent, which this host must hold.
    Agent,
}

/// One agent's supervisor. Every agent in storage has one, whether it is running, starting
/// or failed, so update and delete always find it (#42).
struct AgentProcess {
    handle: JoinHandle<()>,
    shutdown: watch::Sender<bool>,
    /// Set before a stop that is a delete: a replacement instance (update, restart) resumes
    /// the turns this one was running, a delete does not.
    deleting: Arc<AtomicBool>,
    status: SharedStatus,
    current: CurrentAgent,
    /// The agent's request channel, registered once and handed from one supervisor to the
    /// next across updates, so nothing sent during a swap is lost (#43).
    recv: crate::transport::Inbox,
}

impl AgentProcess {
    /// Ask the supervisor to stop, and wait for it to, up to `STOP_TIMEOUT`.
    async fn stop(self, agent_id: &str) {
        let _ = self.shutdown.send(true);
        let abort = self.handle.abort_handle();
        if tokio::time::timeout(STOP_TIMEOUT, self.handle)
            .await
            .is_err()
        {
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

/// The agent host: it builds and runs **only** the agents it was told to hold.
///
/// Which agents those are is placement's decision, not the host's
/// (`specs/048-agent-service-seam` contracts/agent-host-placement.md). The host keeps the
/// supervisors, the running instances, the builds and the resumption of pending turns.
pub struct AgentHost {
    deps: VizierDependencies,
    /// Commands from `agents.cmd`, subscribed before this struct exists so none is missed.
    commands: crate::transport::AgentCommands,
    processes: HashMap<AgentId, AgentProcess>,
    /// Builds run as their own tasks, so one slow build (an Ollama pull, an embedder) does
    /// not hold up every other agent's commands and health checks (#52).
    builds: JoinSet<PendingBuild>,
    /// Agents with a build in flight. Other creates, updates and deletes of them are
    /// refused until it finishes, so two swaps of one agent never race.
    building: HashSet<AgentId>,
    /// The agents this host holds, and the subscription that carries their commands. A
    /// command reaches exactly one handler because only the owner subscribes to
    /// `agent.{id}.cmd` (`specs/048-agent-service-seam` contracts/agents-client.md, rule 2).
    scoped: HashMap<AgentId, JoinHandle<()>>,
    /// Where the scoped subscriptions hand the commands they read.
    scoped_tx: mpsc::UnboundedSender<AgentCommand>,
    /// The commands the scoped subscriptions read, handled on the command loop like the
    /// fleet-wide ones.
    scoped_rx: mpsc::UnboundedReceiver<AgentCommand>,
}

impl AgentHost {
    /// Start the host with the agents it is told to hold.
    ///
    /// The host never reads the agent list itself: `held` is placement's answer. Each
    /// agent's *configuration* is read from storage, which is stored state and not the
    /// running agent's state.
    pub async fn new(deps: VizierDependencies, held: Vec<AgentId>) -> Result<Self> {
        let commands = deps.transport.serve_agent_commands().await?;
        let (scoped_tx, scoped_rx) = mpsc::unbounded_channel();
        let mut agents = Self {
            deps,
            commands,
            processes: HashMap::new(),
            builds: JoinSet::new(),
            building: HashSet::new(),
            scoped: HashMap::new(),
            scoped_tx,
            scoped_rx,
        };

        for agent_id in held {
            let Some(config) = agents.deps.storage.get_agent(&agent_id).await? else {
                // Placed but not stored: nothing to run.
                tracing::warn!(agent_id = %agent_id, "the placement named an agent that is not stored");
                continue;
            };
            agents.take(agent_id, config).await?;
        }

        // A Workspace mention saved just before the last stop, and never sent, is sent now. Every
        // held agent's request channel is registered by this point, so it queues like any other
        // (spec 053 R4).
        let hub = crate::workspaces::Hub::from(&agents.deps);
        tokio::spawn(async move { crate::workspaces::redispatch_pending(&hub).await });

        Ok(agents)
    }

    /// Take an agent on: subscribe to its subjects, register its request channel, re-send
    /// the turns that were pending, and start it.
    pub async fn take(&mut self, agent_id: AgentId, config: AgentConfig) -> Result<()> {
        tracing::info!("starting agent: {}", agent_id);
        let recv = self.deps.transport.register_agent(agent_id.clone()).await;
        self.subscribe_scoped(&agent_id).await?;
        self.resend_pending_turns(&agent_id).await;
        self.start(agent_id.clone(), config, recv, None);
        self.publish_health(&agent_id);
        Ok(())
    }

    /// Publish an agent's health view, so a gateway can read it without asking the owner.
    fn publish_health(&self, agent_id: &AgentId) {
        let Some((id, process)) = self.processes.get_key_value(agent_id) else {
            return;
        };
        let status = AgentStatus::health(
            &process.status,
            &self.deps.channel_board,
            &self.deps.sso_failures,
            id,
        );
        crate::agents::views::publish_health(&self.deps, id, &status);
    }

    /// Let an agent go. The running turns end the way a delete ends them.
    pub async fn release(&mut self, agent_id: &AgentId) -> Result<()> {
        self.release_scoped(agent_id);
        self.deps.transport.unregister_agent(agent_id).await;
        if let Some(process) = self.processes.remove(agent_id) {
            process.deleting.store(true, Ordering::SeqCst);
            process.stop(agent_id).await;
        }
        Ok(())
    }

    /// Subscribe to one agent's commands, so the seam can reach it.
    ///
    /// Only the agent's owner subscribes to its subject, so a command reaches exactly one
    /// handler: the bus delivers at most once, so a command a non-owner consumed would be
    /// lost. An agent this host does not hold has no subscription, so a command for it is
    /// answered by nobody - the *no owner* answer, at once.
    async fn subscribe_scoped(&mut self, agent_id: &AgentId) -> Result<()> {
        if self.scoped.contains_key(agent_id) {
            return Ok(());
        }
        let mut commands = self.deps.transport.serve_agent_commands_for(agent_id).await?;
        let tx = self.scoped_tx.clone();
        let handle = tokio::spawn(async move {
            while let Some(cmd) = commands.recv().await {
                if tx.send(cmd).is_err() {
                    break;
                }
            }
        });
        self.scoped.insert(agent_id.clone(), handle);
        Ok(())
    }

    /// Stop answering commands for an agent this host no longer holds.
    fn release_scoped(&mut self, agent_id: &AgentId) {
        if let Some(handle) = self.scoped.remove(agent_id) {
            handle.abort();
        }
    }

    /// Requests accepted before vizier went down and not finished are sent again, in the
    /// order they were accepted: a turn that had begun resumes, the rest run in turn
    /// (`specs/023-persist-turn-activity`). Their replies are published on the session
    /// events, where an open WebUI topic or a channel reader picks them up.
    async fn resend_pending_turns(&self, agent_id: &AgentId) {
        use crate::storage::turn::{TurnState, TurnStorage};
        let pending = match self.deps.storage.pending_turns(agent_id).await {
            Ok(pending) => pending,
            Err(e) => {
                tracing::error!(agent_id = %agent_id, "could not read pending turns: {e}");
                return;
            }
        };
        if pending.is_empty() {
            return;
        }
        tracing::info!(agent_id = %agent_id, count = pending.len(), "re-sending turns that were pending at shutdown");
        for turn in pending {
            let envelope = crate::transport::VizierRequestEnvelope {
                response_tx: self.deps.transport.broadcast_sender(turn.session.clone()).await.ok(),
                session: turn.session,
                request: turn.request,
                turn_id: Some(turn.id),
                resumed: turn.state == TurnState::Running,
            };
            if let Err(e) = self.deps.transport.resend(envelope).await {
                tracing::warn!(agent_id = %agent_id, "could not re-send a pending turn: {e}");
            }
        }
    }

    /// Spawn a supervisor for the agent on an already-registered request channel.
    fn start(
        &mut self,
        agent_id: AgentId,
        config: AgentConfig,
        recv: crate::transport::Inbox,
        prebuilt: Option<BuiltAgent>,
    ) {
        let (shutdown, shutdown_rx) = watch::channel(false);
        let status = SharedStatus::default();
        let current = CurrentAgent::default();
        let deleting = Arc::new(AtomicBool::new(false));
        let supervisor = Supervisor {
            deleting: deleting.clone(),
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
                deleting,
                status,
                current,
                recv,
            },
        );
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut integration_changes = self.deps.transport.subscribe_integration_changes();
        // A published view is good for ten seconds, so the host refreshes them before that
        // (contracts/shared-state-views.md, rule 3). Without the refresh an idle agent's
        // view would expire and the WebUI would show it as unknown.
        let mut refresh = tokio::time::interval(VIEW_REFRESH);
        refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        refresh.tick().await;
        loop {
            tokio::select! {
                _ = refresh.tick() => {
                    let ids: Vec<AgentId> = self.processes.keys().cloned().collect();
                    for agent_id in &ids {
                        self.publish_health(agent_id);
                    }
                }
                cmd = self.commands.recv() => match cmd {
                    Some(cmd) => self.handle_command(cmd, Scoped::Fleet).await,
                    None => return Err(anyhow::anyhow!("the agent command channel closed")),
                },
                // The commands of the agents this host holds, read on `agent.{id}.cmd`.
                cmd = self.scoped_rx.recv() => match cmd {
                    Some(cmd) => self.handle_command(cmd, Scoped::Agent).await,
                    None => return Err(anyhow::anyhow!("the agent command channel closed")),
                },
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

    async fn handle_command(&mut self, cmd: AgentCommand, scoped: Scoped) {
        if scoped == Scoped::Agent {
            // A command about one agent reaches only the host that holds it. An agent this
            // host does not hold is not its to act on: the slot is dropped, so the caller
            // reads the *owner lost* failure rather than a wrong answer.
            let agent_id = cmd.agent_id().to_string();
            if !self.processes.contains_key(&agent_id) {
                tracing::warn!(agent_id, "refusing a command for an agent this host does not hold");
                return;
            }
        }
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
                        AgentStatus::health(
                            &process.status,
                            &self.deps.channel_board,
                            &self.deps.sso_failures,
                            id,
                        )
                    })
                    .collect();
                let _ = resp.send(statuses);
            }
            AgentCommand::AgentHealth { agent_id, resp } => {
                // One agent's health. An agent this host does not hold has no answer: the
                // slot is dropped and the caller reads the *owner lost* failure.
                let status = self.processes.get(&agent_id).map(|process| {
                    AgentStatus::health(
                        &process.status,
                        &self.deps.channel_board,
                        &self.deps.sso_failures,
                        &agent_id,
                    )
                });
                if let Some(status) = status {
                    let _ = resp.send(status);
                }
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

        // A new agent's identity is seeded as the memory document `core/soul` when it starts
        // (specs/043-unify-core-and-memory). Writing the legacy `agent_core` row here would
        // give every new agent a "carried over from the identity record" provenance line
        // describing a record that never held anything of its own.

        // No addon is seeded here: a new agent starts with no integrations and the Office
        // addon is installed from its Addons page like any other (specs/036-office-addon R12).

        let recv = self.deps.transport.register_agent(agent_id.clone()).await;
        // The new agent is held from here on, so its own command subject is answered by
        // this host.
        if let Err(e) = self.subscribe_scoped(&agent_id).await {
            return AgentCommandResult::Error(format!(
                "failed to answer commands for agent '{}': {e}",
                agent_id
            ));
        }
        let summary = self.summary(&agent_id, &config).await;
        self.start(agent_id.clone(), config, recv, Some(built));
        // Reads the integrations left behind by an earlier agent of the same id: the process
        // did not exist yet when any such change was announced (specs/036-office-addon R13).
        self.refresh_integrations(&agent_id);
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
        // Collect the transient files before the storage delete drops their rows, so their
        // blobs can be removed afterwards (`specs/035-add-artifacts` R9).
        let files = self
            .deps
            .session_files
            .files_for_agent(agent_id)
            .await
            .unwrap_or_default();
        // Language servers rooted in this agent's projects stop before their rows go.
        for project in crate::storage::project::ProjectStorage::list_projects(
            &*self.deps.storage,
            agent_id,
            true,
        )
            .await
            .unwrap_or_default()
        {
            self.deps.lsp.close_project(&project.id).await;
        }
        if let Err(e) = self.deps.storage.delete_agent(agent_id).await {
            return AgentCommandResult::Error(format!("failed to delete agent: {}", e));
        }
        self.deps.session_files.delete_blobs(&files).await;
        self.deps.artifacts.delete_agent_files(agent_id).await;
        for integration in attached {
            self.deps
                .transport
                .publish_integration_change(IntegrationChanged {
                    integration_id: integration.id,
                    family: integration.family,
                    agents: vec![agent_id.to_string()],
                });
        }

        self.deps
            .transport
            .unregister_agent(&agent_id.to_string())
            .await;
        // The agent is gone, so nothing answers its command subject any more.
        self.release_scoped(&agent_id.to_string());
        if let Some(process) = self.processes.remove(agent_id) {
            process.deleting.store(true, Ordering::SeqCst);
            process.stop(agent_id).await;
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
            avatar_url: None,
            owner_username: None,
            owner_id: None,
            shared_to: Vec::new(),
        })
    }
}
