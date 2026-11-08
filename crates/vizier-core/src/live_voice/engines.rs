//! The one seam a session reaches its speech providers through (`specs/055-live-voice` FR-036).
//!
//! One trait, so a session is written once and the tests fake one implementation instead of two
//! providers. The cascade implementation is the only one that ships: recognise the clip, then
//! synthesise each segment. A speech-to-speech adapter would be a second implementation behind the
//! same trait, and is deliberately out of scope.

use std::sync::Arc;

use crate::Result;
use crate::schema::agent::{SttToolSettings, TtsToolSettings};
use crate::stt::VizierStt;
use crate::tts::VizierTts;

/// What a session needs from speech: hear a clip, say a sentence.
#[async_trait::async_trait]
pub trait LiveVoiceEngines: Send + Sync {
    /// The clip as text. An empty string is "nothing intelligible", not an error (FR-031).
    async fn transcribe(&self, wav: &[u8], filename: &str, language: Option<&str>)
    -> Result<String>;
    /// The sentence as audio, in whatever container the provider returns.
    async fn speak(&self, text: &str) -> Result<Vec<u8>>;
}

/// The shipped implementation: the existing recogniser and voice, in sequence.
pub struct CascadeEngines {
    stt: VizierStt,
    tts: VizierTts,
    voice: String,
    speed: f32,
}

impl CascadeEngines {
    /// Build from the session's settings, falling back to the agent's own speech settings.
    ///
    /// A live voice block that names its own recogniser or voice wins; otherwise the agent's
    /// `stt` / `tts` settings are used whatever their tool flags say (FR-036a) — those flags gate
    /// the *tools*, and this is not a tool.
    pub async fn new(
        stt: &SttToolSettings,
        tts: &TtsToolSettings,
        deps: &crate::dependencies::VizierDependencies,
        workspace: &str,
        agent_id: &str,
    ) -> Result<Self> {
        let stt = VizierStt::new(stt, deps, workspace, agent_id).await?;
        let tts_model = VizierTts::new(tts, deps, workspace, agent_id).await?;
        Ok(Self {
            stt,
            tts: tts_model,
            voice: tts
                .voice
                .clone()
                .unwrap_or_else(|| tts.provider.default_voice().to_string()),
            speed: tts.speed.unwrap_or(1.0),
        })
    }
}

#[async_trait::async_trait]
impl LiveVoiceEngines for CascadeEngines {
    async fn transcribe(
        &self,
        wav: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String> {
        let text = self.stt.transcribe(wav, filename, language).await?;
        Ok(text.trim().to_string())
    }

    async fn speak(&self, text: &str) -> Result<Vec<u8>> {
        self.tts.generate_speech(text, &self.voice, self.speed).await
    }
}

/// The engines one session holds, or the reason it has none.
pub type SharedEngines = Arc<dyn LiveVoiceEngines>;

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake that records what it was asked, which is what the session tests need.
    pub struct FakeEngines {
        pub transcript: String,
        pub fail_speech: bool,
    }

    #[async_trait::async_trait]
    impl LiveVoiceEngines for FakeEngines {
        async fn transcribe(
            &self,
            _wav: &[u8],
            _filename: &str,
            _language: Option<&str>,
        ) -> Result<String> {
            Ok(self.transcript.clone())
        }

        async fn speak(&self, text: &str) -> Result<Vec<u8>> {
            if self.fail_speech {
                return Err(crate::VizierError("the voice provider refused".into()));
            }
            // One silent 20 ms of PCM16 per segment, so a test can count segments by length.
            Ok(vec![0u8; text.len().min(320) * 2])
        }
    }
}
