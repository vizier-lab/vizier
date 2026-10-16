//! Every Discord/Telegram bot the process runs, one supervised reader per enabled, assigned
//! channel connection (research R3). Readers belong to the connection, not to an agent
//! instance: an agent update or restart never touches them, and changing one connection never
//! touches another (contracts/runtime.md G6–G8).
//!
//! `reconcile` compares storage with what is running and starts, stops or restarts the
//! difference. It is level-triggered: it runs at startup, on every channel `IntegrationChanged`
//! and when the event stream lags, and always reads storage rather than the event.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinHandle;

use crate::channels::readers::{HealthSlot, Reader, supervise};
use crate::dependencies::VizierDependencies;
use crate::schema::{
    AgentId, ChannelHealth, ChannelState, IntegrationFamily, IntegrationStatus,
};
use crate::storage::integration::IntegrationStorage;

/// How long a stopping reader may take to close before it is aborted. A Telegram long poll
/// doesn't notice a stop, so it is aborted after this.
const STOP_TIMEOUT: Duration = Duration::from_secs(3);

/// A running connection's agent and reader health.
#[derive(Clone)]
pub struct BoardEntry {
    pub agent_id: AgentId,
    pub health: HealthSlot,
}

/// Each running connection's health by integration id, shared with agent health and
/// integration status.
pub type ChannelBoard = Arc<RwLock<HashMap<String, BoardEntry>>>;

/// The readers running for `agent_id`, as `AgentHealthStatus.channels` reports them.
pub fn health_for(board: &ChannelBoard, agent_id: &str) -> Vec<ChannelHealth> {
    let board = board.read().unwrap_or_else(|p| p.into_inner());
    let mut health: Vec<ChannelHealth> = board
        .values()
        .filter(|entry| entry.agent_id == agent_id)
        .map(|entry| entry.health.read().unwrap_or_else(|p| p.into_inner()).clone())
        .collect();
    health.sort_by_key(|h| h.platform.to_string());
    health
}

/// A running connection's status, or `None` when no reader runs for it.
pub fn status_of(board: &ChannelBoard, integration_id: &str) -> Option<IntegrationStatus> {
    let board = board.read().unwrap_or_else(|p| p.into_inner());
    let health = board
        .get(integration_id)?
        .health
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let last_error = health.last_error.unwrap_or_default();
    Some(match health.state {
        ChannelState::Running => IntegrationStatus::Running { tools: None },
        ChannelState::Reconnecting => IntegrationStatus::Reconnecting {
            last_error,
            next_retry_at: health.next_retry_at,
        },
        ChannelState::Failed => IntegrationStatus::Failed { last_error },
    })
}

/// What a reader runs as: its agent, and its settings as they were when it started.
type Running = (AgentId, String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Start(String),
    Stop(String),
    Restart(String),
}

/// What to do to turn `running` into `desired`, both keyed by integration id. A reader that
/// is unchanged is left alone, even a failed one: it waits for its token to change.
pub fn plan(desired: &HashMap<String, Running>, running: &HashMap<String, Running>) -> Vec<Step> {
    let ids: BTreeSet<&String> = desired.keys().chain(running.keys()).collect();
    ids.into_iter()
        .filter_map(|id| match (desired.get(id), running.get(id)) {
            (Some(_), None) => Some(Step::Start(id.clone())),
            (None, Some(_)) => Some(Step::Stop(id.clone())),
            (Some(want), Some(have)) if want != have => Some(Step::Restart(id.clone())),
            _ => None,
        })
        .collect()
}

struct RunningReader {
    running: Running,
    reader: Reader,
    stopping: Arc<AtomicBool>,
    task: JoinHandle<()>,
}

pub struct ChannelConnections {
    deps: VizierDependencies,
    readers: Mutex<HashMap<String, RunningReader>>,
}

