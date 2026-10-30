//! Hugging Face inference: transcription and image generation, as rig-core 0.39
//! `providers/huggingface/` sent them to the `hf-inference` route.

use base64::Engine;
use serde_json::{Value, json};

use super::openai_shaped::Endpoint;
use crate::image_generation::VizierImageGenModel;
use crate::stt::VizierSttModel;
use crate::{Result, VizierError};

pub const HUGGINGFACE_INFERENCE_URL: &str = "https://router.huggingface.co/hf-inference/models";

pub struct HuggingFaceStt(pub Endpoint);

#[async_trait::async_trait]
impl VizierSttModel for HuggingFaceStt {
    async fn transcribe(&self, audio: &[u8], _filename: &str, _language: Option<&str>) -> Result<String> {
        let body = json!({"inputs": base64::engine::general_purpose::STANDARD.encode(audio)});
        let path = format!("/{}", self.0.model);
        let req = self
            .0
            .post(&path)
            .await
            .map_err(|e| VizierError(format!("huggingface transcription: {e}")))?;
        let reply = crate::llm::http::send_json(req.json(&body))
            .await
            .map_err(|e| VizierError(format!("huggingface transcription: {e}")))?;
        if let Some(e) = reply.get("error") {
            return Err(VizierError(format!("huggingface transcription: {e}")));
        }
        reply
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| VizierError("huggingface transcription: no text in the response".into()))
    }
}

pub struct HuggingFaceImages(pub Endpoint);

#[async_trait::async_trait]
impl VizierImageGenModel for HuggingFaceImages {
    async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<(Vec<u8>, String)> {
        let (width, height) = size
            .and_then(|s| s.split_once('x'))
            .and_then(|(w, h)| Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?)))
            .unwrap_or((256, 256));
        let body = json!({"inputs": prompt, "parameters": {"width": width, "height": height}});
        let path = format!("/{}", self.0.model);
        let req = self
            .0
            .post(&path)
            .await
            .map_err(|e| VizierError(format!("huggingface image generation: {e}")))?;
        let response = crate::llm::http::send(req.json(&body))
            .await
            .map_err(|e| VizierError(format!("huggingface image generation: {e}")))?;
        let mime = response
            .headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .filter(|m| m.starts_with("image/"))
            .unwrap_or("image/png")
            .to_string();
        Ok((response.body, mime))
    }
}
