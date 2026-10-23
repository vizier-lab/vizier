//! Keeps one agent running (#42, #43).
//!
//! The manager registers an agent's request channel before anything about the agent is
//! built, and hands the receiving end to a supervisor that keeps it across restarts: requests
//! sent while the agent is starting or restarting wait in the channel instead of failing with
//! "not registered". The supervisor builds the agent, runs it, and when either step fails
//! (an error, or a panic in the request loop) records why and tries again with backoff,
//! answering requests with an error while it waits.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::agents::agent::VizierAgent;
use crate::agents::memory_ops;
use crate::agents::process::agent_process;
use crate::channels::connections::{ChannelBoard, health_for};
use crate::config::provider::ProviderId;
use crate::dependencies::VizierDependencies;
use crate::embedding::VizierEmbedder;
use crate::indexer::VizierIndexer;
use crate::indexer::sqlite::SqliteIndexer;
use crate::schema::{
    AgentConfig, AgentHealthStatus, AgentId, AgentRunState, ErrorKind,
    VizierResponse, VizierResponseContent,
};
use crate::storage::memory::MemoryStorage;
use crate::storage::provider::ProviderStorage;
use crate::transport::VizierRequestEnvelope;

const RESTART_BASE: Duration = Duration::from_secs(2);
const RESTART_MAX: Duration = Duration::from_secs(300);
/// An agent (or a channel reader) that ran this long before failing starts its backoff over.
pub(crate) const HEALTHY_RUN: Duration = Duration::from_secs(60);

/// How long to wait before the next restart, after `failures` failures in a row.
pub(crate) fn restart_delay(failures: u32) -> Duration {
    RESTART_BASE
        .saturating_mul(1u32 << failures.saturating_sub(1).min(16))
        .min(RESTART_MAX)
}

/// What it takes to run an agent, built before anything already running is stopped.
pub struct BuiltAgent {
    agent: Arc<VizierAgent>,
    indexer: Option<VizierIndexer>,
}

impl BuiltAgent {
    /// Everything that can fail about starting an agent: pulling an Ollama model, building
    /// the embedder and indexer, and the agent itself (provider, tools, skills).
    ///
    /// Wraps [`Self::build_quietly`] so the agent's setup shows up in the WebUI from the
    /// moment a build starts — a boot build or a form submit. The guard reports a failure
    /// if this future is dropped or panics, so the WebUI never keeps a stale "building".
    pub async fn build(
        deps: &VizierDependencies,
        agent_id: &AgentId,
        config: &AgentConfig,
    ) -> Result<Self> {
        let guard = deps
            .setup
            .build_started(
                agent_id,
                config.owner_id.clone(),
                format!("Building agent '{}'", config.name),
            );
        let result = Self::build_quietly(deps, agent_id, config).await;
        match &result {
            Ok(_) => guard.finished(),
            Err(e) => guard.failed(format!("{e:#}")),
        }
        result
    }

    /// The build without a setup entry of its own, for the supervisor's retries: an agent
    /// that keeps failing would otherwise flash "Building agent" at every backoff. Any model
    /// download it starts still reports itself.
    pub async fn build_quietly(
        deps: &VizierDependencies,
        agent_id: &AgentId,
        config: &AgentConfig,
    ) -> Result<Self> {
        if config.provider == ProviderId::OLLAMA {
            // The `ollama` built-in's base URL: saved, `OLLAMA_BASE_URL`, or the default.
            let resolved = crate::llm::resolve::resolve(
                deps,
                &ProviderId::OLLAMA,
                &config.model,
                &Default::default(),
            )
            .await?;
            crate::utils::ollama::ollama_pull_model(
                &resolved.base_url,
                &config.model,
                &deps.setup,
                agent_id,
                crate::setup::DownloadKind::Model,
            )
            .await?;
        }

        let indexer = build_indexer(deps, agent_id, config).await?;
        let agent = Arc::new(VizierAgent::new(agent_id.clone(), deps, config, indexer.clone()).await?);
        Ok(Self { agent, indexer })
    }
}

async fn build_indexer(
    deps: &VizierDependencies,
    agent_id: &AgentId,
    config: &AgentConfig,
) -> Result<Option<VizierIndexer>> {
    let (emb_settings, _idx_cfg) = match (&config.embedding, &config.indexer) {
        (Some(e), Some(i)) => (e, i),
        _ => return Ok(None),
    };

    let conn = deps.sqlite_conn.clone();

    let embedder = Arc::new(
        VizierEmbedder::from_agent_settings(emb_settings, deps, &deps.config.workspace, agent_id)
            .await?,
    );
    let sqlite_idx = SqliteIndexer::new(conn, embedder).await?;
    Ok(Some(VizierIndexer::build(sqlite_idx)))
}

/// The supervisor's view of its agent, read by `HealthCheck`.
#[derive(Debug, Clone, Default)]
pub struct AgentStatus {
    pub state: AgentRunState,
    pub last_error: Option<String>,
    pub failures: u32,
    pub next_retry_at: Option<chrono::DateTime<Utc>>,
    pub model: Option<crate::schema::AgentModelInfo>,
    pub warnings: Vec<String>,
}

