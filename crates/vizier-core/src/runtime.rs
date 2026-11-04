//! One running engine on one workspace (`specs/037-core-extraction/` research R7, R14).
//!
//! [`VizierRuntime::start`] opens storage, runs the startup migrations, registers the agents
//! and spawns the core subsystems the selected [`ServerRoleSet`] owns (agent manager, scheduler,
//! channel readers, the transport's command handler) onto the **caller's** tokio runtime. A
//! subsystem that stops is reported through [`VizierRuntime::wait`], [`VizierRuntime::events`]
//! and [`VizierRuntime::health`]; the runtime never exits the process. What to do about it is
//! the host's call: the server binary exits so it can be restarted whole (#51), an embedding app
//! may show it.

use std::{
    collections::HashMap,
    fs::File,
    future::Future,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use futures::FutureExt;
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinSet,
};

use crate::{
    agents::AgentHost,
    agents::placement::Placement,
    channels::ChannelReaders,
    config::VizierConfig,
    dependencies::VizierDependencies,
    error::VizierError,
    scheduler::VizierScheduler,
    schema::AgentHealthStatus,
    server_role::{ServerRole, ServerRoleSet},
    transport::{SessionEvent, VizierTransport},
};

/// Whether a subsystem may work right now (Fleet, #161/#159).
///
/// The scheduler and the channel readers ask before they do anything, so a node that holds no
/// lease stays idle instead of doing the same work twice. Core ships [`AlwaysYes`], which is the
/// single-node answer; `vizier-fleet` supplies the lease-backed one (#166, #167).
pub trait Gate: Send + Sync {
    /// `false` means "not now": do not run, and try again later.
    fn may_run(&self) -> bool;
}

/// The gate of a node that holds everything by definition: the default, and today's behaviour.
#[derive(Debug, Clone, Copy, Default)]
pub struct AlwaysYes;

impl Gate for AlwaysYes {
    fn may_run(&self) -> bool {
        true
    }
}

/// What [`VizierRuntime::start`] needs.
#[derive(Clone)]
pub struct RuntimeOptions {
    /// Must name its workspace explicitly: the engine never derives where data lives from the
    /// environment or the home directory.
    pub config: VizierConfig,
    /// The subsystems this process runs. Defaults to every role, which is one node doing
    /// everything: the same process, and the same behaviour as before Fleet.
    pub roles: ServerRoleSet,
    /// Asked by the scheduler and the channel readers before they work. `None` is
    /// [`AlwaysYes`]; anything else comes from the host (`vizier-fleet`).
    pub gate: Option<Arc<dyn Gate>>,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            config: VizierConfig::default(),
            roles: ServerRoleSet::all(),
            gate: None,
        }
    }
}

impl std::fmt::Debug for RuntimeOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeOptions")
            .field("config", &self.config)
            .field("roles", &self.roles)
            .field(
                "gate",
                &self.gate.as_ref().map_or("always-yes", |_| "from the host"),
            )
            .finish()
    }
}

/// The runtime's lifecycle, broadcast to every [`VizierRuntime::events`] receiver.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeEvent {
    /// Every core subsystem has been spawned. Each receiver gets it first.
    Started,
    /// A subsystem returned, failed or panicked. It is not restarted.
    SubsystemStopped { name: &'static str, outcome: String },
    /// An exit was requested over the transport (e.g. `vizier shutdown`).
    ExitRequested,
}

/// A receiver of [`RuntimeEvent`]s, from [`VizierRuntime::events`].
pub struct RuntimeEvents {
    started: bool,
    rx: broadcast::Receiver<RuntimeEvent>,
}

impl RuntimeEvents {
    /// The next event; `Err` once the runtime is gone, or `Lagged` if events were missed.
    pub async fn recv(&mut self) -> Result<RuntimeEvent, broadcast::error::RecvError> {
        if std::mem::take(&mut self.started) {
            return Ok(RuntimeEvent::Started);
        }
        self.rx.recv().await
    }
}

/// Why [`VizierRuntime::wait`] returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeExit {
    Requested,
    SubsystemStopped { name: &'static str, outcome: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubsystemState {
    Running,
    Stopped { outcome: String },
}

/// A snapshot of the runtime's subsystems and of every agent's health.
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// The roles this process runs, in [`ServerRoleSet`] order.
    pub roles: ServerRoleSet,
    pub subsystems: Vec<(&'static str, SubsystemState)>,
    /// Empty when the agent manager doesn't answer.
    pub agents: Vec<AgentHealthStatus>,
}

/// A running engine. Dropping it aborts its subsystems, like [`VizierRuntime::shutdown`]
/// minus the wait, and releases the workspace.
pub struct VizierRuntime {
    deps: VizierDependencies,
    roles: ServerRoleSet,
    subsystems: Subsystems,
    readers: Option<Arc<ChannelReaders>>,
    events: broadcast::Sender<RuntimeEvent>,
    _lock: WorkspaceLock,
}

