//! Language servers for project chats (`specs/054-project-lsp`).
//!
//! [`LspHost`] owns every running server of the instance, keyed `(project, root, language)`. A
//! server starts the first time something needs it, in a spawned task, and a caller never waits
//! for it (FR-017). Everything about running servers is in memory.

pub mod announce;
pub mod diagnostics;
pub mod languages;
pub mod position;
pub mod query;
pub mod rpc;
pub mod search_path;
pub mod server;
pub mod sync;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use self::languages::LanguageDef;
use self::server::{Server, ServerSpec, StartError};
use crate::schema::{LspCapacity, LspServerStatus, ServerOverride};

/// A crash is retried this many times inside [`RESTART_WINDOW`], then the server reads `failed`.
const MAX_RESTARTS: usize = 3;
const RESTART_WINDOW: Duration = Duration::from_secs(600);

/// Everything needed to find or start the server for one file.
#[derive(Debug, Clone)]
pub struct Target {
    pub agent_id: String,
    pub project_id: String,
    /// The project's main folder; the status shows roots relative to it.
    pub folder: PathBuf,
    /// Where the server runs (absolute).
    pub root: PathBuf,
    pub language: &'static LanguageDef,
    pub server_override: Option<ServerOverride>,
}

impl Target {
    fn key(&self) -> Key {
        (self.project_id.clone(), self.root.clone(), self.language.id)
    }

    fn command(&self) -> &str {
        match &self.server_override {
            Some(o) => &o.command,
            None => self.language.command,
        }
    }
}

type Key = (String, PathBuf, &'static str);

/// The server target for a file in a project chat, or `None` when the switch is off, the file is
/// not inside the chat's folder, or no language owns it.
pub fn target_for(
    agent_id: &str,
    project: &crate::projects::context::ProjectTurnContext,
    file: &Path,
) -> Option<Target> {
    if !project.lsp_enabled || !file.starts_with(&project.cwd) {
        return None;
    }
    let language = languages::language_for(file)?;
    Some(Target {
        agent_id: agent_id.to_owned(),
        project_id: project.project_id.clone(),
        folder: project.folder.clone(),
        root: languages::find_root(file, &project.cwd, language.root),
        language,
        server_override: project.lsp_overrides.get(language.id).cloned(),
    })
}

/// What a caller learns about the server for a file. Never waits.
#[derive(Clone)]
pub enum Availability {
    Ready(Arc<Server>),
    Starting { detail: Option<String> },
    Missing { command: String, install_hint: String, reason: String },
    Failed { reason: String },
    /// The project's switch went off, or the project is gone.
    Closed,
    /// The instance is at its server limit and every running server has a question in flight.
    Busy,
}

impl std::fmt::Debug for Availability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Availability::Ready(_) => write!(f, "Ready"),
            Availability::Starting { detail } => write!(f, "Starting({detail:?})"),
            Availability::Missing { command, .. } => write!(f, "Missing({command})"),
            Availability::Failed { reason } => write!(f, "Failed({reason})"),
            Availability::Closed => write!(f, "Closed"),
            Availability::Busy => write!(f, "Busy"),
        }
    }
}

enum Phase {
    Starting,
    Running(Arc<Server>),
    Missing(String),
    Failed(String),
}

struct Entry {
    epoch: u64,
    phase: Phase,
    target: Target,
    restarts: VecDeque<Instant>,
}

#[derive(Default)]
struct State {
    entries: HashMap<Key, Entry>,
    /// Projects whose switch is off or that are gone (R10): a start for one is refused, so a racing
    /// start cannot leave a server behind.
    closed: HashSet<String>,
    next_gen: u64,
}

/// Called with the host, the agent id and the project id whenever a server of that project
/// changes state or progress. The caller throttles.
type Listener = announce::Listener;

/// A server unused this long stops (FR-019).
const IDLE_AFTER: Duration = Duration::from_secs(600);
const SWEEP_EVERY: Duration = Duration::from_secs(30);

struct Inner {
    max_servers: usize,
    idle_after: Duration,
    sweep_every: Duration,
    sweeper_started: std::sync::atomic::AtomicBool,
    state: Mutex<State>,
    listener: Mutex<Option<Listener>>,
}

