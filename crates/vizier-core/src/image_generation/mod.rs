use std::sync::Arc;

use crate::dependencies::VizierDependencies;
use crate::llm::media::{
    self,
    bedrock::BedrockImages,
    huggingface::HuggingFaceImages,
    hyperbolic::HyperbolicImages,
    openai_shaped::{ImageApi, ImageSizing, OpenAiImages},
};
use crate::schema::agent::{ImageGenProvider, ImageGenToolSettings};
use crate::Result;

/// An image an agent passed in to steer generation: the bytes and the media type the session
/// file recorded for them.
#[derive(Clone)]
pub struct ReferenceImage {
    pub mime: String,
    pub data: Vec<u8>,
}

#[async_trait::async_trait]
pub trait VizierImageGenModel: Send + Sync {
    /// A text-only request when `references` is empty; an edit or variation when it is not.
    async fn generate(
        &self,
        prompt: &str,
        size: Option<&str>,
        references: &[ReferenceImage],
    ) -> Result<(Vec<u8>, String)>;
}

pub struct VizierImageGen(Arc<dyn VizierImageGenModel>);

impl VizierImageGen {
    pub async fn new(settings: &ImageGenToolSettings, deps: &VizierDependencies) -> Result<Self> {
        let provider = &settings.provider;
        let model = settings
            .model
            .clone()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| provider.default_model().into());
        let Some(id) = provider.credentials() else {
            unreachable!("every image provider has credentials");
        };
        let endpoint = media::endpoint(deps, &id, model).await?;
        let model: Arc<dyn VizierImageGenModel> = match provider {
            ImageGenProvider::Openai => Arc::new(OpenAiImages {
                endpoint,
                api: ImageApi::Generations(ImageSizing::Size),
            }),
            ImageGenProvider::Openrouter => Arc::new(OpenAiImages {
                endpoint,
                api: ImageApi::OpenRouter,
            }),
            ImageGenProvider::Xai => Arc::new(OpenAiImages {
                endpoint,
                api: ImageApi::Generations(ImageSizing::AspectRatio("1:1")),
            }),
            ImageGenProvider::Huggingface => Arc::new(HuggingFaceImages(endpoint)),
            ImageGenProvider::Hyperbolic => Arc::new(HyperbolicImages(endpoint)),
            ImageGenProvider::Bedrock => Arc::new(BedrockImages::new(endpoint)?),
        };

        Ok(Self(model))
    }

    pub async fn generate(
        &self,
        prompt: &str,
        size: Option<&str>,
        references: &[ReferenceImage],
    ) -> Result<(Vec<u8>, String)> {
        self.0.generate(prompt, size, references).await
    }
}
