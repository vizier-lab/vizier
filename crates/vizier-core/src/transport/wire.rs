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
