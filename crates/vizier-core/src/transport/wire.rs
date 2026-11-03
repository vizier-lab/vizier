//! A turn's request as it travels on `agent.{id}.req` (contracts/wire-messages.md).

use serde::{Deserialize, Serialize};
use vizier_schema::{BusError, Envelope, Subject};

use super::VizierRequestEnvelope;
use crate::agents::client::ClientBinding;
use crate::schema::{VizierRequest, VizierSession};

/// `Envelope.kind` of a turn's request.
pub const KIND_REQUEST: &str = "request";

pub const HEADER_TURN_ID: &str = "x-vizier-turn-id";
pub const HEADER_RESUMED: &str = "x-vizier-resumed";
pub const HEADER_TRACEPARENT: &str = "traceparent";
pub const HEADER_TRACESTATE: &str = "tracestate";

/// The request itself plus the fields `VizierRequest` keeps out of its own serde form because
/// they are never stored or accepted from a client.
#[derive(Serialize, Deserialize)]
struct RequestWire {
    session: VizierSession,
    request: VizierRequest,
    #[serde(default)]
    transient: bool,
    #[serde(default)]
    client: Option<ClientBinding>,
}

/// The request's envelope, addressed to reply at `reply_to`. The trace and the journal ids ride
/// as headers so a re-sent request keeps them.
pub fn encode_request(
    envelope: &VizierRequestEnvelope,
    reply_to: Option<Subject>,
) -> Result<Envelope, BusError> {
    let wire = RequestWire {
        session: envelope.session.clone(),
        transient: envelope.request.transient,
        client: envelope.request.client.clone(),
        request: envelope.request.clone(),
    };
    let mut out = Envelope::json(KIND_REQUEST, &wire)?;
    out.reply_to = reply_to;
    out.queued_at_ms = envelope.request.trace.enqueued_at_ms;
    if let Some(parent) = &envelope.request.trace.traceparent {
        out.headers.insert(HEADER_TRACEPARENT.into(), parent.clone());
    }
    if let Some(state) = &envelope.request.trace.tracestate {
        out.headers.insert(HEADER_TRACESTATE.into(), state.clone());
    }
    if let Some(id) = envelope.turn_id {
        out.headers.insert(HEADER_TURN_ID.into(), id.to_string());
    }
    if envelope.resumed {
        out.headers.insert(HEADER_RESUMED.into(), "1".into());
    }
    Ok(out)
}

/// The inverse of [`encode_request`], except for `response_tx`, which the receiver builds from
/// `envelope.reply_to` (the caller of this function does).
pub fn decode_request(envelope: &Envelope) -> Result<VizierRequestEnvelope, BusError> {
    let wire: RequestWire = envelope.decode()?;
    let mut request = wire.request;
    request.transient = wire.transient;
    request.client = wire.client;
    request.trace.traceparent = envelope.header(HEADER_TRACEPARENT).map(str::to_string);
    request.trace.tracestate = envelope.header(HEADER_TRACESTATE).map(str::to_string);
    request.trace.enqueued_at_ms = envelope.queued_at_ms;
    Ok(VizierRequestEnvelope {
        session: wire.session,
        request,
        response_tx: None,
        turn_id: envelope.header(HEADER_TURN_ID).and_then(|v| v.parse().ok()),
        resumed: envelope.header(HEADER_RESUMED).is_some(),
    })
}

// --- agent commands ---------------------------------------------------------------------------

use std::collections::HashMap;

use tokio::sync::oneshot;

use crate::schema::{
    AgentCommand, AgentCommandResult, AgentHealthStatus, IntegrationStatus, agent::AgentConfig,
};

/// `Envelope.kind` of an agent command request.
pub const KIND_AGENT_COMMAND: &str = "agent_command";
/// `Envelope.kind` of its reply.
pub const KIND_AGENT_REPLY: &str = "agent_reply";

