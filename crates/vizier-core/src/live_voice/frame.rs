//! The live voice wire types (`specs/055-live-voice/contracts/live-voice-frames.md`).
//!
//! Two families, deliberately separate types so the compiler holds the lane: [`LiveVoiceControl`]
//! is the only thing a `SessionFrame` may carry, and [`LiveVoiceSpeech`] is the only thing that may
//! travel the per-connection lane. Publishing audio as a session event is therefore a type error
//! rather than a review rule (`review.md` D9, D17).

use serde::{Deserialize, Serialize};

/// The sample rate of every PCM payload in this release.
pub const SAMPLE_RATE: u32 = 16_000;
/// The only `format` value in this release.
pub const FORMAT: &str = "pcm16";
/// One client audio chunk: 100 ms of PCM16 mono at 16 kHz.
pub const CHUNK_SAMPLES: usize = (SAMPLE_RATE as usize) / 10;
/// The bytes one chunk carries before base64.
pub const CHUNK_BYTES: usize = CHUNK_SAMPLES * 2;
/// Base64 of one chunk is 4/3 of the bytes, rounded up to a multiple of four.
pub const CHUNK_BASE64: usize = CHUNK_BYTES.div_ceil(3) * 4;
/// The largest `base64` a client may send in one `live_voice_audio` frame: one chunk plus half of
/// another, about 6.3 KB of text.
///
/// T015 asks for "larger than one chunk (~6 KB of base64)" to be refused. One chunk is 4 268
/// characters, so two chunks (8 536) is twice the task's bound and would let a client drift; this
/// is the bound a client with a slightly different resampler still fits inside.
pub const MAX_AUDIO_FRAME_BASE64: usize = CHUNK_BASE64 + CHUNK_BASE64 / 2;

/// The FR-006 state word, one for one, so the client never invents a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Off,
    Connecting,
    Listening,
    Waiting,
    Speaking,
    /// Transient: the audio stopped and the turn was cancelled, the session is still live.
    Interrupted,
    Reconnecting,
    Ended,
    Failed,
}

impl SessionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Connecting => "connecting",
            Self::Listening => "listening",
            Self::Waiting => "waiting",
            Self::Speaking => "speaking",
            Self::Interrupted => "interrupted",
            Self::Reconnecting => "reconnecting",
            Self::Ended => "ended",
            Self::Failed => "failed",
        }
    }

    /// Whether the session is over; every terminal state carries a reason (FR-028, SC-009).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Ended | Self::Failed)
    }
}

/// Why a session ended, or why a frame reports a failure. One list, used for `state.reason` and
/// for `error.kind`, so the client has one vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    PressedAgain,
    LeftConversation,
    PageClosed,
    PermissionWithdrawn,
    ConversationDeleted,
    AgentChanged,
    Idle,
    ConnectionLost,
    ServerRestart,
    NoDevice,
    NoRecognizer,
    NoVoice,
    ProviderFailed,
    Unintelligible,
}

impl EndReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PressedAgain => "pressed_again",
            Self::LeftConversation => "left_conversation",
            Self::PageClosed => "page_closed",
            Self::PermissionWithdrawn => "permission_withdrawn",
            Self::ConversationDeleted => "conversation_deleted",
            Self::AgentChanged => "agent_changed",
            Self::Idle => "idle",
            Self::ConnectionLost => "connection_lost",
            Self::ServerRestart => "server_restart",
            Self::NoDevice => "no_device",
            Self::NoRecognizer => "no_recognizer",
            Self::NoVoice => "no_voice",
            Self::ProviderFailed => "provider_failed",
            Self::Unintelligible => "unintelligible",
        }
    }
}