/// The instance's language servers. Cheap to clone.
#[derive(Clone)]
pub struct LspHost {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for LspHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspHost").field("max_servers", &self.inner.max_servers).finish()
    }
}

impl Default for LspHost {
    fn default() -> Self {
        Self::new(crate::config::LspConfig::default().max_servers)
    }
}

impl LspHost {
    pub fn new(max_servers: usize) -> Self {
        Self::with_timing(max_servers, IDLE_AFTER, SWEEP_EVERY)
    }

    /// [`LspHost::new`] with the idle limit and sweep interval chosen, for tests.
    pub fn with_timing(max_servers: usize, idle_after: Duration, sweep_every: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                max_servers: max_servers.max(1),
                idle_after,
                sweep_every,
                sweeper_started: std::sync::atomic::AtomicBool::new(false),
                state: Mutex::new(State::default()),
                listener: Mutex::new(None),
            }),
        }
    }

    pub fn max_servers(&self) -> usize {
        self.inner.max_servers
    }

    pub fn set_listener(&self, listener: Listener) {
        *self.inner.listener.lock() = Some(listener);
    }

    fn announce(inner: &Arc<Inner>, agent_id: &str, project_id: &str) {
        let listener = inner.listener.lock().clone();
        if let Some(listener) = listener {
            listener(&LspHost { inner: inner.clone() }, agent_id, project_id);
        }
    }

    /// The server for a target. A server that is not running yet is started in the background and
    /// the answer is [`Availability::Starting`].
    pub fn ensure(&self, target: &Target) -> Availability {
        let key = target.key();
        let mut state = self.inner.state.lock();
        if state.closed.contains(&target.project_id) {
            return Availability::Closed;
        }
        if let Some(entry) = state.entries.get(&key) {
            if let Phase::Running(server) = &entry.phase {
                server.touch();
            }
            return Self::availability(entry);
        }
        // At the limit: stop the least recently used server that is not mid-question.
        let in_use = state
            .entries
            .values()
            .filter(|e| matches!(e.phase, Phase::Starting | Phase::Running(_)))
            .count();
        let mut evicted: Option<(String, String, Arc<Server>)> = None;
        if in_use >= self.inner.max_servers {
            let victim = state
                .entries
                .iter()
                .filter_map(|(k, e)| match &e.phase {
                    Phase::Running(server) if server.in_flight() == 0 => {
                        Some((k.clone(), server.idle_for()))
                    }
                    _ => None,
                })
                .max_by_key(|(_, idle)| *idle)
                .map(|(k, _)| k);
            let Some(victim) = victim else {
                return Availability::Busy;
            };
            if let Some(entry) = state.entries.remove(&victim)
                && let Phase::Running(server) = entry.phase
            {
                evicted = Some((entry.target.agent_id.clone(), victim.0.clone(), server));
            }
        }
        state.next_gen += 1;
        let epoch = state.next_gen;
        state.entries.insert(
            key.clone(),
            Entry {
                epoch,
                phase: Phase::Starting,
                target: target.clone(),
                restarts: VecDeque::new(),
            },
        );
        drop(state);
        if let Some((agent_id, project_id, server)) = evicted {
            tokio::spawn(async move { server.stop().await });
            Self::announce(&self.inner, &agent_id, &project_id);
        }
        self.start_sweeper();
        Self::spawn_start(self.inner.clone(), key, epoch);
        Self::announce(&self.inner, &target.agent_id, &target.project_id);
        Availability::Starting { detail: None }
    }

    /// One task per host, started with the first server, that stops what has been idle too long.
    fn start_sweeper(&self) {
        use std::sync::atomic::Ordering;
        if self.inner.sweeper_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(&self.inner);
        let every = self.inner.sweep_every;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(every).await;
                let Some(inner) = weak.upgrade() else { return };
                let host = LspHost { inner };
                let idle: Vec<Key> = {
                    let state = host.inner.state.lock();
                    state
                        .entries
                        .iter()
                        .filter_map(|(k, e)| match &e.phase {
                            Phase::Running(server)
                                if server.in_flight() == 0
                                    && server.idle_for() >= host.inner.idle_after =>
                            {
                                Some(k.clone())
                            }
                            _ => None,
                        })
                        .collect()
                };
                for key in idle {
                    host.remove_where(&key.0.clone(), |k, _| *k == key).await;
                }
            }
        });
    }

    /// What is known about a target, starting nothing.
    pub fn peek(&self, target: &Target) -> Option<Availability> {
        let state = self.inner.state.lock();
        if state.closed.contains(&target.project_id) {
            return Some(Availability::Closed);
        }
        state.entries.get(&target.key()).map(Self::availability)
    }

    fn availability(entry: &Entry) -> Availability {
        match &entry.phase {
            Phase::Starting => Availability::Starting { detail: None },
            Phase::Running(server) => {
                if server.is_ready() {
                    Availability::Ready(server.clone())
                } else {
                    Availability::Starting {
                        detail: server.busy_detail(),
                    }
                }
            }
            Phase::Missing(reason) => Availability::Missing {
                command: entry.target.command().to_owned(),
                install_hint: entry.target.language.install_hint.to_owned(),
                reason: reason.clone(),
            },
            Phase::Failed(reason) => Availability::Failed {
                reason: reason.clone(),
            },
        }
    }

    fn spawn_start(inner: Arc<Inner>, key: Key, epoch: u64) {
        tokio::spawn(async move {
            let target = {
                let state = inner.state.lock();
                match state.entries.get(&key) {
                    Some(e) if e.epoch == epoch => e.target.clone(),
                    _ => return,
                }
            };
            let outcome = Self::launch(&inner, &target).await;
            let project_id = target.project_id.clone();
            let agent_id = target.agent_id.clone();
            match outcome {
                Ok(server) => {
                    let keep = {
                        let mut state = inner.state.lock();
                        let closed = state.closed.contains(&project_id);
                        match state.entries.get_mut(&key) {
                            Some(e) if e.epoch == epoch && !closed => {
                                e.phase = Phase::Running(server.clone());
                                true
                            }
                            _ => false,
                        }
                    };
                    if !keep {
                        server.stop().await;
                        return;
                    }
                    Self::announce(&inner, &agent_id, &project_id);
                    Self::watch(inner, key, epoch, server);
                }
                Err(error) => {
                    let phase = match error {
                        StartError::Missing(reason) => Phase::Missing(reason),
                        StartError::Failed(reason) => Phase::Failed(reason),
                    };
                    {
                        let mut state = inner.state.lock();
                        match state.entries.get_mut(&key) {
                            Some(e) if e.epoch == epoch => e.phase = phase,
                            _ => return,
                        }
                    }
                    Self::announce(&inner, &agent_id, &project_id);
                }
            }
        });
    }

    async fn launch(inner: &Arc<Inner>, target: &Target) -> Result<Arc<Server>, StartError> {
        let found = search_path::resolve(target.command()).await.ok_or_else(|| {
            StartError::Missing(format!("{} was not found", target.command()))
        })?;
        let (args, env) = match &target.server_override {
            Some(o) => (
                o.args.clone(),
                o.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            ),
            None => (
                target.language.args.iter().map(|a| a.to_string()).collect(),
                HashMap::new(),
            ),
        };
        let spec = ServerSpec {
            root: target.root.clone(),
            program: found.program,
            args,
            env: search_path::filtered_env(std::env::vars(), found.path.as_deref(), &env),
            initialization_options: target
                .language
                .initialization_options
                .and_then(|text| serde_json::from_str(text).ok()),
        };
        let project_id = target.project_id.clone();
        let agent_id = target.agent_id.clone();
        let listener_inner = inner.clone();
        Server::start(
            spec,
            Some(Arc::new(move || {
                Self::announce(&listener_inner, &agent_id, &project_id)
            })),
        )
        .await
    }

    /// Restart a crashed server, up to the limit, and note a requested stop.
    fn watch(inner: Arc<Inner>, key: Key, epoch: u64, server: Arc<Server>) {
        tokio::spawn(async move {
            let mut rx = server.subscribe_exit();
            let info = match rx.wait_for(|e| e.is_some()).await {
                Ok(info) => info.clone(),
                Err(_) => return,
            };
            let Some(info) = info else { return };
            if info.requested {
                return;
            }
            let project_id = key.0.clone();
            let agent_id = inner
                .state
                .lock()
                .entries
                .get(&key)
                .map(|e| e.target.agent_id.clone())
                .unwrap_or_default();
            let retry = {
                let mut state = inner.state.lock();
                let closed = state.closed.contains(&project_id);
                match state.entries.get_mut(&key) {
                    Some(e) if e.epoch == epoch && !closed => {
                        let now = Instant::now();
                        e.restarts.retain(|t| now.duration_since(*t) < RESTART_WINDOW);
                        e.restarts.push_back(now);
                        if e.restarts.len() > MAX_RESTARTS {
                            let reason = info.stderr.trim();
                            e.phase = Phase::Failed(if reason.is_empty() {
                                match info.code {
                                    Some(code) => format!("the server exited with code {code}"),
                                    None => "the server stopped".into(),
                                }
                            } else {
                                reason.to_owned()
                            });
                            Some(false)
                        } else {
                            e.phase = Phase::Starting;
                            Some(true)
                        }
                    }
                    _ => None,
                }
            };
            Self::announce(&inner, &agent_id, &project_id);
            if retry == Some(true) {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Self::spawn_start(inner, key, epoch);
            }
        });
    }

    /// The status the strip and popup show for one project.
    pub fn status(&self, project_id: &str) -> Vec<LspServerStatus> {
        let state = self.inner.state.lock();
        let mut out: Vec<LspServerStatus> = state
            .entries
            .iter()
            .filter(|((p, _, _), _)| p == project_id)
            .map(|((_, root, _), entry)| {
                let root = position::relative(&entry.target.folder, root)
                    .unwrap_or_else(|| root.to_string_lossy().into_owned());
                let mut status = LspServerStatus {
                    language: entry.target.language.id.to_owned(),
                    name: entry.target.language.name.to_owned(),
                    root,
                    state: String::new(),
                    detail: None,
                    reason: None,
                    install_hint: None,
                    overridden: entry.target.server_override.is_some(),
                };
                match Self::availability(entry) {
                    Availability::Ready(_) => status.state = "ready".into(),
                    Availability::Starting { detail } => {
                        status.state = "starting".into();
                        status.detail = detail;
                    }
                    Availability::Missing { install_hint, reason, .. } => {
                        status.state = "missing".into();
                        status.reason = Some(reason);
                        status.install_hint = Some(install_hint);
                    }
                    Availability::Failed { reason } => {
                        status.state = "failed".into();
                        status.reason = Some(reason);
                    }
                    Availability::Closed | Availability::Busy => status.state = "failed".into(),
                }
                status
            })
            .collect();
        out.sort_by(|a, b| (&a.name, &a.root).cmp(&(&b.name, &b.root)));
        out
    }

    /// The running servers of a project that are ready, with their roots.
    pub fn ready_servers(&self, project_id: &str) -> Vec<(PathBuf, Arc<Server>)> {
        let state = self.inner.state.lock();
        state
            .entries
            .iter()
            .filter(|((p, _, _), _)| p == project_id)
            .filter_map(|((_, root, _), entry)| match &entry.phase {
                Phase::Running(server) if server.is_ready() => Some((root.clone(), server.clone())),
                _ => None,
            })
            .collect()
    }

    /// Tell the ready servers under `cwd` about files a command or a branch switch changed, in the
    /// background. A server that would have to be told of more than 5,000 changes restarts.
    pub fn notify_changes(&self, project_id: &str, cwd: &Path) {
        let servers: Vec<(Key, Arc<Server>)> = {
            let state = self.inner.state.lock();
            state
                .entries
                .iter()
                .filter(|((p, root, _), _)| p == project_id && root.starts_with(cwd))
                .filter_map(|(k, e)| match &e.phase {
                    Phase::Running(server) if server.is_ready() => Some((k.clone(), server.clone())),
                    _ => None,
                })
                .collect()
        };
        for ((project_id, root, language), server) in servers {
            let host = self.clone();
            tokio::spawn(async move {
                if let sync::Changes::TooMany(n) = server.notify_changes(&root).await {
                    tracing::info!(changes = n, "too many changes for a language server; restarting it");
                    host.restart(&project_id, language, Some(&root)).await;
                }
            });
        }
    }

    /// Servers holding a process, instance-wide. `missing` and `failed` hold none.
    pub fn capacity(&self) -> LspCapacity {
        let state = self.inner.state.lock();
        LspCapacity {
            in_use: state
                .entries
                .values()
                .filter(|e| matches!(e.phase, Phase::Starting | Phase::Running(_)))
                .count(),
            max: self.inner.max_servers,
        }
    }

    /// Remove the entries a filter picks and stop their processes. Returns how many went.
    async fn remove_where(&self, project_id: &str, pick: impl Fn(&Key, &Entry) -> bool) -> usize {
        let removed: Vec<Entry> = {
            let mut state = self.inner.state.lock();
            let keys: Vec<Key> = state
                .entries
                .iter()
                .filter(|(k, e)| k.0 == project_id && pick(k, e))
                .map(|(k, _)| k.clone())
                .collect();
            keys.iter().filter_map(|k| state.entries.remove(k)).collect()
        };
        let count = removed.len();
        let agent_id = removed.first().map(|e| e.target.agent_id.clone());
        let stops = removed.into_iter().filter_map(|e| match e.phase {
            Phase::Running(server) => Some(async move { server.stop().await }),
            _ => None,
        });
        futures::future::join_all(stops).await;
        if let Some(agent_id) = agent_id {
            Self::announce(&self.inner, &agent_id, project_id);
        }
        count
    }

    /// Stop every server of a project.
    pub async fn stop_project(&self, project_id: &str) -> usize {
        self.remove_where(project_id, |_, _| true).await
    }

    /// Switch off or remove: stop everything and refuse starts until [`LspHost::open_project`].
    pub async fn close_project(&self, project_id: &str) {
        self.inner.state.lock().closed.insert(project_id.to_owned());
        self.stop_project(project_id).await;
    }

    /// Switch on: allow starts again.
    pub fn open_project(&self, project_id: &str) {
        self.inner.state.lock().closed.remove(project_id);
    }

    pub fn is_closed(&self, project_id: &str) -> bool {
        self.inner.state.lock().closed.contains(project_id)
    }

    /// Stop the servers whose root is `path` or inside it (a deleted worktree).
    pub async fn stop_under(&self, project_id: &str, path: &Path) -> usize {
        self.remove_where(project_id, |k, _| k.1.starts_with(path)).await
    }

    /// Stop one language's servers in a project (an override changed).
    pub async fn stop_language(&self, project_id: &str, language: &str) -> usize {
        self.remove_where(project_id, |k, _| k.2 == language).await
    }

    /// Forget and stop a language's servers, optionally at one root only. The next use starts
    /// them again, clearing the restart count and any `failed` or `missing` entry. Returns the
    /// targets that were listed, so the caller can start them again.
    pub async fn restart(
        &self,
        project_id: &str,
        language: &str,
        root: Option<&Path>,
    ) -> Vec<Target> {
        let targets: Vec<Target> = {
            let state = self.inner.state.lock();
            state
                .entries
                .iter()
                .filter(|(k, _)| {
                    k.0 == project_id && k.2 == language && root.is_none_or(|r| k.1 == r)
                })
                .map(|(_, e)| e.target.clone())
                .collect()
        };
        for target in &targets {
            let key = target.key();
            self.remove_where(project_id, |k, _| *k == key).await;
        }
        for target in &targets {
            self.ensure(target);
        }
        targets
    }

    /// Runtime shutdown.
    pub async fn stop_all(&self) {
        let projects: Vec<String> = {
            let state = self.inner.state.lock();
            let mut ids: Vec<String> = state.entries.keys().map(|k| k.0.clone()).collect();
            ids.sort();
            ids.dedup();
            ids
        };
        futures::future::join_all(projects.iter().map(|p| self.stop_project(p))).await;
    }
}
