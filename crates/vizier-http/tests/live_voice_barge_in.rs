//! T044: an interruption stops the audio **and** cancels the turn (`specs/055-live-voice`
//! FR-027).
//!
//! The second half is the one that used to be missing: the session stopped speaking while the
//! agent kept working, so the answer was still written and still saved to history. These tests
//! cover the mechanism itself — `LiveVoiceSession::abort_turn` — and are therefore independent of
//! *when* it is called; the trigger moved from `speech_start` to the end of the utterance by the
//! User's decision of 2026-11-08 21:19, and that lives in the socket, not here.
//!
//! The real primitives are used throughout: a `VizierTransport`, an agent inbox registered on it
//! (so a `Command("abort")` request is observable), a `ClientConnection` lane for the audio, and
//! the session broadcast for the control frames.

#![cfg(feature = "live-voice")]

use std::sync::Arc;
use std::time::Duration;

use vizier_core::agents::client::ClientConnection;
use vizier_core::live_voice::frame::{InterruptReason, SessionState};
use vizier_core::live_voice::{LiveVoiceSession, Utterance};
use vizier_core::schema::agent::LiveVoiceSettings;
use vizier_core::schema::{
    VizierChannelId, VizierRequestContent, VizierSession,
};
use vizier_core::transport::{Inbox, SessionFrame, VizierTransport};

/// How long a test waits for a frame or a request before deciding it will not come.
const WAIT: Duration = Duration::from_millis(500);

fn session_of(agent: &str) -> VizierSession {
    VizierSession(
        agent.to_string(),
        VizierChannelId::from_web("olive", "web"),
        Some("topic-1".into()),
    )
}

fn describe(content: &VizierRequestContent) -> String {
    match content {
        VizierRequestContent::Command(command) => format!("command:{command}"),
        VizierRequestContent::Chat(text) => format!("chat:{text}"),
        other => format!("{other}"),
    }
}

/// A session on a real transport, with the agent's inbox registered and its lane captured.
struct Harness {
    session: Arc<LiveVoiceSession>,
    inbox: Inbox,
    lane: tokio::sync::mpsc::Receiver<serde_json::Value>,
    events: tokio::sync::broadcast::Receiver<vizier_core::transport::SessionEvent>,
}

impl Harness {
    async fn new() -> Self {
        let transport = VizierTransport::new();
        let inbox = transport.register_agent("helper".into()).await;
        let (out_tx, lane) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
        let events = transport.subscribe_session_events();
        let session = LiveVoiceSession::new(
            session_of("helper"),
            "olive".into(),
            Some("id".into()),
            LiveVoiceSettings::default(),
            ClientConnection::new(out_tx),
            transport,
        );
        session.set_state(SessionState::Listening);
        Self { session, inbox, lane, events }
    }

    /// Start a turn the way the session does, and let it reach the point where it is waiting for
    /// the reply. Returns once the request has been accepted by the agent's inbox.
    async fn start_turn(&mut self, engines: Arc<dyn vizier_core::live_voice::LiveVoiceEngines>) {
        let session = self.session.clone();
        tokio::spawn(async move { session.run_turn(engines, "hello".into()).await });
        let envelope = tokio::time::timeout(WAIT, self.inbox.recv_request())
            .await
            .expect("the turn submits its request")
            .expect("the inbox is open");
        assert_eq!(describe(&envelope.request.content), "chat:hello");
        // `thinking()` has run by now; give the pump a moment to deliver it.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    /// Every request the agent's inbox received, drained.
    ///
    /// `recv_request` waits forever on an empty inbox, so this polls with a timeout and stops at
    /// the first quiet gap. A blocking drain is how two tests in this file hung on the first run.
    async fn drain_requests(&self) -> Vec<String> {
        let mut kinds = Vec::new();
        while let Ok(Ok(envelope)) =
            tokio::time::timeout(Duration::from_millis(50), self.inbox.recv_request()).await
        {
            kinds.push(describe(&envelope.request.content));
        }
        kinds
    }

    /// The next state word published, waiting up to `wait`.
    ///
    /// The broadcast is fed by the event pump, so a state is never observable the instant it is
    /// published; a bare `try_recv` would be a race rather than a test.
    async fn next_state(&mut self, wait: Duration) -> Option<SessionState> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() {
                return None;
            }
            let Ok(Ok(event)) = tokio::time::timeout(left, self.events.recv()).await else {
                return None;
            };
            if let SessionFrame::LiveVoice(vizier_core::live_voice::LiveVoiceControl::State {
                state,
                ..
            }) = event.frame
            {
                return Some(state);
            }
        }
    }

    /// The states published until `want` is seen, or until the wait runs out.
    async fn states_until(&mut self, want: SessionState, wait: Duration) -> Vec<SessionState> {
        let deadline = tokio::time::Instant::now() + wait;
        let mut seen = Vec::new();
        while tokio::time::Instant::now() < deadline {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            match self.next_state(left).await {
                Some(state) => {
                    let done = state == want;
                    seen.push(state);
                    if done {
                        break;
                    }
                }
                None => break,
            }
        }
        seen
    }
}

/// A session that answers without speaking; these tests only need the turn's shape.
struct SilentEngines;