pub type SharedStatus = Arc<RwLock<AgentStatus>>;

/// The agent instance a supervisor is running right now, if any: what an integration refresh
/// or status query reaches without going through the request queue.
pub type CurrentAgent = Arc<RwLock<Option<Arc<VizierAgent>>>>;

impl AgentStatus {
    /// `channels` are the readers of the connections attached to the agent, which run
    /// whatever state the agent is in (research R3).
    pub fn health(
        status: &SharedStatus,
        board: &ChannelBoard,
        agent_id: &AgentId,
    ) -> AgentHealthStatus {
        let status = status.read().unwrap_or_else(|p| p.into_inner()).clone();
        AgentHealthStatus {
            channels: health_for(board, agent_id),
            agent_id: agent_id.clone(),
            alive: status.state == AgentRunState::Running,
            state: status.state,
            last_error: status.last_error,
            failures: status.failures,
            next_retry_at: status.next_retry_at,
            model: status.model,
            warnings: status.warnings,
        }
    }
}

fn set_status(status: &SharedStatus, f: impl FnOnce(&mut AgentStatus)) {
    f(&mut status.write().unwrap_or_else(|p| p.into_inner()));
}

pub struct Supervisor {
    pub agent_id: AgentId,
    pub deps: VizierDependencies,
    pub config: AgentConfig,
    pub recv: flume::Receiver<VizierRequestEnvelope>,
    pub shutdown_rx: watch::Receiver<bool>,
    /// True when the stop in progress is a delete (see `AgentProcess::deleting`).
    pub deleting: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub status: SharedStatus,
    pub current: CurrentAgent,
}

impl Supervisor {
    /// Run the agent until shut down, restarting it whenever building or running it fails.
    /// `prebuilt` is used for the first run, so an update can build before it swaps.
    pub async fn run(mut self, mut prebuilt: Option<BuiltAgent>) {
        let mut failures: u32 = 0;
        loop {
            let built = match prebuilt.take() {
                Some(built) => Ok(built),
                None => {
                    set_status(&self.status, |s| s.state = AgentRunState::Starting);
                    tokio::select! {
                        built = async {
                            if failures == 0 {
                                BuiltAgent::build(&self.deps, &self.agent_id, &self.config).await
                            } else {
                                BuiltAgent::build_quietly(&self.deps, &self.agent_id, &self.config)
                                    .await
                            }
                        } => built,
                        _ = wait_for_shutdown(&mut self.shutdown_rx) => break,
                    }
                }
            };

            let error = match built {
                Ok(built) => {
                    if failures > 0 {
                        tracing::info!(agent_id = %self.agent_id, "agent recovered");
                    }
                    let info = built.agent.model_info();
                    let model = Some(crate::schema::AgentModelInfo {
                        provider: info.provider.clone(),
                        model: info.model.clone(),
                        context_window: info.context_window,
                        max_output: info.max_output,
                        context_source: info.context_source,
                    });
                    let warnings = built.agent.health_warnings();
                    set_status(&self.status, |s| {
                        *s = AgentStatus {
                            state: AgentRunState::Running,
                            failures: s.failures,
                            model,
                            warnings,
                            ..Default::default()
                        }
                    });
                    let started = Instant::now();
                    match self.run_once(built).await {
                        Ok(()) => break,
                        Err(e) => {
                            if started.elapsed() >= HEALTHY_RUN {
                                failures = 0;
                            }
                            format!("agent stopped unexpectedly: {e}")
                        }
                    }
                }
                Err(e) => format!("agent failed to start: {e:#}"),
            };

            failures += 1;
            let wait = restart_delay(failures);
            tracing::error!(
                agent_id = %self.agent_id,
                failures,
                retry_in_secs = wait.as_secs(),
                "{error}"
            );
            set_status(&self.status, |s| {
                *s = AgentStatus {
                    state: AgentRunState::Failed,
                    last_error: Some(error.clone()),
                    failures,
                    next_retry_at: chrono::Duration::from_std(wait)
                        .ok()
                        .map(|w| Utc::now() + w),
                    ..Default::default()
                }
            });

            if !self.wait_to_retry(wait, &error).await {
                break;
            }
        }
        set_status(&self.status, |s| s.state = AgentRunState::Stopped);
    }

