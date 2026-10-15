use std::sync::Arc;

use anyhow::Result;

use crate::{
    dependencies::VizierDependencies,
    llm::media::{
        self,
        cohere::CohereEmbeddings,
        gemini::{GeminiEmbeddings, gemini_dimensions},
        ollama::OllamaEmbeddings,
        openai_shaped::{OpenAiEmbeddings, openai_dimensions},
    },
    schema::agent::EmbeddingConfig,
};

pub mod fastembed;

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
        settings: &EmbeddingConfig,
        deps: &VizierDependencies,
        workspace: &str,
    ) -> Result<Self> {
        use crate::schema::agent::EmbeddingProvider;

        let provider = &settings.provider;
        let model = settings.model.clone();
        let Some(id) = provider.credentials() else {
            let variant = crate::config::embedding::LocalEmbeddingModelVariant::from_name(&model)
                .ok_or_else(|| anyhow::anyhow!("unknown local embedding model: {model}"))?;
            return Ok(Self::build(fastembed::LocalEmbedder::new(
                &variant.to_fastembed(),
                workspace,
            )?));
        };
        let endpoint = media::endpoint(deps, &id, model.clone()).await?;
        Ok(match provider {
            EmbeddingProvider::Ollama => {
                if !model.is_empty() {
                    crate::utils::ollama::ollama_pull_model(&endpoint.base_url, &model).await?;
                }
                Self::build(OllamaEmbeddings(endpoint))
            }
            EmbeddingProvider::Gemini => Self::build(GeminiEmbeddings {
                dimensions: gemini_dimensions(&model),
                endpoint,
            }),
            EmbeddingProvider::Cohere => Self::build(CohereEmbeddings {
                endpoint,
                input_type: "search_document",
            }),
            // rig 0.39 sent `dimensions` for the OpenAI models it knew, by name, on OpenAI and
            // Copilot only.
            EmbeddingProvider::Openai | EmbeddingProvider::Copilot => Self::build(OpenAiEmbeddings {
                dimensions: openai_dimensions(&model),
                endpoint,
            }),
            _ => Self::build(OpenAiEmbeddings {
                endpoint,
                dimensions: None,
            }),
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
