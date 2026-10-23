use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use crate::storage::turn::TurnStorage;
use crate::schema::{
    AgentCommand, AgentHealthStatus, AgentId, BackgroundJobSnapshot, CommandRequest, CommandResponse, FileCommand, IntegrationChanged,
    MemoryOpEnvelope, ReactionEntry, VizierAttachment, VizierRequest, VizierResponse, VizierSession,
};

/// How many session events a slow subscriber may fall behind before it starts losing them.
/// Every WebUI turn's frames go through here, so it is sized for several busy turns at once.
/// A lagging WebSocket logs and carries on; the WebUI reconciles by re-reading the job list,
/// and history once the turn ends.
const SESSION_EVENTS_CAPACITY: usize = 1024;
/// Integration changes are rare; a subscriber that still falls behind reconciles everything.
const INTEGRATION_EVENTS_CAPACITY: usize = 64;
/// How long a health check waits for the agent manager.
const HEALTH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Something pushed to whoever is watching a session, outside of a turn they started
/// themselves: a background job changing state, or the frames of a turn a background report
/// woke. WebUI sockets subscribe, each filtering on its own session, and Discord/Telegram
/// readers subscribe for the woken turns of their own channels (`channels::woken`); a session
/// nobody can watch (task, dream, inter-agent, subagent) has its events dropped.
#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub session: VizierSession,
    pub frame: SessionFrame,
}

#[derive(Debug, Clone)]
pub enum SessionFrame {
    Response(VizierResponse),
    Job(BackgroundJobSnapshot),
    /// The full current reaction set of one message, after any change to it.
    Reactions {
        history_uid: String,
        reactions: Vec<ReactionEntry>,
    },
    /// The display copy of a client tool call (`specs/019-client-tools`), for every socket on
    /// the session but `connection_id`, which got the call itself directly.
    ClientToolCall {
        connection_id: String,
        call: crate::agents::client::ClientCallFrame,
    },
    /// How a client tool call ended.
    ClientCallOutcome {
        call_id: String,
        outcome: crate::schema::ClientCallOutcome,
    },
    /// A deferrable tool's call was deferred: any socket open on the session may deliver it.
    ClientCallDeferred { call_id: String },
    /// One step of a running turn, saved as it happened (`specs/023-persist-turn-activity`),
    /// in the shape `GET …/history` returns so a client can merge it by uid.
    HistoryEntry(crate::schema::SessionHistory),
    /// A client tool call's newly saved state, echoed to the saver as its confirmation.
    ClientCallState {
        call_id: String,
        state: serde_json::Value,
    },
}

#[derive(Debug, Clone)]
pub struct DreamCommand {
    pub agent_id: AgentId,
    pub cycle_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VizierRequestEnvelope {
    pub session: VizierSession,
    pub request: VizierRequest,
    #[serde(skip)]
    pub response_tx: Option<flume::Sender<VizierResponse>>,
    /// The `session_turn` row journaling this request, when it is durable.
    #[serde(skip)]
    pub turn_id: Option<i64>,
    /// Re-sent after a restart with its turn already begun: the agent resumes it.
    #[serde(skip)]
    pub resumed: bool,
}

/// `VizierStorage` has no `Debug`; this lets the transport keep deriving it.
#[derive(Clone)]
struct JournalHandle(crate::storage::VizierStorage);

impl std::fmt::Debug for JournalHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JournalHandle")
    }
}

impl std::ops::Deref for JournalHandle {
    type Target = crate::storage::VizierStorage;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct VizierTransport {
    agent_channels: Arc<RwLock<HashMap<AgentId, flume::Sender<VizierRequestEnvelope>>>>,

    memory_op_channels: Arc<RwLock<HashMap<AgentId, flume::Sender<MemoryOpEnvelope>>>>,

    command_request_channel: Arc<(
        flume::Sender<CommandRequest>,
        flume::Receiver<CommandRequest>,
    )>,

    command_response_channel: Arc<(
        flume::Sender<CommandResponse>,
        flume::Receiver<CommandResponse>,
    )>,

    agent_command_channel: Arc<(flume::Sender<AgentCommand>, flume::Receiver<AgentCommand>)>,

    exit_channel: Arc<(flume::Sender<bool>, flume::Receiver<bool>)>,

    dream_command_channel: Arc<(flume::Sender<DreamCommand>, flume::Receiver<DreamCommand>)>,

    file_command_channel: Arc<(flume::Sender<FileCommand>, flume::Receiver<FileCommand>)>,

    session_events: tokio::sync::broadcast::Sender<SessionEvent>,

    integration_events: tokio::sync::broadcast::Sender<IntegrationChanged>,

    /// Where durable requests are journaled (`specs/023-persist-turn-activity`). `None` for
    /// a transport nothing restarts around, such as the tests'.
    journal: Option<JournalHandle>,
}

impl VizierTransport {
    /// A transport that journals durable requests in `storage`.
    pub fn durable(storage: crate::storage::VizierStorage) -> Self {
        Self { journal: Some(JournalHandle(storage)), ..Self::new() }
    }

