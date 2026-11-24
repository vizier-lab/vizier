use std::sync::Arc;

use crate::dependencies::VizierDependencies;
use crate::llm::media::{
    self,
    bedrock::BedrockImages,
    huggingface::HuggingFaceImages,
    hyperbolic::HyperbolicImages,
    openai_shaped::{ImageApi, ImageSizing, OpenAiImages},
    venice::VeniceImages,
};
use crate::agents::agent::model::usage::UsageScope;
use crate::schema::agent::{ImageGenProvider, ImageGenToolSettings};
use crate::schema::usage::{MediaUnits, UsageKind};
use crate::usage::record::{MediaMeter, Measured, Reported};
use crate::Result;

/// A generated image and what the provider said it used (spec 065).
pub struct GeneratedImage {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub reported: Reported,
}

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

    /// [`Self::generate`] with the provider's own usage figures; a client that reads them from
    /// its response overrides this (spec 065 research R3).
    async fn generate_reported(
        &self,
        prompt: &str,
        size: Option<&str>,
        references: &[ReferenceImage],
    ) -> Result<GeneratedImage> {
        let (bytes, mime) = self.generate(prompt, size, references).await?;
        Ok(GeneratedImage {
            bytes,
            mime,
            reported: Reported::default(),
        })
    }
}

pub struct VizierImageGen {
    model: Arc<dyn VizierImageGenModel>,
    meter: MediaMeter,
}

impl VizierImageGen {
    pub async fn new(
        settings: &ImageGenToolSettings,
        deps: &VizierDependencies,
        agent_id: &str,
    ) -> Result<Self> {
        let provider = &settings.provider;
        let model = settings
            .model
            .clone()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| provider.default_model().into());
        let Some(id) = provider.credentials() else {
            let meter = MediaMeter::new(
                agent_id,
                crate::usage::record::provider_name(provider),
                model,
                true,
                deps.storage.clone(),
            );
            return Ok(Self {
                model: Arc::new(media::dummyplug::DummyplugImages),
                meter,
            });
        };
        let meter = MediaMeter::new(agent_id, id.to_string(), model.clone(), false, deps.storage.clone());
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
            ImageGenProvider::Venice => Arc::new(VeniceImages(endpoint)),
            ImageGenProvider::Together => Arc::new(OpenAiImages {
                endpoint,
                api: ImageApi::Together,
            }),
            ImageGenProvider::Dummyplug => unreachable!("dummyplug returned above"),
        };

        Ok(Self { model, meter })
    }

    pub async fn generate(
        &self,
        prompt: &str,
        size: Option<&str>,
        references: &[ReferenceImage],
        scope: &UsageScope,
    ) -> Result<(Vec<u8>, String)> {
        let started = std::time::Instant::now();
        let out = self.model.generate_reported(prompt, size, references).await?;
        self.meter.record(
            scope,
            UsageKind::Image,
            out.reported,
            Measured {
                units: MediaUnits {
                    images: 1,
                    ..Default::default()
                },
                input_tokens: None,
                variant: size.filter(|s| !s.trim().is_empty()).map(str::to_string),
            },
            started.elapsed(),
        );
        Ok((out.bytes, out.mime))
    }
}
