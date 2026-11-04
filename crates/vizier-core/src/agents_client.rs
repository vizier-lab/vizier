//! The seam between the gateway side and the agents (`specs/163-agent-service-seam`).
//!
//! One way to reach an agent: the [`AgentsClient`] trait. `vizier-http` and `core::service`
//! call it; they never read a running agent's memory. The local implementation reaches the
//! agent manager over the in-process bus, on the subjects a networked implementation will
//! use, so the contract is exercised by the same path either way.
//!
//! The contract is written down in
//! `specs/163-agent-service-seam/contracts/agents-client.md`: the operations, the subjects they
//! address, the errors they raise, and the rules that hold for every implementation.
//!
//! # Addressing
//!
//! Every operation addresses the agent and nothing else. No operation takes a host, a node or
//! a lease, and none of them reveals which node owns the agent.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::schema::{
    AgentCommand, AgentCommandResult, AgentConfig, AgentHealthStatus, AgentSummary, ErrorKind,
    IntegrationStatus, VizierRequest, VizierRequestContent, VizierResponse, VizierSession,
};
use crate::transport::VizierTransport;
use crate::{Result, VizierError};

/// The frames of one turn, as the agent produces them.
///
/// The stream ends when the turn ends. A turn that was cut short carries an error frame with
/// [`ErrorKind::AgentUnavailable`], the way a restarted agent reports it today.
pub struct AgentTurn {
    source: TurnSource,
}

enum TurnSource {
    /// The real thing: the reply stream the transport carries over the bus.
    Bus(crate::transport::ResponseStream),
    /// A scripted turn: what a fake hands over.
    Scripted(tokio::sync::mpsc::UnboundedReceiver<VizierResponse>),
}

impl AgentTurn {
    /// A turn that is already over, carrying the frames it produced.
    pub fn finished(frames: Vec<VizierResponse>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        for frame in frames {
            let _ = tx.send(frame);
        }
        drop(tx);
        Self { source: TurnSource::Scripted(rx) }
    }

    /// The next frame of the turn, or `None` when the turn ended.
    pub async fn recv(&mut self) -> Option<VizierResponse> {
        match &mut self.source {
            TurnSource::Bus(stream) => stream.recv_async().await.ok(),
            TurnSource::Scripted(frames) => frames.recv().await,
        }
    }
}

/// Why an agent could not be reached.
///
/// The two reasons are told apart, because an operator has to be able to tell "the fleet has
/// not placed this agent yet" from "the worker went away" (FR-013). The reason rides in the
/// error, not only in the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentUnavailableReason {
    /// Nobody is subscribed to the agent's subject: the fleet has not placed it yet. The bus
    /// answers `NoResponders`, so this is known at once and is never a timeout (FR-014).
    NoOwner,
    /// A responder existed but the reply never came: the owner went away mid-request. The bus
    /// answers `Timeout`, so the request ends within its deadline and never hangs (FR-015).
    OwnerLost,
}

impl AgentUnavailableReason {
    /// How the reason reads in an error message.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoOwner => "no owner",
            Self::OwnerLost => "owner lost",
        }
    }
}

impl std::fmt::Display for AgentUnavailableReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How long an operation that answers from the agent's *current* state waits for its
/// owner. A lost owner is noticed within this (FR-015, SC-006): the request ends with
/// the unavailable error instead of hanging.
pub const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// How long an operation that *changes* the agent waits. A build can pull a model, so
/// this is the deadline the manager's own commands have today; shortening it would
/// refuse an update that is still on its way (FR-021).
pub const CHANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// The marker a refusal carries, so the service layer can tell a refusal - the `400` it
/// has always been - from a failure to reach the agent.
pub const REFUSAL_MARKER: &str = "agent_refused";

/// The refusal inside a message, if that is what it is.
pub fn refusal(message: &str) -> Option<String> {
    let rest = message.strip_prefix(REFUSAL_MARKER)?;
    Some(rest.strip_prefix(": ").unwrap_or(rest).to_string())
}

