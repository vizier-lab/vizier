//! Live voice: a spoken conversation with an agent (`specs/055-live-voice`).
//!
//! The whole module is behind the `provider-live-voice` feature, declared in `lib.rs` — a build
//! without it is indistinguishable from one from before this feature existed (FR-041). It reuses
//! the existing speech providers ([`crate::stt`], [`crate::tts`]) and adds no dependency.
//!
//! Three things live here and nowhere else:
//!
//! - [`frame`]: the wire types. Control and audio are **separate types**, so a `SessionFrame` — the
//!   broadcast — cannot carry audio by construction.
//! - [`utterance`]: one utterance's PCM, and the WAV a recogniser is handed.
//! - [`segmenter`]: reply text into speakable segments.
//!
//! The session manager and the engines follow in the same module; nothing here touches the
//! transport, so each piece is unit-tested without a socket.

pub mod frame;
pub mod segmenter;
pub mod utterance;

pub use frame::{
    CHUNK_BASE64, CHUNK_BYTES, CHUNK_SAMPLES, ClientAction, EndReason, FORMAT, InterruptReason,
    LiveVoiceControl, LiveVoiceSpeech, SAMPLE_RATE, SessionState, SpeechKind,
};
pub use segmenter::{Segment, segment};
pub use utterance::{MIN_UTTERANCE_MS, Utterance, wav_header};

/// How many audio frames may sit in one socket's outbound queue before the rest of that episode is
/// dropped (contract §Delivery path). 16 frames is ~1.6 s of speech: past that the person is
/// hearing the past, and the audio is worth less than the text that shares the socket.
pub const LIVE_VOICE_SOCKET_BACKLOG: usize = 16;

/// The top-level key every server frame carries, so a client that does not know it ignores it.
pub const FRAME_KEY: &str = "live_voice";
