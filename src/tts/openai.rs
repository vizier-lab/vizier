use rig_core::audio_generation::AudioGenerationModel as _;
use rig_core::prelude::AudioGenerationClient;
use rig_core::providers::openai;

use crate::tts::mp3_to_wav;
use crate::Result;
use crate::tts::VizierTtsModel;

pub struct OpenAiTtsModel {
    model: openai::audio_generation::AudioGenerationModel,
}

impl OpenAiTtsModel {
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
            model: client.audio_generation_model(model),
        }
    }
}

#[async_trait::async_trait]
impl VizierTtsModel for OpenAiTtsModel {
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