/// A refusal, as the seam reports it.
fn refuse(message: impl Into<String>) -> VizierError {
    VizierError(format!("{REFUSAL_MARKER}: {}", message.into()))
}

/// The one failure the seam raises for an agent it cannot reach.
///
/// The kind is the one the WebUI and the channels read today - [`ErrorKind::AgentUnavailable`] -
/// so nothing new reaches the person. The reason is carried in the message, and
/// [`AgentUnavailable::parse`] reads it back, so a caller can tell the two reasons apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentUnavailable {
    pub agent_id: String,
    pub reason: AgentUnavailableReason,
}

impl AgentUnavailable {
    /// The marker every availability message starts with.
    pub const MARKER: &'static str = "agent_unavailable";

    pub fn new(agent_id: impl Into<String>, reason: AgentUnavailableReason) -> Self {
        Self { agent_id: agent_id.into(), reason }
    }

    /// The kind the WebUI and the channels read today.
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::AgentUnavailable
    }

    /// The message this failure travels as.
    pub fn message(&self) -> String {
        format!("{}: {}: {}", Self::MARKER, self.agent_id, self.reason)
    }

    /// The availability failure inside a message, if that is what it is.
    pub fn parse(message: &str) -> Option<Self> {
        let rest = message.strip_prefix(Self::MARKER)?.strip_prefix(": ")?;
        let (agent_id, reason) = rest.rsplit_once(": ")?;
        let reason = match reason {
            "no owner" => AgentUnavailableReason::NoOwner,
            "owner lost" => AgentUnavailableReason::OwnerLost,
            _ => return None,
        };
        Some(Self { agent_id: agent_id.to_string(), reason })
    }
}

impl std::fmt::Display for AgentUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "agent '{}' is unavailable: {}", self.agent_id, self.reason)
    }
}

impl std::error::Error for AgentUnavailable {}

impl From<AgentUnavailable> for VizierError {
    fn from(value: AgentUnavailable) -> Self {
        VizierError(value.message())
    }
}

/// The availability failure a bus failure means.
///
/// The bus already draws the line: `NoResponders` is "a request found nobody subscribed" and
/// `Timeout` is "a responder existed but did not answer in time"
/// (`crates/vizier-schema/src/error.rs`). The seam inherits the distinction, which is what
/// makes "no owner" fail at once and "owner lost" end within the deadline.
pub fn availability(agent_id: &str, error: vizier_schema::BusError) -> VizierError {
    let reason = match error {
        vizier_schema::BusError::NoResponders => AgentUnavailableReason::NoOwner,
        vizier_schema::BusError::Timeout => AgentUnavailableReason::OwnerLost,
        other => return VizierError(other.to_string()),
    };
    AgentUnavailable::new(agent_id, reason).into()
}

/// The one way the gateway side reaches an agent.
///
/// Every operation returns [`crate::Result`], so there is no bespoke error type
/// (Principle V). The trait is object-safe and is held as `Arc<dyn AgentsClient>` on
/// `VizierDependencies`, so a test can put a fake there without touching the engine.
#[async_trait]
pub trait AgentsClient: Send + Sync {
    /// Send a request to an agent and receive its frames until the turn ends.
    async fn request(
        &self,
        agent_id: &str,
        session: &VizierSession,
        request: VizierRequest,
    ) -> Result<AgentTurn>;

    /// Abort the turn running in a session.
    async fn abort(&self, agent_id: &str, session: &VizierSession) -> Result<()>;

    /// Create an agent. It has no owner yet: the fleet places it.
    async fn create(&self, agent_id: &str, config: AgentConfig) -> Result<AgentSummary>;

    /// Replace an agent's configuration. The running turns are resumed.
    async fn update(&self, agent_id: &str, config: AgentConfig) -> Result<AgentSummary>;

    /// Delete an agent. The running turns are dropped.
    async fn delete(&self, agent_id: &str, delete_workspace: bool) -> Result<AgentSummary>;

    /// Stop an agent and start it again from its stored configuration.
    async fn restart(&self, agent_id: &str) -> Result<AgentSummary>;

    /// That one agent's health.
    async fn health(&self, agent_id: &str) -> Result<AgentHealthStatus>;

