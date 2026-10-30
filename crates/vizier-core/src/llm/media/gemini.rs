//! Gemini media: transcription through `generateContent` with the audio inline, and
//! `batchEmbedContents`. Ported from rig-core 0.39 `providers/gemini/{transcription,embedding}`.

use base64::Engine;
use serde_json::{Value, json};

use super::openai_shaped::Endpoint;
use crate::embedding::VizierEmbeddingModel;
use crate::stt::VizierSttModel;
use crate::{Result, VizierError};

/// rig 0.39's transcription instruction, kept verbatim.
const TRANSCRIPTION_PREAMBLE: &str =
    "Translate the provided audio exactly. Do not add additional information.";

pub struct GeminiStt(pub Endpoint);

#[async_trait::async_trait]
impl VizierSttModel for GeminiStt {
    async fn transcribe(&self, audio: &[u8], filename: &str, _language: Option<&str>) -> Result<String> {
        let mime = mime_guess::from_path(filename)
            .first()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "audio/mpeg".into());
        let body = json!({
            "contents": [{"role": "user", "parts": [{"thought": false, "inlineData": {
                "mimeType": mime,
                "data": base64::engine::general_purpose::STANDARD.encode(audio),
            }}]}],
            "generationConfig": {},
            "systemInstruction": {"role": "model", "parts": [{"thought": false, "text": TRANSCRIPTION_PREAMBLE}]},
        });
        let path = format!("/models/{}:generateContent", self.0.model);
        let req = self
            .0
            .post(&path)
            .await
            .map_err(|e| VizierError(format!("gemini transcription: {e}")))?;
        let reply = crate::llm::http::send_json(req.json(&body))
            .await
            .map_err(|e| VizierError(format!("gemini transcription: {e}")))?;
        reply
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| VizierError("gemini transcription: response content contains no text".into()))
    }
}

pub struct GeminiEmbeddings {
    pub endpoint: Endpoint,
    pub dimensions: usize,
}

/// rig 0.39's dimensions for Gemini embedding models; 768 otherwise.
pub fn gemini_dimensions(model: &str) -> usize {
    match model {
        "embedding-001" => 3072,
        "text-embedding-004" => 768,
        _ => 768,
    }
}

#[async_trait::async_trait]
impl VizierEmbeddingModel for GeminiEmbeddings {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        let mut all = self.embed_texts(vec![text.to_string()]).await?;
        all.pop()
            .ok_or_else(|| anyhow::anyhow!("gemini embeddings: no vector returned"))
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }
        let model = &self.endpoint.model;
        let requests: Vec<Value> = documents
            .iter()
            .map(|doc| {
                json!({
                    "model": format!("models/{model}"),
                    "content": {"parts": [{"text": doc}]},
                    "output_dimensionality": self.dimensions,
                })
            })
            .collect();
        let path = format!("/models/{model}:batchEmbedContents");
        let req = self
            .endpoint
            .post(&path)
            .await
            .map_err(|e| anyhow::anyhow!("gemini embeddings: {e}"))?;
        let reply = crate::llm::http::send_json(req.json(&json!({"requests": requests})))
            .await
            .map_err(|e| anyhow::anyhow!("gemini embeddings: {e}"))?;
        let embeddings = reply
            .get("embeddings")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("gemini embeddings: no embeddings in the response"))?;
        if embeddings.len() != documents.len() {
            anyhow::bail!(
                "gemini embeddings: {} vectors for {} inputs",
                embeddings.len(),
                documents.len()
            );
        }
        Ok(embeddings
            .iter()
            .map(|e| {
                e.get("values")
                    .and_then(Value::as_array)
                    .map(|v| v.iter().filter_map(Value::as_f64).collect())
                    .unwrap_or_default()
            })
            .collect())
    }
}
