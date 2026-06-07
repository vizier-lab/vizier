use rig_core::audio_generation::AudioGenerationModel as _;
use rig_core::prelude::AudioGenerationClient;
use rig_core::providers::openrouter;

use crate::tts::mp3_to_wav;
use crate::Result;
use crate::tts::VizierTtsModel;

pub struct OpenRouterTtsModel {
    model: openrouter::AudioGenerationModel,
}

impl OpenRouterTtsModel {
    pub fn new(api_key: String, model: String) -> Self {
        let client =
            openrouter::Client::new(&api_key).expect("failed to create openrouter client");

        Self {
            model: client.audio_generation_model(model),
        }
    }
}

#[async_trait::async_trait]
impl VizierTtsModel for OpenRouterTtsModel {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let response = self
            .model
            .audio_generation_request()
            .text(text)
            .voice(voice)
            .speed(speed)
            .send()
            .await
            .map_err(|e| crate::VizierError(e.to_string()))?;

        mp3_to_wav(&response.audio)
    }
}
