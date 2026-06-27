use anyhow::Result;
use rig_core::{
    client::Nothing,
    providers::{
        anthropic, azure, chatgpt, cohere, copilot, deepseek, galadriel, gemini, groq,
        huggingface, hyperbolic, llamafile, minimax, mira, mistral, moonshot, ollama, openai,
        openrouter, perplexity, together, xai, xiaomimimo, zai,
    },
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
        // If a base_url is provided, route through the builder so the same
        // rig-core client can serve both `ProviderVariant::openai` (fixed
        // endpoint) and `ProviderVariant::custom` (arbitrary OpenAI-compatible
        // endpoint).
        //
        // `openai::Client` is the Responses API client (POSTs `/responses`).
        // For third-party OpenAI-compatible upstreams that only implement the
        // legacy `/chat/completions` endpoint (LM Studio, vLLM, llama.cpp
        // server, opencode.ai/zen, etc.), use `openai::CompletionsClient`
        // instead — see the impl below.
        let client = match resolved.base_url.as_deref().filter(|s| !s.is_empty()) {
            Some(url) => {
                let mut builder = openai::Client::builder().api_key(&resolved.api_key);
                builder = builder.base_url(url);
                builder.build()?
            }
            None => openai::Client::new(&resolved.api_key)?,
        };
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<openai::CompletionsClient> for VizierModelImpl<openai::CompletionsClient> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<openai::CompletionsClient> {
        // Chat Completions API client (POSTs `/chat/completions`). Used by
        // `ProviderVariant::custom` so the generic OpenAI-compatible provider
        // works against upstreams that only implement the legacy endpoint.
        let client = match resolved.base_url.as_deref().filter(|s| !s.is_empty()) {
            Some(url) => {
                let mut builder = openai::CompletionsClient::builder().api_key(&resolved.api_key);
                builder = builder.base_url(url);
                builder.build()?
            }
            None => openai::CompletionsClient::new(&resolved.api_key)?,
        };
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

#[async_trait::async_trait]
impl VizierModelBuilder<groq::Client> for VizierModelImpl<groq::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<groq::Client> {
        let client: groq::Client = groq::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<mistral::Client> for VizierModelImpl<mistral::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<mistral::Client> {
        let client: mistral::Client = mistral::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<xai::Client> for VizierModelImpl<xai::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<xai::Client> {
        let client: xai::Client = xai::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<perplexity::Client> for VizierModelImpl<perplexity::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<perplexity::Client> {
        let client: perplexity::Client = perplexity::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<moonshot::Client> for VizierModelImpl<moonshot::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<moonshot::Client> {
        let mut builder = moonshot::Client::builder().api_key(&resolved.api_key);
        if let Some(url) = &resolved.base_url {
            builder = builder.base_url(url);
        }
        let client = builder.build()?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<zai::Client> for VizierModelImpl<zai::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<zai::Client> {
        let mut builder = zai::Client::builder().api_key(resolved.api_key.clone());
        if let Some(url) = &resolved.base_url {
            builder = builder.base_url(url);
        }
        let client = builder.build()?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<minimax::Client> for VizierModelImpl<minimax::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<minimax::Client> {
        let mut builder = minimax::Client::builder().api_key(resolved.api_key.clone());
        if let Some(url) = &resolved.base_url {
            builder = builder.base_url(url);
        }
        let client = builder.build()?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<together::Client> for VizierModelImpl<together::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<together::Client> {
        let client: together::Client = together::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<cohere::Client> for VizierModelImpl<cohere::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<cohere::Client> {
        let client: cohere::Client = cohere::Client::new(resolved.api_key.clone())?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<huggingface::Client> for VizierModelImpl<huggingface::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<huggingface::Client> {
        let client: huggingface::Client = huggingface::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<hyperbolic::Client> for VizierModelImpl<hyperbolic::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<hyperbolic::Client> {
        let client: hyperbolic::Client = hyperbolic::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<galadriel::Client> for VizierModelImpl<galadriel::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<galadriel::Client> {
        let client: galadriel::Client = galadriel::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<mira::Client> for VizierModelImpl<mira::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<mira::Client> {
        let client: mira::Client = mira::Client::new(&resolved.api_key)?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<copilot::Client> for VizierModelImpl<copilot::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<copilot::Client> {
        let client: copilot::Client = copilot::Client::builder()
            .api_key(copilot::CopilotAuth::ApiKey(resolved.api_key.clone()))
            .build()?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<chatgpt::Client> for VizierModelImpl<chatgpt::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<chatgpt::Client> {
        let parts: Vec<&str> = resolved.api_key.splitn(2, ':').collect();
        let (access_token, account_id) = if parts.len() == 2 {
            (parts[0].to_string(), Some(parts[1].to_string()))
        } else {
            (resolved.api_key.clone(), None)
        };

        let mut builder = chatgpt::Client::builder()
            .api_key(chatgpt::ChatGPTAuth::AccessToken { access_token, account_id });
        if let Some(url) = &resolved.base_url {
            builder = builder.base_url(url);
        }
        let client = builder.build()?;
        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierModelBuilder<azure::Client> for VizierModelImpl<azure::Client> {
    async fn init_client(resolved: &ResolvedProvider) -> Result<azure::Client> {
        let endpoint = resolved
            .base_url
            .clone()
            .ok_or_else(|| anyhow::anyhow!("azure resolved provider missing endpoint"))?;
        let client: azure::Client = azure::Client::builder()
            .api_key(resolved.api_key.clone())
            .azure_endpoint(endpoint)
            .build()?;
        Ok(client)
    }
}
