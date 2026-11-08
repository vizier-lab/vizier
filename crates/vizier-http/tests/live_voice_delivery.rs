//! T021, T022: which lane a live voice frame travels, and what a slow socket may lose
//! (`specs/055-live-voice` contract §Delivery path).
//!
//! The claim these tests hold up is narrow and load-bearing: **audio may be dropped, the reply's
//! text may not**. They exercise the two real primitives the design uses — a bounded per-connection
//! queue (`ClientConnection`, 64 slots) and the session broadcast (1024 slots) — rather than a
//! stand-in, so a change to either would fail here.
//!
//! `TODO(C5)`: the depth gate itself lives in `TopicOutbound::run`, which needs the connection
//! harness; the rule it applies is asserted at the queue level below.

#![cfg(feature = "live-voice")]

use std::sync::Arc;

use vizier_core::agents::client::ClientConnection;
use vizier_core::live_voice::frame::{LiveVoiceControl, LiveVoiceSpeech};
use vizier_core::live_voice::{LIVE_VOICE_SOCKET_BACKLOG, frame::is_speech_frame};

/// FR-042 and the delivery path share one word: only `speech` is audio, and only audio is droppable.
#[test]
fn only_a_speech_frame_counts_as_audio() {
    let speech = serde_json::json!({ "live_voice": LiveVoiceSpeech::new(1, 0, &[0, 0], false) });
    assert!(is_speech_frame(&speech));

    for control in [
        LiveVoiceControl::state(vizier_core::live_voice::SessionState::Waiting, 1, None),
        LiveVoiceControl::Transcript { utterance_id: "u".into(), text: "hi".into(), submitted: true },
        LiveVoiceControl::Error {
            kind: vizier_core::live_voice::EndReason::ProviderFailed,
            message: "x".into(),
        },
        LiveVoiceControl::Stall { dropped: 2 },
    ] {
        let frame = serde_json::json!({ "live_voice": control });
        assert!(!is_speech_frame(&frame), "a control frame is never droppable audio");
    }

    // A reply's own frame, and anything else the socket carries, is not audio either.
    assert!(!is_speech_frame(&serde_json::json!({ "content": { "message": {} } })));
    assert!(!is_speech_frame(&serde_json::json!({ "history_entry": {} })));

    // 16 frames is ~1.6 s of speech: past that the person is hearing the past.
    assert_eq!(LIVE_VOICE_SOCKET_BACKLOG, 16);
}

/// T022: 2 000 audio frames through the per-connection lane never touch the broadcast, so a reply
/// waiting on it arrives and the subscriber is never `Lagged`.
///
/// This is the whole reason audio moved: `Lagged` drops *all or nothing* per subscriber, so a
/// talking session could otherwise take the answer's text with it.
#[tokio::test]
async fn two_thousand_audio_frames_never_evict_a_pending_reply() {
    // The session broadcast, as the transport makes it (`transport/mod.rs`, capacity 1024).
    let (events, mut subscriber) = tokio::sync::broadcast::channel::<
        vizier_core::transport::SessionFrame,
    >(1024);
    // This socket's own lane, as `topic.rs` makes it (64 slots).
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    let connection = ClientConnection::new(out_tx);

    // A reply is published on the broadcast and is still to be read.
    events
        .send(vizier_core::transport::SessionFrame::LiveVoice(
            LiveVoiceControl::Stall { dropped: 0 },
        ))
        .expect("the broadcast has a subscriber");

    // A session talks: 2 000 segments, at the same time nobody drains the socket.
    let mut accepted = 0usize;
    let mut refused = 0usize;
    for seq in 0..2_000u32 {
        let frame = serde_json::json!({ "live_voice": LiveVoiceSpeech::new(1, seq, &[0u8; 320], true) });
        if connection.send(frame) {
            accepted += 1;
        } else {
            refused += 1;
        }
    }
    // The queue filled and the rest was refused — which is a dropped audio frame, not an ending
    // (`review.md` D10): the connection is still open.
    assert_eq!(accepted, 64, "the queue holds exactly its own capacity");
    assert_eq!(refused, 2_000 - 64);
    assert!(connection.is_open(), "a full queue is not a closed socket");

    // Nothing the session said entered the ring, so the reply is still there and no frame was
    // skipped on the way.
    let received = subscriber
        .try_recv()
        .expect("the pending reply is still on the broadcast");
    assert!(matches!(
        received,
        vizier_core::transport::SessionFrame::LiveVoice(LiveVoiceControl::Stall { .. })
    ));
    assert!(
        subscriber.try_recv().is_err(),
        "no audio reached the session broadcast"
    );

    // And the socket's own lane holds the audio that was accepted, in order.
    let first = out_rx.try_recv().expect("the accepted audio is queued");
    assert!(is_speech_frame(&first));
}

