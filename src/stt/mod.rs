pub mod elevenlabs;
pub mod gemini;
pub mod groq;
pub mod huggingface;
pub mod mistral;
pub mod openai;

use std::sync::Arc;

use crate::config::provider::ProviderVariant;
use crate::schema::agent::{SttProvider, SttToolSettings};
use crate::storage::VizierStorage;
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
        storage: &Arc<VizierStorage>,
        _workspace: &str,
    ) -> Result<Self> {
        let model: Arc<dyn VizierSttModel> = match &settings.provider {
            SttProvider::Openai => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::openai,
                    "OPENAI_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| SttProvider::Openai.default_model().into());
                Arc::new(openai::OpenAiSttModel::new(resolved.api_key, model))
            }
            SttProvider::Elevenlabs => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::elevenlabs,
                    "ELEVENLABS_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| SttProvider::Elevenlabs.default_model().into());
                Arc::new(elevenlabs::ElevenLabsSttModel::new(resolved.api_key, model))
            }
            SttProvider::Groq => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::groq,
                    "GROQ_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| SttProvider::Groq.default_model().into());
                Arc::new(groq::GroqSttModel::new(resolved.api_key, model))
            }
            SttProvider::Mistral => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::mistral,
                    "MISTRAL_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| SttProvider::Mistral.default_model().into());
                Arc::new(mistral::MistralSttModel::new(resolved.api_key, model))
            }
            SttProvider::Huggingface => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::huggingface,
                    "HUGGINGFACE_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| SttProvider::Huggingface.default_model().into());
                Arc::new(huggingface::HuggingfaceSttModel::new(resolved.api_key, model))
            }
            SttProvider::Gemini => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::gemini,
                    "GEMINI_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| SttProvider::Gemini.default_model().into());
                Arc::new(gemini::GeminiSttModel::new(resolved.api_key, model))
            }
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