    pub fn new() -> Self {
        let command_request_channel = Arc::new(flume::unbounded());
        let command_response_channel = Arc::new(flume::unbounded());
        let agent_command_channel = Arc::new(flume::unbounded());
        let exit_channel = Arc::new(flume::unbounded());
        let dream_command_channel = Arc::new(flume::unbounded());
        let file_command_channel = Arc::new(flume::unbounded());

        Self {
            agent_channels: Arc::new(RwLock::new(HashMap::new())),
            memory_op_channels: Arc::new(RwLock::new(HashMap::new())),
            command_request_channel,
            command_response_channel,
            agent_command_channel,
            exit_channel,
            dream_command_channel,
            file_command_channel,
            session_events: tokio::sync::broadcast::channel(SESSION_EVENTS_CAPACITY).0,
            integration_events: tokio::sync::broadcast::channel(INTEGRATION_EVENTS_CAPACITY).0,
            journal: None,
        }
    }

    /// Publish to every subscriber of the session-event broadcast. Having no subscribers is
    /// the normal case for most sessions, not an error.
    pub fn publish_session_event(&self, ev: SessionEvent) {
        let _ = self.session_events.send(ev);
    }

    pub fn subscribe_session_events(&self) -> tokio::sync::broadcast::Receiver<SessionEvent> {
        self.session_events.subscribe()
    }

    /// Announce a committed integration write (contracts/runtime.md). Subscribers re-read
    /// storage, so a lost event costs nothing once the next one, or a `Lagged`, arrives.
    pub fn publish_integration_change(&self, ev: IntegrationChanged) {
        let _ = self.integration_events.send(ev);
    }

    pub fn subscribe_integration_changes(
        &self,
    ) -> tokio::sync::broadcast::Receiver<IntegrationChanged> {
        self.integration_events.subscribe()
    }

    pub async fn is_agent_registered(&self, agent_id: &AgentId) -> bool {
        self.agent_channels.read().await.contains_key(agent_id)
    }

    pub async fn register_agent(
        &self,
        agent_id: AgentId,
    ) -> flume::Receiver<VizierRequestEnvelope> {
        let (tx, rx) = flume::unbounded();
        let mut channels = self.agent_channels.write().await;
        channels.insert(agent_id, tx);
        rx
    }

    pub async fn unregister_agent(&self, agent_id: &AgentId) {
        let mut channels = self.agent_channels.write().await;
        channels.remove(agent_id);
    }

    pub async fn register_memory_op(&self, agent_id: AgentId) -> flume::Receiver<MemoryOpEnvelope> {
        let (tx, rx) = flume::unbounded();
        let mut channels = self.memory_op_channels.write().await;
        channels.insert(agent_id, tx);
        rx
    }

    pub async fn unregister_memory_op(&self, agent_id: &AgentId) {
        let mut channels = self.memory_op_channels.write().await;
        channels.remove(agent_id);
    }

    pub async fn send_memory_op(
        &self,
        agent_id: &AgentId,
        op: crate::schema::MemoryOpRequest,
    ) -> Result<crate::schema::MemoryOpResponse> {
        let channels = self.memory_op_channels.read().await;
        let tx = channels
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("agent '{}' has no memory-op channel", agent_id))?
            .clone();

        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        tx.send_async(MemoryOpEnvelope { op, response: resp_tx })
            .await?;
        match resp_rx.await? {
            Ok(resp) => Ok(resp),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }
    }

    pub async fn send_request(
        &self,
        session: VizierSession,
        req: VizierRequest,
        response_tx: Option<flume::Sender<VizierResponse>>,
    ) -> Result<()> {
        let mut req = req;
        req.trace.capture();

        // Journaled before it is sent, so a restart between "accepted" and "answered" finds
        // it. A failure to journal costs the durability of this one request, nothing else.
        let mut turn_id = None;
        if let Some(journal) = &self.journal
            && req.is_durable(&session)
        {
            match journal.accept_turn(&session, &req).await {
                Ok(id) => turn_id = Some(id),
                Err(e) => tracing::warn!(agent_id = %session.0, "could not journal a request: {e}"),
            }
        }

        let sent = self
            .resend(VizierRequestEnvelope { session, request: req, response_tx, turn_id, resumed: false })
            .await;
        if sent.is_err()
            && let (Some(journal), Some(id)) = (&self.journal, turn_id)
        {
            let _ = journal.finish_turn(id).await;
        }
        sent
    }

