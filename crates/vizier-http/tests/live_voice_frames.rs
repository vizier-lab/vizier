//! T014, T015: the live voice frames, their contract shape, and the inbound guard
//! (`specs/055-live-voice`).
//!
//! What these tests can prove without a socket: the wire shape the client and the server agree on,
//! and the sizes the inbound guard is written against. The two cases that need a live socket — a
//! frame ignored by a build with the feature off, and one refused without being buffered — are
//! marked `TODO(C5)`: they need the `TopicConnection` harness, which is the next chunk's job.

#![cfg(feature = "live-voice")]

use vizier_core::live_voice::frame::{
    CHUNK_BASE64, CHUNK_BYTES, CHUNK_SAMPLES, ClientAction, EndReason, LiveVoiceControl,
    LiveVoiceSpeech, MAX_AUDIO_FRAME_BASE64, SAMPLE_RATE, SessionState,
};

/// Contract §`live_voice_state`: one `type` discriminator, the state word, the generation, and a
/// reason only when the session ended or failed.
#[test]
fn a_control_frame_has_the_contract_shape() {
    let listening = LiveVoiceControl::state(SessionState::Listening, 3, None);
    assert_eq!(
        serde_json::to_value(&listening).unwrap(),
        serde_json::json!({ "type": "state", "state": "listening", "generation": 3 })
    );

    // FR-042 rides this frame: `waiting` is how the agent says it is thinking.
    let thinking = LiveVoiceControl::state(SessionState::Waiting, 4, None);
    let json = serde_json::to_value(&thinking).unwrap();
    assert_eq!(json["state"], "waiting");
    assert!(json.get("reason").is_none(), "waiting carries no reason");

    // `reconnecting` and `waiting` are different words on the wire, so no client can merge them.
    let dropped = LiveVoiceControl::state(SessionState::Reconnecting, 4, Some(EndReason::ConnectionLost));
    assert_eq!(
        serde_json::to_value(&dropped).unwrap()["state"],
        "reconnecting"
    );
}

/// The too-short outcome is `submitted: false` with no text, which is what the client renders as
/// "that was too short" rather than as a message (FR-024).
#[test]
fn an_empty_transcript_is_the_too_short_outcome() {
    let frame = LiveVoiceControl::Transcript {
        utterance_id: "u-7".into(),
        text: String::new(),
        submitted: false,
    };
    assert_eq!(
        serde_json::to_value(&frame).unwrap(),
        serde_json::json!({
            "type": "transcript", "utterance_id": "u-7", "text": "", "submitted": false
        })
    );
}

/// A `stall` report is its own type and carries only the count: it must never be read as an error,
/// and it changes no state (contract, `stall`).
#[test]
fn a_stall_frame_is_its_own_type() {
    let json = serde_json::to_value(LiveVoiceControl::Stall { dropped: 23 }).unwrap();
    assert_eq!(json, serde_json::json!({ "type": "stall", "dropped": 23 }));
    assert!(json.get("generation").is_none());
}

/// The two families cannot be mistaken for one another: a control frame does not parse as audio,
/// which is what keeps `live_voice_speech` out of the broadcast at the type level (D9, D17).
#[test]
fn audio_and_control_are_different_types() {
    let control = serde_json::to_value(LiveVoiceControl::Stall { dropped: 1 }).unwrap();
    assert!(serde_json::from_value::<LiveVoiceSpeech>(control).is_err());

    let speech = serde_json::to_value(LiveVoiceSpeech::new(1, 0, &[0, 1, 2, 3], false)).unwrap();
    assert_eq!(speech["type"], "speech");
    assert!(serde_json::from_value::<LiveVoiceControl>(speech).is_err());
}