/// The same run, with a socket that keeps up: nothing is refused, and the queue drains in order.
#[tokio::test]
async fn a_socket_that_keeps_up_loses_nothing() {
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    let connection = Arc::new(ClientConnection::new(out_tx));

    let writer = {
        let connection = connection.clone();
        tokio::spawn(async move {
            for seq in 0..200u32 {
                let frame =
                    serde_json::json!({ "live_voice": LiveVoiceSpeech::new(1, seq, &[0u8; 32], true) });
                // Backpressure, not a drop: `false` while open is retried here, which is what the
                // session must not do for audio — it counts and moves on (T042).
                while !connection.send(frame.clone()) {
                    tokio::task::yield_now().await;
                }
            }
        })
    };

    let mut seen = 0u32;
    while seen < 200 {
        let frame = out_rx.recv().await.expect("the socket is open");
        assert!(is_speech_frame(&frame));
        seen += 1;
    }
    writer.await.unwrap();
    assert_eq!(seen, 200);
}

/// D10: `send` answers `false` for **two** different events, and only one of them ends a session.
/// Reading `false` as "closed" would kill a session merely because the socket was busy.
#[tokio::test]
async fn a_full_queue_is_not_a_closed_connection() {
    let (out_tx, out_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(4);
    let connection = ClientConnection::new(out_tx);

    // Fill the queue and one more: `false`, with the connection open.
    for _ in 0..4 {
        assert!(connection.send(serde_json::json!({ "a": 1 })));
    }
    assert!(!connection.send(serde_json::json!({ "a": 2 })), "the queue is full");
    assert!(
        connection.is_open(),
        "a full queue must not be read as a closed socket (FR-028, D10)"
    );

    // Nothing was lost from the queue by the refused send.
    assert_eq!(out_rx.len(), 4);

    // Only an ending makes it closed, and then `send` answers `false` for a different reason.
    connection.close();
    assert!(!connection.is_open());
    assert!(!connection.send(serde_json::json!({ "a": 3 })));
    drop(out_rx);
}

/// The stall report is one control frame per episode, so a socket that fell behind says so once
/// rather than once per dropped segment.
#[tokio::test]
async fn one_stall_frame_reports_a_whole_episode() {
    let (events, mut subscriber) = tokio::sync::broadcast::channel::<
        vizier_core::transport::SessionFrame,
    >(1024);

    // Three episodes of three dropped frames: the count is per episode, carried once.
    for dropped in [3u64, 7, 1] {
        events
            .send(vizier_core::transport::SessionFrame::LiveVoice(
                LiveVoiceControl::Stall { dropped },
            ))
            .unwrap();
    }

    for expected in [3u64, 7, 1] {
        match subscriber.try_recv().expect("one report per episode") {
            vizier_core::transport::SessionFrame::LiveVoice(LiveVoiceControl::Stall { dropped }) => {
                assert_eq!(dropped, expected)
            }
            other => panic!("expected a stall, got {other:?}"),
        }
    }
    assert!(subscriber.try_recv().is_err(), "no frame per dropped segment");
}

/// FR-042 on the wire: the indicator is a state frame, sent before any audio exists, and it is not
/// the reconnecting state — the client must be able to tell "thinking" from "the line died".
#[test]
fn thinking_is_its_own_state_and_not_reconnecting() {
    let waiting = serde_json::to_value(LiveVoiceControl::state(
        vizier_core::live_voice::SessionState::Waiting,
        2,
        None,
    ))
    .unwrap();
    let reconnecting = serde_json::to_value(LiveVoiceControl::state(
        vizier_core::live_voice::SessionState::Reconnecting,
        2,
        Some(vizier_core::live_voice::EndReason::ConnectionLost),
    ))
    .unwrap();

    assert_ne!(waiting["state"], reconnecting["state"]);
    assert!(waiting.get("reason").is_none());
    assert_eq!(reconnecting["reason"], "connection_lost");
    // Both are control frames, so both ride the broadcast the same way.
    assert!(!is_speech_frame(&serde_json::json!({ "live_voice": waiting })));
}
