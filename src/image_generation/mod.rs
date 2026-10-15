use std::sync::Arc;

use crate::dependencies::VizierDependencies;
use crate::llm::media::{
    self,
    huggingface::HuggingFaceImages,
    hyperbolic::HyperbolicImages,
    openai_shaped::{ImageSizing, OpenAiImages},
};
use crate::schema::agent::{ImageGenProvider, ImageGenToolSettings};
use crate::Result;

#[async_trait::async_trait]
pub trait VizierImageGenModel: Send + Sync {
    async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<(Vec<u8>, String)>;
}

pub struct VizierImageGen(Arc<dyn VizierImageGenModel>);

impl VizierImageGen {
    pub async fn new(settings: &ImageGenToolSettings, deps: &VizierDependencies) -> Result<Self> {
        let provider = &settings.provider;
        let model = settings
            .model
            .clone()
            .unwrap_or_else(|| provider.default_model().into());
        let Some(id) = provider.credentials() else {
            unreachable!("every image provider has credentials");
        };
        let endpoint = media::endpoint(deps, &id, model).await?;
        let model: Arc<dyn VizierImageGenModel> = match provider {
            ImageGenProvider::Openai => Arc::new(OpenAiImages {
                endpoint,
                sizing: ImageSizing::Size,
            }),
            ImageGenProvider::Xai => Arc::new(OpenAiImages {
                endpoint,
                sizing: ImageSizing::AspectRatio("1:1"),
            }),
            ImageGenProvider::Huggingface => Arc::new(HuggingFaceImages(endpoint)),
            ImageGenProvider::Hyperbolic => Arc::new(HyperbolicImages(endpoint)),
        };

        Ok(Self(model))
    }

    pub async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<(Vec<u8>, String)> {
        self.0.generate(prompt, size).await
    }
}