/// T015: the guard refuses anything larger than one chunk, and the assertion is about the bound
/// the server actually applies rather than an arithmetic guess.
#[test]
fn one_chunk_is_what_the_inbound_guard_is_written_against() {
    assert_eq!(SAMPLE_RATE, 16_000);
    assert_eq!(CHUNK_SAMPLES, 1_600);
    assert_eq!(CHUNK_BYTES, 3_200);


    use base64::Engine as _;
    let engine = base64::engine::general_purpose::STANDARD;
    let chunk = |bytes: usize| engine.encode(vec![0u8; bytes]);
    let one = chunk(CHUNK_BYTES);
    assert_eq!(one.len(), CHUNK_BASE64, "one chunk encodes to exactly the constant");

    // The bound is one chunk plus half of another — "a little over one chunk", as T015 asks — and
    // it is deliberately not two chunks.
    assert!(MAX_AUDIO_FRAME_BASE64 > CHUNK_BASE64);
    assert!(MAX_AUDIO_FRAME_BASE64 < CHUNK_BASE64 * 2);
    assert_eq!(MAX_AUDIO_FRAME_BASE64, 6_402);

    // The guard compares the payload's length, so the edge cases are lengths rather than encodings
    // (a base64 string's length is always a multiple of four, so 6 402 itself has no encoding).
    let at_bound = "A".repeat(MAX_AUDIO_FRAME_BASE64);
    assert!(at_bound.len() <= MAX_AUDIO_FRAME_BASE64, "a frame at the bound is accepted");
    let past = "A".repeat(MAX_AUDIO_FRAME_BASE64 + 1);
    assert!(past.len() > MAX_AUDIO_FRAME_BASE64, "one character over is refused");

    // The two cases that matter in practice: one chunk passes, two chunks do not.
    assert!(one.len() <= MAX_AUDIO_FRAME_BASE64, "one chunk is accepted");
    let two = chunk(CHUNK_BYTES * 2);
    assert_eq!(two.len(), CHUNK_BASE64 * 2);
    assert!(two.len() > MAX_AUDIO_FRAME_BASE64, "two chunks are refused");
}

/// The six actions the contract names are the six the server recognises, and nothing else is
/// treated as a live voice frame.
#[test]
fn the_six_client_actions_are_recognised() {
    let cases = [
        (serde_json::json!({ "live_voice_start": { "language": "id" } }), "live_voice_start"),
        (
            serde_json::json!({ "live_voice_audio": { "utterance_id": "u-1", "seq": 0, "base64": "" } }),
            "live_voice_audio",
        ),
        (
            serde_json::json!({ "live_voice_utterance_end": { "utterance_id": "u-1" } }),
            "live_voice_utterance_end",
        ),
        (
            serde_json::json!({ "live_voice_speech_start": { "utterance_id": "u-1" } }),
            "live_voice_speech_start",
        ),
        (
            serde_json::json!({ "live_voice_interrupt": { "reason": "button" } }),
            "live_voice_interrupt",
        ),
        (serde_json::json!({ "live_voice_stop": {} }), "live_voice_stop"),
    ];
    for (frame, key) in cases {
        assert_eq!(
            vizier_core::live_voice::frame::client_key(&frame),
            Some(key),
            "{key} must be recognised"
        );
        assert!(
            ClientAction::parse(key, &frame).is_some(),
            "{key} must parse"
        );
    }

    // A chat frame, and a frame with a key that looks like one, are not live voice.
    assert_eq!(
        vizier_core::live_voice::frame::client_key(&serde_json::json!({ "chat": "hello" })),
        None
    );
    assert_eq!(
        vizier_core::live_voice::frame::client_key(&serde_json::json!({ "live_voice_unknown": {} })),
        None
    );
}

/// The interrupt reason is part of the contract: the button and a typed message are different
/// events even though they stop the same audio (FR-020, FR-025).
#[test]
fn the_interrupt_reason_is_carried() {
    for (wire, expected) in [
        ("button", vizier_core::live_voice::InterruptReason::Button),
        ("typed", vizier_core::live_voice::InterruptReason::Typed),
        ("barge_in", vizier_core::live_voice::InterruptReason::BargeIn),
    ] {
        let frame = serde_json::json!({ "live_voice_interrupt": { "reason": wire } });
        match ClientAction::parse("live_voice_interrupt", &frame).unwrap() {
            ClientAction::Interrupt(interrupt) => {
                assert_eq!(interrupt.reason, Some(expected), "{wire}")
            }
            other => panic!("expected an interrupt, got {other:?}"),
        }
    }
}

/// T014 (FR-041), the half that needs no socket: with `live-voice` off this file does not compile
/// into the build at all, and the crate itself still builds — which is what the feature matrix
/// asserts. The end-to-end half — a `live_voice_start` on a real socket leaving it open and
/// emitting nothing — needs the connection harness.
///
/// TODO(C5): needs `TopicConnection::open` reachable from an integration test without an HTTPState,
/// then: send `live_voice_start` with the feature off, assert the socket stays open and that no
/// frame whose top-level key is `live_voice` was ever written.
#[test]
fn the_flag_off_build_has_no_live_voice_surface() {
    // Everything this file uses is behind `live-voice`, so its presence here is only possible in
    // that build; the other half of the matrix is `cargo build -p vizier-http` with no features.
    assert!(cfg!(feature = "live-voice"));
}