/// An [`AgentCommand`] without the in-process slot its answer is read from: what travels on
/// `agents.cmd`. The answer comes back as the bus reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentCommandRequest {
    Create { agent_id: String, config: AgentConfig },
    Update { agent_id: String, config: AgentConfig },
    Delete { agent_id: String, delete_workspace: bool },
    HealthCheck,
    RefreshIntegrations { agent_id: String },
    IntegrationStatus { agent_id: String },
    Restart { agent_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentCommandReply {
    Result(AgentCommandResult),
    Health(Vec<AgentHealthStatus>),
    Status(HashMap<String, IntegrationStatus>),
    Done,
    /// The manager took the command and dropped its slot without answering.
    Dropped,
}

/// Where the answer to a command goes on the side that sent it.
pub enum Awaiting {
    Result(oneshot::Sender<AgentCommandResult>),
    Health(oneshot::Sender<Vec<AgentHealthStatus>>),
    Status(oneshot::Sender<HashMap<String, IntegrationStatus>>),
    Nothing,
}

impl Awaiting {
    /// Hand the bus reply to whoever is waiting on the command.
    pub fn fulfil(self, reply: AgentCommandReply) {
        match (self, reply) {
            (Self::Result(tx), AgentCommandReply::Result(r)) => {
                let _ = tx.send(r);
            }
            (Self::Health(tx), AgentCommandReply::Health(h)) => {
                let _ = tx.send(h);
            }
            (Self::Status(tx), AgentCommandReply::Status(s)) => {
                let _ = tx.send(s);
            }
            _ => {}
        }
    }

    /// The sender's slot is dropped unanswered: its receiver reads a closed channel, as it
    /// did when the manager stopped.
    pub fn abandon(self) {}
}

/// The wire form of `cmd`, and the slot its reply fills.
pub fn split_agent_command(cmd: AgentCommand) -> (AgentCommandRequest, Awaiting) {
    match cmd {
        AgentCommand::Create { agent_id, config, resp } => {
            (AgentCommandRequest::Create { agent_id, config }, Awaiting::Result(resp))
        }
        AgentCommand::Update { agent_id, config, resp } => {
            (AgentCommandRequest::Update { agent_id, config }, Awaiting::Result(resp))
        }
        AgentCommand::Delete { agent_id, delete_workspace, resp } => (
            AgentCommandRequest::Delete { agent_id, delete_workspace },
            Awaiting::Result(resp),
        ),
        AgentCommand::HealthCheck { resp } => (AgentCommandRequest::HealthCheck, Awaiting::Health(resp)),
        AgentCommand::RefreshIntegrations { agent_id } => {
            (AgentCommandRequest::RefreshIntegrations { agent_id }, Awaiting::Nothing)
        }
        AgentCommand::IntegrationStatus { agent_id, resp } => {
            (AgentCommandRequest::IntegrationStatus { agent_id }, Awaiting::Status(resp))
        }
        AgentCommand::Restart { agent_id, resp } => {
            (AgentCommandRequest::Restart { agent_id }, Awaiting::Result(resp))
        }
    }
}

/// What the manager waits on to answer a command it has taken off the bus.
pub enum Answer {
    Result(oneshot::Receiver<AgentCommandResult>),
    Health(oneshot::Receiver<Vec<AgentHealthStatus>>),
    Status(oneshot::Receiver<HashMap<String, IntegrationStatus>>),
    Nothing,
}

impl Answer {
    /// The reply to publish, once the command has been handled. `None` when the handler
    /// dropped its slot without answering.
    pub async fn into_reply(self) -> Option<AgentCommandReply> {
        match self {
            Self::Result(rx) => rx.await.ok().map(AgentCommandReply::Result),
            Self::Health(rx) => rx.await.ok().map(AgentCommandReply::Health),
            Self::Status(rx) => rx.await.ok().map(AgentCommandReply::Status),
            Self::Nothing => Some(AgentCommandReply::Done),
        }
    }
}

/// The in-process command for `request`, and the slot its answer will arrive in.
pub fn open_agent_command(request: AgentCommandRequest) -> (AgentCommand, Answer) {
    match request {
        AgentCommandRequest::Create { agent_id, config } => {
            let (resp, rx) = oneshot::channel();
            (AgentCommand::Create { agent_id, config, resp }, Answer::Result(rx))
        }
        AgentCommandRequest::Update { agent_id, config } => {
            let (resp, rx) = oneshot::channel();
            (AgentCommand::Update { agent_id, config, resp }, Answer::Result(rx))
        }
        AgentCommandRequest::Delete { agent_id, delete_workspace } => {
            let (resp, rx) = oneshot::channel();
            (AgentCommand::Delete { agent_id, delete_workspace, resp }, Answer::Result(rx))
        }
        AgentCommandRequest::HealthCheck => {
            let (resp, rx) = oneshot::channel();
            (AgentCommand::HealthCheck { resp }, Answer::Health(rx))
        }
        AgentCommandRequest::RefreshIntegrations { agent_id } => {
            (AgentCommand::RefreshIntegrations { agent_id }, Answer::Nothing)
        }
        AgentCommandRequest::IntegrationStatus { agent_id } => {
            let (resp, rx) = oneshot::channel();
            (AgentCommand::IntegrationStatus { agent_id, resp }, Answer::Status(rx))
        }
        AgentCommandRequest::Restart { agent_id } => {
            let (resp, rx) = oneshot::channel();
            (AgentCommand::Restart { agent_id, resp }, Answer::Result(rx))
        }
    }
}

// --- memory operations ------------------------------------------------------------------------

use crate::schema::{MemoryOpRequest, MemoryOpResponse};

/// `Envelope.kind` of a memory operation and of its reply.
pub const KIND_MEMORY_OP: &str = "memory_op";
pub const KIND_MEMORY_OP_REPLY: &str = "memory_op_reply";

/// The answer to a memory operation. Errors cross as text; the caller rebuilds its own error.
#[derive(Debug, Serialize, Deserialize)]
pub enum MemoryOpReply {
    Ok(MemoryOpResponse),
    Err { message: String },
}

// --- dreams and the command socket -----------------------------------------------------------

pub const KIND_DREAM_COMMAND: &str = "dream_command";
pub const KIND_COMMAND_REQUEST: &str = "command_request";
pub const KIND_COMMAND_RESPONSE: &str = "command_response";

// --- files ------------------------------------------------------------------------------------

use crate::schema::{FileRecord, VizierAttachment};

pub const KIND_FILE_COMMAND: &str = "file_command";
pub const KIND_FILE_REPLY: &str = "file_reply";

/// File bytes never ride on the bus (a payload is capped at 1 MiB and files are not): the
/// sender stages them in the blob store under `staged` and the message carries the key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileRequest {
    /// Store the staged bytes as an upload named `filename`.
    Upload { filename: String, staged: String },
    /// Read an attachment's bytes; the reply names where they were staged.
    Resolve { attachment: VizierAttachment },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileReply {
    Record(FileRecord),
    /// The attachment's bytes, staged under this key for the requester to read and delete.
    Staged { key: String },
    Err { message: String },
}
