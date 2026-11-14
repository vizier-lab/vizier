mod events;
mod inbox;
mod reply;
pub mod wire;

use crate::time::Timestamp;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use vizier_schema::{Bus, Envelope, InProcess, Subject, SubjectPattern};

pub use inbox::{Inbox, InboxClosed};
pub use reply::{
    ReplyClosed, ResponseSender, ResponseStream, reply_channel as bus_reply_channel, standalone_reply_channel,
};

use crate::storage::{history::HistoryStorage, turn::TurnStorage};
use crate::schema::{
    AgentCommand, AgentHealthStatus, AgentId, CommandRequest, CommandResponse, IntegrationChanged,
    ReactionEntry, VizierAttachment, VizierRequest, VizierResponse, VizierSession,
};

/// How many session events a slow subscriber may fall behind before it starts losing them.
/// Every WebUI turn's frames go through here, so it is sized for several busy turns at once.
/// A lagging WebSocket logs and carries on; the WebUI reconciles by re-reading the job list,
/// and history once the turn ends.
const SESSION_EVENTS_CAPACITY: usize = 1024;
/// Integration changes are rare; a subscriber that still falls behind reconciles everything.
const INTEGRATION_EVENTS_CAPACITY: usize = 64;
/// Inbox events are infrequent and setup progress is throttled, so this is generous; a
/// connection that still falls behind is told to resynchronise.
const INBOX_EVENTS_CAPACITY: usize = 256;
/// How long a file upload or read may take.
const FILE_COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// How long the command socket waits for the process to answer.
const PROCESS_COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// How long a memory operation may take, indexing included.
const MEMORY_OP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// How long a command may take to be handled: a create can pull a model.
const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60 * 60);
/// How long a health check waits for the agent manager.
const HEALTH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Something pushed to whoever is watching a session, outside of a turn they started
/// themselves: a background job changing state, or the frames of a turn a background report
/// woke. WebUI sockets subscribe, each filtering on its own session, and Discord/Telegram
/// readers subscribe for the woken turns of their own channels (`channels::woken`); a session
/// nobody can watch (task, dream, inter-agent, subagent) has its events dropped.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionEvent {
    pub session: VizierSession,
    pub frame: SessionFrame,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SessionFrame {
    Response(VizierResponse),
    /// A detached script changed (spec 058): launched, made a call, ended or was cancelled.
    DetachedScript(crate::schema::DetachedScriptSnapshot),
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
    /// A file saved to the session: from a tool, or attached to a request.
    File(crate::schema::session_file::SessionFileRecord),
    /// The session's title, after it was generated.
    Title { title: String },
    /// A client tool call's newly saved state, echoed to the saver as its confirmation.
    ClientCallState {
        call_id: String,
        state: serde_json::Value,
    },
    /// A question set's state changed (`specs/042-agent-asking-question`): it was answered,
    /// declined or withdrawn. The entry itself is filled from storage on the next read, so this
    /// frame is what makes an open card close without a reload.
    QuestionSet {
        set_id: String,
        state: crate::questions::model::QuestionState,
        answers: Option<Vec<crate::questions::model::Answer>>,
        answerer: Option<String>,
    },
    /// A sensitive-input dialog opening (`specs/042-agent-asking-question`). Sent to every socket
    /// on the conversation and replayed on open, so a second tab or a reload shows the same
    /// dialog; it carries the ask and never a value.
    SensitiveAsk(crate::questions::model::SensitiveAskRecord),
    /// A sensitive-input ask closing, whichever tab or person resolved it.
    SensitiveAskClosed {
        ask_id: String,
        outcome: crate::questions::model::AskState,
    },
}

/// Something that happened in a Workspace (`specs/053-workspace-channel/contracts/socket-frames.md`).
/// It travels the bus on `workspace.{id}.events`. A gateway decides per delivery who may see it:
/// nothing here is checked against a person.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WorkspaceEvent {
    /// A message or note was written.
    Entry {
        entry: crate::schema::WorkspaceEntry,
        mentions: Vec<crate::schema::EntryMention>,
    },
    /// Mentions of one entry changed state.
    Mentions {
        workspace_id: String,
        topic_id: String,
        seq: i64,
        mentions: Vec<crate::schema::EntryMention>,
    },
    /// Membership changed. `removed` names a person or agent who is out.
    MemberChanged {
        workspace_id: String,
        removed: Option<(crate::schema::MemberKind, String)>,
    },
    /// The Workspace was deleted.
    Deleted { workspace_id: String },
}

impl WorkspaceEvent {
    pub fn workspace_id(&self) -> &str {
        match self {
            Self::Entry { entry, .. } => &entry.workspace_id,
            Self::Mentions { workspace_id, .. }
            | Self::MemberChanged { workspace_id, .. }
            | Self::Deleted { workspace_id } => workspace_id,
        }
    }
}

