pub mod huggingface;
pub mod hyperbolic;
pub mod openai;
pub mod xai;

use std::sync::Arc;

use crate::config::provider::ProviderVariant;
use crate::schema::agent::{ImageGenProvider, ImageGenToolSettings};
use crate::storage::VizierStorage;
use crate::{Result, VizierError};

#[async_trait::async_trait]
pub trait VizierImageGenModel: Send + Sync {
    async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<(Vec<u8>, String)>;
}

pub struct VizierImageGen(Arc<dyn VizierImageGenModel>);

impl VizierImageGen {
    pub async fn new(
        settings: &ImageGenToolSettings,
        storage: &Arc<VizierStorage>,
    ) -> Result<Self> {
        let model: Arc<dyn VizierImageGenModel> = match &settings.provider {
            ImageGenProvider::Openai => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::openai,
                    "OPENAI_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| ImageGenProvider::Openai.default_model().into());
                Arc::new(openai::OpenAiImageGenModel::new(resolved.api_key, model))
            }
            ImageGenProvider::Xai => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::xai,
                    "XAI_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| ImageGenProvider::Xai.default_model().into());
                Arc::new(xai::XaiImageGenModel::new(resolved.api_key, model))
            }
            ImageGenProvider::Huggingface => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::huggingface,
                    "HUGGINGFACE_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| ImageGenProvider::Huggingface.default_model().into());
                Arc::new(huggingface::HuggingfaceImageGenModel::new(resolved.api_key, model))
            }
            ImageGenProvider::Hyperbolic => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::hyperbolic,
                    "HYPERBOLIC_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| ImageGenProvider::Hyperbolic.default_model().into());
                Arc::new(hyperbolic::HyperbolicImageGenModel::new(resolved.api_key, model))
            }
        };

        Ok(Self(model))
    }

    pub async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<(Vec<u8>, String)> {
        self.0.generate(prompt, size).await
    }
}
