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
/// Mono. A second channel is not a feature, it is a bug with more bytes.
pub const CHANNELS: u8 = 1;
/// How much audio one client chunk carries.
pub const FRAME_MS: u32 = 100;
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
    /// The client asked for audio this release cannot carry (T102). Its own reason, not a provider
    /// failure: nothing was wrong with the recogniser or the voice, the declared description was.
    UnsupportedAudio,
    /// The API key that opened this session was deleted (T110, FR-063). The call ends because the
    /// credential behind it is gone — the contract promises this within a second of the revocation.
    KeyRevoked,
    /// The API key that opened this session lapsed while the call was running (T110, FR-063).
    KeyExpired,
    /// The same person started a call somewhere else, and one session is all they may have
    /// (T096, FR-051). Its own reason: the socket is still open and the person did not switch
    /// anything off — the call simply went with them.
    Moved,
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
            Self::UnsupportedAudio => "unsupported_audio",
            Self::KeyRevoked => "key_revoked",
            Self::KeyExpired => "key_expired",
            Self::Moved => "moved",
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

/// The audio a client says it is about to send (T102, `contracts/live-voice-frames.md`).
///
/// The release carries one description — 16 kHz, mono, 100 ms, PCM16 — and this is how a client
/// asks for it instead of the server assuming it. A client that says nothing gets the default,
/// which is what keeps a client written before this field existed working.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDescription {
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_ms: u32,
    pub format: String,
}

impl Default for AudioDescription {
    fn default() -> Self {
        Self {
            sample_rate: SAMPLE_RATE,
            channels: CHANNELS,
            frame_ms: FRAME_MS,
            format: FORMAT.into(),
        }
    }
}

impl AudioDescription {
    /// Whether this is the carrier the release speaks, and if not, which number is wrong.
    ///
    /// The message names both numbers, because "unsupported audio" leaves a client author guessing
    /// and a pair of numbers does not.
    pub fn check(&self) -> Result<(), String> {
        let ours = AudioDescription::default();
        if self.sample_rate != ours.sample_rate {
            return Err(format!(
                "this release carries {} Hz audio, and the start frame declared {} Hz",
                ours.sample_rate, self.sample_rate
            ));
        }
        if self.channels != ours.channels {
            return Err(format!(
                "this release carries {} channel(s), and the start frame declared {}",
                ours.channels, self.channels
            ));
        }
        if self.frame_ms != ours.frame_ms {
            return Err(format!(
                "this release carries {} ms chunks, and the start frame declared {} ms",
                ours.frame_ms, self.frame_ms
            ));
        }
        if self.format != ours.format {
            return Err(format!(
                "this release carries {:?}, and the start frame declared {:?}",
                ours.format, self.format
            ));
        }
        Ok(())
    }

    /// One incoming chunk must be in the description the session started with.
    ///
    /// Only the two fields a chunk carries are compared; `channels` and `frame_ms` were settled by
    /// the start frame and are not repeated on every frame.
    pub fn accepts(&self, sample_rate: u32, format: &str) -> Result<(), String> {
        if sample_rate != self.sample_rate {
            return Err(format!(
                "this session carries {} Hz audio, and the frame declared {} Hz",
                self.sample_rate, sample_rate
            ));
        }
        if format != self.format {
            return Err(format!(
                "this session carries {:?}, and the frame declared {:?}",
                self.format, format
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartFrame {
    #[serde(default)]
    pub language: Option<String>,
    /// Absent means the release's own description, so an older client is still valid.
    #[serde(default)]
    pub audio: Option<AudioDescription>,
}

impl StartFrame {
    /// The description this start asks for, defaulted when the client named none.
    pub fn description(&self) -> AudioDescription {
        self.audio.clone().unwrap_or_default()
    }
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

impl InterruptReason {
    /// The wire word, the same one the client sends.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::Typed => "typed",
            Self::BargeIn => "barge_in",
        }
    }
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

    /// T102: the audio description is what the client declares, and the server checks it instead
    /// of assuming it. These are the four rules the socket applies.
    #[test]
    fn the_default_description_is_the_release_carrier() {
        let description = AudioDescription::default();
        assert_eq!(description.sample_rate, 16_000);
        assert_eq!(description.channels, 1);
        assert_eq!(description.frame_ms, 100);
        assert_eq!(description.format, "pcm16");
        assert!(description.check().is_ok());
    }

    #[test]
    fn a_start_without_a_description_gets_the_default() {
        // The shape an older client sends: no `audio` key at all.
        let start: StartFrame = serde_json::from_value(serde_json::json!({ "language": "id" })).unwrap();
        assert_eq!(start.description(), AudioDescription::default());

        // And one that names it keeps what it asked for.
        let named: StartFrame = serde_json::from_value(serde_json::json!({
            "audio": { "sample_rate": 44100, "channels": 1, "frame_ms": 100, "format": "pcm16" }
        }))
        .unwrap();
        assert_eq!(named.description().sample_rate, 44_100);
    }

    #[test]
    fn a_description_the_release_cannot_carry_is_refused_with_both_numbers() {
        let wrong_rate = AudioDescription {
            sample_rate: 44_100,
            ..AudioDescription::default()
        };
        let reason = wrong_rate.check().expect_err("44.1 kHz is not this release's carrier");
        assert!(reason.contains("16000"), "says what it expected: {reason}");
        assert!(reason.contains("44100"), "and what it was given: {reason}");

        let wrong_format = AudioDescription {
            format: "opus".into(),
            ..AudioDescription::default()
        };
        let reason = wrong_format.check().expect_err("opus is reserved, not carried");
        assert!(reason.contains("opus"), "{reason}");

        let wrong_channels = AudioDescription {
            channels: 2,
            ..AudioDescription::default()
        };
        assert!(wrong_channels.check().is_err(), "mono only");

        let wrong_frame = AudioDescription {
            frame_ms: 20,
            ..AudioDescription::default()
        };
        assert!(wrong_frame.check().is_err(), "100 ms chunks only");
    }

    /// T102: the refusal has its own reason. `provider_failed` sends a client author to the
    /// credentials when the bug is a wrong number in their own start frame, and this vocabulary is
    /// public (FR-058).
    #[test]
    fn a_carrier_the_server_cannot_carry_is_its_own_reason() {
        assert_eq!(EndReason::UnsupportedAudio.as_str(), "unsupported_audio");
        assert_ne!(
            EndReason::UnsupportedAudio.as_str(),
            EndReason::ProviderFailed.as_str(),
            "a wrong description is not a provider failure"
        );
        // The wire form is the snake_case name, so a third-party client reads what the server means.
        let json = serde_json::to_value(EndReason::UnsupportedAudio).unwrap();
        assert_eq!(json, serde_json::json!("unsupported_audio"));
        assert!(
            !EndReason::UnsupportedAudio.as_str().contains("provider"),
            "the name must not send anyone to the provider"
        );
    }

    #[test]
    fn a_chunk_outside_the_declared_description_is_refused() {
        let description = AudioDescription::default();
        assert!(description.accepts(16_000, "pcm16").is_ok());
        let reason = description
            .accepts(44_100, "pcm16")
            .expect_err("a chunk from another rate is not this session's audio");
        assert!(reason.contains("16000") && reason.contains("44100"), "{reason}");
        assert!(description.accepts(16_000, "opus").is_err(), "and neither is another format");
    }
}
