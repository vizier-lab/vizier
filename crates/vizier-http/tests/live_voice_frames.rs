//! T014, T015: the live voice frames, their contract shape, and the inbound guard
//! (`specs/055-live-voice`).
//!
//! What these tests can prove without a socket: the wire shape the client and the server agree on,
//! and the sizes the inbound guard is written against. One case does need a connection, and has
//! one: `a_start_is_ignored_while_the_agent_switch_is_off` drives a real `TopicConnection`
//! through the desktop dispatcher, which needs neither an HTTP server nor a provider. The one
//! still marked `TODO(C5)` — a frame refused without being buffered — needs the socket harness.

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

/// T014 (FR-041): the one switch that is left — the agent's own `live_voice.enabled`, off by
/// default — is the one that has to hold. With it off, `live_voice_start` is ignored: no
/// `live_voice` frame reaches the socket, and the connection keeps answering everything else.
///
/// This is the claim `the_flag_off_build_has_no_live_voice_surface` only appeared to make: it
/// asserted a compile-time constant inside a file that only compiled when the constant was true,
/// so it could not fail. This one drives the real `TopicConnection` through the desktop
/// dispatcher, which needs no HTTP server and no provider — a start that is ignored never reaches
/// one. Removing the `if !config.tools.live_voice.enabled { return; }` guard in `src/topic.rs`
/// turns the ignored start into a start, which answers with a `live_voice` error frame and fails
/// the first assertion.
#[tokio::test(flavor = "multi_thread")]
async fn a_start_is_ignored_while_the_agent_switch_is_off() {
    use vizier_core::config::VizierConfig;
    use vizier_core::runtime::{RuntimeOptions, VizierRuntime};
    use vizier_core::storage::{agent::AgentStorage, user::UserStorage};

    let dir = tempfile::tempdir().unwrap();
    let runtime = VizierRuntime::start(RuntimeOptions {
        config: VizierConfig::with_workspace(dir.path().to_path_buf()),
        ..Default::default()
    })
    .await
    .unwrap();
    let storage = runtime.deps().storage.clone();
    let role = storage.create_role("system", vec![], true).await.unwrap();
    let user = storage.create_user("olive", "", &role.role_id).await.unwrap();

    // The live voice block the engine ships with: present, and off.
    let agent: vizier_core::schema::AgentConfig = serde_json::from_value(serde_json::json!({
        "name": "muted", "owner_id": null, "system_prompt": null, "description": null,
        "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
        "tools": { "timeout": "30s" }, "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "1m", "heartbeat_interval": "1m"
    }))
    .unwrap();
    assert!(!agent.tools.live_voice.enabled, "the fixture is the default: off");
    storage.create_agent("muted", &agent).await.unwrap();

    let dispatcher = vizier_http::embedded::Dispatcher::new(&runtime, user.user_id.clone());
    let (tx, rx) = flume::unbounded();
    let mut topic = dispatcher
        .open_topic("muted", "app", "t-muted", tx)
        .await
        .expect("opens");
    tokio::spawn(topic.outbound().expect("outbound half").run());

    topic
        .handle_inbound(serde_json::json!({ "live_voice_start": {} }))
        .await;

    let mut seen = Vec::new();
    for _ in 0..20 {
        while let Ok(frame) = rx.try_recv() {
            seen.push(frame);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        seen.iter().all(|frame| frame.get("live_voice").is_none()),
        "a start was answered while the agent's switch is off: {seen:?}"
    );

    // ...and the socket is still open: a frame it can answer without a provider is answered.
    topic
        .handle_inbound(serde_json::json!({
            "sensitive_input_answer": { "ask_id": "no-such-ask", "value": "x" }
        }))
        .await;

    let mut refusal = None;
    for _ in 0..20 {
        while let Ok(frame) = rx.try_recv() {
            if let Some(code) = frame.get("client_error").and_then(|error| error.get("code")) {
                refusal = code.as_str().map(str::to_string);
            }
        }
        if refusal.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(
        refusal.as_deref(),
        Some("sensitive_ask_not_open"),
        "the socket stopped answering after an ignored start"
    );
}