/// A control frame: the only family that may ride the session broadcast.
///
/// A handful per turn, which the 1024-event ring never notices, and it is how a second tab on the
/// conversation is told a session is active (FR-005) without a per-socket replay log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveVoiceControl {
    /// The FR-006 state word. `waiting` is what carries FR-042: it is sent as soon as the turn is
    /// accepted, before any audio exists, and the client renders it as "the agent is thinking".
    State {
        state: SessionState,
        generation: u64,
        /// Present only for `ended` / `failed` / `reconnecting`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<EndReason>,
    },
    /// One utterance's outcome. `submitted: false` with empty `text` is the too-short case
    /// (FR-024): the audio stopped and nothing was sent.
    Transcript {
        utterance_id: String,
        text: String,
        submitted: bool,
    },
    /// A failure the person must see. A `state` frame always follows, so the state word has one
    /// source of truth (FR-032, SC-009).
    Error { kind: EndReason, message: String },
    /// The one report of a socket that fell behind: how many audio frames this episode dropped.
    /// Carries no generation and changes no state (contract, `stall`).
    Stall { dropped: u64 },
}

impl LiveVoiceControl {
    pub fn state(state: SessionState, generation: u64, reason: Option<EndReason>) -> Self {
        Self::State { state, generation, reason }
    }
}

/// A `live_voice_speech` frame: one segment of the reply, written to the owning socket only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveVoiceSpeech {
    /// Always [`SpeechKind::Speech`]. The discriminator is what makes this type unable to pass for
    /// a control frame.
    #[serde(rename = "type")]
    pub kind: SpeechKind,
    pub generation: u64,
    pub seq: u32,
    pub sample_rate: u32,
    pub format: String,
    pub base64: String,
    #[serde(rename = "final")]
    pub last: bool,
}

/// The one value `LiveVoiceSpeech::kind` may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechKind {
    #[default]
    Speech,
}

impl LiveVoiceSpeech {
    pub fn new(generation: u64, seq: u32, pcm: &[u8], last: bool) -> Self {
        use base64::Engine as _;
        Self {
            kind: SpeechKind::Speech,
            generation,
            seq,
            sample_rate: SAMPLE_RATE,
            format: FORMAT.into(),
            base64: base64::engine::general_purpose::STANDARD.encode(pcm),
            last,
        }
    }
}

/// Whether a server frame is one `live_voice_speech` segment. The socket's outbound loop is the
/// only place allowed to drop it (contract §Delivery path).
pub fn is_speech_frame(frame: &serde_json::Value) -> bool {
    frame
        .get(crate::live_voice::FRAME_KEY)
        .and_then(|body| body.get("type"))
        .and_then(|kind| kind.as_str())
        == Some("speech")
}

/// The six action keys a client may send. Used as a cheap discriminator before the frame is
/// copied or parsed (`review.md` D5).
pub const CLIENT_KEYS: [&str; 6] = [
    "live_voice_start",
    "live_voice_audio",
    "live_voice_utterance_end",
    "live_voice_speech_start",
    "live_voice_interrupt",
    "live_voice_stop",
];

