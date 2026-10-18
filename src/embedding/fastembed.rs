//! Local embeddings with fastembed (no network after the model download).

use std::sync::Arc;

pub use fastembed::EmbeddingModel as FastembedModel;
use fastembed::{InitOptions, TextEmbedding};

use crate::{embedding::VizierEmbeddingModel, utils::build_path};

#[derive(Clone)]
pub struct LocalEmbedder {
    embedder: Arc<TextEmbedding>,
}

impl LocalEmbedder {
    /// The model is cached under `{workspace}/.runtime/embeddings/<model code>`: the Hugging
    /// Face code (e.g. `Qdrant/all-MiniLM-L6-v2-onnx`) is what fastembed < 5 rendered as
    /// `Display`, so existing caches keep loading.
    ///
    /// Progress goes to the WebUI through the setup board; fastembed exposes no byte
    /// callback, so the download is indeterminate and the UI shows a spinner and message.
    pub fn new(
        model: &FastembedModel,
        workspace: &str,
        setup: &crate::setup::SetupBoard,
        agent_id: &str,
    ) -> anyhow::Result<Self> {
        use crate::setup::DownloadKind;

        let info = TextEmbedding::get_model_info(model)
            .map_err(|e| anyhow::anyhow!("unknown local embedding model: {e}"))?;
        let cache = build_path(workspace, &[".runtime", "embeddings", &info.model_code]);
        // An existing, non-empty cache means a load, not a download: don't report one.
        let cached = std::fs::read_dir(&cache).is_ok_and(|mut entries| entries.next().is_some());
        let opts = InitOptions::new(model.to_owned())
            .with_show_download_progress(false)
            .with_cache_dir(cache);

        if !cached {
            setup.download_started(
                agent_id,
                DownloadKind::Embedding,
                format!("Downloading embedding model {}", info.model_code),
            );
        }
        match TextEmbedding::try_new(opts) {
            Ok(embedder) => {
                if !cached {
                    setup.download_finished(agent_id, DownloadKind::Embedding);
                }
                Ok(Self {
                    embedder: Arc::new(embedder),
                })
            }
            Err(e) => {
                let error = anyhow::anyhow!("cannot load the local embedding model: {e}");
                if !cached {
                    setup.download_failed(agent_id, DownloadKind::Embedding, format!("{error:#}"));
                }
                Err(error)
            }
        }
    }
}

#[async_trait::async_trait]
impl VizierEmbeddingModel for LocalEmbedder {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        let mut all = self.embed_texts(vec![text.to_string()]).await?;
        all.pop()
            .ok_or_else(|| anyhow::anyhow!("local embeddings: no vector returned"))
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        let vectors = self
            .embedder
            .embed(documents, None)
            .map_err(|e| anyhow::anyhow!("local embeddings: {e}"))?;
        Ok(vectors
            .into_iter()
            .map(|v| v.into_iter().map(|f| f as f64).collect())
            .collect())
    }
}
