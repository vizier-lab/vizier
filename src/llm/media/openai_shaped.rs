//! OpenAI-shaped media endpoints: transcription, speech, image generation and embeddings, as
//! OpenAI and the providers that copy its API serve them. Request fields and defaults are
//! ported from the matching rig-core 0.39 modules (openai, groq, mistral, openrouter, xai,
//! together, copilot, voyageai); where a provider's endpoint differs, the difference is a
//! field set by the factory, never a provider check here.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};

use crate::embedding::VizierEmbeddingModel;
use crate::image_generation::VizierImageGenModel;
use crate::llm::auth::{CredentialSource, RequestInfo};
use crate::llm::dialect::join_url;
use crate::llm::error::LlmError;
use crate::llm::http;
use crate::stt::VizierSttModel;
use crate::tts::{VizierTtsModel, mp3_to_wav};
use crate::{Result, VizierError};

/// Where and how to reach one provider's media API.
#[derive(Clone)]
pub struct Endpoint {
    pub http: reqwest::Client,
    pub base_url: String,
    pub credential: Arc<dyn CredentialSource>,
    pub model: String,
}

impl Endpoint {
    pub(crate) async fn post(&self, path: &str) -> std::result::Result<reqwest::RequestBuilder, LlmError> {
        let base = self
            .credential
            .base_url()
            .await?
            .unwrap_or_else(|| self.base_url.clone());
        let req = self
            .http
            .post(join_url(&base, path))
            .timeout(Duration::from_secs(300));
        self.credential.authorize(req, &RequestInfo::default()).await
    }
}

fn err(what: &str, e: LlmError) -> VizierError {
    VizierError(format!("{what}: {e}"))
}

// Speech to text -------------------------------------------------------------------------------

/// `POST {base}/audio/transcriptions`, multipart `file`, `model` and optional `language`.
pub struct OpenAiStt(pub Endpoint);

#[async_trait::async_trait]
impl VizierSttModel for OpenAiStt {
    async fn transcribe(&self, audio: &[u8], filename: &str, language: Option<&str>) -> Result<String> {
        let mut form = reqwest::multipart::Form::new()
            .text("model", self.0.model.clone())
            .part(
                "file",
                reqwest::multipart::Part::bytes(audio.to_vec()).file_name(filename.to_string()),
            );
        if let Some(language) = language {
            form = form.text("language", language.to_string());
        }
        let req = self.0.post("/audio/transcriptions").await.map_err(|e| err("transcription", e))?;
        let response = http::send_multipart(req, form)
            .await
            .map_err(|e| err("transcription", e))?;
        let body = response.json().map_err(|e| err("transcription", e))?;
        if let Some(e) = crate::llm::dialect::error_in_body(&body) {
            return Err(VizierError(format!("transcription: {e}")));
        }
        body.get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| VizierError(format!("transcription: no text in {body}")))
    }
}

// Text to speech -------------------------------------------------------------------------------

/// `POST {base}/audio/speech` with `{model, input, voice, speed}`; the MP3 reply becomes WAV.
pub struct OpenAiTts {
    pub endpoint: Endpoint,
    /// OpenRouter needs the format named; OpenAI defaults to MP3.
    pub response_format: Option<&'static str>,
}

#[async_trait::async_trait]
impl VizierTtsModel for OpenAiTts {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let mut body = json!({
            "model": self.endpoint.model,
            "input": text,
            "voice": voice,
            "speed": speed,
        });
        if let Some(format) = self.response_format {
            body["response_format"] = json!(format);
        }
        let req = self.endpoint.post("/audio/speech").await.map_err(|e| err("speech", e))?;
        let response = http::send(req.json(&body)).await.map_err(|e| err("speech", e))?;
        mp3_to_wav(&response.body)
    }
}

/// xAI's own speech API: `POST {base}/tts` with `{text, voice_id, language}`; no model field,
/// and the audio comes back as it is.
pub struct XaiTts(pub Endpoint);

#[async_trait::async_trait]
impl VizierTtsModel for XaiTts {
    async fn generate_speech(&self, text: &str, voice: &str, _speed: f32) -> Result<Vec<u8>> {
        let voice = if voice.is_empty() { "eve" } else { voice };
        let body = json!({"text": text, "voice_id": voice, "language": "en"});
        let req = self.0.post("/tts").await.map_err(|e| err("speech", e))?;
        let response = http::send(req.json(&body)).await.map_err(|e| err("speech", e))?;
        Ok(response.body)
    }
}

// Image generation -----------------------------------------------------------------------------

/// How a provider sizes images: OpenAI takes `size: "WxH"`, xAI only an aspect ratio.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ImageSizing {
    Size,
    AspectRatio(&'static str),
}

/// `POST {base}/images/generations`, decoding `b64_json` (or fetching `url`).
pub struct OpenAiImages {
    pub endpoint: Endpoint,
    pub sizing: ImageSizing,
}

