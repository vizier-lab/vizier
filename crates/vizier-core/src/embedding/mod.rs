use std::sync::Arc;

use anyhow::Result;

use crate::agents::agent::model::usage::media_scope;
use crate::schema::usage::UsageKind;
use crate::usage::record::{MediaMeter, Measured, Reported, estimate_tokens, provider_name};

use crate::{
    dependencies::VizierDependencies,
    llm::media::{
        self,
        cohere::CohereEmbeddings,
        bedrock::BedrockEmbeddings,
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

    /// [`Self::embed_text`] with the provider's own usage figures (spec 065 research R3).
    async fn embed_text_reported(&self, text: &str) -> Result<(Vec<f64>, Reported)> {
        Ok((self.embed_text(text).await?, Reported::default()))
    }

    /// [`Self::embed_texts`] with the provider's own usage figures (spec 065 research R3).
    async fn embed_texts_reported(&self, documents: Vec<String>) -> Result<(Vec<Vec<f64>>, Reported)> {
        Ok((self.embed_texts(documents).await?, Reported::default()))
    }
}

/// An agent's embedding model. Every request it answers is recorded with the ambient
/// [`media_scope`] (spec 065 R2): the indexes reach it through storage, with no session.
pub struct VizierEmbedder {
    model: Arc<Box<dyn VizierEmbeddingModel + Sync + Send + 'static>>,
    meter: Option<MediaMeter>,
}

impl VizierEmbedder {
    fn build<Model: VizierEmbeddingModel + Sync + Send + 'static>(model: Model) -> Self {
        Self {
            model: Arc::new(Box::new(model)),
            meter: None,
        }
    }

    fn metered(mut self, meter: MediaMeter) -> Self {
        self.meter = Some(meter);
        self
    }

    fn record(&self, reported: Reported, texts: &[&str], took: std::time::Duration) {
        if let Some(meter) = &self.meter {
            meter.record(
                &media_scope(),
                UsageKind::Embedding,
                reported,
                Measured {
                    input_tokens: Some(estimate_tokens(texts)),
                    ..Default::default()
                },
                took,
            );
        }
    }

    pub async fn from_agent_settings(
        settings: &EmbeddingConfig,
        deps: &VizierDependencies,
        workspace: &str,
        agent_id: &str,
    ) -> Result<Self> {
        use crate::schema::agent::EmbeddingProvider;

        let provider = &settings.provider;
        let mut model = settings.model.clone();
        if *provider == EmbeddingProvider::Bedrock && model.trim().is_empty() {
            model = crate::llm::media::bedrock::DEFAULT_EMBEDDING_MODEL.to_string();
        }
        let meter = MediaMeter::new(
            agent_id,
            provider
                .credentials()
                .map(|id| id.to_string())
                .unwrap_or_else(|| provider_name(provider)),
            model.clone(),
            matches!(
                provider,
                EmbeddingProvider::Local | EmbeddingProvider::Ollama | EmbeddingProvider::Dummyplug
            ),
            deps.storage.clone(),
        );
        if *provider == EmbeddingProvider::Dummyplug {
            return Ok(Self::build(media::dummyplug::DummyplugEmbeddings).metered(meter));
        }
        let Some(id) = provider.credentials() else {
            let variant = crate::config::embedding::LocalEmbeddingModelVariant::from_name(&model)
                .ok_or_else(|| anyhow::anyhow!("unknown local embedding model: {model}"))?;
            return Ok(Self::build(fastembed::LocalEmbedder::new(
                &variant.to_fastembed(),
                workspace,
                &deps.setup,
                agent_id,
            )?)
            .metered(meter));
        };
        let endpoint = media::endpoint(deps, &id, model.clone()).await?;
        Ok((match provider {
            EmbeddingProvider::Ollama => {
                if !model.is_empty() {
                    crate::utils::ollama::ollama_pull_model(
                        &endpoint.base_url,
                        &model,
                        &deps.setup,
                        agent_id,
                        crate::setup::DownloadKind::Embedding,
                    )
                    .await?;
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
            EmbeddingProvider::Bedrock => Self::build(BedrockEmbeddings::new(endpoint)?),
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
        .metered(meter))
    }
}

#[async_trait::async_trait]
impl VizierEmbeddingModel for VizierEmbedder {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        let started = std::time::Instant::now();
        let (vector, reported) = self.model.embed_text_reported(text).await?;
        self.record(reported, &[text], started.elapsed());
        Ok(vector)
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        let started = std::time::Instant::now();
        let measured: Vec<String> = if self.meter.is_some() {
            documents.clone()
        } else {
            vec![]
        };
        let (vectors, reported) = self.model.embed_texts_reported(documents).await?;
        let texts: Vec<&str> = measured.iter().map(String::as_str).collect();
        self.record(reported, &texts, started.elapsed());
        Ok(vectors)
    }
}