impl VizierRuntime {
    /// Starts the engine on `options.config.workspace`. Must be called inside a tokio
    /// multi-thread runtime. Fails if the workspace is empty or already in use by another
    /// vizier process, or if storage can't be opened.
    ///
    /// Only the subsystems the selected roles own are spawned. Storage, migrations and the
    /// transport are opened either way — every role needs them — so a role this process does not
    /// run simply has no subsystems in [`VizierRuntime::health`].
    pub async fn start(options: RuntimeOptions) -> crate::Result<Self> {
        let RuntimeOptions { config, roles, gate } = options;
        if config.workspace.trim().is_empty() {
            return Err(VizierError(
                "the runtime needs an explicit workspace (config.workspace is empty)".into(),
            ));
        }
        let lock = WorkspaceLock::acquire(Path::new(&config.workspace))?;
        let deps = VizierDependencies::new(config).await.map_err(err)?;
        let gate: Arc<dyn Gate> = gate.unwrap_or_else(|| Arc::new(AlwaysYes));

        let (events, _) = broadcast::channel(64);
        let mut subsystems = Subsystems::reporting(events.clone());

        let mut readers = None;
        if roles.contains(ServerRole::Worker) {
            // Placement decides which agents the local host holds; the host runs only those
            // (`specs/048-agent-service-seam` contracts/agent-host-placement.md). In a single
            // node that is every stored agent, so nothing changes.
            let placement = crate::agents::placement::SingleNodePlacement::new(deps.storage.clone());
            let held = placement.assign().await.map_err(|e| VizierError(e.0))?;
            // Agents first: the host registers every agent's request channel, so the
            // scheduler's immediate first tick queues a due task instead of losing it (#26).
            let mut agents = AgentHost::new(
                deps.clone(),
                held.into_iter().map(|placed| placed.agent_id).collect(),
            )
            .await
            .map_err(err)?;
            // After the agents re-sent their pending turns, so a recovered job finds them
            // running.
            deps.background_jobs.recover().await;
            subsystems.spawn("agent manager", async move { agents.run().await });
        }

        if roles.contains(ServerRole::Control) {
            let mut scheduler = VizierScheduler::new(deps.clone(), gate.clone())
                .await
                .map_err(err)?;
            subsystems.spawn("scheduler", async move { scheduler.run().await });
        }

        if roles.contains(ServerRole::Channels) {
            let running = Arc::new(ChannelReaders::new(deps.clone(), gate));
            let task = running.clone();
            subsystems.spawn("channel readers", async move { task.run().await });
            readers = Some(running);
        }

        // The command handler is what `vizier shutdown`, `vizier agent ps` and the rest of the
        // command socket answer through, so it runs whatever else this process does.
        let transport = deps.clone();
        subsystems.spawn("command handler", async move { transport.run().await });

        Ok(Self {
            deps,
            roles,
            subsystems,
            readers,
            events,
            _lock: lock,
        })
    }

    /// The roles this process runs.
    pub fn roles(&self) -> ServerRoleSet {
        self.roles
    }

    /// Where requests and agent commands go.
    pub fn transport(&self) -> &VizierTransport {
        &self.deps.transport
    }

    /// The engine's internals, for the HTTP adapter and the server binary until application
    /// services replace them. Not part of the API.
    #[doc(hidden)]
    pub fn deps(&self) -> &VizierDependencies {
        &self.deps
    }

    /// The application services: every operation a person can ask of the engine.
    pub fn services(&self) -> crate::service::Services {
        crate::service::Services::new(self.deps.clone())
    }

    /// Lifecycle events from now on, after a [`RuntimeEvent::Started`] (which happened
    /// before any receiver could exist).
    pub fn events(&self) -> RuntimeEvents {
        RuntimeEvents {
            started: true,
            rx: self.events.subscribe(),
        }
    }

    /// The frames pushed to whoever watches a session (background jobs, reactions, the turns
    /// of every WebUI conversation).
    pub fn session_events(&self) -> broadcast::Receiver<SessionEvent> {
        self.deps.transport.subscribe_session_events()
    }

    pub async fn health(&self) -> HealthReport {
        HealthReport {
            roles: self.roles,
            subsystems: self.subsystems.states(),
            agents: self.deps.transport.agent_health().await.unwrap_or_default(),
        }
    }

    /// Resolves when an exit is requested over the transport or a subsystem stops; each is
    /// also broadcast as an event. Cancel-safe.
    pub async fn wait(&mut self) -> RuntimeExit {
        let exit_signal = self.deps.transport.exit_signal();
        tokio::select! {
            _ = exit_signal => {
                let _ = self.events.send(RuntimeEvent::ExitRequested);
                RuntimeExit::Requested
            }
            (name, outcome) = self.subsystems.next_stopped() => {
                RuntimeExit::SubsystemStopped { name, outcome }
            }
        }
    }

    /// Stops every subsystem the way a requested exit does: running turns are aborted and
    /// resume on the next start (`specs/023-persist-turn-activity`). Releases the workspace.
    pub async fn shutdown(mut self) -> crate::Result<()> {
        self.subsystems.abort_all();
        if let Some(readers) = &self.readers {
            readers.shutdown().await;
        }
        Ok(())
    }

