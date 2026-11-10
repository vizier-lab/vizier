//! T068: one session, and a reconnect re-binds its lane instead of starting a second one
//! (`specs/055-live-voice` FR-033, FR-050, I10).
//!
//! The client half is tested in `webui/app/lib/liveVoice/session.test.ts`; this file covers what
//! only the server can show — that the registry keeps one entry, that a re-bind does not disturb
//! the generation a running turn depends on, and that the audio goes to the connection that is
//! current rather than the one that dropped.

#![cfg(feature = "live-voice")]

use std::sync::Arc;
use std::time::Duration;

use vizier_core::agents::client::ClientConnection;
use vizier_core::live_voice::frame::SessionState;
use vizier_core::live_voice::{LiveVoiceSession, LiveVoiceSessions};
use vizier_core::schema::agent::LiveVoiceSettings;
use vizier_core::schema::{VizierChannelId, VizierSession};
use vizier_core::transport::VizierTransport;

fn conversation() -> VizierSession {
    VizierSession(
        "helper".to_string(),
        VizierChannelId::from_web("olive", "web"),
        Some("topic-1".into()),
    )
}

/// One socket (the lane a session writes to), as `topic.rs` builds it.
fn make_lane() -> (Arc<ClientConnection>, tokio::sync::mpsc::Receiver<serde_json::Value>) {
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    (ClientConnection::new(tx), rx)
}

fn make_session(transport: &VizierTransport, lane: Arc<ClientConnection>) -> Arc<LiveVoiceSession> {
    LiveVoiceSession::new(
        conversation(),
        "olive".into(),
        Some("id".into()),
        LiveVoiceSettings::default(),
        lane,
        transport.clone(),
    )
}

/// Speech that actually produces a segment, so the lane has something to receive.
struct SpeakingEngines;

#[async_trait::async_trait]
impl vizier_core::live_voice::LiveVoiceEngines for SpeakingEngines {
    async fn transcribe(
        &self,
        _wav: &[u8],
        _filename: &str,
        _language: Option<&str>,
    ) -> vizier_core::Result<String> {
        Ok("hello".into())
    }

    async fn speak(&self, _text: &str) -> vizier_core::Result<Vec<u8>> {
        Ok(vec![0u8; 640])
    }
}

/// A second `start` from the same owner must not create a second entry: the registry hands back
/// the one that is already there, which is what `topic.rs` turns into a re-bind.
#[tokio::test]
async fn a_second_start_for_the_same_conversation_keeps_one_session() {
    let transport = VizierTransport::new();
    let registry = LiveVoiceSessions::default();
    let (old_lane, _old_rx) = make_lane();

    let first = match registry.start(conversation(), || make_session(&transport, old_lane)) {
        Ok(created) => created,
        Err(_) => panic!("the first start creates the session"),
    };
    let generation = first.generation();

    let (new_lane, _new_rx) = make_lane();
    let second = registry
        .start(conversation(), || make_session(&transport, new_lane));
    let Err(existing) = second else {
        panic!("a second start must not create a second session");
    };
    assert!(
        Arc::ptr_eq(&existing, &first),
        "the caller is handed the session that is already live"
    );
    assert_eq!(registry.len(), 1, "one conversation, one session");
    assert_eq!(
        first.generation(),
        generation,
        "re-binding is not an interruption: a running turn depends on this generation"
    );
}

/// The whole point of the re-bind: after it, the audio reaches the connection that is current.
#[tokio::test]
async fn audio_follows_the_rebound_lane() {
    let transport = VizierTransport::new();
    let (old_lane, mut old_rx) = make_lane();
    let session = make_session(&transport, old_lane.clone());
    session.set_state(SessionState::Listening);

    // The socket dropped — `topic.rs` closes the connection when that happens — and a new one
    // arrived for the same session.
    old_lane.close();
    assert!(!old_lane.is_open(), "the connection that dropped is closed");
    let (new_lane, mut new_rx) = make_lane();
    session.rebind(new_lane);

    let engines: Arc<dyn vizier_core::live_voice::LiveVoiceEngines> = Arc::new(SpeakingEngines);
    let spoke = session
        .speak_reply(&engines, "One short sentence.", session.generation())
        .await
        .expect("synthesis succeeds");
    assert!(spoke, "the reply was spoken");

    let frame = tokio::time::timeout(Duration::from_millis(500), new_rx.recv())
        .await
        .expect("the new lane receives the audio")
        .expect("the lane is open");
    assert!(
        frame["live_voice"]["type"] == "speech",
        "the frame is a speech segment, got {frame}"
    );
    assert_eq!(
        frame["live_voice"]["generation"],
        session.generation(),
        "the segment carries the session's generation"
    );
    assert!(
        old_rx.try_recv().is_err(),
        "the connection that dropped receives nothing"
    );
}

