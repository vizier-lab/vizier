use rig_core::image_generation::ImageGenerationModel as _;
use rig_core::prelude::ImageGenerationClient;
use rig_core::providers::openai;

use crate::image_generation::VizierImageGenModel;
use crate::{Result, VizierError};

pub struct OpenAiImageGenModel {
    model: openai::image_generation::ImageGenerationModel,
}

impl OpenAiImageGenModel {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        let client = if let Some(base_url) = base_url {
            openai::Client::builder()
                .base_url(base_url)
                .api_key(api_key)
                .build()
                .expect("failed to build openai client")
        } else {
            openai::Client::new(&api_key).expect("failed to create openai client")
        };

        Self {
            model: client.image_generation_model(model),
        }
    }
}

#[async_trait::async_trait]
impl VizierImageGenModel for OpenAiImageGenModel {
    async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<(Vec<u8>, String)> {
        let mut builder = self
            .model
            .image_generation_request()
            .prompt(prompt);

        if let Some(size) = size
            && let Some((w, h)) = size
                .split_once('x')
                .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)))
        {
            builder = builder.width(w).height(h);
        }

        let response = builder
            .send()
            .await
            .map_err(|e| crate::VizierError(e.to_string()))?;

        Ok((response.image, "image/png".to_string()))
    }
}