    /// That agent's view of its attached integrations, by integration id.
    async fn integration_status(&self, agent_id: &str)
        -> Result<HashMap<String, IntegrationStatus>>;

    /// Ask the agent to rebuild its integration tools, without a restart.
    async fn refresh_integrations(&self, agent_id: &str) -> Result<()>;
}

/// The seam's local implementation: it reaches the agent manager over the in-process bus.
///
/// It holds no reference to the agent manager and reads nothing from it, so the path it takes
/// is the path a networked implementation takes (research R2). The subjects are the ones in
/// the contract: `agent.{id}.req` for a turn, `agent.{id}.cmd` for a command about one agent,
/// and the fleet-wide `agents.cmd` for a create.
pub struct LocalAgentsClient {
    transport: VizierTransport,
}

impl LocalAgentsClient {
    pub fn new(transport: VizierTransport) -> Self {
        Self { transport }
    }

    /// A bus failure, as the seam's own vocabulary: the two reasons the contract tells apart
    /// come back as themselves, and everything else keeps its message.
    fn failure(&self, agent_id: &str, error: anyhow::Error) -> VizierError {
        match error.downcast::<vizier_schema::BusError>() {
            Ok(bus) => availability(agent_id, bus),
            Err(error) => {
                let text = error.to_string();
                if text.contains("not registered") {
                    // The agent has no request queue here: nobody holds it.
                    AgentUnavailable::new(agent_id, AgentUnavailableReason::NoOwner).into()
                } else {
                    VizierError(text)
                }
            }
        }
    }

    /// The summary a command answered with, or the failure it answered with.
    fn summary(
        answer: std::result::Result<AgentCommandResult, tokio::sync::oneshot::error::RecvError>,
        agent_id: &str,
    ) -> Result<AgentSummary> {
        match answer {
            Ok(AgentCommandResult::Ok(summary)) => Ok(summary),
            Ok(AgentCommandResult::Error(message)) => Err(refuse(message)),
            // The manager took the command and went away without answering.
            Err(_) => Err(AgentUnavailable::new(agent_id, AgentUnavailableReason::OwnerLost).into()),
        }
    }
}

#[async_trait]
impl AgentsClient for LocalAgentsClient {
    async fn request(
        &self,
        agent_id: &str,
        session: &VizierSession,
        request: VizierRequest,
    ) -> Result<AgentTurn> {
        let (tx, stream) = self
            .transport
            .reply_channel()
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        self.transport
            .send_request(session.clone(), request, Some(tx))
            .await
            .map_err(|e| self.failure(agent_id, e))?;
        Ok(AgentTurn { source: TurnSource::Bus(stream) })
    }

    async fn abort(&self, agent_id: &str, session: &VizierSession) -> Result<()> {
        // The abort is the request that aborts a turn today: a `Command("abort")` on the
        // agent's request subject. No new message kind is needed for it to cross a process
        // boundary (research R4).
        let request = VizierRequest {
            timestamp: chrono::Utc::now(),
            user: "system".into(),
            content: VizierRequestContent::Command("abort".into()),
            metadata: serde_json::json!({}),
            ..Default::default()
        };
        self.transport
            .send_request(session.clone(), request, None)
            .await
            .map_err(|e| self.failure(agent_id, e))
    }

    async fn create(&self, agent_id: &str, config: AgentConfig) -> Result<AgentSummary> {
        let (resp, rx) = tokio::sync::oneshot::channel();
        self.transport
            .send_agent_command(AgentCommand::Create {
                agent_id: agent_id.to_string(),
                config,
                resp,
            })
            .await
            .map_err(|e| self.failure(agent_id, e))?;
        Self::summary(rx.await, agent_id)
    }

    async fn update(&self, agent_id: &str, config: AgentConfig) -> Result<AgentSummary> {
        let (resp, rx) = tokio::sync::oneshot::channel();
        self.transport
            .send_agent_command_to(
                agent_id,
                AgentCommand::Update { agent_id: agent_id.to_string(), config, resp },
                CHANGE_TIMEOUT,
            )
            .await
            .map_err(|e| self.failure(agent_id, e))?;
        Self::summary(rx.await, agent_id)
    }