/// rig 0.39's default when no size was asked for.
const DEFAULT_SIZE: (u32, u32) = (256, 256);

fn parse_size(size: Option<&str>) -> (u32, u32) {
    size.and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
        .unwrap_or(DEFAULT_SIZE)
}

#[async_trait::async_trait]
impl VizierImageGenModel for OpenAiImages {
    async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<(Vec<u8>, String)> {
        let model = &self.endpoint.model;
        let mut body = json!({"model": model, "prompt": prompt});
        match self.sizing {
            ImageSizing::Size => {
                let (w, h) = parse_size(size);
                body["size"] = json!(format!("{w}x{h}"));
                // The GPT image models always answer in base64 and refuse the parameter.
                if !model.starts_with("gpt-image") {
                    body["response_format"] = json!("b64_json");
                }
            }
            ImageSizing::AspectRatio(ratio) => {
                body["response_format"] = json!("b64_json");
                body["aspect_ratio"] = json!(ratio);
            }
        }
        let req = self
            .endpoint
            .post("/images/generations")
            .await
            .map_err(|e| err("image generation", e))?;
        let reply = http::send_json(req.json(&body))
            .await
            .map_err(|e| err("image generation", e))?;
        let first = reply
            .pointer("/data/0")
            .ok_or_else(|| VizierError("image generation: missing image data".into()))?;
        if let Some(b64) = first.get("b64_json").and_then(Value::as_str) {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| VizierError(format!("image generation: {e}")))?;
            return Ok((bytes, "image/png".into()));
        }
        if let Some(url) = first.get("url").and_then(Value::as_str) {
            let image = http::send(self.endpoint.http.get(url))
                .await
                .map_err(|e| err("image download", e))?;
            let mime = image
                .headers
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .filter(|m| m.starts_with("image/"))
                .unwrap_or("image/png")
                .to_string();
            return Ok((image.body, mime));
        }
        Err(VizierError("image generation: missing image data".into()))
    }
}

// Embeddings -----------------------------------------------------------------------------------

/// `POST {base}/embeddings` with `{model, input}`, plus `dimensions` when the factory knows
/// them (rig 0.39 sent the model's native size for the OpenAI-family models).
pub struct OpenAiEmbeddings {
    pub endpoint: Endpoint,
    pub dimensions: Option<usize>,
}

/// The `dimensions` rig 0.39 sent for an OpenAI-family embedding model: known sizes only, and
/// never for `text-embedding-ada-002`, which refuses the parameter.
pub fn openai_dimensions(model: &str) -> Option<usize> {
    match model {
        "text-embedding-3-large" => Some(3072),
        "text-embedding-3-small" => Some(1536),
        _ => None,
    }
}

pub(crate) fn order_by_index(items: &[Value], count: usize) -> std::result::Result<Vec<Vec<f64>>, String> {
    let mut indexed: Vec<(usize, Vec<f64>)> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let index = item.get("index").and_then(Value::as_u64).map_or(i, |n| n as usize);
            let vector = item
                .get("embedding")
                .and_then(Value::as_array)
                .map(|v| v.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default();
            (index, vector)
        })
        .collect();
    if indexed.len() != count {
        return Err(format!(
            "the response holds {} embeddings for {count} inputs",
            indexed.len()
        ));
    }
    indexed.sort_by_key(|(i, _)| *i);
    Ok(indexed.into_iter().map(|(_, v)| v).collect())
}

#[async_trait::async_trait]
impl VizierEmbeddingModel for OpenAiEmbeddings {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        let mut all = self.embed_texts(vec![text.to_string()]).await?;
        all.pop()
            .ok_or_else(|| anyhow::anyhow!("embeddings: no vector returned"))
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }
        let mut body = json!({"model": self.endpoint.model, "input": documents});
        if let Some(dims) = self.dimensions {
            body["dimensions"] = json!(dims);
        }
        let req = self
            .endpoint
            .post("/embeddings")
            .await
            .map_err(|e| anyhow::anyhow!("embeddings: {e}"))?;
        let reply = http::send_json(req.json(&body))
            .await
            .map_err(|e| anyhow::anyhow!("embeddings: {e}"))?;
        let data = reply
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("embeddings: no data in the response"))?;
        order_by_index(data, documents.len()).map_err(|e| anyhow::anyhow!("embeddings: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_default_like_rig() {
        assert_eq!(parse_size(None), (256, 256));
        assert_eq!(parse_size(Some("1024x768")), (1024, 768));
        assert_eq!(parse_size(Some("big")), (256, 256));
    }

    #[test]
    fn embeddings_are_returned_in_input_order() {
        let items = vec![
            json!({"index": 1, "embedding": [2.0]}),
            json!({"index": 0, "embedding": [1.0, 0.5]}),
        ];
        assert_eq!(order_by_index(&items, 2).unwrap(), vec![vec![1.0, 0.5], vec![2.0]]);
        assert!(order_by_index(&items, 3).is_err());
    }
}
