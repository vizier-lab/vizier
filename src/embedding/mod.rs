/// originally from yoinked from https://github.com/0xPlaygrounds/rig/blob/main/rig-integrations/rig-fastembed/src/lib.rs
use std::{path::PathBuf, str::FromStr, sync::Arc};

pub use fastembed::EmbeddingModel as FastembedModel;
use fastembed::TextEmbedding;
use rig::embeddings::{self, EmbeddingError};

use fastembed::InitOptions;
use rig::{Embed, embeddings::EmbeddingsBuilder};

#[derive(Clone)]
pub struct Client;

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Create a new `rig-fastembed` client.
    pub fn new() -> Self {
        Self
    }

    pub fn embedding_model(
        &self,
        model: &FastembedModel,
        workspace: Option<String>,
    ) -> EmbeddingModel {
        let model_info = TextEmbedding::get_model_info(model).unwrap();
        let ndims = model_info.dim;

        EmbeddingModel::new(
            model,
            match workspace {
                None => None,
                Some(workspace) => {
                    let path =
                        PathBuf::from_str(&format!("{workspace}/embeddings/{}", model.to_string()))
                            .unwrap();
                    Some(path)
                }
            },
            ndims,
        )
    }

    /// Create an embedding builder with the given embedding model.
    ///
    /// # Example
    /// ```
    /// use rig_fastembed::{Client, FastembedModel};
    ///
    /// // Initialize the Fastembed client
    /// let fastembed_client = Client::new();
    ///
    /// let embeddings = fastembed_client.embeddings(FastembedModel::AllMiniLML6V2Q)
    ///     .simple_document("doc0", "Hello, world!")
    ///     .simple_document("doc1", "Goodbye, world!")
    ///     .build()
    ///     .await
    ///     .expect("Failed to embed documents");
    /// ```
    pub fn embeddings<D: Embed>(
        &self,
        model: &fastembed::EmbeddingModel,
    ) -> EmbeddingsBuilder<EmbeddingModel, D> {
        EmbeddingsBuilder::new(self.embedding_model(model, None))
    }
}

#[derive(Clone)]
pub struct EmbeddingModel {
    embedder: Arc<TextEmbedding>,
    pub model: FastembedModel,
    ndims: usize,
}

impl EmbeddingModel {
    pub fn new(
        model: &fastembed::EmbeddingModel,
        cache_dir: Option<PathBuf>,
        ndims: usize,
    ) -> Self {
        let mut opts = InitOptions::new(model.to_owned()).with_show_download_progress(true);

        if let Some(cache_dir) = cache_dir {
            opts = opts.with_cache_dir(cache_dir);
        }

        let embedder = Arc::new(TextEmbedding::try_new(opts).unwrap());

        Self {
            embedder,
            model: model.to_owned(),
            ndims,
        }
    }
}

impl embeddings::EmbeddingModel for EmbeddingModel {
    const MAX_DOCUMENTS: usize = 1024;

    type Client = Client;

    /// **PANICS**: FastEmbed models cannot be created via this method, which will panic
    fn make(_: &Self::Client, _: impl Into<String>, _: Option<usize>) -> Self {
        panic!("Cannot create a fastembed model via `EmbeddingModel::make`")
    }

    fn ndims(&self) -> usize {
        self.ndims
    }

    async fn embed_texts(
        &self,
        documents: impl IntoIterator<Item = String>,
    ) -> Result<Vec<embeddings::Embedding>, EmbeddingError> {
        let documents_as_strings: Vec<String> = documents.into_iter().collect();

        let documents_as_vec = self
            .embedder
            .embed(documents_as_strings.clone(), None)
            .map_err(|err| EmbeddingError::ProviderError(err.to_string()))?;

        let docs = documents_as_strings
            .into_iter()
            .zip(documents_as_vec)
            .map(|(document, embedding)| embeddings::Embedding {
                document,
                vec: embedding.into_iter().map(|f| f as f64).collect(),
            })
            .collect::<Vec<embeddings::Embedding>>();

        Ok(docs)
    }
}