#[async_trait::async_trait]
impl vizier_core::live_voice::LiveVoiceEngines for SilentEngines {
    async fn transcribe(
        &self,
        _wav: &[u8],
        _filename: &str,
        _language: Option<&str>,
    ) -> vizier_core::Result<String> {
        Ok("hello".into())
    }

    async fn speak(&self, _text: &str) -> vizier_core::Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

/// The load-bearing test: an interruption sends `Command("abort")`, exactly once, after the
/// generation has moved. Without it the agent keeps working and the answer is still saved.
#[tokio::test]
async fn a_barge_in_cancels_the_running_turn_exactly_once() {
    let mut h = Harness::new().await;
    h.start_turn(Arc::new(SilentEngines)).await;

    let before = h.session.generation();
    h.session.abort_turn(InterruptReason::BargeIn).await;
    assert_eq!(
        h.session.generation(),
        before + 1,
        "an interruption moves the generation, so in-flight segments are stale"
    );

    let kinds = h.drain_requests().await;
    assert_eq!(
        kinds.iter().filter(|kind| *kind == "command:abort").count(),
        1,
        "the turn is cancelled by exactly one abort, got {kinds:?}"
    );
    assert_eq!(h.session.state(), SessionState::Listening);
}

#[tokio::test]
async fn a_segment_from_the_old_generation_is_dropped() {
    let mut h = Harness::new().await;
    h.start_turn(Arc::new(SilentEngines)).await;
    let old = h.session.generation();
    h.session.abort_turn(InterruptReason::BargeIn).await;

    // Every speech frame carries the generation it belongs to, so a segment from the old one is
    // stale by the time it would be written (FR-022).
    let stale = vizier_core::live_voice::LiveVoiceSpeech::new(old, 0, &[0u8; 32], false);
    assert!(
        stale.generation < h.session.generation(),
        "the old segment is stale"
    );
    assert!(
        h.lane.try_recv().is_err(),
        "no audio is written after the interruption"
    );
}

/// FR-042: the indicator cannot outlive the turn it describes.
#[tokio::test]
async fn the_thinking_indicator_goes_off_with_the_interruption() {
    let mut h = Harness::new().await;
    h.start_turn(Arc::new(SilentEngines)).await;

    // A turn announces `waiting` first — that is the indicator.
    let seen = h.states_until(SessionState::Waiting, WAIT).await;
    assert_eq!(
        seen.last(),
        Some(&SessionState::Waiting),
        "the turn must announce thinking, got {seen:?}"
    );

    h.session.abort_turn(InterruptReason::BargeIn).await;
    let after = h.states_until(SessionState::Listening, WAIT).await;
    assert_eq!(
        after.last(),
        Some(&SessionState::Listening),
        "the interruption returns the session to listening, got {after:?}"
    );

    // And nothing puts the indicator back on: a cancelled turn has no silence left to explain.
    let later: Vec<SessionState> = {
        let mut seen = Vec::new();
        while let Some(state) = h.next_state(Duration::from_millis(150)).await {
            seen.push(state);
        }
        seen
    };
    assert!(
        !later.contains(&SessionState::Waiting),
        "no turn may re-announce thinking after its interruption, got {later:?}"
    );
    assert_ne!(h.session.state(), SessionState::Waiting);
}

/// FR-021: the control is available for the whole session, so pressing it with nothing running
/// must cost nothing — no abort, no state change, no audio.
#[tokio::test]
async fn interrupting_an_idle_session_sends_no_abort() {
    let mut h = Harness::new().await;
    h.session.abort_turn(InterruptReason::Button).await;
    h.session.abort_turn(InterruptReason::Typed).await;

    let kinds = h.drain_requests().await;
    assert!(
        kinds.iter().all(|kind| kind != "command:abort"),
        "a press with nothing running costs nothing (FR-021), got {kinds:?}"
    );
    assert_eq!(h.session.state(), SessionState::Listening);
    assert!(h.lane.try_recv().is_err(), "and writes no audio");
}

/// The three reasons are distinct values on the wire, so a test or a log can tell them apart.
#[test]
fn the_interrupt_reasons_are_not_interchangeable() {
    assert_eq!(InterruptReason::BargeIn.as_str(), "barge_in");
    assert_eq!(InterruptReason::Button.as_str(), "button");
    assert_eq!(InterruptReason::Typed.as_str(), "typed");
    assert_ne!(
        InterruptReason::BargeIn.as_str(),
        InterruptReason::Button.as_str()
    );
}

/// An interruption also cancels the utterance being captured, so its audio is never submitted.
#[test]
fn an_interruption_cancels_the_open_utterance() {
    let transport = VizierTransport::new();
    let (tx, _rx) = tokio::sync::mpsc::channel::<serde_json::Value>(8);
    let session = LiveVoiceSession::new(
        session_of("helper"),
        "olive".into(),
        None,
        LiveVoiceSettings::default(),
        ClientConnection::new(tx),
        transport,
    );
    session.open_utterance("u-1");
    session.push_audio("u-1", &[0u8; 3200]);
    let taken: Option<Utterance> = session.take_utterance("u-1");
    assert!(taken.is_some());
    assert!(session.take_utterance("u-1").is_none(), "it is taken once");
}