    async fn delete(&self, agent_id: &str, delete_workspace: bool) -> Result<AgentSummary> {
        let (resp, rx) = tokio::sync::oneshot::channel();
        self.transport
            .send_agent_command_to(
                agent_id,
                AgentCommand::Delete { agent_id: agent_id.to_string(), delete_workspace, resp },
                CHANGE_TIMEOUT,
            )
            .await
            .map_err(|e| self.failure(agent_id, e))?;
        Self::summary(rx.await, agent_id)
    }

    async fn restart(&self, agent_id: &str) -> Result<AgentSummary> {
        let (resp, rx) = tokio::sync::oneshot::channel();
        self.transport
            .send_agent_command_to(
                agent_id,
                AgentCommand::Restart { agent_id: agent_id.to_string(), resp },
                CHANGE_TIMEOUT,
            )
            .await
            .map_err(|e| self.failure(agent_id, e))?;
        Self::summary(rx.await, agent_id)
    }

    async fn health(&self, agent_id: &str) -> Result<AgentHealthStatus> {
        let (resp, rx) = tokio::sync::oneshot::channel();
        self.transport
            .send_agent_command_to(
                agent_id,
                AgentCommand::AgentHealth { agent_id: agent_id.to_string(), resp },
                REPLY_TIMEOUT,
            )
            .await
            .map_err(|e| self.failure(agent_id, e))?;
        rx.await
            .map_err(|_| AgentUnavailable::new(agent_id, AgentUnavailableReason::OwnerLost).into())
    }

    async fn integration_status(
        &self,
        agent_id: &str,
    ) -> Result<HashMap<String, IntegrationStatus>> {
        let (resp, rx) = tokio::sync::oneshot::channel();
        self.transport
            .send_agent_command_to(
                agent_id,
                AgentCommand::IntegrationStatus { agent_id: agent_id.to_string(), resp },
                REPLY_TIMEOUT,
            )
            .await
            .map_err(|e| self.failure(agent_id, e))?;
        // An agent that is not running has no integrations to report, which is what the
        // manager answers today.
        Ok(rx.await.unwrap_or_default())
    }

    async fn refresh_integrations(&self, agent_id: &str) -> Result<()> {
        self.transport
            .send_agent_command_to(
                agent_id,
                AgentCommand::RefreshIntegrations { agent_id: agent_id.to_string() },
                REPLY_TIMEOUT,
            )
            .await
            .map_err(|e| self.failure(agent_id, e))
    }
}

/// One call the fake was asked for.
#[derive(Debug, Clone)]
pub enum FakeCall {
    Request { agent_id: String, session: VizierSession, request: VizierRequest },
    Abort { agent_id: String, session: VizierSession },
    Create { agent_id: String, config: AgentConfig },
    Update { agent_id: String, config: AgentConfig },
    Delete { agent_id: String, delete_workspace: bool },
    Restart { agent_id: String },
    Health { agent_id: String },
    IntegrationStatus { agent_id: String },
    RefreshIntegrations { agent_id: String },
}

impl FakeCall {
    /// The agent the call was about.
    pub fn agent_id(&self) -> &str {
        match self {
            Self::Request { agent_id, .. }
            | Self::Abort { agent_id, .. }
            | Self::Create { agent_id, .. }
            | Self::Update { agent_id, .. }
            | Self::Delete { agent_id, .. }
            | Self::Restart { agent_id }
            | Self::Health { agent_id }
            | Self::IntegrationStatus { agent_id }
            | Self::RefreshIntegrations { agent_id } => agent_id,
        }
    }
}

#[derive(Default)]
struct FakeState {
    calls: Vec<FakeCall>,
    health: HashMap<String, AgentHealthStatus>,
    integrations: HashMap<String, HashMap<String, IntegrationStatus>>,
    turns: HashMap<String, Vec<VizierResponse>>,
    summaries: HashMap<String, AgentSummary>,
    failure: Option<String>,
}