    /// Sends an envelope as it is, without journaling it again: a request recovered at
    /// startup, or handed on to the next instance of an agent.
    pub async fn resend(&self, envelope: VizierRequestEnvelope) -> Result<()> {
        let channels = self.agent_channels.read().await;
        let tx = channels
            .get(&envelope.session.0)
            .ok_or_else(|| anyhow::anyhow!("agent '{}' not registered", envelope.session.0))?;
        tx.send_async(envelope).await?;
        Ok(())
    }

    /// A response sender that republishes every frame onto the session-event broadcast, for a
    /// turn nobody is waiting on (a woken report, a recovered request).
    pub fn broadcast_sender(&self, session: VizierSession) -> flume::Sender<VizierResponse> {
        let (tx, rx) = flume::unbounded::<VizierResponse>();
        let transport = self.clone();
        tokio::spawn(async move {
            while let Ok(frame) = rx.recv_async().await {
                transport.publish_session_event(SessionEvent {
                    session: session.clone(),
                    frame: SessionFrame::Response(frame),
                });
            }
        });
        tx
    }

    pub async fn send_command_request(&self, req: CommandRequest) -> Result<()> {
        Ok(self.command_request_channel.0.send_async(req).await?)
    }

    pub async fn recv_command_request(&self) -> Result<CommandRequest> {
        Ok(self.command_request_channel.1.recv_async().await?)
    }

    pub async fn send_command_response(&self, req: CommandResponse) -> Result<()> {
        Ok(self.command_response_channel.0.send_async(req).await?)
    }

    pub async fn recv_command_response(&self) -> Result<CommandResponse> {
        Ok(self.command_response_channel.1.recv_async().await?)
    }

    pub async fn send_agent_command(&self, cmd: AgentCommand) -> Result<()> {
        Ok(self.agent_command_channel.0.send_async(cmd).await?)
    }

    pub async fn recv_agent_command(&self) -> Result<AgentCommand> {
        Ok(self.agent_command_channel.1.recv_async().await?)
    }

    pub async fn send_dream_command(&self, cmd: DreamCommand) -> Result<()> {
        Ok(self.dream_command_channel.0.send_async(cmd).await?)
    }

    pub async fn recv_dream_command(&self) -> Result<DreamCommand> {
        Ok(self.dream_command_channel.1.recv_async().await?)
    }

    pub async fn send_file_command(&self, cmd: FileCommand) -> Result<()> {
        Ok(self.file_command_channel.0.send_async(cmd).await?)
    }

    pub async fn recv_file_command(&self) -> Result<FileCommand> {
        Ok(self.file_command_channel.1.recv_async().await?)
    }

    pub async fn send_file_upload(
        &self,
        filename: String,
        content: Vec<u8>,
    ) -> Result<crate::schema::FileRecord> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.send_file_command(FileCommand::Upload {
            filename,
            content,
            response: tx,
        })
        .await?;
        rx.await?
    }

    pub async fn send_file_resolve(
        &self,
        attachment: VizierAttachment,
    ) -> Result<Vec<u8>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.send_file_command(FileCommand::Resolve {
            attachment,
            response: tx,
        })
        .await?;
        rx.await?
    }

    pub async fn exit_signal(&self) -> Result<bool> {
        Ok(self.exit_channel.1.recv_async().await?)
    }

    /// Every agent's health from the agent manager, or an error if it doesn't answer within
    /// `HEALTH_TIMEOUT`: a monitor gets an answer rather than a hang (#51).
    pub async fn agent_health(&self) -> Result<Vec<AgentHealthStatus>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.send_agent_command(AgentCommand::HealthCheck { resp: tx })
            .await?;
        match tokio::time::timeout(HEALTH_TIMEOUT, rx).await {
            Ok(Ok(statuses)) => Ok(statuses),
            Ok(Err(_)) => Err(anyhow::anyhow!("the agent manager is not running")),
            Err(_) => Err(anyhow::anyhow!("the agent manager did not answer in time")),
        }
    }

    /// Answer commands from the command socket until the process stops.
    pub async fn run(&self) -> Result<()> {
        let req_rx = self.command_request_channel.1.clone();
        let res = self.command_response_channel.0.clone();
        while let Ok(req) = req_rx.recv_async().await {
            let exit = matches!(req, CommandRequest::Exit);
            let response = match req {
                CommandRequest::Exit => CommandResponse::Ok("vizier is stopping".into()),
                CommandRequest::HealthCheck => match self.agent_health().await {
                    Ok(statuses) => {
                        CommandResponse::Ok(serde_json::to_string(&statuses).unwrap_or_default())
                    }
                    Err(e) => CommandResponse::Error(format!("health check failed: {e}")),
                },
                other => CommandResponse::Error(format!("unsupported command: {other:?}")),
            };
            let _ = res.send_async(response).await;
            // After the answer is on its way, since the process exits on this signal.
            if exit {
                let _ = self.exit_channel.0.send_async(true).await;
            }
        }
        Err(anyhow::anyhow!("the command request channel closed"))
    }
}
