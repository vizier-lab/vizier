use rig_core::audio_generation::AudioGenerationModel as _;
use rig_core::prelude::AudioGenerationClient;
use rig_core::providers::hyperbolic;

use crate::tts::VizierTtsModel;
use crate::{Result, VizierError};

pub struct HyperbolicTtsModel {
    model: hyperbolic::AudioGenerationModel<reqwest::Client>,
}

impl HyperbolicTtsModel {
    pub fn new(api_key: String, model: String) -> Self {
        let client =
            hyperbolic::Client::new(&api_key).expect("failed to create hyperbolic client");

        Self {
            model: client.audio_generation_model(model),
        }
    }
}

#[async_trait::async_trait]
impl VizierTtsModel for HyperbolicTtsModel {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let response = self
            .model
            .audio_generation_request()
            .text(text)
            .voice(voice)
            .speed(speed)
            .send()
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(response.audio)
    }
}
