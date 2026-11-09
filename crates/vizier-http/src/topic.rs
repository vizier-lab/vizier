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
    access::require,
    schema::{
        VizierAttachmentContent, VizierChannelId, VizierRequest, VizierRequestContent, VizierSession,
    },
    service::ServiceError,
    storage::agent::AgentStorage,
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

use crate::{
    api::v1::agents::channel::{
        ClientLimits, WebSocketReactionMessage, bind_client_tools, check_client_context, check_client_event,
        client_call_state, client_error, client_tool_result, question_error, spawn_delivery, webui_reaction,
    },
    state::HTTPState,
};

/// What the server decides about a person's message, whatever the client sent.
///
/// A message starts a fresh background chain, so the depth is never the client's to set. Its time is
/// the server's clock on receipt, on every channel: a client's own value would misorder history and
/// move the cutoff it is compared with (`specs/052-unify-local-timezone`). The field stays required
/// on input so existing clients keep working.
fn stamp_on_receipt(request: &mut vizier_core::schema::VizierRequest) {
    request.background_depth = 0;
    request.timestamp = vizier_core::time::now();
}

/// Whether this caller may open a live voice session (T109, FR-062).
///
/// A **scoped** API key must name the capability; an unscoped key carries its owner's permissions
/// and is not affected, which is what keeps every key made before scopes existed working. A person
/// signed in normally is not an API key at all and is allowed here — the surface is gated by the
/// agent's own `live_voice` switch, not by a permission a person would have to be granted.
/// Whether this caller owns that call (T110, FR-063).
///
/// The name alone is not enough: every API key presents an empty username, so two different keys —
/// of one person or of two — would look like the same owner and could interrupt and end each
/// other's session in a shared conversation. The key id is what separates them, and a person signed
/// in normally has none, so a person's session and a key's session can never be confused either way
/// round. Extracted from the socket arm so this rule can be asserted without an `HTTPState`.
fn owns_session(
    session: &vizier_core::live_voice::LiveVoiceSession,
    principal: &vizier_core::auth::Principal,
) -> bool {
    session.owner == principal.username
        && session.key_origin().map(|origin| origin.key_id)
            == principal.method.key_id().map(str::to_string)
}

/// Whether the key behind this caller has passed its expiry (T110, FR-063).
///
/// Only an API key can lapse; a person's session has no expiry to pass.
fn key_lapsed(principal: &vizier_core::auth::Principal) -> bool {
    match &principal.method {
        vizier_core::auth::AuthMethod::ApiKey { expires_at, .. } => {
            expires_at.is_some_and(|at| at <= vizier_core::time::now())
        }
        _ => false,
    }
}

