use anyhow::Result;
use rig_core::{
    client::Nothing,
    providers::{anthropic, deepseek, gemini, llamafile, ollama, openai, openrouter, xiaomimimo},
};

use crate::{
    agents::agent::model::{VizierModelBuilder, VizierModelImpl},
    provider_keys::ResolvedProvider,
};

#[async_trait::async_trait]
impl VizierModelBuilder<ollama::Client> for VizierModelImpl<ollama::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<ollama::Client> {
        let base_url = resolved
            .base_url
            .clone()
            .ok_or_else(|| anyhow::anyhow!("ollama resolved provider missing base_url"))?;

        let client: ollama::Client = ollama::Client::builder()
            .base_url(base_url)
            .api_key(Nothing)
            .build()?;

        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<openrouter::Client> for VizierModelImpl<openrouter::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<openrouter::Client> {
        let client: openrouter::Client = openrouter::Client::new(resolved.api_key.clone())?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<deepseek::Client> for VizierModelImpl<deepseek::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<deepseek::Client> {
        let client: deepseek::Client = deepseek::Client::new(resolved.api_key.clone())?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<anthropic::Client> for VizierModelImpl<anthropic::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<anthropic::Client> {
        let client: anthropic::Client = anthropic::Client::new(resolved.api_key.clone())?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<openai::Client> for VizierModelImpl<openai::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<openai::Client> {
        let client: openai::Client = openai::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<gemini::Client> for VizierModelImpl<gemini::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<gemini::Client> {
        let client: gemini::Client = gemini::Client::new(resolved.api_key.clone())?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<xiaomimimo::Client> for VizierModelImpl<xiaomimimo::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<xiaomimimo::Client> {
        let client: xiaomimimo::Client = xiaomimimo::Client::new(resolved.api_key.clone())?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<llamafile::Client> for VizierModelImpl<llamafile::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<llamafile::Client> {
        let base_url = resolved
            .base_url
            .clone()
            .ok_or_else(|| anyhow::anyhow!("llama_cpp resolved provider missing base_url"))?;

        let client: llamafile::Client = llamafile::Client::builder()
            .base_url(base_url)
            .api_key(Nothing)
            .build()?;

        Ok(client)
    }
}
