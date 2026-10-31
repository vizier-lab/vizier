//! One person's live connection to one conversation, independent of how frames travel.
//!
//! The WebSocket handler and the desktop app's IPC both drive a [`TopicConnection`]: frames
//! the client sends go to [`TopicConnection::handle_inbound`], and everything the engine
//! pushes (a turn's frames, background jobs, reactions, client tool calls) arrives on the
//! `out` channel given to [`TopicConnection::open`].

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use vizier_core::{
    agents::client::ClientConnection,
    auth::Principal,
    schema::{
        VizierAttachmentContent, VizierChannelId, VizierRequest, VizierRequestContent, VizierSession,
    },
    service::ServiceError,
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

use crate::{
    api::v1::agents::channel::{
        ClientLimits, WebSocketReactionMessage, bind_client_tools, check_client_event, client_call_state,
        client_error, client_tool_result, spawn_delivery, webui_reaction,
    },
    state::HTTPState,
};

pub struct TopicConnection {
    session: VizierSession,
    /// The caller and the service layer, kept so every inbound turn re-checks the project
    /// permission (spec 040 FR-018: revocation applies to the next turn, not the next socket).
    principal: Principal,
    services: vizier_core::service::Services,
    transport: VizierTransport,
    storage: Arc<vizier_core::storage::VizierStorage>,
    limits: ClientLimits,
    connection: Arc<ClientConnection>,
    client_rx: Option<mpsc::Receiver<serde_json::Value>>,
    events: Option<broadcast::Receiver<SessionEvent>>,
    out: flume::Sender<serde_json::Value>,
}

/// The engine-to-client half of a [`TopicConnection`]; `run` ends when `out` is dropped.
pub struct TopicOutbound {
    session: VizierSession,
    transport: VizierTransport,
    storage: Arc<vizier_core::storage::VizierStorage>,
    limits: ClientLimits,
    connection: Arc<ClientConnection>,
    client_rx: mpsc::Receiver<serde_json::Value>,
    events: broadcast::Receiver<SessionEvent>,
    out: flume::Sender<serde_json::Value>,
}

/// At most ten relative, non-empty mention paths of at most 1,024 bytes each.
fn validate_mentions(mentions: &[String]) -> Result<(), String> {
    if mentions.len() > 10 {
        return Err("at most 10 files can be mentioned in one message".into());
    }
    for mention in mentions {
        if mention.trim().is_empty() {
            return Err("a mention cannot be empty".into());
        }
        if mention.len() > 1024 {
            return Err("a mention path is too long".into());
        }
        if std::path::Path::new(mention).is_absolute() {
            return Err(format!("mention {mention:?} must be relative to the project folder"));
        }
    }
    Ok(())
}

impl TopicConnection {
    pub async fn open(
        state: &HTTPState,
        principal: &Principal,
        agent_id: &str,
        channel_id: &str,
        topic_id: &str,
        out: flume::Sender<serde_json::Value>,
    ) -> Result<Self, ServiceError> {
        let services = vizier_core::service::Services::new(state.deps.clone());
        vizier_core::service::topics::authorize_topic(&services, principal, agent_id, channel_id)
            .await?;

        let session = VizierSession(
            agent_id.to_string(),
            VizierChannelId::from_web(&principal.username, channel_id),
            Some(topic_id.to_string()),
        );
        let limits = ClientLimits {
            events: state.client_events.clone(),
            state_saves: state.client_state_saves.clone(),
            deferred: state.deferred_client_calls.clone(),
        };
        // The client tool calls' frames, as JSON values: the engine has no socket types.
        let (client_tx, client_rx) = mpsc::channel::<serde_json::Value>(64);
        // This connection's end of the client tool calls made to it. Closed with the
        // connection, which fails every call still waiting on it with `client_disconnected`.
        let connection = ClientConnection::new(client_tx);

        // Calls deferred while no client was here are this connection's to deliver first.
        spawn_delivery(&state.storage, &state.transport, &session, &connection, &limits.deferred, None);

        // Background job changes, reactions, and the frames of every turn on this session: the
        // ones a background report woke and the ones a person started from any connection,
        // this one included. A turn's frames are published only there, so nothing arrives twice.
        let events = state.transport.subscribe_session_events();

        Ok(Self {
            session,
            principal: principal.clone(),
            services,
            transport: state.transport.clone(),
            storage: state.storage.clone(),
            limits,
            connection,
            client_rx: Some(client_rx),
            events: Some(events),
            out,
        })
    }

    /// Takes the outbound half; call once, then drive `run()` next to `handle_inbound`.
    pub fn outbound(&mut self) -> Option<TopicOutbound> {
        Some(TopicOutbound {
            session: self.session.clone(),
            transport: self.transport.clone(),
            storage: self.storage.clone(),
            limits: self.limits.clone(),
            connection: self.connection.clone(),
            client_rx: self.client_rx.take()?,
            events: self.events.take()?,
            out: self.out.clone(),
        })
    }

    pub fn close(&self) {
        self.connection.close();
    }

    pub async fn handle_inbound(&self, frame: serde_json::Value) {
        let send_frame = |frame: serde_json::Value| {
            let out = self.out.clone();
            async move {
                let _ = out.send_async(frame).await;
            }
        };
        {
                        let text_str = frame.to_string();
                        let frame = Some(frame);
                        let client_frame = |key: &str| frame.as_ref().and_then(|f| f.get(key)).filter(|p| p.is_object());

                        if let Some(payload) = client_frame("client_tool_result") {
                            if let Err(error) = client_tool_result(&self.connection, payload) {
                                send_frame(error).await;
                            }
                        } else if let Some(payload) = client_frame("client_call_state") {
                            if let Err(error) = client_call_state(&self.storage, &self.transport, &self.session, &self.limits, payload).await {
                                send_frame(error).await;
                            }
                        } else if let Ok(reaction_msg) = serde_json::from_value::<WebSocketReactionMessage>(frame.clone().unwrap_or_default()) {
                            let payload = reaction_msg.reaction;
                            if let Err(message) = webui_reaction(&self.storage, &self.transport, &self.session, &payload).await {
                                let frame = serde_json::json!({
                                    "reaction_error": { "message_uid": payload.message_uid, "message": message }
                                });
                                send_frame(frame).await;
                            }
                        } else if let Ok(request) = serde_json::from_value::<VizierRequest>(frame.clone().unwrap_or_default()) {
                            let mut request = request.clone();
                            // A person's message starts a fresh chain; the depth is never
                            // the client's to set.
                            request.background_depth = 0;
                            // `@` mentions are a project-chat feature; elsewhere the field is
                            // ignored (contracts/http-api.md §6).
                            if self.session.1.project_id().is_some() {
                                if let Err(message) = validate_mentions(&request.mentions) {
                                    send_frame(client_error("invalid_mentions", None, message)).await;
                                    return;
                                }
                            } else {
                                request.mentions.clear();
                            }

                            // A project socket re-checks the caller's permission on every turn
                            // (FR-018), so a revocation between two frames refuses the next one.
                            let Some(channel_id) = self.session.1.web_channel_id() else {
                                return;
                            };
                            if self.session.1.project_id().is_some() {
                                let check = async {
                                    let config = vizier_core::service::topics::authorize_view(
                                        &self.services,
                                        &self.principal,
                                        &self.session.0,
                                    )
                                    .await?;
                                    vizier_core::service::topics::authorize_project_channel(
                                        &self.services,
                                        &self.principal,
                                        &config,
                                        &self.session.0,
                                        &channel_id,
                                    )
                                    .await
                                }
                                .await;
                                if let Err(e) = check {
                                    let code = if e.status() == 404 {
                                        "project_not_found"
                                    } else {
                                        "forbidden"
                                    };
                                    send_frame(client_error(code, None, e.message().to_string()))
                                        .await;
                                    return;
                                }
                            }

                            // Refused before anything is sent, so no turn starts (FR-002).
                            if let Err(error) = bind_client_tools(&mut request, &self.connection) {
                                send_frame(error).await;
                                return;
                            }
                            if let VizierRequestContent::ClientEvent(event) = &mut request.content
                                && let Err(error) = check_client_event(&self.storage, &self.session, &self.limits, event).await
                            {
                                send_frame(error).await;
                                return;
                            }
                            if request.attachments.iter().any(|attachment| {
                                matches!(
                                    attachment.content,
                                    VizierAttachmentContent::Artifact { .. }
                                )
                            }) {
                                send_frame(client_error(
                                    "invalid_attachment",
                                    None,
                                    "request attachments must be uploads; artifact references are created by the agent",
                                ))
                                .await;
                                return;
                            }
                            for attachment in request.attachments.iter_mut() {
                                match self.transport.send_file_resolve(attachment.clone()).await {
                                    Ok(content) => {
                                        match self.transport.send_file_upload(attachment.filename.clone(), content).await {
                                            Ok(file_record) => {
                                                attachment.content = VizierAttachmentContent::Local(file_record.url);
                                            }
                                            Err(e) => {
                                                tracing::error!("failed to upload resolved attachment: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("failed to resolve attachment: {}", e);
                                    }
                                }
                            }

                            let (response_tx, response_rx) = flume::unbounded();
                            let session = self.session.clone();
                            if let Err(err) = self.transport.send_request(session.clone(), request, Some(response_tx)).await {
                                tracing::error!("failed to send request: {}", err);
                                return;
                            }

                            // The turn's frames go to every socket on the session, not to
                            // this one: a person who leaves the page mid-turn and comes back
                            // (or has it open in another tab) still sees the rest of it. This
                            // socket receives them through `session_events` like the others.
                            let transport = self.transport.clone();
                            tokio::spawn(async move {
                                while let Ok(response) = response_rx.recv_async().await {
                                    transport.publish_session_event(SessionEvent {
                                        session: session.clone(),
                                        frame: SessionFrame::Response(response),
                                    });
                                }
                            });
                        } else {
                            tracing::debug!("unrecognized WebSocket message: {}", &text_str[..text_str.len().min(200)]);
                        }
                    
        }
    }
}

impl TopicOutbound {
    pub async fn run(mut self) {
        let mut events_open = true;
        loop {
            tokio::select! {
                Some(frame) = self.client_rx.recv() => {
                    if self.out.send_async(frame).await.is_err() {
                        return;
                    }
                }
            event = self.events.recv(), if events_open => {
                match event {
                    Ok(ev) if ev.session == self.session => {
                        let json = match ev.frame {
                            SessionFrame::Response(response) => serde_json::to_value(&response).ok(),
                            SessionFrame::Job(snapshot) => {
                                Some(serde_json::json!({ "background_job": snapshot }))
                            }
                            SessionFrame::Reactions { history_uid, reactions } => Some(
                                serde_json::json!({
                                    "reactions": { "message_uid": history_uid, "reactions": reactions }
                                })
                            ),
                            // The declaring socket already has the call itself, sent directly.
                            SessionFrame::ClientToolCall { connection_id, call } => (connection_id
                                != self.connection.id)
                                .then(|| call.wire(false)),
                            SessionFrame::ClientCallOutcome { call_id, outcome } => Some(
                                serde_json::json!({
                                    "client_tool_outcome": { "call_id": call_id, "outcome": outcome }
                                })
                            ),
                            SessionFrame::ClientCallDeferred { call_id } => {
                                spawn_delivery(&self.storage, &self.transport, &self.session, &self.connection, &self.limits.deferred, Some(vec![call_id]));
                                None
                            }
                            SessionFrame::HistoryEntry(entry) => {
                                Some(serde_json::json!({ "history_entry": entry }))
                            }
                            SessionFrame::File(file) => {
                                Some(serde_json::json!({ "session_file": { "file": file } }))
                            }
                            SessionFrame::Title { title } => {
                                Some(serde_json::json!({ "session_title": { "title": title } }))
                            }
                            SessionFrame::ClientCallState { call_id, state } => Some(
                                serde_json::json!({
                                    "client_call_state": { "call_id": call_id, "state": state }
                                })
                            ),
                        };
                        if let Some(json) = json {
                            if self.out.send_async(json).await.is_err() { return; }
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        // The client re-reads the job list on reconnect, so a dropped frame
                        // never leaves the tray wrong for good.
                        tracing::warn!("websocket for {:?} lagged, {} session event(s) dropped", self.session, skipped);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        events_open = false;
                    }
                }
            }

                else => return,
            }
        }
    }
}
