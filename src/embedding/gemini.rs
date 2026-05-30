use rig_core::{embeddings, providers::gemini};

use crate::embedding::VizierEmbeddingModel;

#[async_trait::async_trait]
impl VizierEmbeddingModel for gemini::EmbeddingModel {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        Ok(<Self as embeddings::EmbeddingModel>::embed_text(self, text)
            .await?
            .vec)
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        Ok(
            <Self as embeddings::EmbeddingModel>::embed_texts(self, documents)
                .await?
                .iter()
                .map(|item| item.vec.clone())
                .collect::<Vec<_>>(),
        )
    }
}
