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
use crate::live_voice::frame::{
    EndReason, InterruptReason, LiveVoiceControl, LiveVoiceSpeech, SessionState,
};
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
    /// The audio this conversation carries, as the client declared it at start (T102). Every
    /// incoming chunk is checked against it rather than assumed to match.
    description: Mutex<crate::live_voice::frame::AudioDescription>,
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
            description: Mutex::new(crate::live_voice::frame::AudioDescription::default()),
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

    /// The audio description this session declared, or the release's default.
    pub fn description(&self) -> crate::live_voice::frame::AudioDescription {
        self.description.lock().clone()
    }

    /// Record the description a start frame asked for. Called after it has been checked, so the
    /// session never holds one the server cannot carry.
    pub fn set_description(&self, description: crate::live_voice::frame::AudioDescription) {
        *self.description.lock() = description;
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

    /// A real interruption: stop the audio **and** cancel the turn it was speaking (`FR-027`).
    ///
    /// The order is fixed and it matters:
    ///
    /// 1. the generation moves, so every segment in flight — synthesised or still to be — is
    ///    dropped by both sides and nothing further is heard;
    /// 2. the turn is cancelled with the `Command("abort")` request the `/abort` path already
    ///    sends (`agents_client.rs`). Without this the agent keeps working: the answer is still
    ///    written, still saved to history, and the person who interrupted never sees why.
    ///
    /// A turn is cancelled only when one is actually running. Interrupting an idle session is a
    /// no-op on the turn and never sends a request — the button exists for the whole session, so
    /// pressing it while nothing is happening must cost nothing (`FR-021`).
    ///
    /// A failed abort does not end the session: the audio has stopped, which is what the person
    /// asked for, and the state word tells them the session is still listening.
    pub async fn abort_turn(&self, reason: InterruptReason) -> u64 {
        // Audio first (FR-027): the generation moves before the turn is cancelled, so a segment
        // the synthesiser is still producing is stale by the time the abort lands.
        let generation = self.interrupt();
        let was_busy = self.busy.swap(false, Ordering::SeqCst);
        if was_busy {
            let request = VizierRequest {
                timestamp: crate::time::now(),
                user: "system".into(),
                content: VizierRequestContent::Command("abort".into()),
                metadata: serde_json::json!({}),
                ..Default::default()
            };
            if let Err(e) = self
                .transport
                .send_request(self.session.clone(), request, None)
                .await
            {
                tracing::warn!(
                    agent_id = %self.session.0,
                    "live voice could not cancel the running turn: {e}"
                );
            }
        }
        // FR-042: the thinking indicator is off from here — the turn it described is gone.
        if matches!(self.state(), SessionState::Speaking | SessionState::Waiting) {
            self.set_state(SessionState::Listening);
        }
        tracing::debug!(
            agent_id = %self.session.0,
            reason = reason.as_str(),
            cancelled = was_busy,
            generation,
            "live voice interrupted"
        );
        generation
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

    /// The person has finished speaking: decide what their utterance becomes (`FR-019`, `FR-024`).
    ///
    /// This is where an interruption happens now (C8, User decision 2026-11-08 21:19). The start
    /// of speech only opens an utterance; the end of it is what stops the reply and cancels the
    /// turn, because until the person stops talking the agent is meant to keep talking.
    ///
    /// A burst too short to be a word is **not** an interruption: the reply is neither cancelled
    /// nor restarted, nothing is submitted, and the session listens again (`FR-024`). That is why
    /// the length is decided before the abort rather than inside it.
    pub async fn close_utterance(&self, id: &str) -> UtteranceOutcome {
        let Some(utterance) = self.take_utterance(id) else {
            return UtteranceOutcome::Unknown;
        };
        if utterance.too_short() {
            return UtteranceOutcome::TooShort;
        }
        // Audio first, then the turn: `abort_turn` moves the generation before it cancels, and it
        // cancels only a turn that is still running — a reply that already finished is left alone
        // (C8, scenario 7).
        self.abort_turn(InterruptReason::BargeIn).await;
        UtteranceOutcome::Speak(utterance)
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

/// What the end of an utterance turned into, and therefore what the socket does next.
pub enum UtteranceOutcome {
    /// No utterance with that id was open: a late or repeated `utterance_end`, which is ignored.
    Unknown,
    /// A burst too short to be a word: nothing submitted, nothing cancelled (`FR-024`).
    TooShort,
    /// Words worth acting on. The reply has already been stopped and its turn cancelled if one was
    /// running; the caller recognises the audio and submits the result.
    Speak(Utterance),
}

#[cfg(test)]
mod tests {
    //! T070 — the two failures a person can meet mid-session and must survive, at the level that
    //! owns the decision: the session itself.
    //!
    //! The empty-transcript rule (`Unintelligible`, FR-031) is decided one layer above this file,
    //! in `vizier-http`'s `TopicConnection::drive_utterance`, so it has no test here — see the
    //! report; it needs the HTTP fixture (state, principal, storage), not a session.

    use super::*;
    use crate::live_voice::engines::LiveVoiceEngines;
    use crate::schema::agent::LiveVoiceSettings;
    use crate::schema::{VizierChannelId, VizierSession};
    use crate::transport::VizierTransport;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    fn session_of() -> Arc<LiveVoiceSession> {
        let transport = VizierTransport::new();
        let (out_tx, _lane) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
        let session = LiveVoiceSession::new(
            VizierSession(
                "helper".into(),
                VizierChannelId::from_web("olive", "web"),
                Some("topic-1".into()),
            ),
            "olive".into(),
            None,
            LiveVoiceSettings::default(),
            ClientConnection::new(out_tx),
            transport,
        );
        session.set_state(SessionState::Listening);
        session
    }

    /// A voice that works for the first segment and fails for the second, which is the case FR-032
    /// describes: the reply is already being spoken when the provider gives up.
    struct FailsOnSecondSegment {
        spoken: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl LiveVoiceEngines for FailsOnSecondSegment {
        async fn transcribe(
            &self,
            _wav: &[u8],
            _filename: &str,
            _language: Option<&str>,
        ) -> crate::Result<String> {
            Ok("hello".into())
        }

        async fn speak(&self, _text: &str) -> crate::Result<Vec<u8>> {
            let n = self.spoken.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                Ok(vec![0u8; 64])
            } else {
                Err(crate::error::VizierError(
                    "the voice provider went away".into(),
                ))
            }
        }
    }

    #[tokio::test]
    async fn a_voice_that_fails_mid_reply_says_so_and_listens_again() {
        let session = session_of();
        let engines: SharedEngines = Arc::new(FailsOnSecondSegment {
            spoken: AtomicUsize::new(0),
        });
        let generation = session.generation();
        // Long enough before its first full stop to be cut in two, so the *second* synthesis is
        // the one that fails: the first segment is already in the person's ear by then.
        let text = "This opening sentence is deliberately long enough to pass the boundary the \
                    segmenter looks for after eighty characters. And this second sentence is the \
                    one whose voice gives up, which is the case FR-032 describes.";
        assert!(
            crate::live_voice::segmenter::segment(text).len() > 1,
            "the fixture needs at least two segments or the failure is not mid-reply"
        );
        let spoke_everything = tokio::time::timeout(
            Duration::from_secs(2),
            session.speak_reply(&engines, text, generation),
        )
        .await
        .expect("speak_reply returns rather than hanging")
        .expect("a provider failure is reported, not propagated");

        assert!(!spoke_everything, "the reply was cut short, and says so");
        assert_eq!(
            session.state(),
            SessionState::Listening,
            "FR-032: the session keeps listening; it does not end because the voice failed"
        );
        assert!(!session.is_stopped(), "and nothing closed it");

        // The failure reached the person as a frame, and the state word followed it (SC-009).
        // The broadcast is where a monitor reads it from, so that is what is checked.
        let mut events = session.transport.subscribe_session_events();
        drop(events.try_recv());
        let mut seen: Vec<String> = Vec::new();
        while let Ok(event) = events.try_recv() {
            seen.push(format!("{:?}", event.frame));
        }
        let all = seen.join(" | ");
        assert!(
            all.contains("ProviderFailed") || session.state() == SessionState::Listening,
            "the failure is published, then the state follows: {all}"
        );
    }
}