    /// Serve requests with a built agent until it stops. `Ok` is a requested stop (shutdown,
    /// or the request channel closed by a delete); `Err` is anything else, a panic included.
    async fn run_once(&mut self, built: BuiltAgent) -> Result<()> {
        let BuiltAgent { agent, indexer } = built;
        let memory_ops = self.start_memory_ops(indexer.as_ref()).await;

        *self.current.write().unwrap_or_else(|p| p.into_inner()) = Some(agent.clone());
        // An integration saved while the agent was being built may not be in what it read;
        // one refresh catches it up, reusing everything that didn't change.
        tokio::spawn({
            let agent = agent.clone();
            async move { agent.refresh_integrations().await }
        });

        // Its own task, so a panic in the request loop ends this run instead of the supervisor.
        // Aborted with the supervisor (a stop that timed out), rather than detached (#52).
        let mut process = AbortOnDrop(tokio::spawn(agent_process(
            agent,
            self.agent_id.clone(),
            self.deps.clone(),
            self.config.clone(),
            indexer,
            self.recv.clone(),
            self.shutdown_rx.clone(),
            self.deleting.clone(),
        )));
        let result = (&mut process.0).await;
        *self.current.write().unwrap_or_else(|p| p.into_inner()) = None;

        if let Some(handle) = memory_ops {
            handle.abort();
            self.deps.transport.unregister_memory_op(&self.agent_id).await;
        }

        match result {
            Ok(Ok(())) if self.is_shut_down() || self.recv.is_disconnected() => Ok(()),
            Ok(Ok(())) => Err(anyhow::anyhow!("request loop ended")),
            Ok(Err(e)) => Err(e),
            Err(e) if e.is_panic() => Err(anyhow::anyhow!("request loop panicked")),
            Err(e) => Err(e.into()),
        }
    }

    /// The memory-op channel and handler, plus the one-time passage conversion, for an agent
    /// with an indexer.
    async fn start_memory_ops(
        &self,
        indexer: Option<&VizierIndexer>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        let idx = indexer?.clone();
        let agent_id = self.agent_id.clone();

        // One-time passage conversion for this agent's existing corpus (FR-038).
        //
        // It lives here, and not in `dependencies.rs` where every other migration lives,
        // because **the indexer does not exist there**: it is built per agent from that
        // agent's own embedding config, which is why the two memory migrations in
        // `dependencies.rs` pass `NoopIndexer` and leave semantic search to catch up (research
        // Decision 8). It must also use *this* agent's indexer rather than any shared one,
        // since two agents configured with different embedding models write into one
        // fixed-dimension vector table.
        //
        // Spawned detached rather than awaited, so the agent serves requests throughout
        // (FR-040). It holds neither the agent's request channel nor the sqlite write lock for
        // the duration: `reconcile_all_passages` takes the connection lock per statement and
        // yields between bundles.
        {
            let storage = self.deps.storage.clone();
            let agent_id = agent_id.clone();
            let limits = self.config.chunking.clone();
            let idx = idx.clone();
            tokio::spawn(async move {
                let started = std::time::Instant::now();
                match storage.reconcile_agent_passages(&agent_id, &limits, &idx).await {
                    // FR-042: counts processed, counts failed, and the reason for each failure —
                    // the reasons are logged per document by the reconcile itself. Silent when
                    // there was nothing to do, which is every start after the first.
                    Ok((0, 0)) => {}
                    Ok((converted, failed)) => tracing::info!(
                        agent_id,
                        converted,
                        failed,
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "memory passage conversion finished"
                    ),
                    Err(e) => tracing::warn!(
                        agent_id,
                        "memory passage conversion failed; memories stay readable and are \
                         retried on the next start: {e}"
                    ),
                }
            });
        }

        let rx = self.deps.transport.register_memory_op(agent_id.clone()).await;
        let storage = (*self.deps.storage).clone();
        let settings = memory_ops::MemoryOpSettings {
            chunking: self.config.chunking.clone(),
            per_document: self.config.auto_context.per_document,
        };
        Some(tokio::spawn(async move {
            if let Err(e) = memory_ops::handle_memory_ops(rx, idx, agent_id, storage, settings).await
            {
                tracing::error!("memory_ops handler exited with error: {}", e);
            }
        }))
    }

    /// Wait out the backoff, answering each request that arrives with `error` rather than
    /// leaving it queued behind an agent that may stay down. `false` when the supervisor
    /// should stop instead of retrying.
    async fn wait_to_retry(&mut self, wait: Duration, error: &str) -> bool {
        let deadline = tokio::time::sleep(wait);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => return true,
                _ = wait_for_shutdown(&mut self.shutdown_rx) => return false,
                envelope = self.recv.recv_async() => match envelope {
                    Ok(envelope) => reject(envelope, error).await,
                    Err(_) => return false,
                },
            }
        }
    }

    fn is_shut_down(&self) -> bool {
        *self.shutdown_rx.borrow() || self.shutdown_rx.has_changed().is_err()
    }
}

/// A task that is aborted when its handle is dropped, instead of carrying on detached.
struct AbortOnDrop<T>(tokio::task::JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Resolves once a stop is requested, or the sender is gone (nobody can request one).
async fn wait_for_shutdown(shutdown_rx: &mut watch::Receiver<bool>) {
    let _ = shutdown_rx.wait_for(|stop| *stop).await;
}

async fn reject(envelope: VizierRequestEnvelope, error: &str) {
    tracing::debug!(agent_id = %envelope.session.0, "rejecting request while the agent is down");
    if let Some(tx) = envelope.response_tx {
        let _ = tx
            .send_async(VizierResponse {
                timestamp: Utc::now(),
                content: VizierResponseContent::Error {
                    kind: ErrorKind::AgentUnavailable,
                    message: format!(
                        "{}. It is restarted automatically; try again later.",
                        error.trim_end_matches('.')
                    ),
                },
                ..Default::default()
            })
            .await;
    }
}
