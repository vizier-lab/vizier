use rig_core::image_generation::ImageGenerationModel as _;
use rig_core::prelude::ImageGenerationClient;
use rig_core::providers::hyperbolic;

use crate::image_generation::VizierImageGenModel;
use crate::{Result, VizierError};

pub struct HyperbolicImageGenModel {
    model: hyperbolic::ImageGenerationModel<reqwest::Client>,
}

impl HyperbolicImageGenModel {
    pub fn new(api_key: String, model: String) -> Self {
        let client =
            hyperbolic::Client::new(&api_key).expect("failed to create hyperbolic client");

        Self {
            model: client.image_generation_model(model),
        }
    }
}

#[async_trait::async_trait]
impl VizierImageGenModel for HyperbolicImageGenModel {
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
            .map_err(|e| VizierError(e.to_string()))?;

        Ok((response.image, "image/png".to_string()))
    }
}