/// A seam that records what it was asked and answers from a script.
///
/// The gateway-side test runs with **no agent host running** (FR-016): this is the second
/// implementation the trait has in this issue, beside the local one.
#[derive(Clone, Default)]
pub struct FakeAgentsClient {
    state: Arc<Mutex<FakeState>>,
}

impl FakeAgentsClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every call the fake was asked for, in order.
    pub fn calls(&self) -> Vec<FakeCall> {
        self.state.lock().expect("the fake's state").calls.clone()
    }

    /// The last call the fake was asked for.
    pub fn last_call(&self) -> Option<FakeCall> {
        self.state.lock().expect("the fake's state").calls.last().cloned()
    }

    /// What `health` answers for `agent_id`.
    pub fn set_health(&self, agent_id: &str, status: AgentHealthStatus) {
        self.state
            .lock()
            .expect("the fake's state")
            .health
            .insert(agent_id.to_string(), status);
    }

    /// What `integration_status` answers for `agent_id`.
    pub fn set_integration_status(
        &self,
        agent_id: &str,
        status: HashMap<String, IntegrationStatus>,
    ) {
        self.state
            .lock()
            .expect("the fake's state")
            .integrations
            .insert(agent_id.to_string(), status);
    }

    /// The frames `request` answers with.
    pub fn set_turn(&self, agent_id: &str, frames: Vec<VizierResponse>) {
        self.state
            .lock()
            .expect("the fake's state")
            .turns
            .insert(agent_id.to_string(), frames);
    }

    /// What `create`, `update`, `delete` and `restart` answer with.
    pub fn set_summary(&self, agent_id: &str, summary: AgentSummary) {
        self.state
            .lock()
            .expect("the fake's state")
            .summaries
            .insert(agent_id.to_string(), summary);
    }

    /// Every operation fails with this message. Use it to script an *unavailable* answer:
    /// `AgentUnavailable::new("alpha", AgentUnavailableReason::NoOwner).message()`.
    pub fn fail_with(&self, message: impl Into<String>) {
        self.state.lock().expect("the fake's state").failure = Some(message.into());
    }

    /// The answer a command gives, or the scripted failure.
    fn answer_summary(&self, agent_id: &str) -> Result<AgentSummary> {
        let state = self.state.lock().expect("the fake's state");
        if let Some(message) = &state.failure {
            return Err(VizierError(message.clone()));
        }
        Ok(state
            .summaries
            .get(agent_id)
            .cloned()
            .unwrap_or_else(|| default_summary(agent_id)))
    }
}

/// The summary the fake answers with when nothing else was scripted.
fn default_summary(agent_id: &str) -> AgentSummary {
    AgentSummary {
        agent_id: agent_id.to_string(),
        name: agent_id.to_string(),
        description: None,
        avatar_url: None,
        owner_username: None,
        owner_id: None,
        shared_to: Vec::new(),
    }
}

#[async_trait]
impl AgentsClient for FakeAgentsClient {
    async fn request(
        &self,
        agent_id: &str,
        session: &VizierSession,
        request: VizierRequest,
    ) -> Result<AgentTurn> {
        let frames = {
            let mut state = self.state.lock().expect("the fake's state");
            state.calls.push(FakeCall::Request {
                agent_id: agent_id.to_string(),
                session: session.clone(),
                request: request.clone(),
            });
            if let Some(message) = &state.failure {
                return Err(VizierError(message.clone()));
            }
            state.turns.get(agent_id).cloned().unwrap_or_default()
        };
        Ok(AgentTurn::finished(frames))
    }

    async fn abort(&self, agent_id: &str, session: &VizierSession) -> Result<()> {
        let mut state = self.state.lock().expect("the fake's state");
        state.calls.push(FakeCall::Abort {
            agent_id: agent_id.to_string(),
            session: session.clone(),
        });
        match &state.failure {
            Some(message) => Err(VizierError(message.clone())),
            None => Ok(()),
        }
    }

    async fn create(&self, agent_id: &str, config: AgentConfig) -> Result<AgentSummary> {
        self.state.lock().expect("the fake's state").calls.push(FakeCall::Create {
            agent_id: agent_id.to_string(),
            config,
        });
        self.answer_summary(agent_id)
    }

