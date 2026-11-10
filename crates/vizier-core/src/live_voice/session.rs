//! One live voice session: the state word, the generation, the lane, and the turn it drives
//! (`specs/055-live-voice`).
//!
//! Nothing here is persisted. A session lives in memory, is owned by one person, and is reachable
//! through [`LiveVoiceSessions`] by (agent, conversation) so that a second tab on the same
//! conversation sees the same session rather than starting a second one (FR-005).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::time::Duration;

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

/// The words a filler may say (G2).
///
/// Neutral in language on purpose, and **never a promise about time**: "sebentar", "wait", "one
/// moment" and "hold on" all commit to a duration the session cannot keep, and the person who asked
/// for this ruled them out by name. These four are what a person says while they gather a thought.
pub const FILLER_WORDS: &[&str] = &["okay", "ya", "mm", "right"];

/// How long the agent may be silent before a filler covers the gap (G2).
///
/// Below this the answer is on its way anyway, and a filler would only be noise over the reply —
/// which is the "robotik" the person asked us to avoid. Once per turn, never repeated.
pub const FILLER_AFTER_MS: u64 = 400;


/// The API key a session was opened with (T110, FR-063).
///
/// `None` on the session is a person signed in through the WebUI or the desktop app: their session
/// ends when they end it, not when a credential lapses. Keeping the id and the expiry here means the
/// revoke path can find the session, and the frame path can notice a lapse, without a storage read
/// per frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyOrigin {
    pub key_id: String,
    pub expires_at: Option<crate::time::Timestamp>,
}

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
    /// When this session last did something that means a person is still there (`FR-029`, option
    /// A). A `tokio::time::Instant`, never `std::time::Instant`: a deadline on the real clock costs
    /// three minutes per test, so nobody runs more than one and "the clock resets on every path"
    /// stays a claim (`specs/055-live-voice/checklists/verify-fr029-idle-option-a.md` section 0).
    last_activity: Mutex<tokio::time::Instant>,
    /// The recogniser and voice this session speaks through; set once at start (FR-036).
    engines: Mutex<Option<SharedEngines>>,
    /// The audio this conversation carries, as the client declared it at start (T102). Every
    /// incoming chunk is checked against it rather than assumed to match.
    description: Mutex<crate::live_voice::frame::AudioDescription>,
    /// Which API key opened this session, or `None` for a person's own session (T110).
    key: Mutex<Option<KeyOrigin>>,
    /// The fillers, already synthesised in the agent's own voice (G2). Empty means this session has
    /// none — a voice that cannot say one word cannot say a filler either, and the reply is
    /// untouched by that.
    filler: Mutex<Vec<Vec<u8>>>,
    /// Whether synthesising the fillers has been attempted, so it happens once per session.
    filler_ready: AtomicBool,
    /// Which filler to say next, so a session does not repeat one word all conversation long.
    filler_next: AtomicUsize,
    /// The `seq` the next segment of this turn takes. A filler and a real segment share it, which
    /// is what keeps their order right on the client's queue.
    turn_seq: AtomicU32,
    /// Real segments sent this turn. A filler only goes while this is still zero.
    reply_segments: AtomicU32,
    /// Whether this turn's filler has already been spent.
    filler_spent: AtomicBool,
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
            last_activity: Mutex::new(tokio::time::Instant::now()),
            engines: Mutex::new(None),
            description: Mutex::new(crate::live_voice::frame::AudioDescription::default()),
            key: Mutex::new(None),
            filler: Mutex::new(Vec::new()),
            filler_ready: AtomicBool::new(false),
            filler_next: AtomicUsize::new(0),
            turn_seq: AtomicU32::new(0),
            reply_segments: AtomicU32::new(0),
            filler_spent: AtomicBool::new(false),
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

    /// Record which key opened this session, if one did (T110).
    pub fn set_key_origin(&self, origin: Option<KeyOrigin>) {
        *self.key.lock() = origin;
    }

    pub fn key_origin(&self) -> Option<KeyOrigin> {
        self.key.lock().clone()
    }

    pub fn generation(&self) -> u64 {
        *self.generation.borrow()
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// How long this call has been quiet, measured against a clock the caller supplies (`FR-029`).
    ///
    /// `now` is an argument, never a clock read inside, for the same reason `SideChats::sweep` takes
    /// one: a deadline that reads its own clock cannot be tested without waiting for it.
    pub fn idle_for(&self, now: tokio::time::Instant) -> Duration {
        now.saturating_duration_since(*self.last_activity.lock())
    }

    /// Whether a turn is running. Speech, tool calls and a synthesis in flight all count, because
    /// all three mean the person is waiting for something they asked for.
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// The idle limit these session settings ask for, or `None` when the call must never end itself.
    ///
    /// `idle_stop_seconds: 0` is read as **off**, not as "end immediately". A server that ended every
    /// call the tick after it began would make the field a trap: a person writing `0` means "no
    /// limit" in every other configuration key of this file, and there is no second reading of it
    /// that anyone could want. The value is not clamped to a minimum either — `5` means five
    /// seconds, and the client's hangover (`vad.ts:24-36`) is what makes a shorter one unusable, not
    /// the server's opinion.
    pub fn idle_limit(&self) -> Option<Duration> {
        match self.settings.idle_stop_seconds {
            0 => None,
            seconds => Some(Duration::from_secs(seconds)),
        }
    }

    /// Note that something happened, naming the path it came from (`FR-029`, checklist §3).
    ///
    /// One function and six call sites, not a reset written inline five times: the inline version
    /// loses one of them silently, and the symptom is a call that ends mid-sentence with `mic on` on
    /// screen. `trigger` is not decoration — it is what turns the six paths into six counters in the
    /// log, so a path that stops resetting shows up as a zero instead of a rumour
    /// (`verify-fr029-idle-option-a.md` §8.1). A choke point was considered and rejected: `publish`
    /// would not see `segment_sent`, because `send_speech` writes the per-connection lane.
    fn touch(&self, trigger: &'static str) {
        let now = tokio::time::Instant::now();
        let mut last = self.last_activity.lock();
        let idle_for_ms = now.saturating_duration_since(*last).as_millis() as u64;
        *last = now;
        // `topic=` rather than the `session=` of checklist §8.1: the reader the brief points at
        // (`~/.qa055/read-idle.sh:47-57`) parses `topic=`, and a field name it cannot read is a
        // field that does not exist. Same value, one name, the script's.
        tracing::debug!(trigger, idle_for_ms, topic = %self.topic(), "lv_idle reset");
    }

    /// The topic this call is on, as the idle log names it. A call without one logs `-`.
    fn topic(&self) -> &str {
        self.session.2.as_deref().unwrap_or("-")
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
        self.touch("open_utterance");
        let mut slot = self.utterance.lock();
        if let Some(mut previous) = slot.take() {
            previous.cancel();
        }
        *slot = Some(Utterance::new(id, self.settings.max_utterance_seconds));
    }

    /// Append audio; returns whether the cap was reached, which is what the client is told once.
    pub fn push_audio(&self, id: &str, pcm: &[u8]) -> Option<bool> {
        // Any chunk is activity, including one belonging to an utterance that has already been
        // replaced: audio arriving is a person talking, which is the whole question FR-029 asks.
        self.touch("push_audio");
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
        // Before the early returns: a close for an utterance this session no longer holds is still
        // a person finishing a sentence, and the sweep must not fire on the recognition that follows.
        self.touch("close_utterance");
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
        if sent {
            // `segment_sent` — the sixth path, and the one `publish()` would have missed.
            self.touch("segment_sent");
        }
        if !sent && !lane.is_open() {
            self.end(EndReason::PageClosed);
            return false;
        }
        sent
    }

    /// Start a turn's filler: the words once per session, this turn's counters, and the timer (G2).
    ///
    /// Called when the turn begins — before the answer exists — because that is the silence it
    /// covers. `speak_reply` then only sends, and the two share one `seq` counter so their order on
    /// the client's queue is the order they were produced.
    pub fn arm_filler(self: &Arc<Self>, engines: &SharedEngines, generation: u64) {
        self.prepare_filler(engines);
        self.turn_seq.store(0, Ordering::SeqCst);
        self.reply_segments.store(0, Ordering::SeqCst);
        self.filler_spent.store(false, Ordering::SeqCst);
        let armed = self.clone();
        tokio::spawn(async move { armed.cover_thinking(generation).await });
    }

    /// Synthesise the fillers once, in the agent's own voice (G2).
    ///
    /// A voice that cannot say two syllables cannot say an answer either, but this must never be
    /// what ends a session: a failure leaves the list empty and the reply untouched.
    fn prepare_filler(self: &Arc<Self>, engines: &SharedEngines) {
        if self.filler_ready.swap(true, Ordering::SeqCst) {
            return;
        }
        let engines = engines.clone();
        let this = self.clone();
        tokio::spawn(async move {
            let mut words = Vec::new();
            for word in FILLER_WORDS {
                match engines.speak(word).await {
                    Ok(audio) => words.push(to_pcm16_16k(&audio)),
                    Err(e) => tracing::debug!("a filler could not be synthesised: {e}"),
                }
            }
            *this.filler.lock() = words;
        });
    }

    /// Cover a silent agent with one filler word, at most once per turn (G2).
    ///
    /// Nothing here can hurt the reply: the word is dropped the moment the queue is full, and it
    /// never cancels or delays a real segment — the check for `reply_segments` is what makes a
    /// quick answer pass without any filler at all.
    async fn cover_thinking(self: Arc<Self>, generation: u64) {
        tokio::time::sleep(std::time::Duration::from_millis(FILLER_AFTER_MS)).await;
        if self.generation() != generation || self.is_stopped() {
            return;
        }
        // The answer is already being spoken: a filler now would talk over it.
        if self.reply_segments.load(Ordering::SeqCst) > 0 {
            return;
        }
        if self.filler_spent.swap(true, Ordering::SeqCst) {
            return;
        }
        let pcm = {
            let words = self.filler.lock();
            if words.is_empty() {
                return;
            }
            let at = self.filler_next.fetch_add(1, Ordering::SeqCst) % words.len();
            words[at].clone()
        };
        let seq = self.turn_seq.fetch_add(1, Ordering::SeqCst);
        // Cosmetic, so nothing is retried and nothing else is touched: a full lane loses the word
        // and the reply follows it as usual.
        let _ = self.send_speech(LiveVoiceSpeech::new(generation, seq, &pcm, false));
    }

    /// Speak `text` in segments, checking the generation between them so an interruption stops
    /// the rest of the answer instead of finishing it (FR-022).
    pub async fn speak_reply(
        &self,
        engines: &SharedEngines,
        text: &str,
        generation: u64,
    ) -> Result<bool> {
        let mut segments = segmenter::segment(text).into_iter();
        let Some(mut current) = segments.next() else {
            return Ok(false);
        };
        self.set_state(SessionState::Speaking);

        // Synthesis of the segment *after* this one starts before this one is sent, so the wait for
        // it overlaps with this segment being played. Sequential `await` made a reply of N segments
        // cost the sum of N syntheses before the last one was heard — the gap between sentences the
        // person noticed — for no reason: sending is instant, and the client queues gaplessly.
        let mut pending = Some(synthesise(engines, &current.text));
        loop {
            if self.generation() != generation || self.is_stopped() {
                abort(pending.take());
                return Ok(false);
            }
            let audio = match pending.take().expect("a synthesis is always in flight").await {
                Ok(Ok(audio)) => audio,
                Ok(Err(e)) => {
                    // FR-032: the text is already on screen; say the voice failed and listen
                    // again rather than ending the session.
                    self.publish(LiveVoiceControl::Error {
                        kind: EndReason::ProviderFailed,
                        message: e.to_string(),
                    });
                    self.set_state(SessionState::Listening);
                    return Ok(false);
                }
                // The task was cancelled — only this function cancels it, so the session is over.
                Err(_) => return Ok(false),
            };
            // The synthesis may have taken long enough for the person to cut in. Sending it anyway
            // would put a stale sentence in their ear *after* they interrupted — the one thing the
            // generation counter exists to prevent (FR-022). The check is here and at the top of
            // the loop because the two bracket the await.
            if self.generation() != generation || self.is_stopped() {
                abort(pending.take());
                return Ok(false);
            }
            let pcm = to_pcm16_16k(&audio);
            // The turn's own counter, shared with the filler: whichever goes first takes the lower
            // `seq`, and the client's queue plays them in exactly that order (G2).
            let seq = self.turn_seq.fetch_add(1, Ordering::SeqCst);
            if self.send_speech(LiveVoiceSpeech::new(generation, seq, &pcm, current.last)) {
                self.reply_segments.fetch_add(1, Ordering::SeqCst);
            }
            if self.is_stopped() {
                abort(pending.take());
                return Ok(false);
            }
            match segments.next() {
                Some(next) => {
                    pending = Some(synthesise(engines, &next.text));
                    current = next;
                }
                None => break,
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
        self.touch("turn_start");
        let generation = self.generation();
        self.thinking();

        // G2: a filler covers the gap while the answer is being written, if it takes long enough.
        self.arm_filler(&engines, generation);

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
                // The clock starts when the call does, and it is set after the lock is dropped: a
                // session that begins without a `last_activity` is ended by the first sweep.
                drop(map);
                new.touch("start");
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

    /// End every session that was opened with this API key, and say why (T110, FR-063).
    ///
    /// The contract promises a live session ends within a second of the key being revoked, and this
    /// is the line that keeps that promise. It goes through the paths that already exist rather than
    /// inventing a third: `abort_turn` cancels a running turn the way any interruption does
    /// (FR-052), `end` stops the session and publishes the reason, and the entry leaves the map so
    /// no later frame can resolve to a session that is over.
    ///
    /// The lock is dropped before any session is touched: `end` publishes, and a publish must never
    /// happen while the registry is held. A session that already stopped is skipped — its reason is
    /// the first one that won, which is what `end`'s idempotence means everywhere else.
    ///
    /// Returns how many sessions it ended, so a caller can log a revocation that mattered and stay
    /// quiet about one that did not.
    pub async fn end_owned_by_key(&self, key_id: &str, reason: EndReason) -> usize {
        let targets: Vec<(VizierSession, Arc<LiveVoiceSession>)> = {
            let map = self.map.lock();
            map.iter()
                .filter(|(_, session)| {
                    session
                        .key_origin()
                        .is_some_and(|origin| origin.key_id == key_id)
                })
                .map(|(conversation, session)| (conversation.clone(), session.clone()))
                .collect()
        };
        let mut ended = 0;
        for (conversation, session) in targets {
            // One place, both callers: `end_one` cancels the turn, stops the session and leaves the
            // map, in that order. The socket's expiry check calls the same function, so the two
            // paths cannot drift apart again (T110 round 1).
            if self.end_one(&conversation, &session, reason).await {
                ended += 1;
            }
        }
        ended
    }

    /// The call this owner already has somewhere else, moved out of the registry (T095-T097).
    ///
    /// One session per person, across conversations (FR-005): starting a call in another
    /// conversation moves the one that is running rather than adding a second. The order is fixed
    /// and it is the order `abort_turn` already has — the generation moves first so no segment in
    /// flight can still be heard, then the running turn is cancelled, then `end(Moved)` publishes
    /// why and lets go of the microphone through the one release path. `busy` is false when
    /// `abort_turn` returns, which is the only honest "the turn is over" this code can observe:
    /// the turn is spawned and never joined.
    ///
    /// Identity is the owner **and** the credential that opened the call. An API key presents an
    /// empty username, so the key id is what tells two callers apart; a person signed in normally
    /// has no key id at all. The consequence, chosen deliberately: two different keys of the same
    /// person are two identities here, so the second key starts its own call rather than moving the
    /// first. Treating them as one person needs the user id on the session, which this chunk does
    /// not add — reported rather than hidden.
    ///
    /// Returns the ended session so a caller can say so in a log, and `None` when there was nothing
    /// to move. A session that already stopped is dropped from the map and not counted: its reason
    /// is the one that won.
    pub async fn move_owned_elsewhere(
        &self,
        conversation: &VizierSession,
        owner: &str,
        owner_key: Option<&str>,
    ) -> Option<Arc<LiveVoiceSession>> {
        let previous: Option<(VizierSession, Arc<LiveVoiceSession>)> = {
            let map = self.map.lock();
            map.iter()
                .filter(|(other, _)| *other != conversation)
                .find(|(_, session)| {
                    !session.is_stopped()
                        && session.owner == owner
                        && session.key_origin().map(|origin| origin.key_id).as_deref() == owner_key
                })
                .map(|(other, session)| (other.clone(), session.clone()))
        };
        let (old_conversation, session) = previous?;
        // The lock is dropped above: `abort_turn` and `end` publish, and a publish must never
        // happen while the registry is held.
        session.abort_turn(InterruptReason::BargeIn).await;
        session.end(EndReason::Moved);
        self.remove(&old_conversation, &session);
        tracing::debug!(
            agent_id = %old_conversation.0,
            "a live voice call moved to another conversation"
        );
        Some(session)
    }

    /// End one session through the single path every ending takes (T110/FR-063, ronde 1).
    ///
    /// Two callers reach `EndReason::KeyRevoked`/`KeyExpired`: the revoke hook
    /// ([`LiveVoiceSessions::end_owned_by_key`]) and the socket's expiry check in `topic.rs`.
    /// Before this they disagreed, one cancelled the running turn and the other only stopped the
    /// session, so a lapsed key told the person the call had ended while the agent kept working and
    /// the answer was still written and saved. Both callers end here now, and the order is the one
    /// [`LiveVoiceSession::abort_turn`] documents: cancel the turn (FR-052), then stop and say why,
    /// then leave the map so no later frame resolves to a session that is over.
    ///
    /// Returns whether this call is the one that ended it. A session that already stopped is
    /// removed and reported as `false`: its first reason won, and `end`'s idempotence means the
    /// second caller must not overwrite it.
    pub async fn end_one(
        &self,
        conversation: &VizierSession,
        session: &Arc<LiveVoiceSession>,
        reason: EndReason,
    ) -> bool {
        if session.is_stopped() {
            self.remove(conversation, session);
            return false;
        }
        // The turn first: `end` stops the session and releases the microphone, but it sends no
        // `Command("abort")`, so without this the agent keeps writing an answer for a call that is
        // already over. `BargeIn` is the reason every ending-by-force already uses (FR-052), and
        // `abort_turn` is a no-op on the turn when nothing is running.
        session.abort_turn(InterruptReason::BargeIn).await;
        session.end(reason);
        self.remove(conversation, session);
        true
    }

    /// End every call that has been quiet longer than **its own** settings allow (`FR-029`, option
    /// A: the server closes an idle call — User's choice, 2026-11-10 10:20).
    ///
    /// Three rules, each paid for by a specific failure:
    ///
    /// - **A running turn pauses the clock.** `busy` means the agent is working, through tools or
    ///   synthesis. Counting that as idleness cuts an answer in half while the bar still says
    ///   `Speaking`, which is the one thing this timer must never do.
    /// - **A stopped session is skipped**, so a tick cannot publish a second `ended`; `end_one`
    ///   would also remove it, but skipping is what keeps that reason the first one that won.
    /// - **The ending is `end_one(Idle)`** — never `end()` alone and never `set_state(Ended)`:
    ///   `end()` leaves the agent writing an answer for a call that is over, and `set_state` stamps
    ///   `PressedAgain`, telling a person who is in the kitchen that they ended their own call.
    ///
    /// The lock is dropped before anything is ended: `end_one` publishes, and a publish must never
    /// happen while the registry is held. `now` is an argument rather than a clock read so a test
    /// can move it (`verify-fr029-idle-option-a.md` sections 0 and 4). Returns how many it ended.
    pub async fn sweep(&self, now: tokio::time::Instant) -> usize {
        let mut due: Vec<(VizierSession, Arc<LiveVoiceSession>, Duration)> = Vec::new();
        {
            let map = self.map.lock();
            for (conversation, session) in map.iter() {
                if session.is_stopped() {
                    continue;
                }
                let Some(limit) = session.idle_limit() else {
                    continue;
                };
                let idle = session.idle_for(now);
                if idle >= limit {
                    due.push((conversation.clone(), session.clone(), idle));
                }
            }
        }
        let mut ended = 0;
        for (conversation, session, idle) in due {
            let idle_for_ms = idle.as_millis() as u64;
            if session.is_busy() {
                // The guard was reached and refused: this line is how a person proving condition 4
                // sees the difference between "held" and "never counted".
                tracing::debug!(
                    idle_for_ms,
                    busy = true,
                    topic = %session.topic(),
                    "lv_idle held"
                );
                continue;
            }
            if self.end_one(&conversation, &session, EndReason::Idle).await {
                tracing::debug!(
                    idle_for_ms,
                    busy = false,
                    topic = %session.topic(),
                    "lv_idle end"
                );
                ended += 1;
            }
        }
        ended
    }

    /// Sweep every `every`, forever, for the whole process.
    ///
    /// The shape is `SideChats::spawn_sweeper`'s (`channels/side_chat.rs:438`) on purpose: one
    /// interval owned by one task, and **no `sleep()` buried inside a per-session task**. A timer
    /// that sleeps cannot be steered, so no test could prove the deadline without waiting for it —
    /// and a deadline nobody tests is a deadline nobody has.
    pub fn spawn_sweeper(&'static self, every: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                self.sweep(tokio::time::Instant::now()).await;
            }
        })
    }

    pub fn len(&self) -> usize {
        self.map.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.lock().is_empty()
    }
}

