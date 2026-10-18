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

impl VizierStt {
    pub async fn new(
        settings: &SttToolSettings,
        deps: &VizierDependencies,
        workspace: &str,
        agent_id: &str,
    ) -> Result<Self> {
        let provider = &settings.provider;
        let model = settings
            .model
            .clone()
            .unwrap_or_else(|| provider.default_model().into());
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
