use std::sync::Arc;

use anyhow::Result;
use rig_core::client::{EmbeddingsClient, Nothing};

use crate::{
    config::provider::ProviderVariant,
    provider_keys::{resolve_local_provider, resolve_provider_key},
    schema::agent::EmbeddingToolSettings,
    storage::VizierStorage,
};

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

    pub async fn from_agent_settings(
        settings: &EmbeddingToolSettings,
        storage: &Arc<VizierStorage>,
        workspace: &str,
    ) -> Result<Self> {
        use crate::schema::agent::EmbeddingProvider;

        Ok(match settings.provider {
            EmbeddingProvider::Local => {
                let variant = crate::config::embedding::LocalEmbeddingModelVariant::from_name(
                    &settings.model,
                )
                .ok_or_else(|| {
                    anyhow::anyhow!("unknown local embedding model: {}", settings.model)
                })?;
                let model = fastembed::Client::new()
                    .embedding_model(&variant.to_fastembed(), Some(workspace.to_string()));
                Self::build(model)
            }
            EmbeddingProvider::Ollama => {
                let resolved = resolve_local_provider(
                    storage,
                    ProviderVariant::ollama,
                    "OLLAMA_BASE_URL",
                    "http://localhost:11434",
                )
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
                let base_url = settings
                    .base_url
                    .clone()
                    .or(resolved.base_url)
                    .unwrap_or_else(|| "http://localhost:11434".to_string());
                let model_name = settings.model.clone();
                if !model_name.is_empty() {
                    crate::utils::ollama::ollama_pull_model(&base_url, &model_name).await?;
                }
                let model = rig_core::providers::ollama::Client::builder()
                    .base_url(base_url)
                    .api_key(Nothing)
                    .build()?
                    .embedding_model(&model_name);
                Self::build(model)
            }
            EmbeddingProvider::Openai => {
                let resolved = resolve_provider_key(
                    storage,
                    ProviderVariant::openai,
                    "OPENAI_API_KEY",
                )
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
                let model_name = settings.model.clone();
                let model = rig_core::providers::openai::Client::new(resolved.api_key)?
                    .embedding_model(&model_name);
                Self::build(model)
            }
            EmbeddingProvider::Gemini => {
                let resolved = resolve_provider_key(
                    storage,
                    ProviderVariant::gemini,
                    "GEMINI_API_KEY",
                )
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
                let model_name = settings.model.clone();
                let model = rig_core::providers::gemini::Client::new(resolved.api_key)?
                    .embedding_model(&model_name);
                Self::build(model)
            }
            EmbeddingProvider::Openrouter => {
                let resolved = resolve_provider_key(
                    storage,
                    ProviderVariant::openrouter,
                    "OPENROUTER_API_KEY",
                )
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
                let model_name = settings.model.clone();
                let model = rig_core::providers::openrouter::Client::new(resolved.api_key)?
                    .embedding_model(&model_name);
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
