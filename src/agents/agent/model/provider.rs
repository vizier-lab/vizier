use anyhow::Result;
use rig::{
    client::Nothing,
    providers::{anthropic, deepseek, gemini, ollama, openai, openrouter},
};

use crate::{
    agents::agent::model::{VizierModelBuilder, VizierModelImpl},
    schema::ProviderEntryConfig,
};

#[async_trait::async_trait]
impl VizierModelBuilder<ollama::Client> for VizierModelImpl<ollama::Client> {
    async fn init_client(provider_config: &ProviderEntryConfig) -> Result<ollama::Client> {
        let base_url = match provider_config {
            ProviderEntryConfig::Ollama { base_url } => base_url.clone(),
            _ => anyhow::bail!("expected Ollama provider config"),
        };

        let client: ollama::Client = ollama::Client::builder()
            .base_url(base_url)
            .api_key(Nothing)
            .build()?;

        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<openrouter::Client> for VizierModelImpl<openrouter::Client> {
    async fn init_client(provider_config: &ProviderEntryConfig) -> Result<openrouter::Client> {
        let api_key = match provider_config {
            ProviderEntryConfig::Openrouter { api_key } => api_key.clone(),
            _ => anyhow::bail!("expected Openrouter provider config"),
        };

        let client: openrouter::Client = openrouter::Client::new(api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<deepseek::Client> for VizierModelImpl<deepseek::Client> {
    async fn init_client(provider_config: &ProviderEntryConfig) -> Result<deepseek::Client> {
        let api_key = match provider_config {
            ProviderEntryConfig::Deepseek { api_key } => api_key.clone(),
            _ => anyhow::bail!("expected Deepseek provider config"),
        };

        let client: deepseek::Client = deepseek::Client::new(api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<anthropic::Client> for VizierModelImpl<anthropic::Client> {
    async fn init_client(provider_config: &ProviderEntryConfig) -> Result<anthropic::Client> {
        let api_key = match provider_config {
            ProviderEntryConfig::Anthropic { api_key } => api_key.clone(),
            _ => anyhow::bail!("expected Anthropic provider config"),
        };

        let client: anthropic::Client = anthropic::Client::new(api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<openai::Client> for VizierModelImpl<openai::Client> {
    async fn init_client(provider_config: &ProviderEntryConfig) -> Result<openai::Client> {
        let (api_key, base_url) = match provider_config {
            ProviderEntryConfig::Openai { api_key, base_url } => (api_key.clone(), base_url.clone()),
            _ => anyhow::bail!("expected Openai provider config"),
        };

        let client: openai::Client = if let Some(base_url) = base_url {
            openai::Client::builder()
                .base_url(base_url)
                .api_key(api_key)
                .build()?
        } else {
            openai::Client::new(api_key)?
        };

        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<gemini::Client> for VizierModelImpl<gemini::Client> {
    async fn init_client(provider_config: &ProviderEntryConfig) -> Result<gemini::Client> {
        let api_key = match provider_config {
            ProviderEntryConfig::Gemini { api_key } => api_key.clone(),
            _ => anyhow::bail!("expected Gemini provider config"),
        };

        let client: gemini::Client = gemini::Client::new(api_key)?;
        Ok(client)
    }
}