/// Whether `frame` is a live voice action at all, and which one.
pub fn client_key(frame: &serde_json::Value) -> Option<&'static str> {
    CLIENT_KEYS.iter().copied().find(|key| frame.get(*key).is_some())
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartFrame {
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioFrame {
    pub utterance_id: String,
    pub seq: u32,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_format")]
    pub format: String,
    pub base64: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UtteranceIdFrame {
    pub utterance_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InterruptFrame {
    #[serde(default)]
    pub reason: Option<InterruptReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterruptReason {
    Button,
    Typed,
    BargeIn,
}

fn default_sample_rate() -> u32 {
    SAMPLE_RATE
}

fn default_format() -> String {
    FORMAT.into()
}

/// One parsed client action.
#[derive(Debug, Clone)]
pub enum ClientAction {
    Start(StartFrame),
    Audio(AudioFrame),
    UtteranceEnd(UtteranceIdFrame),
    SpeechStart(UtteranceIdFrame),
    Interrupt(InterruptFrame),
    Stop,
}

impl ClientAction {
    /// Parse the payload under `key`. `None` when the shape does not match the contract: the
    /// caller answers with `live_voice_error` rather than guessing.
    pub fn parse(key: &str, frame: &serde_json::Value) -> Option<Self> {
        let payload = frame.get(key)?;
        let value = if payload.is_null() { &serde_json::Value::Null } else { payload };
        Some(match key {
            "live_voice_start" => Self::Start(serde_json::from_value(value.clone()).ok()?),
            "live_voice_audio" => Self::Audio(serde_json::from_value(value.clone()).ok()?),
            "live_voice_utterance_end" => {
                Self::UtteranceEnd(serde_json::from_value(value.clone()).ok()?)
            }
            "live_voice_speech_start" => {
                Self::SpeechStart(serde_json::from_value(value.clone()).ok()?)
            }
            "live_voice_interrupt" => Self::Interrupt(serde_json::from_value(value.clone()).ok()?),
            "live_voice_stop" => Self::Stop,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contract §`live_voice_state`: the exact shape the client parses.
    #[test]
    fn a_state_frame_has_the_contract_shape() {
        let frame = LiveVoiceControl::state(SessionState::Interrupted, 8, None);
        let json = serde_json::to_value(&frame).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "type": "state", "state": "interrupted", "generation": 8 })
        );

        let ended = LiveVoiceControl::state(SessionState::Ended, 9, Some(EndReason::Idle));
        assert_eq!(
            serde_json::to_value(&ended).unwrap(),
            serde_json::json!({
                "type": "state", "state": "ended", "generation": 9, "reason": "idle"
            })
        );
    }

    /// Contract §`live_voice_speech`: one segment, with the discriminator the client switches on.
    #[test]
    fn a_speech_frame_has_the_contract_shape() {
        let frame = LiveVoiceSpeech::new(8, 3, &[0u8, 1, 2, 3], false);
        let json = serde_json::to_value(&frame).unwrap();
        assert_eq!(json["type"], "speech");
        assert_eq!(json["generation"], 8);
        assert_eq!(json["seq"], 3);
        assert_eq!(json["sample_rate"], 16000);
        assert_eq!(json["format"], "pcm16");
        assert_eq!(json["final"], false);
        assert_eq!(json["base64"], "AAECAw==");
    }

    /// The stall frame carries no generation and no state (contract, `stall`).
    #[test]
    fn a_stall_frame_carries_only_the_count() {
        assert_eq!(
            serde_json::to_value(LiveVoiceControl::Stall { dropped: 23 }).unwrap(),
            serde_json::json!({ "type": "stall", "dropped": 23 })
        );
    }

    /// The type split: a control frame cannot deserialise into a speech frame, which is what stops
    /// audio being published as a session event (`review.md` D17).
    #[test]
    fn a_control_frame_is_not_a_speech_frame() {
        let control = serde_json::to_value(LiveVoiceControl::Stall { dropped: 1 }).unwrap();
        assert!(serde_json::from_value::<LiveVoiceSpeech>(control).is_err());
    }

    /// A chunk is 100 ms of PCM16 mono: the size the inbound guard is written against.
    #[test]
    fn one_chunk_is_a_hundred_milliseconds() {
        assert_eq!(CHUNK_SAMPLES, 1600);
        assert_eq!(CHUNK_BYTES, 3200);
        assert!(CHUNK_BASE64 >= 4264 && CHUNK_BASE64 < 4272);
    }

    #[test]
    fn the_client_keys_are_recognised_before_parsing() {
        let frame = serde_json::json!({ "live_voice_audio": { "utterance_id": "u-1", "seq": 0, "base64": "" } });
        assert_eq!(client_key(&frame), Some("live_voice_audio"));
        let action = ClientAction::parse("live_voice_audio", &frame).unwrap();
        match action {
            ClientAction::Audio(audio) => {
                assert_eq!(audio.utterance_id, "u-1");
                assert_eq!(audio.sample_rate, SAMPLE_RATE);
                assert_eq!(audio.format, FORMAT);
            }
            other => panic!("expected audio, got {other:?}"),
        }
        // A frame from an older client carries none of the keys.
        assert_eq!(client_key(&serde_json::json!({ "chat": "hello" })), None);
    }
}