    async fn update(&self, agent_id: &str, config: AgentConfig) -> Result<AgentSummary> {
        self.state.lock().expect("the fake's state").calls.push(FakeCall::Update {
            agent_id: agent_id.to_string(),
            config,
        });
        self.answer_summary(agent_id)
    }

    async fn delete(&self, agent_id: &str, delete_workspace: bool) -> Result<AgentSummary> {
        self.state.lock().expect("the fake's state").calls.push(FakeCall::Delete {
            agent_id: agent_id.to_string(),
            delete_workspace,
        });
        self.answer_summary(agent_id)
    }

    async fn restart(&self, agent_id: &str) -> Result<AgentSummary> {
        self.state.lock().expect("the fake's state").calls.push(FakeCall::Restart {
            agent_id: agent_id.to_string(),
        });
        self.answer_summary(agent_id)
    }

    async fn health(&self, agent_id: &str) -> Result<AgentHealthStatus> {
        let mut state = self.state.lock().expect("the fake's state");
        state.calls.push(FakeCall::Health { agent_id: agent_id.to_string() });
        if let Some(message) = &state.failure {
            return Err(VizierError(message.clone()));
        }
        Ok(state.health.get(agent_id).cloned().unwrap_or_else(|| {
            AgentHealthStatus {
                agent_id: agent_id.to_string(),
                alive: false,
                live: false,
                state: crate::schema::AgentRunState::Stopped,
                last_error: None,
                failures: 0,
                next_retry_at: None,
                channels: Vec::new(),
                model: None,
                warnings: Vec::new(),
            }
        }))
    }

    async fn integration_status(
        &self,
        agent_id: &str,
    ) -> Result<HashMap<String, IntegrationStatus>> {
        let mut state = self.state.lock().expect("the fake's state");
        state
            .calls
            .push(FakeCall::IntegrationStatus { agent_id: agent_id.to_string() });
        if let Some(message) = &state.failure {
            return Err(VizierError(message.clone()));
        }
        Ok(state.integrations.get(agent_id).cloned().unwrap_or_default())
    }

