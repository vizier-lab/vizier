//! Cohere embeddings: `POST {base}/embed` with `{model, texts, input_type}`, as rig-core 0.39
//! `providers/cohere/embeddings.rs` sent them (Cohere's v1 API).

use serde_json::{Value, json};

use super::openai_shaped::Endpoint;
use crate::embedding::VizierEmbeddingModel;

/// The v1 API host; not the OpenAI-compatibility URL the catalogue uses for chat.
pub const COHERE_EMBED_BASE_URL: &str = "https://api.cohere.ai/v1";

pub struct CohereEmbeddings {
    pub endpoint: Endpoint,
    pub input_type: &'static str,
}

#[async_trait::async_trait]
impl VizierEmbeddingModel for CohereEmbeddings {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        let mut all = self.embed_texts(vec![text.to_string()]).await?;
        all.pop()
            .ok_or_else(|| anyhow::anyhow!("cohere embeddings: no vector returned"))
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }
        let body = json!({
            "model": self.endpoint.model,
            "texts": documents,
            "input_type": self.input_type,
        });
        let req = self
            .endpoint
            .post("/embed")
            .await
            .map_err(|e| anyhow::anyhow!("cohere embeddings: {e}"))?;
        let reply = crate::llm::http::send_json(req.json(&body))
            .await
            .map_err(|e| anyhow::anyhow!("cohere embeddings: {e}"))?;
        // v1 answers `embeddings: [[…]]`; with `embedding_types` it nests them under `float`.
        let embeddings = reply
            .get("embeddings")
            .and_then(|e| e.as_array().or_else(|| e.get("float").and_then(Value::as_array)))
            .ok_or_else(|| anyhow::anyhow!("cohere embeddings: no embeddings in the response"))?;
        if embeddings.len() != documents.len() {
            anyhow::bail!(
                "cohere embeddings: {} vectors for {} inputs",
                embeddings.len(),
                documents.len()
            );
        }
        Ok(embeddings
            .iter()
            .map(|v| {
                v.as_array()
                    .map(|v| v.iter().filter_map(Value::as_f64).collect())
                    .unwrap_or_default()
            })
            .collect())
    }
}
