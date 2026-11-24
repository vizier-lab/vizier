pub mod elevenlabs;
#[cfg(feature = "provider-media")]
pub mod whisper;

use std::sync::Arc;

use crate::dependencies::VizierDependencies;
use crate::llm::media::{self, gemini::GeminiStt, huggingface::HuggingFaceStt, openai_shaped::OpenAiStt};
use crate::agents::agent::model::usage::UsageScope;
use crate::schema::agent::{SttProvider, SttToolSettings};
use crate::schema::usage::{MediaUnits, UsageKind};
use crate::usage::record::{MediaMeter, Measured, Reported, wav_duration_ms};
use crate::Result;

/// A transcript and what the provider said it used (spec 065).
pub struct Transcript {
    pub text: String,
    pub reported: Reported,
}

#[async_trait::async_trait]
pub trait VizierSttModel: Send + Sync {
    async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String>;

    /// [`Self::transcribe`] with the provider's own usage figures (spec 065 research R3).
    async fn transcribe_reported(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<Transcript> {
        Ok(Transcript {
            text: self.transcribe(audio, filename, language).await?,
            reported: Reported::default(),
        })
    }
}

pub struct VizierStt {
    model: Arc<dyn VizierSttModel>,
    meter: MediaMeter,
}

/// The model a setting resolves to: what it names, or the provider's documented default.
fn resolved_model(settings: &SttToolSettings) -> String {
    settings
        .model
        .clone()
        .unwrap_or_else(|| settings.provider.default_model().into())
}

impl VizierStt {
    pub async fn new(
        settings: &SttToolSettings,
        deps: &VizierDependencies,
        workspace: &str,
        agent_id: &str,
    ) -> Result<Self> {
        let provider = &settings.provider;
        let model = resolved_model(settings);
        let meter = MediaMeter::new(
            agent_id,
            provider
                .credentials()
                .map(|id| id.to_string())
                .unwrap_or_else(|| crate::usage::record::provider_name(provider)),
            model.clone(),
            provider.credentials().is_none(),
            deps.storage.clone(),
        );
        let model: Arc<dyn VizierSttModel> = match (provider, provider.credentials()) {
            (SttProvider::Dummyplug, _) => Arc::new(media::dummyplug::DummyplugStt),
            #[cfg(feature = "provider-media")]
            (SttProvider::Whisper, _) => Arc::new(whisper::WhisperSttModel::new(
                model,
                workspace,
                deps.setup.clone(),
                agent_id.to_string(),
            )),
            #[cfg(not(feature = "provider-media"))]
            (SttProvider::Whisper, _) => {
                return Err(crate::VizierError(
                    "local speech recognition (whisper) is not compiled into this build".into(),
                ));
            }
            (SttProvider::Elevenlabs, Some(id)) => Arc::new(elevenlabs::ElevenLabsSttModel::new(
                media::api_key(deps, &id).await?,
                model,
            )),
            (SttProvider::Gemini, Some(id)) => {
                Arc::new(GeminiStt(media::endpoint(deps, &id, model).await?))
            }
            (SttProvider::Huggingface, Some(id)) => {
                Arc::new(HuggingFaceStt(media::endpoint(deps, &id, model).await?))
            }
            // OpenRouter is OpenAI-compatible (`POST /audio/transcriptions`, multipart); the
            // explicit arm keeps it from falling through the catch-all (research R3).
            (SttProvider::Openrouter, Some(id)) => {
                Arc::new(OpenAiStt(media::endpoint(deps, &id, model).await?))
            }
            (_, Some(id)) => Arc::new(OpenAiStt(media::endpoint(deps, &id, model).await?)),
            (_, None) => unreachable!("only whisper and dummyplug need no credentials"),
        };

        Ok(Self { model, meter })
    }

    pub async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
        scope: &UsageScope,
    ) -> Result<String> {
        let started = std::time::Instant::now();
        let out = self.model.transcribe_reported(audio, filename, language).await?;
        self.meter.record(
            scope,
            UsageKind::Transcription,
            out.reported,
            Measured {
                units: MediaUnits {
                    audio_ms: wav_duration_ms(audio).unwrap_or(0),
                    ..Default::default()
                },
                ..Default::default()
            },
            started.elapsed(),
        );
        Ok(out.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 028 FR-004: an empty model uses the documented default, and the language is carried
    /// through to the OpenAI-shaped multipart client (the `stt_openai` golden fixture pins the
    /// shape: `model`, `file`, and `language` when given).
    #[test]
    fn openrouter_resolves_its_documented_default_and_keeps_the_language() {
        let settings = SttToolSettings {
            provider: SttProvider::Openrouter,
            model: None,
            language: Some("en".into()),
        };
        assert_eq!(resolved_model(&settings), "openai/whisper-large-v3");
        assert_eq!(settings.language.as_deref(), Some("en"));

        let explicit = SttToolSettings {
            provider: SttProvider::Openrouter,
            model: Some("openai/whisper-large-v3".into()),
            language: None,
        };
        assert_eq!(resolved_model(&explicit), "openai/whisper-large-v3");
    }
}
