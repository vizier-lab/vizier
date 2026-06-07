use serde_json::json;

use crate::tts::mp3_to_wav;
use crate::Result;
use crate::tts::VizierTtsModel;

pub struct ElevenLabsTtsModel {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl ElevenLabsTtsModel {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }
}

#[async_trait::async_trait]
impl VizierTtsModel for ElevenLabsTtsModel {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let url = format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}",
            voice
        );

        let response = self
            .client
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&json!({
                "text": text,
                "model_id": self.model,
                "voice_settings": {
                    "speed": speed,
                }
            }))
            .send()
            .await
            .map_err(|e| crate::VizierError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".into());
            return Err(crate::VizierError(format!("{}: {}", status, body)));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| crate::VizierError(e.to_string()))?;

        mp3_to_wav(&bytes)
    }
}