    async fn refresh_integrations(&self, agent_id: &str) -> Result<()> {
        let mut state = self.state.lock().expect("the fake's state");
        state
            .calls
            .push(FakeCall::RefreshIntegrations { agent_id: agent_id.to_string() });
        match &state.failure {
            Some(message) => Err(VizierError(message.clone())),
            None => Ok(()),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{AgentRunState, VizierChannelId};

    fn session(agent: &str) -> VizierSession {
        VizierSession(agent.into(), VizierChannelId::System, None)
    }

    fn chat(text: &str) -> VizierRequest {
        VizierRequest {
            timestamp: chrono::Utc::now(),
            user: "tester".into(),
            content: VizierRequestContent::Chat(text.into()),
            metadata: serde_json::json!({}),
            ..Default::default()
        }
    }

    fn health(agent_id: &str) -> AgentHealthStatus {
        AgentHealthStatus {
            agent_id: agent_id.into(),
            alive: true,
            live: false,
            state: AgentRunState::Running,
            last_error: None,
            failures: 0,
            next_retry_at: None,
            channels: Vec::new(),
            model: None,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn an_availability_failure_reads_back_with_its_reason() {
        for reason in [
            AgentUnavailableReason::NoOwner,
            AgentUnavailableReason::OwnerLost,
        ] {
            let failure = AgentUnavailable::new("alpha", reason);
            let message = failure.message();
            assert!(message.starts_with(AgentUnavailable::MARKER));
            assert_eq!(failure.kind().as_str(), "agent_unavailable");
            let read = AgentUnavailable::parse(&message).expect("the reason survives the message");
            assert_eq!(read.agent_id, "alpha");
            assert_eq!(read.reason, reason);
        }
        assert!(AgentUnavailable::parse("something else").is_none());
    }

    #[test]
    fn a_refusal_reads_back_without_its_marker() {
        let error = refuse("agent 'alpha' already exists");
        assert_eq!(
            refusal(&error.0).as_deref(),
            Some("agent 'alpha' already exists")
        );
        assert!(refusal("a plain failure").is_none());
    }

    #[tokio::test]
    async fn the_fake_records_what_it_was_asked() {
        let fake = FakeAgentsClient::new();
        fake.set_health("alpha", health("alpha"));
        fake.set_summary("alpha", default_summary("alpha"));

        let status = fake.health("alpha").await.unwrap();
        assert_eq!(status.agent_id, "alpha");
        assert!(status.alive);

        let calls = fake.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].agent_id(), "alpha");
    }

    /// An agent's configuration, built the way the other tests build one.
    fn config() -> crate::schema::AgentConfig {
        serde_json::from_value(serde_json::json!({
            "name": "a", "owner_id": null, "system_prompt": null, "description": null,
            "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
            "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
            "prompt_timeout": "1m", "heartbeat_interval": "1m"
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn the_fake_carries_the_config_it_was_given() {
        let fake = FakeAgentsClient::new();
        let config = config();
        fake.update("alpha", config.clone()).await.unwrap();
        match fake.last_call().expect("a call") {
            FakeCall::Update { agent_id, config: got } => {
                assert_eq!(agent_id, "alpha");
                assert_eq!(got.model, config.model);
                assert_eq!(got.name, config.name);
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn the_fake_answers_a_turn_with_its_scripted_frames() {
        let fake = FakeAgentsClient::new();
        fake.set_turn(
            "alpha",
            vec![VizierResponse {
                content: crate::schema::VizierResponseContent::Message {
                    content: "one".into(),
                    stats: None,
                },
                ..Default::default()
            }],
        );
        let mut turn = fake.request("alpha", &session("alpha"), chat("hi")).await.unwrap();
        let frame = turn.recv().await.expect("a frame");
        assert!(matches!(
            frame.content,
            crate::schema::VizierResponseContent::Message { ref content, .. } if content == "one"
        ));
        assert!(turn.recv().await.is_none(), "the turn ended");
    }

    #[tokio::test]
    async fn a_request_to_an_agent_nobody_owns_fails_at_once() {
        // No owner: the bus answers `NoResponders`, so the failure is known at once and is
        // never a timeout (FR-014, SC-005).
        let transport = crate::transport::VizierTransport::new();
        let client = LocalAgentsClient::new(transport);
        let started = std::time::Instant::now();
        let error = client
            .health("ghost")
            .await
            .expect_err("an agent nobody owns is unavailable");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "the failure came at once, not after a timeout"
        );
        let failure = AgentUnavailable::parse(&error.0).expect("an availability failure");
        assert_eq!(failure.reason, AgentUnavailableReason::NoOwner);
        assert_eq!(failure.agent_id, "ghost");
    }

    #[tokio::test]
    async fn a_request_whose_owner_never_answers_ends_with_the_owner_lost() {
        // A responder that never answers: the bus times out and the failure says the owner
        // was lost, and the request never hangs (FR-015, SC-006).
        let transport = crate::transport::VizierTransport::new();
        let bus = transport.bus().clone();
        let mut sub = bus
            .messaging
            .subscribe(vizier_schema::SubjectPattern::parse("agent.alpha.cmd").unwrap())
            .await
            .unwrap();
        // The subscription exists but nothing ever answers on it.
        tokio::spawn(async move {
            let mut envelope = match sub.recv().await {
                Some((_, envelope)) => envelope,
                None => return,
            };
            // Hold the envelope: no reply is ever published.
            let _ = &mut envelope;
            std::future::pending::<()>().await;
        });

        let client = LocalAgentsClient::new(transport);
        let started = std::time::Instant::now();
        let error = client
            .health("alpha")
            .await
            .expect_err("a lost owner is unavailable");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "the request ended, it did not hang"
        );
        let failure = AgentUnavailable::parse(&error.0).expect("an availability failure");
        assert_eq!(failure.reason, AgentUnavailableReason::OwnerLost);
        assert_eq!(failure.agent_id, "alpha");
    }
}