fn may_start_live_voice(principal: &vizier_core::auth::Principal) -> bool {
    match principal.method {
        vizier_core::auth::AuthMethod::ApiKey { scoped: true, .. } => principal
            .permissions
            .iter()
            .any(|c| c == vizier_core::service::api_keys::CAPABILITY_LIVE_VOICE),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::{key_lapsed, may_start_live_voice, owns_session, stamp_on_receipt};
    use vizier_core::schema::{VizierRequest, VizierRequestContent};

    /// T109/FR-062: who may open a live voice session, and the three cases that matter.
    #[test]
    fn a_scoped_key_may_start_live_voice_only_when_it_names_the_capability() {
        let principal = |permissions: &[&str], method| vizier_core::auth::Principal {
            user_id: "olive".into(),
            username: "olive".into(),
            role: vizier_core::storage::user::Role {
                role_id: "r".into(),
                name: "role".into(),
                permissions: vec!["users:manage".into()],
                is_system: false,
                created_at: vizier_core::time::now(),
            },
            permissions: permissions.iter().map(|p| p.to_string()).collect(),
            method,
        };
        let api_key = |scoped| vizier_core::auth::AuthMethod::ApiKey {
            scoped,
            key_id: Some("k1".into()),
            expires_at: None,
        };

        // A scoped key that names the capability: allowed.
        assert!(may_start_live_voice(&principal(&["live_voice"], api_key(true))));
        // A scoped key with an empty scope, or one naming something else: refused.
        assert!(!may_start_live_voice(&principal(&[], api_key(true))));
        assert!(!may_start_live_voice(&principal(&["other"], api_key(true))));
        // An unscoped key keeps its owner's permissions and is not affected by this rule.
        assert!(may_start_live_voice(&principal(&[], api_key(false))));
        // A person signed in normally is not an API key and is allowed here.
        assert!(may_start_live_voice(&principal(
            &[],
            vizier_core::auth::AuthMethod::Jwt
        )));
    }

    /// T110: two keys on one conversation are two owners. Without the key id, both present an empty
    /// username and either could interrupt and end the other's call.
    #[test]
    fn only_the_key_that_opened_a_call_owns_it() {
        let transport = vizier_core::transport::VizierTransport::new();
        let (out_tx, _rx) = tokio::sync::mpsc::channel(8);
        let session = vizier_core::live_voice::LiveVoiceSession::new(
            vizier_core::schema::VizierSession(
                "helper".into(),
                vizier_core::schema::VizierChannelId::from_web("olive", "web"),
                Some("topic-1".into()),
            ),
            String::new(),
            None,
            vizier_core::schema::agent::LiveVoiceSettings::default(),
            vizier_core::agents::client::ClientConnection::new(out_tx),
            transport,
        );
        let who = |method| vizier_core::auth::Principal {
            user_id: "olive".into(),
            username: String::new(),
            role: vizier_core::storage::user::Role {
                role_id: "r".into(),
                name: "role".into(),
                permissions: vec![],
                is_system: false,
                created_at: vizier_core::time::now(),
            },
            permissions: vec![],
            method,
        };
        let key = |id: &str| {
            who(vizier_core::auth::AuthMethod::ApiKey {
                scoped: true,
                key_id: Some(id.into()),
                expires_at: None,
            })
        };

        // Opened with key A: key A owns it, key B does not.
        session.set_key_origin(Some(vizier_core::live_voice::KeyOrigin {
            key_id: "key-a".into(),
            expires_at: None,
        }));
        assert!(owns_session(&session, &key("key-a")));
        assert!(!owns_session(&session, &key("key-b")), "another key is not the owner");
        // A person signed in normally is not either, and never will be for a key's session.
        assert!(!owns_session(
            &session,
            &who(vizier_core::auth::AuthMethod::Jwt)
        ));

        // A person's own session: theirs, and no key's.
        session.set_key_origin(None);
        assert!(owns_session(&session, &who(vizier_core::auth::AuthMethod::Jwt)));
        assert!(!owns_session(&session, &key("key-a")));
    }

    /// T110/FR-063: only a key can lapse, and a lapsed key ends the call it opened.
    #[test]
    fn a_key_that_has_lapsed_is_noticed_and_a_persons_session_never_is() {
        let principal = |method| vizier_core::auth::Principal {
            user_id: "olive".into(),
            username: "olive".into(),
            role: vizier_core::storage::user::Role {
                role_id: "r".into(),
                name: "role".into(),
                permissions: vec![],
                is_system: false,
                created_at: vizier_core::time::now(),
            },
            permissions: vec![],
            method,
        };
        let key = |expires_at| vizier_core::auth::AuthMethod::ApiKey {
            scoped: true,
            key_id: Some("k1".into()),
            expires_at,
        };
        let past = vizier_core::time::now() - chrono::Duration::seconds(5);
        let future = vizier_core::time::now() + chrono::Duration::hours(1);

        assert!(key_lapsed(&principal(key(Some(past)))), "a key past its expiry has lapsed");
        assert!(!key_lapsed(&principal(key(Some(future)))), "a key with time left has not");
        assert!(!key_lapsed(&principal(key(None))), "a key that never expires never lapses");
        assert!(
            !key_lapsed(&principal(vizier_core::auth::AuthMethod::Jwt)),
            "a person's session has no expiry to pass"
        );
    }

    #[test]
    fn a_clients_own_time_and_depth_are_discarded() {
        let sent: VizierRequest = serde_json::from_value(serde_json::json!({
            "timestamp": "2001-01-01T00:00:00Z",
            "user": "alice",
            "content": { "chat": "hello" },
            "metadata": {}
        }))
        .unwrap();
        assert!(matches!(sent.content, VizierRequestContent::Chat(_)));
        let mut request = sent;
        request.background_depth = 3;

        let before = vizier_core::time::now();
        stamp_on_receipt(&mut request);

        assert!(request.timestamp >= before, "stamped with the server's time, not 2001");
        assert_eq!(request.background_depth, 0);
    }
}

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
    /// Where this process finds the socket a request names; the binding holds only its id.
    connections: vizier_core::agents::client::ClientConnections,
    /// The shared question service (`specs/042-agent-asking-question`): a submission is validated
    /// and claimed through it before a turn is queued.
    questions: Arc<vizier_core::questions::Questions>,
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
        };
        // The client tool calls' frames, as JSON values: the engine has no socket types.
        let (client_tx, client_rx) = mpsc::channel::<serde_json::Value>(64);
        // This connection's end of the client tool calls made to it. Closed with the
        // connection, which fails every call still waiting on it with `client_disconnected`.
        let connection = ClientConnection::new(client_tx);

        // Calls deferred while no client was here are this connection's to deliver first.
        spawn_delivery(&state.storage, &state.transport, &session, &connection, None);

        // A dialog still open survives a reload or a second tab: the asks are replayed from
        // storage, so nothing about a live ask is held only in the process that asked
        // (`specs/042-agent-asking-question` FR-048).
        if let Ok(asks) = vizier_core::storage::question::SensitiveAskStorage::sensitive_asks(
            &*state.storage,
            &session,
            vizier_core::questions::model::AskState::Open,
        )
        .await
        {
            for ask in asks {
                state
                    .transport
                    .publish_session_event(vizier_core::transport::SessionEvent {
                        session: session.clone(),
                        frame: vizier_core::transport::SessionFrame::SensitiveAsk(ask),
                    });
            }
        }

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
            connections: state.deps.client_connections.clone(),
            questions: state.deps.questions.clone(),
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
        self.connections.remove(&self.connection.id);
    }

    /// One live voice action (`specs/055-live-voice`).
    ///
    /// Everything here is authorised exactly like a chat message on the same socket: the session
    /// belongs to the principal that started it, and everybody else is told it is active rather
    /// than being allowed to end it (FR-005).
    #[cfg(feature = "live-voice")]
    pub async fn handle_live_voice(&self, key: &str, frame: serde_json::Value) {
        use vizier_core::live_voice::frame::{ClientAction, EndReason, LiveVoiceControl, SessionState};

        let Some(action) = ClientAction::parse(key, &frame) else {
            self.send_live_voice(LiveVoiceControl::Error {
                kind: EndReason::ProviderFailed,
                message: "the frame did not match the live voice contract".into(),
            });
            return;
        };
        let sessions = vizier_core::live_voice::sessions();

        // Every action but `start` needs the session the caller owns.
        // Who owns this call (T110). The name alone is not enough: every API key presents an empty
        // username, so two different keys — of the same person or of two people — would look like
        // the same owner and could interrupt and end each other's session in a shared conversation.
        // The key id is what separates them, and a person signed in normally has none, so their
        // session and a key's session can never be confused either way round.
        let owned = |session: &vizier_core::live_voice::LiveVoiceSession| {
            owns_session(session, &self.principal)
        };

        // T110/FR-063: a key past its expiry ends the call it opened. The door already refuses an
        // expired key — the storage lookup filters them — so what is left is a session opened while
        // the key was valid and still running when it lapses. Checked when a frame arrives rather
        // than by a timer, so the call ends at the next frame rather than on the exact second: the
        // alternative is a timer per session for a rare event, and a client that has gone quiet is
        // not talking to anyone anyway.
        if key_lapsed(&self.principal) {
            if let Some(session) = sessions.get(&self.session).filter(|s| owned(s)) {
                if session.key_origin().is_some() {
                    session.end(EndReason::KeyExpired);
                    sessions.remove(&self.session, &session);
                }
            }
        }

        match action {
            ClientAction::Start(start) => {
                // T102: the audio description is asked for, not assumed. This runs before the
                // registry is read or written and before any state changes, so a client asking for
                // a carrier this release does not speak leaves nothing behind — no session, no
                // half-open microphone, no state word.
                //
                // The kind is `ProviderFailed` for consistency with the other refusals in this
                // file, all of which are contract violations. It is a poor name for this one; a
                // dedicated `unsupported_audio` variant is the honest fix and is not added here
                // without the Lead's word.
                // T109/FR-062: a key scoped to live voice can start one, and a scoped key that
                // does not name it cannot. `None` is an unscoped key carrying its owner's
                // permissions, which is every key made before scopes existed; those are not
                // affected. The check is here rather than per-endpoint because a scoped key's
                // other permissions are already gone — only this surface is reachable by a key
                // whose whole purpose is to reach it.
                if !may_start_live_voice(&self.principal) {
                    self.send_live_voice(LiveVoiceControl::Error {
                        kind: EndReason::PermissionWithdrawn,
                        message: "this key is not allowed to use live voice".into(),
                    });
                    return;
                }
                let description = start.description();
                if let Err(reason) = description.check() {
                    self.send_live_voice(LiveVoiceControl::Error {
                        kind: EndReason::UnsupportedAudio,
                        message: reason,
                    });
                    return;
                }
                if let Some(existing) = sessions.get(&self.session) {
                    if owned(&existing) && !existing.is_stopped() {
                        // Idempotent: the same person pressing start on a reconnected socket
                        // re-binds the lane, which is what keeps the session from going silent
                        // (T067).
                        existing.rebind(self.connection.clone());
                        existing.set_state(SessionState::Listening);
                        return;
                    }
                    self.send_live_voice(LiveVoiceControl::Error {
                        kind: EndReason::ProviderFailed,
                        message: "a live voice session is already active on this conversation".into(),
                    });
                    return;
                }

                let config = match self.services.deps().storage.get_agent(&self.session.0).await {
                    Ok(Some(config)) => config,
                    _ => {
                        self.send_live_voice(LiveVoiceControl::Error {
                            kind: EndReason::AgentChanged,
                            message: "the agent is not available".into(),
                        });
                        return;
                    }
                };
                if !config.tools.live_voice.enabled {
                    // FR-041: with the switch off the frame is ignored and the socket stays open.
                    return;
                }
                let settings = config.tools.live_voice.settings.clone();
                let stt = settings.stt.clone().unwrap_or_else(|| config.tools.stt.settings.clone());
                let tts = settings.tts.clone().unwrap_or_else(|| config.tools.tts.settings.clone());
                let engines = match vizier_core::live_voice::CascadeEngines::new(
                    &stt,
                    &tts,
                    self.services.deps(),
                    &self.services.deps().config.workspace,
                    &self.session.0,
                )
                .await
                {
                    Ok(engines) => std::sync::Arc::new(engines)
                        as std::sync::Arc<dyn vizier_core::live_voice::LiveVoiceEngines>,
                    Err(e) => {
                        self.send_live_voice(LiveVoiceControl::Error {
                            kind: EndReason::ProviderFailed,
                            message: format!("the speech providers could not start: {e}"),
                        });
                        return;
                    }
                };
                // T095-T097 (FR-005): one call per person, across conversations. Starting here
                // moves the one that is running elsewhere instead of leaving two behind. This runs
                // after the engines are ready, so a start that cannot begin does not kill a call
                // that is already working.
                if let Some(previous) = sessions
                    .move_owned_elsewhere(
                        &self.session,
                        &self.principal.username,
                        self.principal.method.key_id(),
                    )
                    .await
                {
                    debug_assert_eq!(previous.state(), SessionState::Ended);
                }
                let language = stt.language.clone();
                let session = sessions
                    .start(self.session.clone(), || {
                        vizier_core::live_voice::LiveVoiceSession::new(
                            self.session.clone(),
                            self.principal.username.clone(),
                            language,
                            settings,
                            self.connection.clone(),
                            self.transport.clone(),
                        )
                    })
                    .unwrap_or_else(|existing| existing);
                session.set_engines(engines);
                session.set_description(description);
                // T110: remember which key opened this call, so revoking or lapsing it can end the
                // call later. `None` for a person signed in through the WebUI or the desktop app.
                session.set_key_origin(match &self.principal.method {
                    vizier_core::auth::AuthMethod::ApiKey { key_id, expires_at, .. } => {
                        Some(vizier_core::live_voice::KeyOrigin {
                            key_id: key_id.clone().unwrap_or_default(),
                            expires_at: *expires_at,
                        })
                    }
                    _ => None,
                });
                session.set_state(SessionState::Listening);
            }
            ClientAction::Audio(audio) => {
                let Some(session) = sessions.get(&self.session).filter(|s| owned(s)) else {
                    return;
                };
                // T015: a little over one chunk (~6.3 KB of base64) is the most a frame may carry.
                // Anything larger is refused outright rather than buffered, so a client cannot make
                // the server hold an unbounded utterance one frame at a time.
                // T102: a chunk that is not in the description the session declared is refused
                // rather than decoded as if it were. One bad frame is not a reason to take the
                // microphone away, so the session stays exactly as it was — listening.
                if let Err(reason) = session.description().accepts(audio.sample_rate, &audio.format)
                {
                    self.send_live_voice(LiveVoiceControl::Error {
                        kind: EndReason::UnsupportedAudio,
                        message: reason,
                    });
                    return;
                }
                if audio.base64.len() > vizier_core::live_voice::MAX_AUDIO_FRAME_BASE64 {
                    self.send_live_voice(LiveVoiceControl::Error {
                        kind: EndReason::ProviderFailed,
                        message: "an audio chunk was larger than one chunk of the contract".into(),
                    });
                    return;
                }
                let pcm = match base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    &audio.base64,
                ) {
                    Ok(pcm) => pcm,
                    Err(_) => return,
                };
                // The cap closes the utterance the way a spoken end would: the same decision runs,
                // so a long answer cannot leave the session capturing forever.
                if session.push_audio(&audio.utterance_id, &pcm) == Some(true) {
                    match session.close_utterance(&audio.utterance_id).await {
                        vizier_core::live_voice::UtteranceOutcome::Speak(utterance) => {
                            self.drive_utterance(session, utterance).await;
                        }
                        _ => {}
                    }
                }
            }
            ClientAction::UtteranceEnd(end) => {
                let Some(session) = sessions.get(&self.session).filter(|s| owned(s)) else {
                    return;
                };
                // The end of the utterance is the interruption (C8, FR-019): the reply stops and
                // its turn is cancelled only now, because while the person speaks the agent is
                // meant to keep speaking. A burst too short to be a word cancels nothing
                // (FR-024) — that decision lives in the session, not here.
                match session.close_utterance(&end.utterance_id).await {
                    vizier_core::live_voice::UtteranceOutcome::Unknown => {}
                    vizier_core::live_voice::UtteranceOutcome::TooShort => {
                        self.send_live_voice(LiveVoiceControl::Transcript {
                            utterance_id: end.utterance_id.clone(),
                            text: String::new(),
                            submitted: false,
                        });
                    }
                    vizier_core::live_voice::UtteranceOutcome::Speak(utterance) => {
                        self.drive_utterance(session, utterance).await;
                    }
                }
            }
            ClientAction::SpeechStart(start) => {
                let Some(session) = sessions.get(&self.session).filter(|s| owned(s)) else {
                    return;
                };
                // The start of speech interrupts nothing (C8): it opens the utterance the audio
                // that follows belongs to, and the agent keeps talking. The interruption happens
                // when this utterance ends, in the arm above.
                session.open_utterance(&start.utterance_id);
            }
            ClientAction::Interrupt(interrupt) => {
                let Some(session) = sessions.get(&self.session).filter(|s| owned(s)) else {
                    return;
                };
                // C7 deferred the interrupt *control* to the backlog, so a `button` reason is a
                // client that has not caught up: it is treated exactly like the spoken
                // interruption rather than as a third behaviour. `typed` remains, for a surface
                // that still has a text field: it stops the audio and the session stays live
                // (FR-025). With nothing running either costs nothing (FR-021).
                let reason = match interrupt.reason {
                    Some(vizier_core::live_voice::InterruptReason::Typed) => {
                        vizier_core::live_voice::InterruptReason::Typed
                    }
                    _ => vizier_core::live_voice::InterruptReason::BargeIn,
                };
                session.abort_turn(reason).await;
            }
            ClientAction::Stop => {
                let Some(session) = sessions.get(&self.session).filter(|s| owned(s)) else {
                    return;
                };
                session.end(EndReason::PressedAgain);
                sessions.remove(&self.session, &session);
            }
        }
    }

    #[cfg(feature = "live-voice")]
    /// Recognise one closed utterance and, if it said anything, drive a spoken turn.
    async fn drive_utterance(
        &self,
        session: std::sync::Arc<vizier_core::live_voice::LiveVoiceSession>,
        utterance: vizier_core::live_voice::Utterance,
    ) {
        use vizier_core::live_voice::frame::{EndReason, LiveVoiceControl};

        if utterance.too_short() {
            // FR-024: the audio stopped, nothing is submitted, the session listens again.
            self.send_live_voice(LiveVoiceControl::Transcript {
                utterance_id: utterance.id.clone(),
                text: String::new(),
                submitted: false,
            });
            return;
        }
        let Some(engines) = session.engines() else {
            return;
        };
        let text = match engines
            .transcribe(&utterance.wav(), &utterance.filename(), session.language.as_deref())
            .await
        {
            Ok(text) => text,
            Err(e) => {
                self.send_live_voice(LiveVoiceControl::Error {
                    kind: EndReason::ProviderFailed,
                    message: format!("recognition failed: {e}"),
                });
                return;
            }
        };
        if text.is_empty() {
            // FR-031: told, nothing submitted, listening again.
            self.send_live_voice(LiveVoiceControl::Error {
                kind: EndReason::Unintelligible,
                message: "that was not intelligible; say it again".into(),
            });
            return;
        }
        self.send_live_voice(LiveVoiceControl::Transcript {
            utterance_id: utterance.id.clone(),
            text: text.clone(),
            submitted: true,
        });
        let engines = engines.clone();
        tokio::spawn(async move { session.run_turn(engines, text).await });
    }

    #[cfg(feature = "live-voice")]
    /// Control frames go to this socket straight away (they are also on the broadcast, which is
    /// how the other tabs learn about the session).
    fn send_live_voice(&self, control: vizier_core::live_voice::LiveVoiceControl) {
        let json = serde_json::json!({ "live_voice": control });
        if let Err(e) = self.out.try_send(json) {
            tracing::debug!("live voice control frame not queued: {e}");
        }
    }

    pub async fn handle_inbound(&self, frame: serde_json::Value) {
        let send_frame = |frame: serde_json::Value| {
            let out = self.out.clone();
            async move {
                let _ = out.send_async(frame).await;
            }
        };

        #[cfg(feature = "live-voice")]
        // Live voice takes its own branch **before** the frame is stringified below: an audio
        // chunk is 4.3 KB and arrives ten times a second, and copying it once more per chunk on
        // the way in is a cost with nothing to show for it (`review.md` D5).
        if let Some(key) = vizier_core::live_voice::frame::client_key(&frame) {
            self.handle_live_voice(key, frame).await;
            return;
        }

        {
                        let text_str = frame.to_string();
                        let frame = Some(frame);
                        let client_frame = |key: &str| frame.as_ref().and_then(|f| f.get(key)).filter(|p| p.is_object());

                        if let Some(payload) = client_frame("sensitive_input_answer") {
                            let ask_id = payload.get("ask_id").and_then(|v| v.as_str()).unwrap_or_default();
                            let reply = if payload.get("decline").and_then(|v| v.as_bool()) == Some(true) {
                                Some(vizier_core::questions::AskReply::Declined)
                            } else {
                                match payload.get("value").and_then(|v| v.as_str()) {
                                    // Never trimmed or normalised: the value is what the person
                                    // typed (spec 042 FR-037). Only emptiness is refused.
                                    Some(value) if value.trim().is_empty() => {
                                        send_frame(client_error(
                                            "sensitive_value_empty",
                                            None,
                                            "a value is required; dismiss the dialog to decline",
                                        ))
                                        .await;
                                        return;
                                    }
                                    Some(value) if value.len() > vizier_core::questions::limits::MAX_SENSITIVE_VALUE_BYTES => {
                                        send_frame(client_error(
                                            "sensitive_value_too_large",
                                            None,
                                            format!(
                                                "the value is {} bytes; the limit is {}",
                                                value.len(),
                                                vizier_core::questions::limits::MAX_SENSITIVE_VALUE_BYTES
                                            ),
                                        ))
                                        .await;
                                        return;
                                    }
                                    Some(value) => Some(vizier_core::questions::AskReply::Value(value.to_string())),
                                    None => None,
                                }
                            };
                            // Only an ask open in *this* conversation can be answered from this socket: an
                            // id from another conversation reads as not open (contract §4).
                            let here = self
                                .questions
                                .open_asks(&self.session)
                                .iter()
                                .any(|open| open == ask_id);
                            let answered = here
                                && reply.is_some_and(|reply| self.questions.answer(ask_id, reply));
                            if !answered {
                                send_frame(client_error(
                                    "sensitive_ask_not_open",
                                    Some(ask_id),
                                    "this dialog is no longer open",
                                ))
                                .await;
                            }
                        } else if let Some(payload) = client_frame("client_tool_result") {
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
                            stamp_on_receipt(&mut request);
                            let Some(channel_id) = self.session.1.web_channel_id() else {
                                if request.model_override.is_some() {
                                    send_frame(client_error(
                                        "model_override_unsupported",
                                        None,
                                        "model override is supported only for WebUI chat channels",
                                    ))
                                    .await;
                                }
                                return;
                            };
                            if request.model_override.is_some() {
                                let supported_channel = self
                                    .session
                                    .1
                                    .web_channel_id()
                                    .is_some_and(|channel| {
                                        channel == "vizier-webui"
                                            || vizier_core::projects::channel::is_project_channel(&channel)
                                    });
                                let supported_kind = matches!(
                                    &request.content,
                                    VizierRequestContent::Chat(_) | VizierRequestContent::AudioChat(..)
                                );
                                if !supported_channel || !supported_kind {
                                    send_frame(client_error(
                                        "model_override_unsupported",
                                        None,
                                        "model override is supported only for WebUI chat and audio chat",
                                    ))
                                    .await;
                                    return;
                                }
                                let principal = match self
                                    .services
                                    .resolve(&self.principal.user_id, self.principal.method.clone())
                                    .await
                                {
                                    Ok(principal) => principal,
                                    Err(error) => {
                                        send_frame(client_error("model_override_forbidden", None, error.message()))
                                            .await;
                                        return;
                                    }
                                };
                                if let Err(error) = require(&principal, "model_override") {
                                    send_frame(client_error("model_override_forbidden", None, error.message()))
                                        .await;
                                    return;
                                }
                                if let Some(choice) = &request.model_override
                                    && let Err(error) = vizier_core::service::catalogue::validate_chat_model_override(
                                        self.services.deps(),
                                        choice,
                                    )
                                    .await
                                {
                                    send_frame(client_error(
                                        error.code().unwrap_or("model_override_unavailable"),
                                        None,
                                        error.message(),
                                    ))
                                    .await;
                                    return;
                                }
                            }
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
                            if self.session.1.project_id().is_some() {
                                let current_principal = if request.model_override.is_some() {
                                    match self.services.resolve(&self.principal.user_id, self.principal.method.clone()).await {
                                        Ok(principal) => principal,
                                        Err(error) => {
                                            send_frame(client_error("forbidden", None, error.message())).await;
                                            return;
                                        }
                                    }
                                } else {
                                    self.principal.clone()
                                };
                                let check = async {
                                    let config = vizier_core::service::topics::authorize_view(
                                        &self.services,
                                        &current_principal,
                                        &self.session.0,
                                    )
                                    .await?;
                                    vizier_core::service::topics::authorize_project_channel(
                                        &self.services,
                                        &current_principal,
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

                            // What the client says it can render: checked here, like the tool
                            // declarations, so a malformed note is refused before a turn starts.
                            if let Err(error) = check_client_context(&mut request) {
                                send_frame(error).await;
                                return;
                            }

                            // Refused before anything is sent, so no turn starts (FR-002).
                            if let Err(error) = bind_client_tools(&mut request, &self.connection, &self.connections) {
                                send_frame(error).await;
                                return;
                            }
                            if let VizierRequestContent::ClientEvent(event) = &mut request.content
                                && let Err(error) = check_client_event(&self.storage, &self.session, &self.limits, event).await
                            {
                                send_frame(error).await;
                                return;
                            }
                            // A question answer is claimed before the turn is queued, so a
                            // second tab finds the set closed instead of starting a duplicate
                            // turn (spec 042 FR-017).
                            if matches!(request.content, VizierRequestContent::QuestionAnswer(_))
                            {
                                let VizierRequestContent::QuestionAnswer(answer) = request.content
                                else {
                                    unreachable!("just matched a question answer")
                                };
                                let set_id = answer.set_id.clone();
                                match vizier_core::questions::submit::submit(
                                    &self.storage,
                                    &self.transport,
                                    &self.questions,
                                    &self.session,
                                    answer,
                                    &self.principal.username,
                                )
                                .await
                                {
                                    Ok(prepared) => request = prepared,
                                    Err(error) => {
                                        send_frame(question_error(&set_id, &error)).await;
                                        return;
                                    }
                                }
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

                            let (response_tx, response_rx) = match self.transport.reply_channel().await {
                                Ok(pair) => pair,
                                Err(_) => return,
                            };
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
        // Audio frames this socket dropped in the current stall episode (T021, T042).
        let mut dropped: u64 = 0;
        loop {
            tokio::select! {
                Some(frame) = self.client_rx.recv() => {
                    // The one place a live voice frame may be dropped (`specs/055-live-voice`
                    // contract §Delivery path): **audio**, and only audio, when this socket is
                    // behind. Every other frame — `live_voice_state`, `transcript`, `error`,
                    // `stall`, a reply's `Response` — is written whatever the depth, so a reply's
                    // text can never be lost to a session that is talking too fast.
                    //
                    // `dropped` is this loop's own counter: the session keeps none (T042), so
                    // there is exactly one number and exactly one report.
                    let mut write = true;
                    #[cfg(feature = "live-voice")]
                    if vizier_core::live_voice::frame::is_speech_frame(&frame) {
                        if self.out.len() > vizier_core::live_voice::LIVE_VOICE_SOCKET_BACKLOG {
                            dropped += 1;
                            write = false;
                        } else if dropped > 0 {
                            self.transport.publish_session_event(SessionEvent {
                                session: self.session.clone(),
                                frame: SessionFrame::LiveVoice(
                                    vizier_core::live_voice::LiveVoiceControl::Stall { dropped },
                                ),
                            });
                            dropped = 0;
                        }
                    }
                    if write && self.out.send_async(frame).await.is_err() {
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
                                spawn_delivery(&self.storage, &self.transport, &self.session, &self.connection, Some(vec![call_id]));
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
                            SessionFrame::QuestionSet { set_id, state, answers, answerer } => {
                                Some(serde_json::json!({
                                    "question_set": {
                                        "set_id": set_id,
                                        "state": state,
                                        "answers": answers,
                                        "answerer": answerer,
                                    }
                                }))
                            }
                            // The dialog carries the ask and never a value (spec 042 FR-044).
                            SessionFrame::SensitiveAsk(ask) => Some(serde_json::json!({
                                "sensitive_ask": {
                                    "ask_id": ask.ask_id,
                                    "label": ask.label,
                                    "purpose": ask.purpose,
                                    // What a person may submit: the redactor's own bounds, so the
                                    // dialog can warn before submission, not after.
                                    "protected": true,
                                    "min_bytes": vizier_core::questions::limits::MIN_SENSITIVE_VALUE_BYTES,
                                    "max_bytes": vizier_core::questions::limits::MAX_SENSITIVE_VALUE_BYTES,
                                    "expires_at": ask.requested_at
                                        + chrono::Duration::from_std(
                                            vizier_core::questions::limits::ASK_WAIT
                                        ).unwrap_or_default(),
                                }
                            })),
                            SessionFrame::SensitiveAskClosed { ask_id, outcome } => {
                                Some(serde_json::json!({
                                    "sensitive_ask_closed": { "ask_id": ask_id, "outcome": outcome }
                                }))
                            }
                            // Live voice **control** only (`specs/055-live-voice`, contract
                            // §Delivery path): `state`, `transcript`, `error`, `stall`. Audio is
                            // never here — `live_voice_speech` is written per connection by the
                            // session, and the payload type makes sending it here a compile error.
                            // The same serialiser serves both lanes, so their field names cannot
                            // drift (`analysis.md` M15).
                            #[cfg(feature = "live-voice")]
                            SessionFrame::LiveVoice(control) => serde_json::to_value(&control)
                                .ok()
                                .map(|control| serde_json::json!({ "live_voice": control })),
                        };
                        if let Some(json) = json {
                            if self.out.send_async(json).await.is_err() { return; }
                        }
                    }
                    // Another session's event, or one this socket does not handle.
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        // A lagging subscriber cannot be repaired mid-burst, so this arm still
                        // drops the frames it skipped. What it can no longer drop is live voice
                        // **audio**: `live_voice_speech` never enters this ring (it is written to
                        // the owning socket's own queue, contract §Delivery path), so a lost
                        // `SessionFrame::Response` — the reply text — can never be caused by a
                        // session talking. The client re-reads the job list on reconnect, so a
                        // dropped job frame never leaves the tray wrong for good.
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