impl ChannelConnections {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps, readers: Mutex::new(HashMap::new()) }
    }

    /// Bring the running readers in line with storage.
    pub async fn reconcile(&self) {
        let integrations = match self.deps.storage.list_integrations().await {
            Ok(integrations) => integrations,
            Err(e) => {
                tracing::warn!("cannot read channel connections, leaving readers as they are: {e}");
                return;
            }
        };
        let kinds = &self.deps.integration_kinds;
        let desired: HashMap<String, (Running, crate::schema::Integration)> = integrations
            .into_iter()
            .filter(|i| i.family == IntegrationFamily::Channel && i.enabled && i.agents.len() == 1)
            .filter(|i| kinds.get(&i.kind).is_some_and(|k| k.missing(&i.settings).is_empty()))
            .map(|i| {
                let running = (i.agents[0].clone(), i.settings.to_string());
                (i.id.clone(), (running, i))
            })
            .collect();

        let mut readers = self.readers.lock().await;
        let have: HashMap<String, Running> =
            readers.iter().map(|(id, r)| (id.clone(), r.running.clone())).collect();
        let want: HashMap<String, Running> =
            desired.iter().map(|(id, (running, _))| (id.clone(), running.clone())).collect();

        for step in plan(&want, &have) {
            let (stop, start) = match &step {
                Step::Start(id) => (None, Some(id)),
                Step::Stop(id) => (Some(id), None),
                Step::Restart(id) => (Some(id), Some(id)),
            };
            if let Some(id) = stop
                && let Some(reader) = readers.remove(id)
            {
                self.board().remove(id);
                stop_reader(reader).await;
            }
            if let Some(id) = start
                && let Some((running, integration)) = desired.get(id)
                && let Some(reader) = self.start(integration, running.clone())
            {
                readers.insert(id.clone(), reader);
            }
        }
    }

    fn board(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<String, BoardEntry>> {
        self.deps.channel_board.write().unwrap_or_else(|p| p.into_inner())
    }

    fn start(&self, integration: &crate::schema::Integration, running: Running) -> Option<RunningReader> {
        let kind = self.deps.integration_kinds.get(&integration.kind)?;
        let agent_id = running.0.clone();
        let (platform, reader) = kind.reader(integration, &agent_id, &self.deps)?;
        tracing::info!(integration = %integration.id, agent_id, %platform, "starting {platform} reader");

        let health: HealthSlot = Arc::new(RwLock::new(ChannelHealth {
            platform,
            state: ChannelState::Running,
            last_error: None,
            failures: 0,
            next_retry_at: None,
        }));
        self.board().insert(
            integration.id.clone(),
            BoardEntry { agent_id: agent_id.clone(), health: health.clone() },
        );
        let stopping = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn(supervise(agent_id, platform, reader.clone(), stopping.clone(), health));
        Some(RunningReader { running, reader, stopping, task })
    }

    /// Reconcile on every channel change until the process stops.
    pub async fn watch(self: Arc<Self>) {
        let mut events = self.deps.transport.subscribe_integration_changes();
        loop {
            match events.recv().await {
                Ok(event) if event.family == IntegrationFamily::Channel => self.reconcile().await,
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => self.reconcile().await,
                Err(RecvError::Closed) => return,
            }
        }
    }

    /// Close every reader, on process stop.
    pub async fn shutdown_all(&self) {
        let mut readers = self.readers.lock().await;
        for (id, reader) in readers.drain() {
            self.board().remove(&id);
            stop_reader(reader).await;
        }
    }
}

async fn stop_reader(reader: RunningReader) {
    reader.stopping.store(true, Ordering::SeqCst);
    if let Err(e) = reader.reader.shutdown().await {
        tracing::warn!("channel reader did not close cleanly: {e:#}");
    }
    let abort = reader.task.abort_handle();
    if tokio::time::timeout(STOP_TIMEOUT, reader.task).await.is_err() {
        abort.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str, &str)]) -> HashMap<String, Running> {
        entries
            .iter()
            .map(|(id, agent, settings)| (id.to_string(), (agent.to_string(), settings.to_string())))
            .collect()
    }

    #[test]
    fn starts_what_is_missing_and_stops_what_is_gone() {
        let steps = plan(&map(&[("a", "dp-a", "t1")]), &map(&[("b", "dp-b", "t2")]));
        assert_eq!(steps, [Step::Start("a".into()), Step::Stop("b".into())]);
    }

    #[test]
    fn restarts_on_a_token_or_agent_change() {
        let running = map(&[("a", "dp-a", "t1"), ("b", "dp-b", "t2")]);
        let desired = map(&[("a", "dp-a", "t1-new"), ("b", "dp-c", "t2")]);
        assert_eq!(plan(&desired, &running), [Step::Restart("a".into()), Step::Restart("b".into())]);
    }

    #[test]
    fn leaves_an_unchanged_reader_alone_even_a_failed_one() {
        let running = map(&[("a", "dp-a", "t1")]);
        assert!(plan(&running.clone(), &running).is_empty());
    }

    #[test]
    fn health_and_status_come_from_the_board() {
        let board = ChannelBoard::default();
        let slot: HealthSlot = Arc::new(RwLock::new(ChannelHealth {
            platform: crate::schema::ChannelPlatform::Telegram,
            state: ChannelState::Failed,
            last_error: Some("Unauthorized".into()),
            failures: 1,
            next_retry_at: None,
        }));
        board.write().unwrap().insert("t".into(), BoardEntry { agent_id: "dp-a".into(), health: slot });
        assert_eq!(health_for(&board, "dp-a").len(), 1);
        assert!(health_for(&board, "dp-b").is_empty());
        assert_eq!(
            status_of(&board, "t"),
            Some(IntegrationStatus::Failed { last_error: "Unauthorized".into() })
        );
        assert_eq!(status_of(&board, "other"), None);
    }
}