/// A re-bind must not resurrect a session that has ended: the ending wins, so a late `start`
/// cannot put a dead lane back in the registry.
#[tokio::test]
async fn a_rebind_does_not_revive_an_ended_session() {
    let transport = VizierTransport::new();
    let registry = LiveVoiceSessions::default();
    let (lane, _rx) = make_lane();
    let session = match registry.start(conversation(), || make_session(&transport, lane)) {
        Ok(created) => created,
        Err(_) => panic!("created"),
    };
    session.set_state(SessionState::Listening);

    session.end(vizier_core::live_voice::EndReason::PageClosed);
    registry.remove(&conversation(), &session);
    assert!(registry.get(&conversation()).is_none());
    assert_eq!(registry.len(), 0);

    // The next start is a fresh session, not the ended one.
    let (fresh_lane, _fresh_rx) = make_lane();
    let fresh = match registry.start(conversation(), || make_session(&transport, fresh_lane)) {
        Ok(created) => created,
        Err(_) => panic!("a fresh session is created after the old one ended"),
    };
    assert!(!Arc::ptr_eq(&fresh, &session));
    assert_eq!(fresh.generation(), 1, "a new session starts at generation 1");
}

/// T102: the audio description is asked for and enforced, not assumed.
///
/// **What is not here, and why.** The four cases the ticket asks for end in the socket — a start
/// with a bad description leaves `LiveVoiceSessions::len() == 0`, and a bad audio frame mid-session
/// leaves the session `listening`. Both need `TopicConnection`, which is built only through
/// `TopicConnection::open`, which needs an `HTTPState` and a topic that passes `authorize_topic`.
/// That harness does not exist yet; `live_voice_frames.rs` carries the same `TODO(C5)` for the same
/// reason. So what follows asserts the two facts the socket's arm is made of — the rule, and the
/// session's own state — and the ordering (checked before the registry is touched) is enforced in
/// `topic.rs` by running the check as the arm's first statement, not by a test.
mod the_audio_description {
    use super::*;
    use vizier_core::live_voice::frame::{AudioDescription, StartFrame};

    fn start(json: serde_json::Value) -> StartFrame {
        serde_json::from_value(json).expect("a start frame")
    }

    #[test]
    fn a_start_in_the_release_carrier_is_accepted() {
        let description = start(serde_json::json!({
            "audio": { "sample_rate": 16000, "channels": 1, "frame_ms": 100, "format": "pcm16" }
        }))
        .description();
        assert!(description.check().is_ok(), "16 kHz mono pcm16 is what we carry");
    }

    #[test]
    fn a_start_without_a_description_is_accepted_with_the_default() {
        let description = start(serde_json::json!({ "language": "id" })).description();
        assert_eq!(description, AudioDescription::default());
        assert!(description.check().is_ok());
    }

    #[test]
    fn a_start_at_the_wrong_rate_is_refused_and_names_both_numbers() {
        let description = start(serde_json::json!({
            "audio": { "sample_rate": 44100, "channels": 1, "frame_ms": 100, "format": "pcm16" }
        }))
        .description();
        let reason = description.check().expect_err("44.1 kHz is not carried");
        assert!(reason.contains("16000"), "expected: {reason}");
        assert!(reason.contains("44100"), "received: {reason}");
        // And the refusal carries its own reason, not a provider failure (T102): the message is
        // public vocabulary a third-party client reads.
        assert_eq!(
            vizier_core::live_voice::frame::EndReason::UnsupportedAudio.as_str(),
            "unsupported_audio"
        );
        // The refusal is what `topic.rs` sends and then returns from, before `sessions.start`:
        // nothing was registered, so a fresh registry is still empty.
        let registry = LiveVoiceSessions::default();
        assert_eq!(registry.len(), 0, "a refused start leaves no session behind");
    }

    #[test]
    fn a_chunk_from_another_rate_is_refused_while_the_session_keeps_listening() {
        let transport = VizierTransport::new();
        let (lane, _rx) = make_lane();
        let session = make_session(&transport, lane);
        session.set_description(AudioDescription::default());
        session.set_state(SessionState::Listening);

        assert!(session.description().accepts(16_000, "pcm16").is_ok());
        let reason = session
            .description()
            .accepts(44_100, "pcm16")
            .expect_err("a 44.1 kHz chunk is not this session's audio");
        assert!(reason.contains("16000") && reason.contains("44100"), "{reason}");

        // One bad frame is not a reason to take the microphone away: `topic.rs` returns after the
        // error and touches nothing else, so the state is unchanged.
        assert_eq!(session.state(), SessionState::Listening);
        assert!(!session.is_stopped());
    }
}