/// Something the inbox stream (`specs/051-realtime-notifications`) turns into a frame for the
/// people who may see it. It travels the bus on `inbox.events` and is never sent to a client
/// as it is: each gateway decides per delivery who gets what, from the audience facts carried
/// here.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum InboxEvent {
    /// One notification row was inserted. `audience` is the row's `audience_user_id`. The
    /// item's own `source_session` is not serialized, so it travels as its own field.
    Created {
        item: crate::schema::NotificationItem,
        audience: Option<String>,
        source_session: Option<VizierSession>,
        /// Who wrote it (a Workspace message): never told about their own message.
        #[serde(default)]
        author: Option<String>,
    },
    /// One person's read state changed. `id: None` means mark-all.
    Read {
        user_id: String,
        id: Option<i64>,
        read: bool,
    },
    /// Live setup of one agent. `SetupState` skips `owner_id` in serde, so it is carried here.
    Setup {
        state: crate::setup::SetupState,
        owner_id: Option<String>,
    },
    SetupCleared { agent_id: String },
    /// Rows may have changed (a conversation deleted, a title arrived): the clients concerned
    /// reload. `user` limits it to one person, `agent_id` to the viewers of one agent.
    Stale {
        user: Option<String>,
        agent_id: Option<String>,
    },
    /// A project's language servers changed (`specs/054-project-lsp`). Delivered to people who
    /// can use the project; the snapshot holds no override values.
    ProjectLsp {
        agent_id: String,
        snapshot: crate::schema::LspStatusSnapshot,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DreamCommand {
    pub agent_id: AgentId,
    pub cycle_id: Option<String>,
}

/// A request as the agent receives it: decoded from the bus, with the reply stream attached.
#[derive(Debug, Clone)]
pub struct VizierRequestEnvelope {
    pub session: VizierSession,
    pub request: VizierRequest,
    pub response_tx: Option<ResponseSender>,
    /// The `session_turn` row journaling this request, when it is durable.
    pub turn_id: Option<i64>,
    /// Re-sent after a restart with its turn already begun: the agent resumes it.
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
    /// The agents that have a request queue, so a request to one that has none fails at once.
    inboxes: Arc<RwLock<HashMap<AgentId, Inbox>>>,

    bus: Bus,


    exit_channel: Arc<(flume::Sender<bool>, flume::Receiver<bool>)>,

    /// Where file bytes are staged for a file command (see [`wire::FileRequest`]).
    staging: Option<Staging>,

    session_events: tokio::sync::broadcast::Sender<SessionEvent>,

    /// The event pump's outbox, started on first use (it needs a runtime).
    pump: Arc<std::sync::OnceLock<tokio::sync::mpsc::UnboundedSender<events::Outgoing>>>,

    integration_events: tokio::sync::broadcast::Sender<IntegrationChanged>,

    inbox_events: tokio::sync::broadcast::Sender<InboxEvent>,

    workspace_events: tokio::sync::broadcast::Sender<WorkspaceEvent>,

    /// Where durable requests are journaled (`specs/023-persist-turn-activity`). `None` for
    /// a transport nothing restarts around, such as the tests'.
    journal: Option<JournalHandle>,
}

impl VizierTransport {
    #[doc(hidden)]
    /// A transport that journals durable requests in `storage`.
    pub fn durable(storage: crate::storage::VizierStorage) -> Self {
        Self { journal: Some(JournalHandle(storage)), ..Self::new() }
    }

    #[doc(hidden)]
    pub fn new() -> Self {
        let exit_channel = Arc::new(flume::unbounded());

        Self {
            inboxes: Arc::new(RwLock::new(HashMap::new())),
            bus: InProcess::new().bus(),
            exit_channel,
            staging: None,
            session_events: tokio::sync::broadcast::channel(SESSION_EVENTS_CAPACITY).0,
            integration_events: tokio::sync::broadcast::channel(INTEGRATION_EVENTS_CAPACITY).0,
            inbox_events: tokio::sync::broadcast::channel(INBOX_EVENTS_CAPACITY).0,
            workspace_events: tokio::sync::broadcast::channel(INBOX_EVENTS_CAPACITY).0,
            pump: Arc::new(std::sync::OnceLock::new()),
            journal: None,
        }
    }

    /// The pump's outbox, starting the pump on first use. `None` outside a runtime.
    fn pump(&self) -> Option<&tokio::sync::mpsc::UnboundedSender<events::Outgoing>> {
        if let Some(outbox) = self.pump.get() {
            return Some(outbox);
        }
        tokio::runtime::Handle::try_current().ok()?;
        Some(self.pump.get_or_init(|| {
            events::start(
                self.bus.clone(),
                self.session_events.clone(),
                self.integration_events.clone(),
                self.inbox_events.clone(),
                self.workspace_events.clone(),
            )
        }))
    }

    #[doc(hidden)]
    /// Publish an inbox event on `inbox.events`. Having no subscribers is the normal case, and a
    /// failure to publish never reaches the work that caused the event.
    pub fn publish_inbox_event(&self, ev: InboxEvent) {
        match self.pump() {
            Some(outbox) => {
                let _ = outbox.send(events::Outgoing::Inbox(ev));
            }
            None => {
                let _ = self.inbox_events.send(ev);
            }
        }
    }

    /// Receive every inbox event, from this node or another. Subscribe **before** reading a
    /// snapshot so nothing between the two is missed. A subscriber that falls behind sees
    /// `Lagged` and should resynchronise.
    pub fn subscribe_inbox_events(&self) -> tokio::sync::broadcast::Receiver<InboxEvent> {
        let _ = self.pump();
        self.inbox_events.subscribe()
    }

    #[doc(hidden)]
    /// Publish a Workspace event on `workspace.{id}.events`. Having no subscribers is the normal
    /// case, and a failure to publish never reaches the work that caused it.
    pub fn publish_workspace_event(&self, ev: WorkspaceEvent) {
        match self.pump() {
            Some(outbox) => {
                let _ = outbox.send(events::Outgoing::Workspace(ev));
            }
            None => {
                let _ = self.workspace_events.send(ev);
            }
        }
    }

    /// Receive every Workspace event, from this node or another. A subscriber that falls behind
    /// sees `Lagged` and should resynchronise from the last `seq` it holds.
    pub fn subscribe_workspace_events(&self) -> tokio::sync::broadcast::Receiver<WorkspaceEvent> {
        let _ = self.pump();
        self.workspace_events.subscribe()
    }

    #[doc(hidden)]
    /// Publish a session event on `session.{id}.events`. Having no subscribers is the normal
    /// case for most sessions, not an error.
    pub fn publish_session_event(&self, ev: SessionEvent) {
        match self.pump() {
            Some(outbox) => {
                let _ = outbox.send(events::Outgoing::Session(ev));
            }
            // No runtime, so no bus: a plain local delivery.
            None => {
                let _ = self.session_events.send(ev);
            }
        }
    }

    /// Receive every session event, from this node or another. A subscriber that falls behind
    /// loses events and sees `Lagged`.
    pub fn subscribe_session_events(&self) -> tokio::sync::broadcast::Receiver<SessionEvent> {
        // Starting the pump here means its subscription exists before the first event is sent.
        let _ = self.pump();
        self.session_events.subscribe()
    }

    #[doc(hidden)]
    /// Announce a committed integration write (contracts/runtime.md). Subscribers re-read
    /// storage, so a lost event costs nothing once the next one, or a `Lagged`, arrives.
    pub fn publish_integration_change(&self, ev: IntegrationChanged) {
        match self.pump() {
            Some(outbox) => {
                let _ = outbox.send(events::Outgoing::Integration(ev));
            }
            None => {
                let _ = self.integration_events.send(ev);
            }
        }
    }

    #[doc(hidden)]
    pub fn subscribe_integration_changes(
        &self,
    ) -> tokio::sync::broadcast::Receiver<IntegrationChanged> {
        let _ = self.pump();
        self.integration_events.subscribe()
    }

    #[doc(hidden)]
    pub async fn is_agent_registered(&self, agent_id: &AgentId) -> bool {
        self.inboxes.read().await.contains_key(agent_id)
    }

    /// The bus this transport runs on.
    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    /// A reply stream and its sender, for a request whose answer the caller waits for.
    pub async fn reply_channel(&self) -> Result<(ResponseSender, ResponseStream), vizier_schema::BusError> {
        bus_reply_channel(&self.bus).await
    }

    #[doc(hidden)]
    pub async fn register_agent(&self, agent_id: AgentId) -> Inbox {
        let subject = Subject::agent_req(&agent_id);
        let mut inboxes = self.inboxes.write().await;
        // A queue registered twice would be fed twice by the bus; the old one ends.
        if let Some(old) = inboxes.remove(&agent_id) {
            old.close();
        }
        let sub = match self.bus.messaging.subscribe(SubjectPattern::from(&subject)).await {
            Ok(sub) => sub,
            Err(e) => {
                // The in-process backend cannot refuse; a backend that does leaves an inbox that
                // is closed from the start, which the supervisor reports as a stopped agent.
                tracing::error!(%agent_id, "could not subscribe to the agent's requests: {e}");
                let (_tx, sub) = vizier_schema::Subscription::channel(1);
                let inbox = Inbox::new(sub, self.bus.messaging.clone());
                inbox.close();
                return inbox;
            }
        };
        let inbox = Inbox::new(sub, self.bus.messaging.clone());
        inboxes.insert(agent_id, inbox.clone());
        inbox
    }

    #[doc(hidden)]
    pub async fn unregister_agent(&self, agent_id: &AgentId) {
        if let Some(inbox) = self.inboxes.write().await.remove(agent_id) {
            inbox.close();
        }
    }

    #[doc(hidden)]
    /// Start taking memory operations for `agent_id` off the bus. Dropping the returned inbox
    /// stops it, and a later operation finds no handler.
    pub async fn register_memory_op(&self, agent_id: AgentId) -> Result<MemoryOpInbox> {
        let sub = self
            .bus
            .messaging
            .subscribe(SubjectPattern::from(&Subject::agent_memop(&agent_id)))
            .await?;
        Ok(MemoryOpInbox { sub, messaging: self.bus.messaging.clone() })
    }

    #[doc(hidden)]
    pub async fn send_memory_op(
        &self,
        agent_id: &AgentId,
        op: crate::schema::MemoryOpRequest,
    ) -> Result<crate::schema::MemoryOpResponse> {
        let envelope = Envelope::json(wire::KIND_MEMORY_OP, &op)?;
        let reply = self
            .bus
            .messaging
            .request(&Subject::agent_memop(agent_id), envelope, MEMORY_OP_TIMEOUT)
            .await
            .map_err(|e| match e {
                vizier_schema::BusError::NoResponders => {
                    anyhow::anyhow!("agent '{}' has no memory-op channel", agent_id)
                }
                other => anyhow::Error::from(other),
            })?;
        match reply.decode::<wire::MemoryOpReply>()? {
            wire::MemoryOpReply::Ok(response) => Ok(response),
            wire::MemoryOpReply::Err { message } => Err(anyhow::anyhow!(message)),
        }
    }

    pub async fn send_request(
        &self,
        session: VizierSession,
        req: VizierRequest,
        response_tx: Option<ResponseSender>,
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

    #[doc(hidden)]
    /// Sends an envelope as it is, without journaling it again: a request recovered at
    /// startup, or handed on to the next instance of an agent.
    pub async fn resend(&self, envelope: VizierRequestEnvelope) -> Result<()> {
        if !self.inboxes.read().await.contains_key(&envelope.session.0) {
            return Err(anyhow::anyhow!("agent '{}' not registered", envelope.session.0));
        }
        let subject = Subject::agent_req(&envelope.session.0);
        let reply_to = envelope.response_tx.as_ref().map(|tx| tx.subject().clone());
        let message = wire::encode_request(&envelope, reply_to)?;
        match self.bus.messaging.publish(&subject, message).await {
            Ok(()) => {
                // The agent's side owns the reply stream and its end from here on.
                if let Some(tx) = envelope.response_tx {
                    tx.into_subject();
                }
                Ok(())
            }
            // Dropping the sender ends the requester's stream, as a dropped flume sender did.
            Err(e) => Err(e.into()),
        }
    }

    #[doc(hidden)]
    /// How the turn running in `session` ends, for work that outlived a restart and has no
    /// response channel of its own to read: the first terminal reply stored after `since`, or
    /// else the first one published. A channel that closes reads as a restart.
    pub async fn await_session_outcome(
        &self,
        session: &VizierSession,
        since: Timestamp,
    ) -> crate::agents::subagents::PieceOutcome {
        use crate::agents::subagents::{AGENT_RESTARTED, PieceOutcome, classify};
        use crate::schema::SessionHistoryContent;
        let mut events = self.subscribe_session_events();
        loop {
            // Checked first and again after a lag: a reply published before the subscription
            // is in history by then, since it is saved before it is sent.
            if let Some(journal) = &self.journal
                && let Ok(history) = journal.list_session_history(session.clone(), None, None, None).await
                && let Some(outcome) = history.iter().rev().find_map(|entry| match &entry.content {
                    SessionHistoryContent::Response(r) if r.timestamp > since => classify(&r.content),
                    _ => None,
                })
            {
                return outcome;
            }
            loop {
                match events.recv().await {
                    Ok(ev) if &ev.session == session => {
                        if let SessionFrame::Response(r) = ev.frame
                            && let Some(outcome) = classify(&r.content)
                        {
                            return outcome;
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return PieceOutcome::Failed(AGENT_RESTARTED.to_string());
                    }
                }
            }
        }
    }

    #[doc(hidden)]
    /// A response sender that republishes every frame onto the session-event broadcast, for a
    /// turn nobody is waiting on (a woken report, a recovered request).
    pub async fn broadcast_sender(&self, session: VizierSession) -> Result<ResponseSender> {
        let (tx, mut rx) = self.reply_channel().await?;
        let transport = self.clone();
        tokio::spawn(async move {
            while let Ok(frame) = rx.recv_async().await {
                transport.publish_session_event(SessionEvent {
                    session: session.clone(),
                    frame: SessionFrame::Response(frame),
                });
            }
        });
        Ok(tx)
    }

    #[doc(hidden)]
    /// Ask this process a command (the command socket's) and wait for its answer.
    pub async fn send_command(&self, req: CommandRequest) -> Result<CommandResponse> {
        let envelope = Envelope::json(wire::KIND_COMMAND_REQUEST, &req)?;
        let reply = self
            .bus
            .messaging
            .request(&Subject::process_cmd(), envelope, PROCESS_COMMAND_TIMEOUT)
            .await
            .map_err(|e| match e {
                vizier_schema::BusError::NoResponders => anyhow::anyhow!("the command handler is not running"),
                other => anyhow::Error::from(other),
            })?;
        Ok(reply.decode::<CommandResponse>()?)
    }

    /// Send a command to the agent manager over the bus and wait for it to be handled. The
    /// answer lands in the command's own slot, so callers read it as before.
    pub async fn send_agent_command(&self, cmd: AgentCommand) -> Result<()> {
        let (request, awaiting) = wire::split_agent_command(cmd);
        let envelope = Envelope::json(wire::KIND_AGENT_COMMAND, &request)?;
        let reply = self
            .bus
            .messaging
            .request(&Subject::agents_cmd(), envelope, COMMAND_TIMEOUT)
            .await
            .map_err(|e| match e {
                vizier_schema::BusError::NoResponders => anyhow::anyhow!("the agent manager is not running"),
                vizier_schema::BusError::Timeout => anyhow::anyhow!("the agent manager did not answer in time"),
                other => anyhow::Error::from(other),
            })?;
        match reply.decode::<wire::AgentCommandReply>()? {
            wire::AgentCommandReply::Dropped => awaiting.abandon(),
            reply => awaiting.fulfil(reply),
        }
        Ok(())
    }

    /// Send a command to the agent that owns `agent_id`, over `agent.{id}.cmd`, and wait for it
    /// to be handled.
    ///
    /// Unlike [`Self::send_agent_command`] the bus failure is not flattened into a message: the
    /// two failures the seam tells apart - nobody subscribed, and nobody answering - come back
    /// as the bus's own error, so the caller can say which one it was
    /// (`specs/048-agent-service-seam` contracts/agents-client.md, "Errors").
    pub async fn send_agent_command_to(
        &self,
        agent_id: &str,
        cmd: AgentCommand,
        timeout: std::time::Duration,
    ) -> Result<()> {
        let (request, awaiting) = wire::split_agent_command(cmd);
        let envelope = Envelope::json(wire::KIND_AGENT_SCOPED_COMMAND, &request)?;
        let reply = self
            .bus
            .messaging
            .request(&Subject::agent_cmd(agent_id), envelope, timeout)
            .await?;
        match reply.decode::<wire::AgentCommandReply>()? {
            wire::AgentCommandReply::Dropped => awaiting.abandon(),
            reply => awaiting.fulfil(reply),
        }
        Ok(())
    }

    #[doc(hidden)]
    /// Start taking agent commands off the bus. The subscription exists when this returns, so a
    /// command sent right after cannot find nobody listening.
    pub async fn serve_agent_commands(&self) -> Result<AgentCommands> {
        let sub = self.bus.messaging.subscribe(SubjectPattern::from(&Subject::agents_cmd())).await?;
        Ok(AgentCommands { sub, messaging: self.bus.messaging.clone(), reply_kind: wire::KIND_AGENT_REPLY })
    }

    #[doc(hidden)]
    /// Start taking one agent's commands off `agent.{id}.cmd`. The subscription exists when this
    /// returns, so a command sent right after cannot find nobody listening.
    ///
    /// Only the agent's owner subscribes to its subject, so a command reaches exactly one
    /// handler: the bus delivers at most once, so a command a non-owner consumed would be lost.
    pub async fn serve_agent_commands_for(&self, agent_id: &str) -> Result<AgentCommands> {
        let sub = self
            .bus
            .messaging
            .subscribe(SubjectPattern::from(&Subject::agent_cmd(agent_id)))
            .await?;
        Ok(AgentCommands {
            sub,
            messaging: self.bus.messaging.clone(),
            reply_kind: wire::KIND_AGENT_SCOPED_REPLY,
        })
    }

    #[doc(hidden)]
    /// Ask the scheduler to run a dream cycle now. Fire and forget: nobody listening is not
    /// an error, as for any published message.
    pub async fn send_dream_command(&self, cmd: DreamCommand) -> Result<()> {
        let envelope = Envelope::json(wire::KIND_DREAM_COMMAND, &cmd)?;
        self.bus.messaging.publish(&Subject::dream_cmd(), envelope).await?;
        Ok(())
    }

    #[doc(hidden)]
    /// Start taking dream commands off the bus; the subscription exists when this returns.
    pub async fn serve_dream_commands(&self) -> Result<DreamCommands> {
        let sub = self.bus.messaging.subscribe(SubjectPattern::from(&Subject::dream_cmd())).await?;
        Ok(DreamCommands { sub })
    }

    #[doc(hidden)]
    /// This transport, staging file bytes in `blobs` for file commands.
    pub fn with_blobs(mut self, blobs: Arc<dyn crate::storage::blob::BlobStore>) -> Self {
        self.staging = Some(Staging(blobs));
        self
    }

    fn staging(&self) -> Result<&Arc<dyn crate::storage::blob::BlobStore>> {
        self.staging
            .as_ref()
            .map(|s| &s.0)
            .ok_or_else(|| anyhow::anyhow!("this transport has no blob store to stage file bytes in"))
    }

    #[doc(hidden)]
    pub async fn send_file_upload(
        &self,
        filename: String,
        content: Vec<u8>,
    ) -> Result<crate::schema::FileRecord> {
        let blobs = self.staging()?;
        let key = new_staging_key()?;
        blobs.put(&key, bytes::Bytes::from(content)).await?;
        let result = self
            .file_request(wire::FileRequest::Upload { filename, staged: key.as_str().to_string() })
            .await;
        // The handler deletes it on success; a failure before it got there leaves it behind.
        if result.is_err() {
            let _ = blobs.delete(&key).await;
        }
        match result? {
            wire::FileReply::Record(record) => Ok(record),
            wire::FileReply::Err { message } => Err(anyhow::anyhow!(message)),
            other => Err(anyhow::anyhow!("unexpected reply to an upload: {other:?}")),
        }
    }

    #[doc(hidden)]
    pub async fn send_file_resolve(&self, attachment: VizierAttachment) -> Result<Vec<u8>> {
        let blobs = self.staging()?;
        match self.file_request(wire::FileRequest::Resolve { attachment }).await? {
            wire::FileReply::Staged { key } => {
                let key = crate::storage::blob::BlobKey::new(&key)?;
                let bytes = blobs.get(&key).await?;
                let _ = blobs.delete(&key).await;
                Ok(bytes.ok_or_else(|| anyhow::anyhow!("the staged file vanished"))?.to_vec())
            }
            wire::FileReply::Err { message } => Err(anyhow::anyhow!(message)),
            other => Err(anyhow::anyhow!("unexpected reply to a file read: {other:?}")),
        }
    }

    async fn file_request(&self, request: wire::FileRequest) -> Result<wire::FileReply> {
        let envelope = Envelope::json(wire::KIND_FILE_COMMAND, &request)?;
        let reply = self
            .bus
            .messaging
            .request(&Subject::files_cmd(), envelope, FILE_COMMAND_TIMEOUT)
            .await
            .map_err(|e| match e {
                vizier_schema::BusError::NoResponders => anyhow::anyhow!("the file manager is not running"),
                other => anyhow::Error::from(other),
            })?;
        Ok(reply.decode::<wire::FileReply>()?)
    }

    #[doc(hidden)]
    /// Start taking file commands off the bus; the subscription exists when this returns.
    pub async fn serve_file_commands(&self) -> Result<FileCommands> {
        let sub = self.bus.messaging.subscribe(SubjectPattern::from(&Subject::files_cmd())).await?;
        Ok(FileCommands { sub, messaging: self.bus.messaging.clone(), staging: self.staging.as_ref().map(|s| s.0.clone()) })
    }

    #[doc(hidden)]
    pub async fn exit_signal(&self) -> Result<bool> {
        Ok(self.exit_channel.1.recv_async().await?)
    }

    #[doc(hidden)]
    /// Every agent's health from the agent manager, or an error if it doesn't answer within
    /// `HEALTH_TIMEOUT`: a monitor gets an answer rather than a hang (#51).
    pub async fn agent_health(&self) -> Result<Vec<AgentHealthStatus>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        // The deadline covers the whole round trip over the bus, not just the wait for the slot.
        match tokio::time::timeout(
            HEALTH_TIMEOUT,
            self.send_agent_command(AgentCommand::HealthCheck { resp: tx }),
        )
        .await
        {
            Ok(sent) => sent?,
            Err(_) => return Err(anyhow::anyhow!("the agent manager did not answer in time")),
        }
        rx.await.map_err(|_| anyhow::anyhow!("the agent manager is not running"))
    }

    #[doc(hidden)]
    /// Answer commands from the command socket until the process stops.
    pub async fn run(&self) -> Result<()> {
        let mut commands = self
            .bus
            .messaging
            .subscribe(SubjectPattern::from(&Subject::process_cmd()))
            .await?;
        while let Some((_, envelope)) = commands.recv().await {
            let req = match envelope.decode::<CommandRequest>() {
                Ok(req) => req,
                Err(e) => {
                    tracing::warn!("dropping an undecodable command: {e}");
                    continue;
                }
            };
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
            if let Ok(out) = Envelope::json(wire::KIND_COMMAND_RESPONSE, &response) {
                let _ = vizier_schema::reply(self.bus.messaging.as_ref(), &envelope, out).await;
            }
            // After the answer is on its way, since the process exits on this signal. The
            // signal itself stays process-local: it is this process's lifecycle, not bus traffic.
            if exit {
                let _ = self.exit_channel.0.send_async(true).await;
            }
        }
        Err(anyhow::anyhow!("the command subscription closed"))
    }
}

#[cfg(test)]
mod bus_tests {
    use std::time::Duration;

    use super::*;
    use crate::schema::{VizierChannelId, VizierRequestContent, VizierResponseContent};

    fn session(agent: &str) -> VizierSession {
        VizierSession(agent.into(), VizierChannelId::System, None)
    }

    fn chat(text: &str) -> VizierRequest {
        VizierRequest {
            timestamp: crate::time::now(),
            user: "tester".into(),
            content: VizierRequestContent::Chat(text.into()),
            metadata: serde_json::json!({}),
            ..Default::default()
        }
    }

    fn text(s: &str) -> VizierResponse {
        VizierResponse {
            content: VizierResponseContent::Message { content: s.into(), stats: None },
            ..Default::default()
        }
    }

    async fn next(inbox: &Inbox) -> VizierRequestEnvelope {
        tokio::time::timeout(Duration::from_secs(5), inbox.recv_request())
            .await
            .expect("a request in time")
            .expect("an open inbox")
    }

    #[tokio::test]
    async fn a_request_round_trips_over_the_bus_and_its_reply_stream_ends() {
        let transport = VizierTransport::new();
        let inbox = transport.register_agent("alpha".into()).await;
        let (tx, rx) = transport.reply_channel().await.unwrap();
        transport.send_request(session("alpha"), chat("hello"), Some(tx)).await.unwrap();

        let got = next(&inbox).await;
        assert!(matches!(&got.request.content, VizierRequestContent::Chat(t) if t == "hello"));
        let answer = got.response_tx.expect("a reply address");
        answer.send_async(text("one")).await.unwrap();
        answer.send_async(text("two")).await.unwrap();
        drop(answer);

        let mut seen = vec![];
        while let Ok(frame) = rx.recv_async().await {
            if let VizierResponseContent::Message { content, .. } = frame.content {
                seen.push(content);
            }
        }
        assert_eq!(seen, ["one", "two"]);
    }

    #[tokio::test]
    async fn a_request_to_an_unregistered_agent_fails_at_once_and_ends_the_stream() {
        let transport = VizierTransport::new();
        let (tx, rx) = transport.reply_channel().await.unwrap();
        let err = transport.send_request(session("ghost"), chat("hi"), Some(tx)).await.unwrap_err();
        assert!(err.to_string().contains("not registered"), "{err}");
        // The dropped sender closed the requester's stream rather than leaving it hanging.
        assert!(tokio::time::timeout(Duration::from_secs(5), rx.recv_async()).await.unwrap().is_err());
    }

    #[tokio::test]
    async fn an_inbox_closes_on_unregister_for_every_clone_after_it_is_drained() {
        let transport = VizierTransport::new();
        let inbox = transport.register_agent("alpha".into()).await;
        let clone = inbox.clone();
        transport.send_request(session("alpha"), chat("queued"), None).await.unwrap();
        transport.unregister_agent(&"alpha".to_string()).await;

        assert!(inbox.is_disconnected() && clone.is_disconnected());
        assert!(!transport.is_agent_registered(&"alpha".to_string()).await);
        // What was queued before the close can still be read, by either handle; then it ends.
        let got = next(&clone).await;
        assert!(matches!(&got.request.content, VizierRequestContent::Chat(t) if t == "queued"));
        assert_eq!(inbox.recv_request().await.unwrap_err(), InboxClosed);
        assert_eq!(clone.recv_request().await.unwrap_err(), InboxClosed);
        // And nothing new is accepted.
        assert!(transport.send_request(session("alpha"), chat("late"), None).await.is_err());
    }

    #[tokio::test]
    async fn requests_queued_while_an_instance_is_swapped_wait_for_the_next_one() {
        let transport = VizierTransport::new();
        let first_instance = transport.register_agent("alpha".into()).await;
        // The supervisor hands the same inbox to the replacement instance.
        let second_instance = first_instance.clone();
        drop(first_instance);
        transport.send_request(session("alpha"), chat("during the swap"), None).await.unwrap();
        let got = next(&second_instance).await;
        assert!(matches!(&got.request.content, VizierRequestContent::Chat(t) if t == "during the swap"));
    }

    #[tokio::test]
    async fn a_realistic_request_survives_the_envelope() {
        let transport = VizierTransport::new();
        let inbox = transport.register_agent("alpha".into()).await;
        let mut request = chat("with everything");
        request.transient = true;
        request.background_depth = 2;
        request.trace.traceparent = Some("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".into());
        request.trace.enqueued_at_ms = Some(1234);
        let envelope = VizierRequestEnvelope {
            session: session("alpha"),
            request,
            response_tx: None,
            turn_id: Some(7),
            resumed: true,
        };
        transport.resend(envelope).await.unwrap();

        let got = next(&inbox).await;
        assert!(got.request.transient);
        assert_eq!(got.request.background_depth, 2);
        assert_eq!(got.turn_id, Some(7));
        assert!(got.resumed);
        assert_eq!(got.session, session("alpha"));
        assert_eq!(
            got.request.trace.traceparent.as_deref(),
            Some("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01")
        );
        // The first send keeps its queue time; a re-send does not restamp it.
        assert_eq!(got.request.trace.enqueued_at_ms, Some(1234));
    }

    #[tokio::test]
    async fn re_registering_replaces_the_queue_instead_of_feeding_two() {
        let transport = VizierTransport::new();
        let old = transport.register_agent("alpha".into()).await;
        let new = transport.register_agent("alpha".into()).await;
        assert!(old.is_disconnected());
        transport.send_request(session("alpha"), chat("one"), None).await.unwrap();
        next(&new).await;
        assert_eq!(old.recv_request().await.unwrap_err(), InboxClosed);
    }
}

/// The agent manager's end of a command subject: `agents.cmd` for the fleet-wide commands,
/// `agent.{id}.cmd` for the ones that name their agent.
pub struct AgentCommands {
    sub: vizier_schema::Subscription,
    messaging: Arc<dyn vizier_schema::Messaging>,
    /// The kind the answer is published under, so a reader knows which subject it answered.
    reply_kind: &'static str,
}

impl AgentCommands {
    /// The next command. Its answer is published to the sender once the handler fills the
    /// command's slot.
    pub async fn recv(&mut self) -> Option<AgentCommand> {
        loop {
            let (_, envelope) = self.sub.recv().await?;
            let request = match envelope.decode::<wire::AgentCommandRequest>() {
                Ok(request) => request,
                Err(e) => {
                    tracing::warn!("dropping an undecodable agent command: {e}");
                    continue;
                }
            };
            let (command, answer) = wire::open_agent_command(request);
            let messaging = self.messaging.clone();
            let kind = self.reply_kind;
            tokio::spawn(async move {
                let reply = answer.into_reply().await.unwrap_or(wire::AgentCommandReply::Dropped);
                if let Ok(out) = Envelope::json(kind, &reply)
                    && let Err(e) = vizier_schema::reply(messaging.as_ref(), &envelope, out).await
                {
                    tracing::warn!("could not answer an agent command: {e}");
                }
            });
            return Some(command);
        }
    }
}

/// An agent's end of `agent.{id}.memop`.
pub struct MemoryOpInbox {
    sub: vizier_schema::Subscription,
    messaging: Arc<dyn vizier_schema::Messaging>,
}

impl MemoryOpInbox {
    /// The next operation, with what it needs to answer it.
    pub async fn recv(&mut self) -> Option<(crate::schema::MemoryOpRequest, MemoryOpReplier)> {
        loop {
            let (_, envelope) = self.sub.recv().await?;
            match envelope.decode::<crate::schema::MemoryOpRequest>() {
                Ok(op) => return Some((op, MemoryOpReplier { messaging: self.messaging.clone(), request: envelope })),
                Err(e) => tracing::warn!("dropping an undecodable memory operation: {e}"),
            }
        }
    }
}

/// Answers one memory operation.
pub struct MemoryOpReplier {
    messaging: Arc<dyn vizier_schema::Messaging>,
    request: Envelope,
}

impl MemoryOpReplier {
    pub async fn answer(self, result: Result<crate::schema::MemoryOpResponse>) {
        let reply = match result {
            Ok(response) => wire::MemoryOpReply::Ok(response),
            Err(e) => wire::MemoryOpReply::Err { message: e.to_string() },
        };
        if let Ok(out) = Envelope::json(wire::KIND_MEMORY_OP_REPLY, &reply)
            && let Err(e) = vizier_schema::reply(self.messaging.as_ref(), &self.request, out).await
        {
            tracing::warn!("could not answer a memory operation: {e}");
        }
    }
}

/// The scheduler's end of the dream-command subject.
pub struct DreamCommands {
    sub: vizier_schema::Subscription,
}

impl DreamCommands {
    pub async fn recv(&mut self) -> Option<DreamCommand> {
        loop {
            let (_, envelope) = self.sub.recv().await?;
            match envelope.decode::<DreamCommand>() {
                Ok(cmd) => return Some(cmd),
                Err(e) => tracing::warn!("dropping an undecodable dream command: {e}"),
            }
        }
    }
}

/// The blob store file bytes are staged in. `BlobStore` has no `Debug`.
#[derive(Clone)]
struct Staging(Arc<dyn crate::storage::blob::BlobStore>);

impl std::fmt::Debug for Staging {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Staging")
    }
}

fn new_staging_key() -> Result<crate::storage::blob::BlobKey> {
    crate::storage::blob::BlobKey::new(&format!("staging/{}", uuid::Uuid::new_v4().simple()))
}

/// The file manager's end of the file-command subject.
pub struct FileCommands {
    sub: vizier_schema::Subscription,
    messaging: Arc<dyn vizier_schema::Messaging>,
    staging: Option<Arc<dyn crate::storage::blob::BlobStore>>,
}

impl FileCommands {
    /// The next command, with what it needs to answer it.
    pub async fn recv(&mut self) -> Option<(wire::FileRequest, FileReplier)> {
        loop {
            let (_, envelope) = self.sub.recv().await?;
            match envelope.decode::<wire::FileRequest>() {
                Ok(request) => {
                    let replier = FileReplier {
                        messaging: self.messaging.clone(),
                        staging: self.staging.clone(),
                        request: envelope,
                    };
                    return Some((request, replier));
                }
                Err(e) => tracing::warn!("dropping an undecodable file command: {e}"),
            }
        }
    }
}

/// Answers one file command, and moves bytes in and out of staging.
pub struct FileReplier {
    messaging: Arc<dyn vizier_schema::Messaging>,
    staging: Option<Arc<dyn crate::storage::blob::BlobStore>>,
    request: Envelope,
}

impl FileReplier {
    /// The bytes a sender staged, removed from staging.
    pub async fn take_staged(&self, key: &str) -> Result<Vec<u8>> {
        let blobs = self.staging.as_ref().ok_or_else(|| anyhow::anyhow!("no staging store"))?;
        let key = crate::storage::blob::BlobKey::new(key)?;
        let bytes = blobs.get(&key).await?;
        let _ = blobs.delete(&key).await;
        Ok(bytes.ok_or_else(|| anyhow::anyhow!("the staged file is gone"))?.to_vec())
    }

    /// Stage `bytes` for the requester to read.
    pub async fn stage(&self, bytes: Vec<u8>) -> Result<wire::FileReply> {
        let blobs = self.staging.as_ref().ok_or_else(|| anyhow::anyhow!("no staging store"))?;
        let key = new_staging_key()?;
        blobs.put(&key, bytes::Bytes::from(bytes)).await?;
        Ok(wire::FileReply::Staged { key: key.as_str().to_string() })
    }

    pub async fn answer(self, reply: wire::FileReply) {
        if let Ok(out) = Envelope::json(wire::KIND_FILE_REPLY, &reply)
            && let Err(e) = vizier_schema::reply(self.messaging.as_ref(), &self.request, out).await
        {
            tracing::warn!("could not answer a file command: {e}");
        }
    }
}

#[cfg(test)]
mod file_tests {
    use std::sync::Arc;

    use super::*;

    /// Bytes bigger than a bus payload still cross, because only a blob key rides the bus.
    #[tokio::test]
    async fn files_over_the_payload_limit_cross_through_staging() {
        use crate::schema::VizierAttachment;
        let dir = tempfile::tempdir().unwrap();
        let blobs: Arc<dyn crate::storage::blob::BlobStore> =
            Arc::new(crate::storage::blob::LocalBlobStore::new(dir.path().to_string_lossy().to_string()));
        let transport = VizierTransport::new().with_blobs(blobs.clone());
        let mut commands = transport.serve_file_commands().await.unwrap();
        let big = vec![7u8; vizier_schema::MAX_PAYLOAD * 2];
        let expected = big.clone();

        // A stand-in for the file manager: reports what it was handed, and returns the same bytes.
        tokio::spawn(async move {
            while let Some((request, replier)) = commands.recv().await {
                let reply = match request {
                    wire::FileRequest::Upload { staged, .. } => {
                        let got = replier.take_staged(&staged).await.unwrap();
                        wire::FileReply::Err { message: format!("got {} bytes", got.len()) }
                    }
                    wire::FileRequest::Resolve { .. } => replier.stage(vec![9u8; 3_000_000]).await.unwrap(),
                };
                replier.answer(reply).await;
            }
        });

        let err = transport.send_file_upload("big.bin".into(), big).await.unwrap_err();
        assert_eq!(err.to_string(), format!("got {} bytes", expected.len()));

        let attachment = VizierAttachment {
            filename: "a.bin".into(),
            content: crate::schema::VizierAttachmentContent::Local("a.bin".into()),
        };
        let bytes = transport.send_file_resolve(attachment).await.unwrap();
        assert_eq!(bytes.len(), 3_000_000);

        // Nothing is left staged.
        let staged = blobs.list(&crate::storage::blob::BlobKey::new("staging").unwrap()).await.unwrap();
        assert!(staged.is_empty(), "{staged:?}");
    }
}