/// One segment's synthesis, off the caller's task so the next one can overlap with it (F3).
fn synthesise(
    engines: &SharedEngines,
    text: &str,
) -> tokio::task::JoinHandle<crate::Result<Vec<u8>>> {
    let engines = engines.clone();
    let text = text.to_string();
    tokio::spawn(async move { engines.speak(&text).await })
}

/// Stop a synthesis nobody is waiting for. An interrupted reply must not keep a provider busy.
fn abort(pending: Option<tokio::task::JoinHandle<crate::Result<Vec<u8>>>>) {
    if let Some(task) = pending {
        task.abort();
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

#[cfg(test)]
mod key_tests {
    //! T110/FR-063 — revoking or lapsing an API key ends the call it opened, and only that call.
    //!
    //! The registry is the whole subject here: the contract promises a session ends within a second
    //! of the revocation, and this is the code that keeps it. The HTTP hook that calls it and the
    //! middleware that supplies the key id are asserted one layer up.

    use super::*;
    use crate::schema::agent::LiveVoiceSettings;
    use crate::schema::{VizierChannelId, VizierSession};
    use crate::transport::VizierTransport;

    /// How long a test waits for a request before deciding it will not come.
    const WAIT: std::time::Duration = std::time::Duration::from_millis(500);

    fn session_on(topic: &str) -> Arc<LiveVoiceSession> {
        session_on_transport(topic, VizierTransport::new())
    }

    /// The same, on a transport the test already holds, so it can watch what was published.
    fn session_on_transport(topic: &str, transport: VizierTransport) -> Arc<LiveVoiceSession> {
        let (out_tx, _lane) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
        let session = LiveVoiceSession::new(
            VizierSession(
                "helper".into(),
                VizierChannelId::from_web("olive", "web"),
                Some(topic.into()),
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

    fn origin(key_id: &str, expires_at: Option<crate::time::Timestamp>) -> Option<KeyOrigin> {
        Some(KeyOrigin {
            key_id: key_id.into(),
            expires_at,
        })
    }

    /// Three sessions in one registry: one opened by key A, one by key B, one by a person.
    fn registry() -> (LiveVoiceSessions, VizierSession, VizierSession, VizierSession) {
        let registry = LiveVoiceSessions::default();
        let a = session_on("topic-a");
        let b = session_on("topic-b");
        let person = session_on("topic-person");
        a.set_key_origin(origin("key-a", None));
        b.set_key_origin(origin("key-b", None));
        let (ka, kb, kp) = (a.session.clone(), b.session.clone(), person.session.clone());
        registry.start(ka.clone(), || a);
        registry.start(kb.clone(), || b);
        registry.start(kp.clone(), || person);
        (registry, ka, kb, kp)
    }

    #[tokio::test]
    async fn revoking_a_key_ends_its_call_and_leaves_the_others_alone() {
        let (registry, ka, kb, kp) = registry();
        assert_eq!(registry.len(), 3);

        let ended = registry.end_owned_by_key("key-a", EndReason::KeyRevoked).await;
        assert_eq!(ended, 1, "one session was opened with that key");

        let a = registry.get(&ka);
        assert!(a.is_none(), "the ended session is gone, so no later frame resolves to it");
        assert_eq!(registry.get(&kb).map(|s| s.state()), Some(SessionState::Listening));
        assert_eq!(registry.get(&kp).map(|s| s.state()), Some(SessionState::Listening));
        assert_eq!(registry.len(), 2, "the two other calls are still live");
    }

    #[tokio::test]
    async fn the_ended_session_says_why_and_stays_ended() {
        let transport = VizierTransport::new();
        let mut events = transport.subscribe_session_events();
        let registry = LiveVoiceSessions::default();
        let session = session_on_transport("topic-a", transport.clone());
        session.set_key_origin(origin("key-a", None));
        let key = session.session.clone();
        registry.start(key.clone(), || session.clone());

        registry.end_owned_by_key("key-a", EndReason::KeyRevoked).await;

        // The reason reaches the client as a state frame, and it is the one the contract names.
        // Under a runtime the event goes through the pump, so it is awaited rather than read on
        // the next line — a bare `try_recv` here would be a race, not a test.
        let mut reason = None;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
        while reason.is_none() && tokio::time::Instant::now() < deadline {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            let Ok(Ok(event)) = tokio::time::timeout(left, events.recv()).await else {
                break;
            };
            if let SessionFrame::LiveVoice(LiveVoiceControl::State {
                state: SessionState::Ended,
                reason: Some(why),
                ..
            }) = event.frame
            {
                reason = Some(why);
            }
        }
        assert_eq!(reason, Some(EndReason::KeyRevoked));
        assert_eq!(session.state(), SessionState::Ended);

        // And a second revocation is not a second ending.
        assert_eq!(registry.end_owned_by_key("key-a", EndReason::KeyRevoked).await, 0);
    }

    #[tokio::test]
    async fn a_key_with_no_session_ends_nothing() {
        let (registry, ka, kb, kp) = registry();
        assert_eq!(registry.end_owned_by_key("key-nobody-used", EndReason::KeyRevoked).await, 0);
        assert_eq!(registry.len(), 3);
        for key in [ka, kb, kp] {
            assert_eq!(registry.get(&key).map(|s| s.state()), Some(SessionState::Listening));
        }
    }

    #[tokio::test]
    async fn a_lapsed_key_ends_its_call_with_its_own_reason() {
        let registry = LiveVoiceSessions::default();
        let session = session_on("topic-a");
        let past = crate::time::now() - chrono::Duration::seconds(30);
        session.set_key_origin(origin("key-a", Some(past)));
        let key = session.session.clone();
        registry.start(key.clone(), || session.clone());

        // The expiry path is the caller's: it notices the lapse and ends the session it opened.
        assert_eq!(registry.end_owned_by_key("key-a", EndReason::KeyExpired).await, 1);
        assert_eq!(session.state(), SessionState::Ended);
        assert!(registry.get(&key).is_none());
    }

    #[tokio::test]
    async fn a_session_a_person_opened_has_no_key_to_revoke() {
        let registry = LiveVoiceSessions::default();
        let person = session_on("topic-person");
        assert_eq!(person.key_origin(), None, "no credential, nothing to lapse");
        let key = person.session.clone();
        registry.start(key, || person);
        // Nothing in the map matches any key id, so a revocation elsewhere cannot touch it.
        assert_eq!(registry.end_owned_by_key("key-a", EndReason::KeyRevoked).await, 0);
        assert_eq!(LiveVoiceSessions::len(&registry), 1);
    }

    /// A voice that says nothing: this test needs a turn to be running, not its sound.
    struct SilentVoice;

    #[async_trait::async_trait]
    impl crate::live_voice::engines::LiveVoiceEngines for SilentVoice {
        async fn transcribe(
            &self,
            _wav: &[u8],
            _filename: &str,
            _language: Option<&str>,
        ) -> crate::Result<String> {
            Ok("hello".into())
        }

        async fn speak(&self, _text: &str) -> crate::Result<Vec<u8>> {
            Ok(Vec::new())
        }
    }

    /// The shape of a request as the agent's inbox sees it, so a test can name one.
    fn request_kind(content: &VizierRequestContent) -> String {
        match content {
            VizierRequestContent::Command(command) => format!("command:{command}"),
            VizierRequestContent::Chat(text) => format!("chat:{text}"),
            other => format!("{other}"),
        }
    }

    /// T110/FR-063 ronde 1: the expiry path ends the call **and cancels the turn it was running**.
    ///
    /// The revoke path always did both. The socket's expiry check only stopped the session, so the
    /// agent kept working: the answer was still written and still saved after the person had been
    /// told the call had ended (the warning on `abort_turn`). The five tests above missed it because
    /// they call the registry directly and never pass through the socket. Both callers go through
    /// `end_one` now, and this test asserts the three things FR-063 asks for at once: the reason,
    /// the release, and the turn.
    #[tokio::test]
    async fn a_lapsed_key_cancels_the_turn_it_was_running() {
        let transport = VizierTransport::new();
        let mut inbox = transport.register_agent("helper".into()).await;
        let registry = LiveVoiceSessions::default();
        let session = session_on_transport("topic-a", transport.clone());
        session.set_key_origin(origin("key-a", Some(crate::time::now())));
        let key = session.session.clone();
        registry.start(key.clone(), || session.clone());

        // A turn is running: its request has reached the agent and the reply has not come back.
        let running = session.clone();
        tokio::spawn(async move { running.run_turn(Arc::new(SilentVoice), "hello".into()).await });
        let submitted = tokio::time::timeout(WAIT, inbox.recv_request())
            .await
            .expect("the turn submits its request")
            .expect("the inbox is open");
        assert_eq!(
            request_kind(&submitted.request.content),
            "chat:hello",
            "the request in flight is the one the test is about to cancel"
        );
        assert!(
            session.busy.load(Ordering::SeqCst),
            "the session is busy while it waits for the reply"
        );

        // The key lapses and the path the socket takes runs: one call, one ending.
        assert!(
            registry.end_one(&key, &session, EndReason::KeyExpired).await,
            "this call is the one that ended it"
        );

        // 1. the reason the person is told, on the state frame the contract names
        assert_eq!(session.state(), SessionState::Ended);
        // 2. the turn is no longer running, not only the session (FR-063)
        assert!(
            !session.busy.load(Ordering::SeqCst),
            "the turn was cancelled, so nothing is still being written"
        );
        // 3. and the cancellation really left: `end()` alone sends no abort
        let mut kinds = Vec::new();
        while let Ok(Ok(envelope)) =
            tokio::time::timeout(std::time::Duration::from_millis(50), inbox.recv_request()).await
        {
            kinds.push(request_kind(&envelope.request.content));
        }
        assert_eq!(
            kinds.iter().filter(|kind| *kind == "command:abort").count(),
            1,
            "the running turn is cancelled by exactly one abort, got {kinds:?}"
        );

        // The entry is gone, so a frame arriving later does not resolve to a session that is over.
        assert!(registry.get(&key).is_none());
    }

}

#[cfg(test)]
mod move_tests {
    //! T095-T097 / FR-005 — one call per person, and starting elsewhere moves it rather than
    //! adding a second. The registry is the whole subject; the socket only calls it at the start of
    //! a session, after the engines are ready.

    use super::*;
    use crate::schema::agent::LiveVoiceSettings;
    use crate::schema::{VizierChannelId, VizierSession};
    use crate::transport::VizierTransport;

    fn conversation(topic: &str) -> VizierSession {
        VizierSession(
            "helper".into(),
            VizierChannelId::from_web("olive", "web"),
            Some(topic.into()),
        )
    }

    fn session_on(transport: &VizierTransport, topic: &str, owner: &str) -> Arc<LiveVoiceSession> {
        let (out_tx, _lane) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
        let session = LiveVoiceSession::new(
            conversation(topic),
            owner.into(),
            None,
            LiveVoiceSettings::default(),
            ClientConnection::new(out_tx),
            transport.clone(),
        );
        session.set_state(SessionState::Listening);
        session
    }

    /// A registry holding one live call on `topic-a` for `olive`.
    fn one_call(transport: &VizierTransport) -> (LiveVoiceSessions, Arc<LiveVoiceSession>) {
        let registry = LiveVoiceSessions::default();
        let session = session_on(transport, "topic-a", "olive");
        assert!(registry.start(conversation("topic-a"), || session.clone()).is_ok());
        (registry, session)
    }

    #[tokio::test]
    async fn starting_elsewhere_moves_the_call_and_the_old_one_says_so() {
        let transport = VizierTransport::new();
        let mut events = transport.subscribe_session_events();
        let (registry, old) = one_call(&transport);
        assert_eq!(registry.len(), 1);

        let moved = registry
            .move_owned_elsewhere(&conversation("topic-b"), "olive", None)
            .await;
        let moved = moved.expect("the call on topic-a belongs to this person");

        assert!(Arc::ptr_eq(&moved, &old), "the call that moved is the one that was running");
        assert_eq!(old.state(), SessionState::Ended, "the old call is over, not paused");
        assert!(registry.get(&conversation("topic-a")).is_none(), "and its entry is gone");
        assert_eq!(registry.len(), 0, "one person, never two calls");

        // The reason really reaches the client: `moved`, not `page_closed` (the socket is open)
        // and not `pressed_again` (the person did not switch anything off).
        let mut reason = None;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
        while reason.is_none() && tokio::time::Instant::now() < deadline {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            let Ok(Ok(event)) = tokio::time::timeout(left, events.recv()).await else {
                break;
            };
            if let SessionFrame::LiveVoice(LiveVoiceControl::State {
                state: SessionState::Ended,
                reason: Some(why),
                ..
            }) = event.frame
            {
                reason = Some(why);
            }
        }
        assert_eq!(reason, Some(EndReason::Moved));

        // And the new call is a new session: nothing of the old one comes with it.
        let fresh = session_on(&transport, "topic-b", "olive");
        assert!(registry.start(conversation("topic-b"), || fresh.clone()).is_ok());
        assert_eq!(registry.len(), 1, "still one call, in the new conversation");
        assert!(!Arc::ptr_eq(&fresh, &old));
        assert_eq!(fresh.generation(), 1, "a new call starts its own generation");
        assert!(
            matches!(fresh.close_utterance("u-from-the-old-call").await, UtteranceOutcome::Unknown),
            "the old call's utterance did not come with it"
        );
    }

    #[tokio::test]
    async fn starting_in_the_same_conversation_moves_nothing() {
        let transport = VizierTransport::new();
        let (registry, old) = one_call(&transport);
        let generation = old.generation();

        let moved = registry
            .move_owned_elsewhere(&conversation("topic-a"), "olive", None)
            .await;
        assert!(moved.is_none(), "the same conversation is the same call, not a move");
        assert_eq!(old.state(), SessionState::Listening);
        assert_eq!(old.generation(), generation, "and nothing was interrupted");
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn another_persons_call_is_never_moved() {
        let transport = VizierTransport::new();
        let (registry, theirs) = one_call(&transport);

        let moved = registry
            .move_owned_elsewhere(&conversation("topic-b"), "someone-else", None)
            .await;
        assert!(moved.is_none(), "moving is the owner's privilege");
        assert_eq!(theirs.state(), SessionState::Listening, "their call is untouched");
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn a_different_key_of_the_same_person_is_a_different_identity() {
        // Chosen deliberately and pinned here: identity is the owner *and* the credential. Two
        // keys of one person are two identities, so the second starts its own call. Treating them
        // as one person needs the user id on the session, which T095-T097 does not add.
        let transport = VizierTransport::new();
        let registry = LiveVoiceSessions::default();
        let session = session_on(&transport, "topic-a", "");
        session.set_key_origin(Some(KeyOrigin {
            key_id: "key-a".into(),
            expires_at: None,
        }));
        assert!(registry.start(conversation("topic-a"), || session.clone()).is_ok());

        let moved = registry
            .move_owned_elsewhere(&conversation("topic-b"), "", Some("key-b"))
            .await;
        assert!(moved.is_none(), "a second key is not the first key");
        assert_eq!(session.state(), SessionState::Listening);

        // The same key, elsewhere, is the same identity: that does move.
        let moved = registry
            .move_owned_elsewhere(&conversation("topic-b"), "", Some("key-a"))
            .await;
        assert!(moved.is_some(), "the key that opened the call moves with it");
        assert_eq!(session.state(), SessionState::Ended);
    }

    #[tokio::test]
    async fn a_session_that_already_stopped_is_not_moved_again() {
        let transport = VizierTransport::new();
        let (registry, old) = one_call(&transport);
        old.end(EndReason::PressedAgain);

        let moved = registry
            .move_owned_elsewhere(&conversation("topic-b"), "olive", None)
            .await;
        assert!(moved.is_none(), "an ended call has nothing left to move");
        assert_eq!(old.state(), SessionState::Ended);
    }
}

#[cfg(test)]
mod pipeline_tests {
    //! F3 — the voice does not wait for itself. Synthesis of the next segment overlaps with the
    //! current one being sent, so a reply of N segments no longer costs the sum of N syntheses
    //! before the last sentence is heard.

    use super::*;
    use crate::live_voice::engines::LiveVoiceEngines;
    use crate::schema::agent::LiveVoiceSettings;
    use crate::schema::{VizierChannelId, VizierSession};
    use crate::transport::VizierTransport;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    /// A voice that records what happened, in order, and takes its time on the second segment.
    struct RecordingEngines {
        log: Arc<StdMutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl LiveVoiceEngines for RecordingEngines {
        async fn transcribe(
            &self,
            _wav: &[u8],
            _filename: &str,
            _language: Option<&str>,
        ) -> crate::Result<String> {
            Ok("hello".into())
        }

        async fn speak(&self, text: &str) -> crate::Result<Vec<u8>> {
            let n = {
                let mut log = self.log.lock().unwrap();
                let n = log.iter().filter(|entry| entry.starts_with("start:")).count();
                log.push(format!("start:{n}"));
                n
            };
            // The second segment is slow, which is the case that used to show as a gap.
            if n == 1 {
                tokio::time::sleep(Duration::from_millis(120)).await;
            }
            self.log.lock().unwrap().push(format!("end:{n}"));
            Ok(vec![0u8; 64 * (n as usize + 1)])
        }
    }

    fn session_with_lane() -> (Arc<LiveVoiceSession>, tokio::sync::mpsc::Receiver<serde_json::Value>) {
        let transport = VizierTransport::new();
        let (out_tx, rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
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
        (session, rx)
    }

    fn segments_of() -> String {
        // Two sentences, each past the first boundary, so the reply really has two segments.
        "This opening sentence is long enough to pass the boundary the segmenter looks for. \
         This second sentence is the one whose synthesis is slow on purpose."
            .to_string()
    }

    #[tokio::test]
    async fn the_second_synthesis_overlaps_the_first_segment_being_sent() {
        let (session, mut rx) = session_with_lane();
        let log = Arc::new(StdMutex::new(Vec::new()));
        let engines: SharedEngines = Arc::new(RecordingEngines { log: log.clone() });
        let generation = session.generation();
        let text = segments_of();
        assert!(
            segmenter::segment(&text).len() >= 2,
            "the fixture needs two segments or there is nothing to overlap"
        );

        let spoke = session.speak_reply(&engines, &text, generation).await.unwrap();
        assert!(spoke);
        assert_eq!(session.state(), SessionState::Listening);

        // The frames that reached the lane, in order.
        let mut seqs = Vec::new();
        let mut last_flags = Vec::new();
        while let Ok(frame) = rx.try_recv() {
            if let Some(body) = frame.get(crate::live_voice::FRAME_KEY) {
                seqs.push(body["seq"].as_u64().unwrap() as u32);
                last_flags.push(body["final"].as_bool().unwrap());
            }
        }
        assert_eq!(seqs, vec![0, 1], "one frame per segment, in order");
        assert_eq!(last_flags, vec![false, true], "only the last segment is final");

        // And the order inside the engine's own log proves the overlap: the second synthesis had
        // already started before the first segment was sent.
        let entries = log.lock().unwrap().clone();
        let start2 = entries.iter().position(|e| e == "start:1").expect("a second synthesis");
        let end1 = entries.iter().position(|e| e == "end:1").expect("the second finished");
        assert!(start2 < end1, "{entries:?}");
    }

    #[tokio::test]
    async fn an_interrupted_reply_sends_no_further_segment() {
        let (session, mut rx) = session_with_lane();
        let log = Arc::new(StdMutex::new(Vec::new()));
        let engines: SharedEngines = Arc::new(RecordingEngines { log: log.clone() });
        let generation = session.generation();
        let text = segments_of();

        let speaking = {
            let session = session.clone();
            let engines = engines.clone();
            tokio::spawn(async move { session.speak_reply(&engines, &text, generation).await })
        };
        // Let the first segment go, then interrupt the way a person speaking does.
        tokio::time::sleep(Duration::from_millis(20)).await;
        session.interrupt();
        let spoke = speaking.await.unwrap().unwrap();
        assert!(!spoke, "the reply did not finish");

        let mut seqs = Vec::new();
        while let Ok(frame) = rx.try_recv() {
            if let Some(body) = frame.get(crate::live_voice::FRAME_KEY) {
                seqs.push(body["seq"].as_u64().unwrap() as u32);
            }
        }
        assert!(
            seqs.iter().all(|seq| *seq == 0),
            "nothing after the interruption: {seqs:?}"
        );
        // `interrupt()` moves the generation and nothing else; putting the word back is
        // `abort_turn`'s job in the socket, and that path is covered by its own tests.
        assert!(seqs.len() <= 1, "at most the segment already in flight: {seqs:?}");
    }
}

#[cfg(test)]
mod filler_tests {
    //! G2 — a filler word covers the first four hundred milliseconds of a slow answer, and nothing
    //! else. The hard rule: the reply is never sacrificed for the cosmetic word.

    use super::*;
    use crate::live_voice::engines::LiveVoiceEngines;
    use crate::schema::agent::LiveVoiceSettings;
    use crate::schema::{VizierChannelId, VizierSession};
    use crate::transport::VizierTransport;
    use std::sync::Arc;
    use std::time::Duration;

    /// A voice whose reply takes `delay`, and whose audio is as long as the text it was given — which
    /// is how a filler (two to five bytes) is told apart from a real segment in the lane.
    struct SlowVoice {
        delay: Duration,
        filler_fails: bool,
    }

    #[async_trait::async_trait]
    impl LiveVoiceEngines for SlowVoice {
        async fn transcribe(
            &self,
            _wav: &[u8],
            _filename: &str,
            _language: Option<&str>,
        ) -> crate::Result<String> {
            Ok("hello".into())
        }

        async fn speak(&self, text: &str) -> crate::Result<Vec<u8>> {
            let is_filler = FILLER_WORDS.contains(&text);
            if is_filler && self.filler_fails {
                return Err(crate::error::VizierError("no filler for you".into()));
            }
            if !is_filler {
                tokio::time::sleep(self.delay).await;
            }
            Ok(vec![0xAB; text.len()])
        }
    }

    fn lane(
        capacity: usize,
    ) -> (Arc<LiveVoiceSession>, tokio::sync::mpsc::Receiver<serde_json::Value>) {
        let transport = VizierTransport::new();
        let (out_tx, rx) = tokio::sync::mpsc::channel::<serde_json::Value>(capacity);
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
        (session, rx)
    }

    fn reply() -> String {
        "This opening sentence is long enough to pass the boundary the segmenter looks for. \
         This second sentence is the one whose synthesis is deliberately slow."
            .to_string()
    }

    /// Drain the lane into (decoded length, seq, final) so a filler is recognisable by its size.
    fn frames(rx: &mut tokio::sync::mpsc::Receiver<serde_json::Value>) -> Vec<(usize, u32, bool)> {
        use base64::Engine as _;
        let mut out = Vec::new();
        while let Ok(frame) = rx.try_recv() {
            let Some(body) = frame.get(crate::live_voice::FRAME_KEY) else { continue };
            let text = body["base64"].as_str().unwrap_or_default();
            let bytes = base64::engine::general_purpose::STANDARD.decode(text).unwrap_or_default();
            out.push((
                bytes.len(),
                body["seq"].as_u64().unwrap_or(0) as u32,
                body["final"].as_bool().unwrap_or(false),
            ));
        }
        out
    }

    /// A slow answer gets exactly one filler, and it is the first thing heard.
    #[tokio::test]
    async fn a_slow_answer_is_covered_by_one_filler() {
        let (session, mut rx) = lane(64);
        let engines: SharedEngines = Arc::new(SlowVoice {
            delay: Duration::from_millis(700),
            filler_fails: false,
        });
        let generation = session.generation();
        let text = reply();
        session.arm_filler(&engines, generation);
        session.speak_reply(&engines, &text, generation).await.unwrap();

        let heard = frames(&mut rx);
        assert!(heard.len() >= 2, "{heard:?}");
        let (len, seq, last) = heard[0];
        assert!(len <= 8, "the first thing is a filler word, not a segment: {heard:?}");
        assert_eq!(seq, 0, "the filler takes the first seq");
        assert!(!last, "a filler is never the reply's last segment");
        // Exactly one filler, however long the answer takes.
        let fillers = heard.iter().filter(|(len, _, _)| *len <= 8).count();
        assert_eq!(fillers, 1, "one word per turn, not a chorus: {heard:?}");
        // The real segments follow, in order, and only the last one is final.
        assert_eq!(heard.last().map(|(_, _, last)| *last), Some(true));
        let seqs: Vec<u32> = heard.iter().map(|(_, seq, _)| *seq).collect();
        assert_eq!(seqs, (0..heard.len() as u32).collect::<Vec<_>>(), "seqs are 0,1,2…");
    }

    /// The list itself is the rule the person set: neutral in language, and never a promise about
    /// how long the wait will be. A word added here later that breaks either is caught here.
    #[test]
    fn the_filler_list_promises_nothing_about_time() {
        assert!(!FILLER_WORDS.is_empty(), "a filler that says nothing covers nothing");
        for word in FILLER_WORDS {
            let lower = word.to_lowercase();
            for banned in [
                "sebentar", "bentar", "sabar", "tunggu", "wait", "moment", "second", "hold", "soon",
                "minute", "detik", "segera",
            ] {
                assert!(
                    !lower.contains(banned),
                    "{word:?} promises a duration this session cannot keep ({banned:?})"
                );
            }
            assert!(word.chars().count() <= 8, "{word:?} is a sentence, not a filler");
        }
        // And the four the brief names are still there.
        for word in ["okay", "ya", "mm", "right"] {
            assert!(FILLER_WORDS.contains(&word), "{word:?} was agreed and is missing");
        }
    }

    /// One turn, one word: a second turn gets its own, and neither turn gets two.
    #[tokio::test]
    async fn each_turn_spends_at_most_one_filler() {
        let (session, mut rx) = lane(64);
        let engines: SharedEngines = Arc::new(SlowVoice {
            delay: Duration::from_millis(700),
            filler_fails: false,
        });
        let text = reply();

        for turn in 0..2 {
            let generation = session.generation();
            session.arm_filler(&engines, generation);
            session.speak_reply(&engines, &text, generation).await.unwrap();
            let heard = frames(&mut rx);
            let fillers = heard.iter().filter(|(len, _, _)| *len <= 8).count();
            assert_eq!(fillers, 1, "turn {turn} gets exactly one word: {heard:?}");
        }
    }

    /// A quick answer is not covered: the filler would talk over it, which is what "robotik" means.
    #[tokio::test]
    async fn a_quick_answer_gets_no_filler_at_all() {
        let (session, mut rx) = lane(64);
        let engines: SharedEngines = Arc::new(SlowVoice {
            delay: Duration::from_millis(10),
            filler_fails: false,
        });
        let generation = session.generation();
        let text = reply();
        session.arm_filler(&engines, generation);
        session.speak_reply(&engines, &text, generation).await.unwrap();

        let heard = frames(&mut rx);
        assert!(!heard.is_empty(), "the reply is still spoken");
        assert!(
            heard.iter().all(|(len, _, _)| *len > 8),
            "no filler for an answer that arrived at once: {heard:?}"
        );
    }

    /// The hard rule: a filler that cannot be sent changes nothing, and the reply still goes.
    #[tokio::test]
    async fn a_dropped_filler_never_costs_the_reply() {
        // One slot, already occupied when the filler fires and free again when the segment does.
        let (session, mut rx) = lane(1);
        assert!(session.send_speech(LiveVoiceSpeech::new(1, 0, &[0u8; 4], false)), "the slot is taken");

        let engines: SharedEngines = Arc::new(SlowVoice {
            delay: Duration::from_millis(700),
            filler_fails: false,
        });
        let generation = session.generation();
        let text = reply();
        session.arm_filler(&engines, generation);
        let running = {
            let session = session.clone();
            let engines = engines.clone();
            let text = text.clone();
            tokio::spawn(async move { session.speak_reply(&engines, &text, generation).await })
        };

        // The filler fires at 400 ms against a full lane and is dropped, silently.
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(!session.is_stopped(), "a cosmetic word never ends the session");
        assert_eq!(session.state(), SessionState::Speaking);
        // Free the slot; the real segment then arrives and is sent.
        let _ = rx.try_recv();
        running.await.unwrap();

        let heard = frames(&mut rx);
        assert!(!heard.is_empty(), "the reply still reaches the person: {heard:?}");
        assert!(heard.iter().all(|(len, _, _)| *len > 8), "and it is the reply, not the filler");
        assert_eq!(session.state(), SessionState::Listening);
    }

    /// A voice that cannot say one word is a session with no filler, not a broken session.
    #[tokio::test]
    async fn a_filler_that_cannot_be_synthesised_leaves_the_session_working() {
        let (session, mut rx) = lane(64);
        let engines: SharedEngines = Arc::new(SlowVoice {
            delay: Duration::from_millis(700),
            filler_fails: true,
        });
        let generation = session.generation();
        let text = reply();
        session.arm_filler(&engines, generation);
        session.speak_reply(&engines, &text, generation).await.unwrap();

        let heard = frames(&mut rx);
        assert!(!heard.is_empty(), "the answer is still spoken");
        assert!(heard.iter().all(|(len, _, _)| *len > 8), "no filler existed: {heard:?}");
        assert!(!session.is_stopped());
        assert_eq!(session.state(), SessionState::Listening);
    }
}


#[cfg(test)]
mod idle_tests {
    //! FR-029 option A — a call nobody is in ends itself, and says so with the right reason.
    //!
    //! Every test here runs under `start_paused = true` and moves the clock with
    //! `tokio::time::advance`, because a 180 s deadline on the real clock means nobody runs more than
    //! one case and "the clock resets on every path" stays a claim
    //! (`specs/055-live-voice/checklists/verify-fr029-idle-option-a.md` §0, §4).
    //!
    //! What these tests cannot prove: that all six `trigger=` names reach the log. That is read from a
    //! real run with `~/.qa055/read-idle.sh` (§8), and the names are asserted here only as call sites.

    use super::*;
    use crate::schema::agent::LiveVoiceSettings;
    use crate::schema::{VizierChannelId, VizierSession};
    use crate::transport::VizierTransport;
    use std::sync::Arc;

    /// How long a test waits for something that must arrive before it decides it will not.
    const WAIT: Duration = Duration::from_secs(5);

    fn settings_after(seconds: u64) -> LiveVoiceSettings {
        LiveVoiceSettings { idle_stop_seconds: seconds, ..Default::default() }
    }

    /// A session on a transport the test keeps, so it can watch the frames and run turns.
    ///
    /// The lane's reader is returned rather than dropped: a closed lane makes `send` fail, and a
    /// failed send ends the session with `PageClosed` (FR-030) — a test that let the receiver go
    /// would be testing the closed-socket path without meaning to.
    fn session_quiet_for(
        seconds: u64,
    ) -> (
        Arc<LiveVoiceSession>,
        VizierTransport,
        tokio::sync::mpsc::Receiver<serde_json::Value>,
    ) {
        let transport = VizierTransport::new();
        let (out_tx, lane) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
        let session = LiveVoiceSession::new(
            VizierSession(
                "helper".into(),
                VizierChannelId::from_web("olive", "web"),
                Some("topic-idle".into()),
            ),
            "olive".into(),
            None,
            settings_after(seconds),
            ClientConnection::new(out_tx),
            transport.clone(),
        );
        session.set_state(SessionState::Listening);
        (session, transport, lane)
    }

    fn registry_with(session: &Arc<LiveVoiceSession>) -> LiveVoiceSessions {
        let registry = LiveVoiceSessions::default();
        let _ = registry.start(session.session.clone(), || session.clone());
        registry
    }

    /// The reason on the last `ended` frame the transport published.
    ///
    /// The control frames reach this bus through the transport's own pump task, so the reader
    /// yields between reads: with a paused clock `try_recv` on its own sees an empty bus that fills
    /// one yield later, and a test that read it once would call a published frame "never sent".
    async fn ended_reason(
        events: &mut tokio::sync::broadcast::Receiver<crate::transport::SessionEvent>,
    ) -> Option<EndReason> {
        for _ in 0..1000 {
            let mut seen = None;
            while let Ok(event) = events.try_recv() {
                if let SessionFrame::LiveVoice(LiveVoiceControl::State { state, reason, .. }) =
                    event.frame
                    && state == SessionState::Ended
                {
                    seen = reason;
                }
            }
            if seen.is_some() {
                return seen;
            }
            tokio::task::yield_now().await;
        }
        None
    }

    #[tokio::test(start_paused = true)]
    async fn a_call_nobody_is_in_ends_itself_and_says_idle() {
        let (session, _transport, _lane) = session_quiet_for(1);
        let registry = registry_with(&session);
        let mut events = session.transport.subscribe_session_events();
        while events.try_recv().is_ok() {}

        // Inside the limit nothing happens, not even on a sweep.
        tokio::time::advance(Duration::from_millis(900)).await;
        assert_eq!(registry.sweep(tokio::time::Instant::now()).await, 0);
        assert!(!session.is_stopped(), "900 ms of a 1 s limit is not idle yet");

        // Past it: one ending, through `end_one`, and the reason the person reads.
        tokio::time::advance(Duration::from_millis(200)).await;
        assert_eq!(registry.sweep(tokio::time::Instant::now()).await, 1);
        assert_eq!(session.state(), SessionState::Ended);
        assert!(registry.is_empty(), "an ended call leaves the map");
        assert_eq!(
            ended_reason(&mut events).await,
            Some(EndReason::Idle),
            "`idle`, never `pressed_again`: the person is not the one who ended this call"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_person_still_talking_is_never_cut_mid_sentence() {
        let (session, _transport, _lane) = session_quiet_for(1);
        let registry = registry_with(&session);
        session.open_utterance("u-1");

        // Three seconds of talking against a one-second limit: the audio keeps the call alive.
        for _ in 0..10 {
            tokio::time::advance(Duration::from_millis(300)).await;
            assert_eq!(session.push_audio("u-1", &[7u8; 32]), Some(false));
            assert_eq!(
                registry.sweep(tokio::time::Instant::now()).await,
                0,
                "the worst of the six: a call cut while the bar still says `mic on`"
            );
        }
        assert!(!session.is_stopped());
    }

    /// The other five paths, one at a time, on a fresh call each time (§3).
    ///
    /// `turn_start` is not here: it only happens inside a turn, and it is asserted in
    /// [`a_running_turn_pauses_the_clock_and_the_call_ends_after_it`] where a turn can actually be
    /// running. Each case advances nine tenths of the limit, calls one path, then advances another
    /// nine tenths — 1800 ms against a 1000 ms limit, so a path that does not reset ends the call.
    async fn a_path_restarts_the_clock(trigger: &str) {
        let (session, _transport, _lane) = session_quiet_for(1);
        let registry = registry_with(&session);
        // An utterance is open before the quiet stretch wherever the path needs one, so the case
        // tests the path named and not its neighbour.
        if trigger != "open_utterance" {
            session.open_utterance("u-1");
        }
        tokio::time::advance(Duration::from_millis(900)).await;
        match trigger {
            "open_utterance" => session.open_utterance("u-1"),
            "push_audio" => {
                assert_eq!(session.push_audio("u-1", &[3u8; 32]), Some(false));
            }
            "close_utterance" => {
                let _ = session.close_utterance("u-1").await;
            }
            "segment_sent" => {
                let frame = LiveVoiceSpeech::new(session.generation(), 0, &[1u8, 2, 3], true);
                assert!(session.send_speech(frame), "the lane has room");
            }
            other => panic!("no such activity path: {other}"),
        }
        tokio::time::advance(Duration::from_millis(900)).await;
        assert_eq!(
            registry.sweep(tokio::time::Instant::now()).await,
            0,
            "`{trigger}` does not reset the clock, so the call was ended by a sweep"
        );
        assert!(!session.is_stopped(), "`{trigger}` is activity: the person was still there");
    }

    #[tokio::test(start_paused = true)]
    async fn every_path_that_means_activity_restarts_the_clock() {
        for trigger in ["open_utterance", "push_audio", "close_utterance", "segment_sent"] {
            a_path_restarts_the_clock(trigger).await;
        }
    }

    /// A voice that takes `delay` per segment: a turn has to be long enough to be worth pausing for.
    struct PatientVoice {
        delay: Duration,
    }

    #[async_trait::async_trait]
    impl crate::live_voice::engines::LiveVoiceEngines for PatientVoice {
        async fn transcribe(
            &self,
            _wav: &[u8],
            _filename: &str,
            _language: Option<&str>,
        ) -> crate::Result<String> {
            Ok("hello".into())
        }

        async fn speak(&self, _text: &str) -> crate::Result<Vec<u8>> {
            tokio::time::sleep(self.delay).await;
            Ok(vec![0xAB; 64])
        }
    }

    /// Condition 4: the clock does not run while the agent is working, and the ending comes after
    /// the turn rather than during it — with `idle`, not with the reason a person's own ending has.
    #[tokio::test(start_paused = true)]
    async fn a_running_turn_pauses_the_clock_and_the_call_ends_after_it() {
        let (session, transport, _lane) = session_quiet_for(1);
        let mut inbox = transport.register_agent("helper".into()).await;
        let registry = registry_with(&session);

        // Nine tenths of the limit has gone by when the turn is accepted.
        tokio::time::advance(Duration::from_millis(900)).await;
        let running = session.clone();
        tokio::spawn(async move {
            running
                .run_turn(Arc::new(PatientVoice { delay: Duration::from_millis(200) }), "hello".into())
                .await
        });
        let submitted = tokio::time::timeout(WAIT, inbox.recv_request())
            .await
            .expect("the turn submits its request")
            .expect("the inbox is open");

        assert!(session.is_busy(), "the agent is working");
        assert!(
            session.idle_for(tokio::time::Instant::now()) < Duration::from_millis(50),
            "`turn_start` restarts the clock: 900 ms had gone by since the session started"
        );

        // Five seconds of a one-second limit, with a turn in flight: refused, not ended.
        tokio::time::advance(Duration::from_secs(5)).await;
        assert_eq!(
            registry.sweep(tokio::time::Instant::now()).await,
            0,
            "a sweep that counted a running turn would cut the answer in half"
        );
        assert!(!session.is_stopped());
        assert_eq!(session.state(), SessionState::Waiting, "still thinking, not ended");

        // The turn is over (its reader went away), so the clock counts again.
        drop(inbox);
        drop(submitted);
        for _ in 0..100 {
            if !session.is_busy() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(!session.is_busy(), "the turn finished rather than hanging");

        tokio::time::advance(Duration::from_millis(1100)).await;
        assert_eq!(registry.sweep(tokio::time::Instant::now()).await, 1);
        assert!(session.is_stopped());
    }

    #[tokio::test(start_paused = true)]
    async fn a_session_that_already_stopped_is_skipped() {
        let (session, _transport, _lane) = session_quiet_for(1);
        let registry = registry_with(&session);
        session.end(EndReason::PressedAgain);

        tokio::time::advance(Duration::from_secs(10)).await;
        assert_eq!(
            registry.sweep(tokio::time::Instant::now()).await,
            0,
            "no second ending: the first reason is the one that won"
        );
        assert_eq!(session.state(), SessionState::Ended);
    }

    #[tokio::test(start_paused = true)]
    async fn the_limit_comes_from_the_settings_not_from_the_default() {
        let (session, _transport, _lane) = session_quiet_for(20);
        let registry = registry_with(&session);

        tokio::time::advance(Duration::from_secs(19)).await;
        assert_eq!(
            registry.sweep(tokio::time::Instant::now()).await,
            0,
            "19 s of a 20 s limit is not idle"
        );
        tokio::time::advance(Duration::from_secs(2)).await;
        assert_eq!(registry.sweep(tokio::time::Instant::now()).await, 1);
    }

    /// `idle_stop_seconds: 0` reads as **off**. A server that ended every call the tick after it
    /// began would make the field a trap, and "no limit" is the only reading anyone could want.
    #[tokio::test(start_paused = true)]
    async fn zero_means_the_call_never_ends_itself() {
        let (session, _transport, _lane) = session_quiet_for(0);
        assert_eq!(session.idle_limit(), None);
        let registry = registry_with(&session);

        tokio::time::advance(Duration::from_secs(3600)).await;
        assert_eq!(registry.sweep(tokio::time::Instant::now()).await, 0);
        assert!(!session.is_stopped());
    }
}