    /// Ends the named subsystem as if it had returned, so tests can watch the runtime report
    /// a stop. Not part of the API.
    #[doc(hidden)]
    pub fn stop_subsystem_for_test(&self, name: &'static str) -> bool {
        self.subsystems.stop_for_test(name)
    }
}

impl Drop for VizierRuntime {
    fn drop(&mut self) {
        self.subsystems.abort_all();
    }
}

fn err(e: anyhow::Error) -> VizierError {
    VizierError(format!("{e:#}"))
}

/// Long-lived tasks by name, each of which reports how it stopped. Shared with the server
/// binary, which runs the HTTP and command servers as its own subsystems next to the runtime.
#[doc(hidden)]
pub struct Subsystems {
    set: JoinSet<()>,
    states: Arc<Mutex<Vec<(&'static str, SubsystemState)>>>,
    stops: Arc<Mutex<HashMap<&'static str, tokio::sync::oneshot::Sender<()>>>>,
    stopped_tx: mpsc::UnboundedSender<(&'static str, String)>,
    stopped_rx: mpsc::UnboundedReceiver<(&'static str, String)>,
    events: Option<broadcast::Sender<RuntimeEvent>>,
}

impl Default for Subsystems {
    fn default() -> Self {
        let (stopped_tx, stopped_rx) = mpsc::unbounded_channel();
        Self {
            set: JoinSet::new(),
            states: Default::default(),
            stops: Default::default(),
            stopped_tx,
            stopped_rx,
            events: None,
        }
    }
}

impl Subsystems {
    /// Subsystems whose stops are also broadcast as [`RuntimeEvent::SubsystemStopped`].
    fn reporting(events: broadcast::Sender<RuntimeEvent>) -> Self {
        Self {
            events: Some(events),
            ..Default::default()
        }
    }

    pub fn spawn<E: std::fmt::Display>(
        &mut self,
        name: &'static str,
        task: impl Future<Output = Result<(), E>> + Send + 'static,
    ) {
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        if let Ok(mut stops) = self.stops.lock() {
            stops.insert(name, stop_tx);
        }
        if let Ok(mut states) = self.states.lock() {
            states.push((name, SubsystemState::Running));
        }
        let states = self.states.clone();
        let stopped = self.stopped_tx.clone();
        let events = self.events.clone();
        self.set.spawn(async move {
            let run = AssertUnwindSafe(task).catch_unwind();
            let outcome = tokio::select! {
                result = run => match result {
                    Ok(Ok(())) => "it returned".to_string(),
                    Ok(Err(e)) => format!("{e:#}"),
                    Err(_) => "it panicked".to_string(),
                },
                Ok(()) = stop_rx => "it returned".to_string(),
            };
            if let Ok(mut states) = states.lock()
                && let Some(entry) = states.iter_mut().find(|(n, _)| *n == name)
            {
                entry.1 = SubsystemState::Stopped {
                    outcome: outcome.clone(),
                };
            }
            if let Some(events) = &events {
                let _ = events.send(RuntimeEvent::SubsystemStopped {
                    name,
                    outcome: outcome.clone(),
                });
            }
            let _ = stopped.send((name, outcome));
        });
    }

    /// The next subsystem to stop, and how. Cancel-safe; pends forever if none ever stops.
    pub async fn next_stopped(&mut self) -> (&'static str, String) {
        match self.stopped_rx.recv().await {
            Some(stopped) => stopped,
            None => std::future::pending().await,
        }
    }

    pub fn states(&self) -> Vec<(&'static str, SubsystemState)> {
        self.states.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn abort_all(&mut self) {
        self.set.abort_all();
    }

    fn stop_for_test(&self, name: &'static str) -> bool {
        let stop = self.stops.lock().ok().and_then(|mut s| s.remove(name));
        stop.is_some_and(|tx| tx.send(()).is_ok())
    }
}

/// An exclusive lock on `{workspace}/.runtime/vizier.lock`, held for the runtime's life: two
/// engines on one workspace would race on the same database and documents.
pub(crate) struct WorkspaceLock(#[allow(dead_code)] File);

impl WorkspaceLock {
    pub(crate) fn acquire(workspace: &Path) -> crate::Result<Self> {
        let dir: PathBuf = workspace.join(".runtime");
        std::fs::create_dir_all(&dir).map_err(|e| {
            VizierError(format!("cannot create {}: {e}", dir.display()))
        })?;
        let path = dir.join("vizier.lock");
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| VizierError(format!("cannot open {}: {e}", path.display())))?;
        match file.try_lock() {
            Ok(()) => Ok(Self(file)),
            Err(std::fs::TryLockError::WouldBlock) => Err(VizierError(format!(
                "workspace {} is already in use by another vizier process",
                workspace.display()
            ))),
            Err(std::fs::TryLockError::Error(e)) => {
                Err(VizierError(format!("cannot lock {}: {e}", path.display())))
            }
        }
    }
}
