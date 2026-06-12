use rig_core::prelude::TranscriptionClient;
use rig_core::providers::mistral;
use rig_core::transcription::{TranscriptionModel as _, TranscriptionRequestBuilder};

use crate::{Result, VizierError};

pub struct MistralSttModel {
    model: mistral::TranscriptionModel,
}

impl MistralSttModel {
    pub fn new(api_key: String, model: String) -> Self {
        let client = mistral::Client::new(&api_key).expect("failed to create mistral client");

        Self {
            model: client.transcription_model(model),
        }
    }
}

#[async_trait::async_trait]
impl crate::stt::VizierSttModel for MistralSttModel {
    async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String> {
        let mut builder = TranscriptionRequestBuilder::new(self.model.clone())
            .data(audio.to_vec())
            .filename(Some(filename.to_string()));

        if let Some(lang) = language {
            builder = builder.language(lang.to_string());
        }

        let response = builder
            .send()
            .await
            .map_err(|e| VizierError(format!("mistral transcription: {e}")))?;

        Ok(response.text)
    }
}
