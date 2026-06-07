pub mod elevenlabs;
pub mod openai;
pub mod openrouter;

use std::sync::Arc;

use crate::config::provider::ProviderConfig;
use crate::schema::agent::{TtsProvider, TtsToolSettings};
use crate::Result;

#[async_trait::async_trait]
pub trait VizierTtsModel: Send + Sync {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>>;
}

pub struct VizierTts(Arc<dyn VizierTtsModel>);

impl VizierTts {
    pub async fn new(settings: &TtsToolSettings, providers: &ProviderConfig) -> Result<Self> {
        let model: Arc<dyn VizierTtsModel> = match &settings.provider {
            TtsProvider::Openai => {
                let api_key = providers
                    .openai
                    .as_ref()
                    .map(|c| c.api_key.clone())
                    .unwrap_or_else(|| std::env::var("OPENAI_API_KEY").unwrap_or_default());
                let model = settings.model.clone().unwrap_or_else(|| "tts-1".into());
                Arc::new(openai::OpenAiTtsModel::new(api_key, model, providers.openai.as_ref().and_then(|c| c.base_url.clone())))
            }
            TtsProvider::Openrouter => {
                let api_key = providers
                    .openrouter
                    .as_ref()
                    .map(|c| c.api_key.clone())
                    .unwrap_or_else(|| std::env::var("OPENROUTER_API_KEY").unwrap_or_default());
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| "openai/gpt-4o-mini-tts-2025-12-15".into());
                Arc::new(openrouter::OpenRouterTtsModel::new(api_key, model))
            }
            TtsProvider::Elevenlabs => {
                let api_key = providers
                    .elevenlabs
                    .as_ref()
                    .map(|c| c.api_key.clone())
                    .unwrap_or_else(|| std::env::var("ELEVENLABS_API_KEY").unwrap_or_default());
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| "eleven_multilingual_v2".into());
                Arc::new(elevenlabs::ElevenLabsTtsModel::new(api_key, model))
            }
        };

        Ok(Self(model))
    }

    pub async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        self.0.generate_speech(text, voice, speed).await
    }
}
