use reqwest::Client;

use crate::{Result, VizierError};

pub struct ElevenLabsSttModel {
    api_key: String,
    model: String,
}

impl ElevenLabsSttModel {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

#[async_trait::async_trait]
impl crate::stt::VizierSttModel for ElevenLabsSttModel {
    async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String> {
        let client = Client::new();

        let file_part = reqwest::multipart::Part::bytes(audio.to_vec())
            .file_name(filename.to_string())
            .mime_str("audio/wav")
            .map_err(|e| VizierError(format!("mime type: {e}")))?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("model_id", self.model.clone());

        if let Some(lang) = language {
            form = form.text("language_code", lang.to_string());
        }

        let response = client
            .post("https://api.elevenlabs.io/v1/speech-to-text")
            .header("xi-api-key", &self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| VizierError(format!("elevenlabs STT request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".into());
            return Err(VizierError(format!(
                "elevenlabs STT error ({}): {}",
                status, body
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| VizierError(format!("elevenlabs STT response parse: {e}")))?;

        let text = json["text"]
            .as_str()
            .ok_or_else(|| VizierError("elevenlabs STT response missing 'text' field".into()))?;

        Ok(text.to_string())
    }
}
