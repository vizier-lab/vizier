pub mod elevenlabs;
pub mod whisper;

use std::sync::Arc;

use crate::dependencies::VizierDependencies;
use crate::llm::media::{self, gemini::GeminiStt, huggingface::HuggingFaceStt, openai_shaped::OpenAiStt};
use crate::schema::agent::{SttProvider, SttToolSettings};
use crate::Result;

#[async_trait::async_trait]
pub trait VizierSttModel: Send + Sync {
    async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String>;
}

pub struct VizierStt(Arc<dyn VizierSttModel>);

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
        let model: Arc<dyn VizierSttModel> = match (provider, provider.credentials()) {
            (SttProvider::Whisper, _) => Arc::new(whisper::WhisperSttModel::new(
                model,
                workspace,
                deps.setup.clone(),
                agent_id.to_string(),
            )),
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
            (_, None) => unreachable!("only whisper needs no credentials"),
        };

        Ok(Self(model))
    }

    pub async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String> {
        self.0.transcribe(audio, filename, language).await
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
