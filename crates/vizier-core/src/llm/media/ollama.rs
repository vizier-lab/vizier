//! Ollama embeddings: `POST {base}/api/embed` with `{model, input}`.

use serde_json::{Value, json};

use super::openai_shaped::Endpoint;
use crate::embedding::VizierEmbeddingModel;

pub struct OllamaEmbeddings(pub Endpoint);

#[async_trait::async_trait]
impl VizierEmbeddingModel for OllamaEmbeddings {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        let mut all = self.embed_texts(vec![text.to_string()]).await?;
        all.pop()
            .ok_or_else(|| anyhow::anyhow!("ollama embeddings: no vector returned"))
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }
        let body = json!({"model": self.0.model, "input": documents});
        let req = self
            .0
            .post("/api/embed")
            .await
            .map_err(|e| anyhow::anyhow!("ollama embeddings: {e}"))?;
        let reply = crate::llm::http::send_json(req.json(&body))
            .await
            .map_err(|e| anyhow::anyhow!("ollama embeddings: {e}"))?;
        let embeddings = reply
            .get("embeddings")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("ollama embeddings: no embeddings in the response"))?;
        if embeddings.len() != documents.len() {
            anyhow::bail!("ollama embeddings: number of returned embeddings does not match input");
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
