use std::sync::Arc;

use anyhow::Result;
use rig_core::client::{EmbeddingsClient, Nothing};

use crate::config::{VizierConfig, embedding::EmbeddingConfig, provider::ProviderConfig};

pub mod fastembed;
pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod openrouter;

#[async_trait::async_trait]
#[allow(unused)]
pub trait VizierEmbeddingModel {
    async fn embed_text(&self, text: &str) -> Result<Vec<f64>>;
    async fn embed_texts(&self, documents: Vec<String>) -> Result<Vec<Vec<f64>>>;
}

pub struct VizierEmbedder(Arc<Box<dyn VizierEmbeddingModel + Sync + Send + 'static>>);

impl VizierEmbedder {
    fn build<Model: VizierEmbeddingModel + Sync + Send + 'static>(model: Model) -> Self {
        Self(Arc::new(Box::new(model)))
    }

    pub async fn new(config: &VizierConfig) -> Result<Self> {
        Self::from_providers(&config.embedding, &config.providers, &config.workspace).await
    }

    pub async fn from_providers(
        embedding_config: &Option<EmbeddingConfig>,
        providers: &ProviderConfig,
        workspace: &str,
    ) -> Result<Self> {
        Ok(match embedding_config.clone().unwrap() {
            EmbeddingConfig::Local { model } => {
                let model = fastembed::Client::new()
                    .embedding_model(&model.to_fastembed(), Some(workspace.to_string()));

                Self::build(model)
            }
            EmbeddingConfig::Ollama { model } => {
                let base_url = providers.ollama.clone().unwrap().base_url;

                crate::utils::ollama::ollama_pull_model(&base_url, &model).await?;

                let model = rig_core::providers::ollama::Client::builder()
                    .base_url(base_url)
                    .api_key(Nothing)
                    .build()?
                    .embedding_model(&model);

                Self::build(model)
            }
            EmbeddingConfig::Openai { model } => {
                let model = rig_core::providers::openai::Client::new(
                    providers.openai.clone().unwrap().api_key,
                )?
                .embedding_model(&model);

                Self::build(model)
            }
            EmbeddingConfig::Gemini { model } => {
                let model = rig_core::providers::gemini::Client::new(
                    providers.gemini.clone().unwrap().api_key,
                )?
                .embedding_model(&model);

                Self::build(model)
            }
            EmbeddingConfig::Openrouter { model } => {
                let model = rig_core::providers::openrouter::Client::new(
                    providers.openrouter.clone().unwrap().api_key,
                )?
                .embedding_model(&model);

                Self::build(model)
            }
        })
    }
}

#[async_trait::async_trait]
impl VizierEmbeddingModel for VizierEmbedder {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        self.0.embed_text(text).await
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        self.0.embed_texts(documents).await
    }
}
