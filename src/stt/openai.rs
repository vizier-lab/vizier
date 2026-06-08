use rig_core::prelude::TranscriptionClient;
use rig_core::providers::openai;
use rig_core::transcription::{TranscriptionModel as _, TranscriptionRequestBuilder};

use crate::{Result, VizierError};

pub struct OpenAiSttModel {
    model: openai::transcription::TranscriptionModel,
}

impl OpenAiSttModel {
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
            model: client.transcription_model(model),
        }
    }
}

#[async_trait::async_trait]
impl crate::stt::VizierSttModel for OpenAiSttModel {
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

        let response = builder.send().await.map_err(|e| {
            VizierError(format!("openai transcription: {e}"))
        })?;

        Ok(response.text)
    }
}
