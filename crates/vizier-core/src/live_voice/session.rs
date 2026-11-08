//! One live voice session: the state word, the generation, the lane, and the turn it drives
//! (`specs/055-live-voice`).
//!
//! Nothing here is persisted. A session lives in memory, is owned by one person, and is reachable
//! through [`LiveVoiceSessions`] by (agent, conversation) so that a second tab on the same
//! conversation sees the same session rather than starting a second one (FR-005).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use tokio::sync::watch;

use crate::Result;
use crate::agents::client::ClientConnection;
use crate::live_voice::engines::SharedEngines;
use crate::live_voice::frame::{EndReason, LiveVoiceControl, LiveVoiceSpeech, SessionState};
use crate::live_voice::segmenter;
use crate::live_voice::utterance::Utterance;
use crate::schema::agent::LiveVoiceSettings;
use crate::schema::{VizierRequest, VizierRequestContent, VizierResponseContent, VizierSession};
use crate::transport::{SessionEvent, SessionFrame, VizierTransport};

/// The session's shared state. One per (agent, conversation), held by the registry.
pub struct LiveVoiceSession {
    pub session: VizierSession,
    /// Who may end it; everybody else is told it is active (FR-005).
    pub owner: String,
    /// The agent's own language, so recognition is asked in the language it is configured for
    /// (FR-037).
    pub language: Option<String>,
    pub settings: LiveVoiceSettings,
    state: Mutex<SessionState>,
    /// Bumped by every interruption; every audio frame carries it and both sides drop anything
    /// below their own value (FR-022).
    generation: watch::Sender<u64>,
    utterance: Mutex<Option<Utterance>>,
    /// The owning socket's lane. Re-bound by an idempotent `live_voice_start`, because a
    /// reconnecting socket is a new `ClientConnection` and the old one is closed (T067).
    lane: Mutex<Arc<ClientConnection>>,
    /// Whether a turn is running, so a second utterance waits rather than interleaves.
    busy: AtomicBool,
    stopped: AtomicBool,
    /// The recogniser and voice this session speaks through; set once at start (FR-036).
    engines: Mutex<Option<SharedEngines>>,
    transport: VizierTransport,
}

impl LiveVoiceSession {
    pub fn new(
        session: VizierSession,
        owner: String,
        language: Option<String>,
        settings: LiveVoiceSettings,
        lane: Arc<ClientConnection>,
        transport: VizierTransport,
    ) -> Arc<Self> {
        let (generation, _) = watch::channel(1u64);
        Arc::new(Self {
            session,
            owner,
            language,
            settings,
            state: Mutex::new(SessionState::Connecting),
            generation,
            utterance: Mutex::new(None),
            lane: Mutex::new(lane),
            busy: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            engines: Mutex::new(None),
            transport,
        })
    }

    /// Attach the engines once, at start.
    pub fn set_engines(&self, engines: SharedEngines) {
        *self.engines.lock() = Some(engines);
    }

    pub fn engines(&self) -> Option<SharedEngines> {
        self.engines.lock().clone()
    }

    pub fn state(&self) -> SessionState {
        *self.state.lock()
    }

    pub fn generation(&self) -> u64 {
        *self.generation.borrow()
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// Re-bind the lane to the socket that just reconnected, without disturbing the session.
    pub fn rebind(&self, lane: Arc<ClientConnection>) {
        *self.lane.lock() = lane;
    }

    /// The control frames ride the session broadcast, so a second tab learns the session exists
    /// and a reload sees the state word without a per-socket replay log.
    fn publish(&self, control: LiveVoiceControl) {
        self.transport.publish_session_event(SessionEvent {
            session: self.session.clone(),
            frame: SessionFrame::LiveVoice(control),
        });
    }

    pub fn set_state(&self, state: SessionState) {
        *self.state.lock() = state;
        let reason = match state {
            SessionState::Ended => Some(EndReason::PressedAgain),
            _ => None,
        };
        self.publish(LiveVoiceControl::state(state, self.generation(), reason));
    }

    /// Move to `waiting` and say so: this is FR-042's indicator, sent as soon as the turn is
    /// accepted and before any audio exists, so the person knows the agent is thinking rather
    /// than that the connection died.
    pub fn thinking(&self) {
        self.set_state(SessionState::Waiting);
    }

    pub fn fail(&self, reason: EndReason, message: impl Into<String>) {
        self.publish(LiveVoiceControl::Error { kind: reason, message: message.into() });
        *self.state.lock() = SessionState::Failed;
        self.publish(LiveVoiceControl::state(
            SessionState::Failed,
            self.generation(),
            Some(reason),
        ));
    }

    /// Stop everything and say why. Idempotent: the first reason wins, and a second call does
    /// nothing, so a closed socket and a pressed button cannot race into two `ended` frames.
    pub fn end(&self, reason: EndReason) {
        if self.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(mut utterance) = self.utterance.lock().take() {
            utterance.cancel();
        }
        *self.state.lock() = SessionState::Ended;
        self.publish(LiveVoiceControl::state(
            SessionState::Ended,
            self.generation(),
            Some(reason),
        ));
    }

    /// An interruption: stop the audio now, and make every in-flight segment stale.
    ///
    /// The order is fixed (FR-027): the generation moves first, so a segment still being
    /// synthesised is dropped rather than played after the person started talking.
    pub fn interrupt(&self) -> u64 {
        self.generation.send_modify(|g| *g += 1);
        let next = *self.generation.borrow();
        if let Some(mut utterance) = self.utterance.lock().take() {
            utterance.cancel();
        }
        next
    }

    /// Open an utterance, or replace the one already open: a second `speech_start` while one is
    /// open ends the first (data model §3).
    pub fn open_utterance(&self, id: &str) {
        let mut slot = self.utterance.lock();
        if let Some(mut previous) = slot.take() {
            previous.cancel();
        }
        *slot = Some(Utterance::new(id, self.settings.max_utterance_seconds));
    }

    /// Append audio; returns whether the cap was reached, which is what the client is told once.
    pub fn push_audio(&self, id: &str, pcm: &[u8]) -> Option<bool> {
        let mut slot = self.utterance.lock();
        let utterance = slot.as_mut()?;
        if utterance.id != id {
            return None;
        }
        utterance.push_pcm16_le(pcm);
        Some(utterance.is_full())
    }

    /// Close the utterance and hand it to the caller, or `None` when it is not the one open.
    pub fn take_utterance(&self, id: &str) -> Option<Utterance> {
        let mut slot = self.utterance.lock();
        match slot.as_ref() {
            Some(utterance) if utterance.id == id => slot.take(),
            _ => None,
        }
    }

    /// Write one segment to the owning socket, and only to it.
    ///
    /// `false` from the lane is **two** events, not one: `try_send` reports `Full` and `Closed`
    /// alike, so a full queue is one dropped audio frame — counted, never an ending — and only a
    /// closed connection ends the session (`review.md` D10).
    fn send_speech(&self, frame: LiveVoiceSpeech) -> bool {
        let lane = self.lane.lock().clone();
        let json = serde_json::json!({ crate::live_voice::FRAME_KEY: frame });
        let sent = lane.send(json);
        if !sent && !lane.is_open() {
            self.end(EndReason::PageClosed);
            return false;
        }
        sent
    }

    /// Speak `text` in segments, checking the generation between them so an interruption stops
    /// the rest of the answer instead of finishing it (FR-022).
    pub async fn speak_reply(
        &self,
        engines: &SharedEngines,
        text: &str,
        generation: u64,
    ) -> Result<bool> {
        let segments = segmenter::segment(text);
        if segments.is_empty() {
            return Ok(false);
        }
        self.set_state(SessionState::Speaking);
        let mut seq = 0u32;
        for segment in segments {
            if self.generation() != generation || self.is_stopped() {
                return Ok(false);
            }
            let audio = match engines.speak(&segment.text).await {
                Ok(audio) => audio,
                Err(e) => {
                    // FR-032: the text is already on screen; say the voice failed and listen
                    // again rather than ending the session.
                    self.publish(LiveVoiceControl::Error {
                        kind: EndReason::ProviderFailed,
                        message: e.to_string(),
                    });
                    self.set_state(SessionState::Listening);
                    return Ok(false);
                }
            };
            let pcm = to_pcm16_16k(&audio);
            if self.send_speech(LiveVoiceSpeech::new(generation, seq, &pcm, segment.last)) {
                seq += 1;
            }
            if self.is_stopped() {
                return Ok(false);
            }
        }
        if self.generation() == generation && !self.is_stopped() {
            self.set_state(SessionState::Listening);
        }
        Ok(true)
    }

    /// Drive one spoken turn: submit the transcript as an ordinary chat request, carrying this
    /// session's own reply sender, and speak the answer.
    ///
    /// The reply arrives on that sender and nowhere else, so what is spoken is provably the answer
    /// to this transcript — no guessing which `Message` on the broadcast was mine (FR-007).
    pub async fn run_turn(self: Arc<Self>, engines: SharedEngines, transcript: String) {
        if self.is_stopped() {
            return;
        }
        self.busy.store(true, Ordering::SeqCst);
        let generation = self.generation();
        self.thinking();

        let (tx, mut stream) = match self.transport.reply_channel().await {
            Ok(pair) => pair,
            Err(e) => {
                self.fail(EndReason::ProviderFailed, format!("no reply channel: {e}"));
                self.busy.store(false, Ordering::SeqCst);
                return;
            }
        };

        let request = VizierRequest {
            timestamp: crate::time::now(),
            user: self.owner.clone(),
            content: VizierRequestContent::Chat(transcript),
            spoken: true,
            ..Default::default()
        };
        if let Err(e) = self
            .transport
            .send_request(self.session.clone(), request, Some(tx))
            .await
        {
            self.fail(EndReason::ProviderFailed, format!("the turn was not accepted: {e}"));
            self.busy.store(false, Ordering::SeqCst);
            return;
        }

        while let Ok(frame) = stream.recv_async().await {
            if self.generation() != generation || self.is_stopped() {
                break;
            }
            match frame.content {
                VizierResponseContent::Message { content, .. } => {
                    // The reply's text is saved by the turn itself; speaking is the extra.
                    let _ = self.speak_reply(&engines, &content, generation).await;
                    break;
                }
                VizierResponseContent::Abort | VizierResponseContent::Empty => break,
                VizierResponseContent::Error { message, .. } => {
                    self.publish(LiveVoiceControl::Error {
                        kind: EndReason::ProviderFailed,
                        message,
                    });
                    break;
                }
                _ => {
                    if frame.ends_turn() {
                        break;
                    }
                }
            }
        }

        if self.generation() == generation && !self.is_stopped() {
            // Whatever happened, the session is listening again: an interruption or a failed turn
            // never leaves it stuck (FR-015, FR-032).
            self.set_state(SessionState::Listening);
        }
        self.busy.store(false, Ordering::SeqCst);
    }
}

/// The provider's container into PCM16 16 kHz mono.
///
/// A provider that already answers in WAV is unwrapped; anything else is passed through, because
/// the client is told the format it is given rather than being lied to about it.
fn to_pcm16_16k(audio: &[u8]) -> Vec<u8> {
    if audio.len() > 44 && &audio[0..4] == b"RIFF" {
        return audio[44..].to_vec();
    }
    audio.to_vec()
}

/// Every live session in the process, keyed by (agent, conversation).
///
/// One registry for the whole process, held beside `Questions` and handed to each connection the
/// same way: per-socket state could not refuse a second tab on the same conversation (FR-005).
#[derive(Default)]
pub struct LiveVoiceSessions {
    map: Mutex<HashMap<VizierSession, Arc<LiveVoiceSession>>>,
}

impl LiveVoiceSessions {
    pub fn get(&self, session: &VizierSession) -> Option<Arc<LiveVoiceSession>> {
        self.map.lock().get(session).cloned()
    }

    /// Insert, or return the session already there. `Err(existing)` is the second-tab case: the
    /// caller is told a session is active rather than being handed someone else's.
    pub fn start(
        &self,
        session: VizierSession,
        make: impl FnOnce() -> Arc<LiveVoiceSession>,
    ) -> std::result::Result<Arc<LiveVoiceSession>, Arc<LiveVoiceSession>> {
        let mut map = self.map.lock();
        match map.get(&session) {
            Some(existing) => Err(existing.clone()),
            None => {
                let new = make();
                map.insert(session, new.clone());
                Ok(new)
            }
        }
    }

    /// Remove a session if it is the one given, so an ending cannot evict its replacement.
    pub fn remove(&self, session: &VizierSession, id: &Arc<LiveVoiceSession>) {
        let mut map = self.map.lock();
        if let Some(current) = map.get(session)
            && Arc::ptr_eq(current, id)
        {
            map.remove(session);
        }
    }

    pub fn len(&self) -> usize {
        self.map.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.lock().is_empty()
    }
}
