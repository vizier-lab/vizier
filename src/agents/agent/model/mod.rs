use std::sync::Arc;

use anyhow::Result;
use rig_core::{
    OneOrMany,
    completion::{CompletionModel, ToolDefinition, Usage},
    message::{AssistantContent, Message},
    providers::{
        anthropic, azure, chatgpt, cohere, copilot, deepseek, galadriel, gemini, groq,
        huggingface, hyperbolic, llamafile, minimax, mira, mistral, moonshot, ollama, openai,
        openrouter, perplexity, together, xai, xiaomimimo, zai,
    },
};

use crate::{
    config::provider::ProviderVariant,
    dependencies::VizierDependencies,
    provider_keys::{
        ResolvedProvider, resolve_chatgpt_provider, resolve_custom_provider, resolve_local_provider,
        resolve_provider_key, resolve_provider_with_base_url, resolve_azure_provider,
    },
    schema::{AgentConfig, AgentId},
};

mod provider;
mod registry;

/// Try to fetch context window size from provider's ModelListing API.
/// Falls back to model name detection.
async fn fetch_context_window_from_api<C>(client: &C, model_name: &str) -> Option<u64>
where
    C: rig_core::client::ModelListingClient,
{
    use rig_core::client::ModelListingClient;

    match client.list_models().await {
        Ok(models) => {
            if let Some(m) = models.iter().find(|m| m.id == model_name) {
                if let Some(ctx) = m.context_length {
                    tracing::debug!(
                        "Fetched context window for {}: {} tokens (from provider API)",
                        model_name,
                        ctx
                    );
                    return Some(ctx as u64);
                }
            }
            tracing::debug!(
                "Model '{}' not found in provider's model list, falling back to name detection",
                model_name
            );
            registry::detect_context_window(model_name)
        }
        Err(e) => {
            tracing::debug!(
                "Failed to list models from provider: {}, falling back to name detection",
                e
            );
            registry::detect_context_window(model_name)
        }
    }
}

#[derive(Clone)]
pub struct VizierModel(Arc<Box<dyn VizierModelTrait + Sync + Send + 'static>>);

impl VizierModel {
    fn build<Model: VizierModelTrait + Sync + Send + 'static>(model: Model) -> Self {
        Self(Arc::new(Box::new(model)))
    }

    pub fn context_window(&self) -> Option<u64> {
        self.0.context_window()
    }

    // NOTE: `mistral` is the rig cloud provider (Mistral AI API).
    pub async fn new(
        agent_id: AgentId,
        deps: VizierDependencies,
        agent_config: &AgentConfig,
    ) -> Result<Self> {
        let resolved = resolve_provider(&deps.storage, &agent_config.provider).await?;
        let model_name = &agent_config.model;
        let max_tokens = agent_config.max_tokens;
        // Config override takes priority over API/pattern detection
        let config_context_window = agent_config.context_window;

        Ok(match agent_config.provider {
            // Providers that support ModelListing — try API first (unless config override)
            ProviderVariant::openrouter => {
                let client = openrouter::Client::new(resolved.api_key.clone())?;
                let context_window = config_context_window.or(
                    fetch_context_window_from_api(&client, model_name).await,
                );
                Self::build(VizierModelImpl::build_with_client(
                    &client,
                    model_name,
                    max_tokens,
                    context_window,
                ))
            }
            ProviderVariant::openai => {
                let client = openai::Client::new(&resolved.api_key)?;
                let context_window = config_context_window.or(
                    fetch_context_window_from_api(&client, model_name).await,
                );
                Self::build(VizierModelImpl::build_with_client(
                    &client,
                    model_name,
                    max_tokens,
                    context_window,
                ))
            }
            ProviderVariant::anthropic => {
                let client = anthropic::Client::new(resolved.api_key.clone())?;
                let context_window = config_context_window.or(
                    fetch_context_window_from_api(&client, model_name).await,
                );
                Self::build(VizierModelImpl::build_with_client(
                    &client,
                    model_name,
                    max_tokens,
                    context_window,
                ))
            }
            ProviderVariant::gemini => {
                let client = gemini::Client::new(resolved.api_key.clone())?;
                let context_window = config_context_window.or(
                    fetch_context_window_from_api(&client, model_name).await,
                );
                Self::build(VizierModelImpl::build_with_client(
                    &client,
                    model_name,
                    max_tokens,
                    context_window,
                ))
            }
            ProviderVariant::deepseek => {
                let client = deepseek::Client::new(resolved.api_key.clone())?;
                let context_window = config_context_window.or(
                    fetch_context_window_from_api(&client, model_name).await,
                );
                Self::build(VizierModelImpl::build_with_client(
                    &client,
                    model_name,
                    max_tokens,
                    context_window,
                ))
            }
            ProviderVariant::mistral => {
                let client = mistral::Client::new(&resolved.api_key)?;
                let context_window = config_context_window.or(
                    fetch_context_window_from_api(&client, model_name).await,
                );
                Self::build(VizierModelImpl::build_with_client(
                    &client,
                    model_name,
                    max_tokens,
                    context_window,
                ))
            }
            ProviderVariant::ollama => {
                let base_url = resolved
                    .base_url
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("ollama resolved provider missing base_url"))?;
                let client = ollama::Client::builder()
                    .base_url(base_url)
                    .api_key(rig_core::client::Nothing)
                    .build()?;
                let context_window = config_context_window.or(
                    fetch_context_window_from_api(&client, model_name).await,
                );
                Self::build(VizierModelImpl::build_with_client(
                    &client,
                    model_name,
                    max_tokens,
                    context_window,
                ))
            }
            ProviderVariant::copilot => {
                let client = copilot::Client::builder()
                    .api_key(copilot::CopilotAuth::ApiKey(resolved.api_key.clone()))
                    .build()?;
                let context_window = config_context_window.or(
                    fetch_context_window_from_api(&client, model_name).await,
                );
                Self::build(VizierModelImpl::build_with_client(
                    &client,
                    model_name,
                    max_tokens,
                    context_window,
                ))
            }
            ProviderVariant::mimo => {
                let client = xiaomimimo::Client::new(resolved.api_key.clone())?;
                let context_window = config_context_window.or(
                    fetch_context_window_from_api(&client, model_name).await,
                );
                Self::build(VizierModelImpl::build_with_client(
                    &client,
                    model_name,
                    max_tokens,
                    context_window,
                ))
            }

            // Providers without ModelListing — use config override or model name detection
            ProviderVariant::groq => Self::build(
                VizierModelImpl::<groq::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::xai => Self::build(
                VizierModelImpl::<xai::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::perplexity => Self::build(
                VizierModelImpl::<perplexity::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::moonshot => Self::build(
                VizierModelImpl::<moonshot::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::zai => Self::build(
                VizierModelImpl::<zai::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::minimax => Self::build(
                VizierModelImpl::<minimax::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::together => Self::build(
                VizierModelImpl::<together::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::cohere => Self::build(
                VizierModelImpl::<cohere::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::huggingface => Self::build(
                VizierModelImpl::<huggingface::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::hyperbolic => Self::build(
                VizierModelImpl::<hyperbolic::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::voyageai => {
                anyhow::bail!("voyageai is an embedding-only provider")
            }
            ProviderVariant::galadriel => Self::build(
                VizierModelImpl::<galadriel::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::mira => Self::build(
                VizierModelImpl::<mira::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::chatgpt => Self::build(
                VizierModelImpl::<chatgpt::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::azure => Self::build(
                VizierModelImpl::<azure::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::custom => Self::build(
                VizierModelImpl::<openai::CompletionsClient>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::llama_cpp => Self::build(
                VizierModelImpl::<llamafile::Client>::build_with_client_fn(&resolved, agent_config, config_context_window).await?,
            ),
            ProviderVariant::elevenlabs => {
                anyhow::bail!("elevenlabs is a TTS-only provider")
            }
        })
    }

    pub async fn new_with_override(
        deps: &VizierDependencies,
        agent_config: &AgentConfig,
        model_override: Option<(ProviderVariant, String)>,
    ) -> Result<Self> {
        let (provider, model_name) = match model_override {
            Some((p, m)) => (p, m),
            None => (agent_config.provider.clone(), agent_config.model.clone()),
        };

        let mut override_config = agent_config.clone();
        override_config.provider = provider.clone();
        override_config.model = model_name;

        let resolved = resolve_provider(&deps.storage, &provider).await?;

        Ok(match override_config.provider {
            ProviderVariant::ollama => Self::build(
                VizierModelImpl::<ollama::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::openai => Self::build(
                VizierModelImpl::<openai::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::anthropic => Self::build(
                VizierModelImpl::<anthropic::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::openrouter => Self::build(
                VizierModelImpl::<openrouter::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::gemini => Self::build(
                VizierModelImpl::<gemini::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::deepseek => Self::build(
                VizierModelImpl::<deepseek::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::mimo => Self::build(
                VizierModelImpl::<xiaomimimo::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::llama_cpp => Self::build(
                VizierModelImpl::<llamafile::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::groq => Self::build(
                VizierModelImpl::<groq::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::mistral => Self::build(
                VizierModelImpl::<mistral::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::xai => Self::build(
                VizierModelImpl::<xai::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::perplexity => Self::build(
                VizierModelImpl::<perplexity::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::moonshot => Self::build(
                VizierModelImpl::<moonshot::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::zai => Self::build(
                VizierModelImpl::<zai::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::minimax => Self::build(
                VizierModelImpl::<minimax::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::together => Self::build(
                VizierModelImpl::<together::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::cohere => Self::build(
                VizierModelImpl::<cohere::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::huggingface => Self::build(
                VizierModelImpl::<huggingface::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::hyperbolic => Self::build(
                VizierModelImpl::<hyperbolic::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::voyageai => {
                anyhow::bail!("voyageai is an embedding-only provider")
            }
            ProviderVariant::galadriel => Self::build(
                VizierModelImpl::<galadriel::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::mira => Self::build(
                VizierModelImpl::<mira::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::copilot => Self::build(
                VizierModelImpl::<copilot::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::chatgpt => Self::build(
                VizierModelImpl::<chatgpt::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::azure => Self::build(
                VizierModelImpl::<azure::Client>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::custom => Self::build(
                VizierModelImpl::<openai::CompletionsClient>::build(&resolved, &override_config).await?,
            ),
            ProviderVariant::elevenlabs => {
                anyhow::bail!("elevenlabs is not a completion model provider")
            }
        })
    }
}

async fn resolve_provider(
    storage: &Arc<crate::storage::VizierStorage>,
    variant: &ProviderVariant,
) -> Result<ResolvedProvider> {
    match variant {
        ProviderVariant::ollama => {
            resolve_local_provider(storage, variant.clone(), "OLLAMA_BASE_URL", "http://localhost:11434").await
                .map_err(|e| anyhow::anyhow!(e.0))
        }
        ProviderVariant::llama_cpp => {
            resolve_local_provider(storage, variant.clone(), "LLAMA_CPP_BASE_URL", "http://localhost:8080").await
                .map_err(|e| anyhow::anyhow!(e.0))
        }
        ProviderVariant::openai => resolve_provider_key(storage, variant.clone(), "OPENAI_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::anthropic => resolve_provider_key(storage, variant.clone(), "ANTHROPIC_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::openrouter => resolve_provider_key(storage, variant.clone(), "OPENROUTER_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::deepseek => resolve_provider_key(storage, variant.clone(), "DEEPSEEK_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::gemini => resolve_provider_key(storage, variant.clone(), "GEMINI_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::mimo => resolve_provider_key(storage, variant.clone(), "XIAOMI_MIMO_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::groq => resolve_provider_key(storage, variant.clone(), "GROQ_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::mistral => resolve_provider_key(storage, variant.clone(), "MISTRAL_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::xai => resolve_provider_key(storage, variant.clone(), "XAI_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::perplexity => resolve_provider_key(storage, variant.clone(), "PERPLEXITY_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::moonshot => resolve_provider_with_base_url(storage, variant.clone(), "MOONSHOT_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::zai => resolve_provider_with_base_url(storage, variant.clone(), "ZAI_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::minimax => resolve_provider_with_base_url(storage, variant.clone(), "MINIMAX_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::together => resolve_provider_key(storage, variant.clone(), "TOGETHER_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::cohere => resolve_provider_key(storage, variant.clone(), "COHERE_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::huggingface => resolve_provider_key(storage, variant.clone(), "HUGGINGFACE_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::hyperbolic => resolve_provider_key(storage, variant.clone(), "HYPERBOLIC_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::voyageai => resolve_provider_key(storage, variant.clone(), "VOYAGE_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::galadriel => resolve_provider_key(storage, variant.clone(), "GALADRIEL_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::mira => resolve_provider_key(storage, variant.clone(), "MIRA_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::copilot => resolve_provider_key(storage, variant.clone(), "COPILOT_API_KEY")
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::chatgpt => resolve_chatgpt_provider(storage)
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::azure => resolve_azure_provider(storage)
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::custom => resolve_custom_provider(storage)
            .await
            .map_err(|e| anyhow::anyhow!(e.0)),
        ProviderVariant::elevenlabs => {
            anyhow::bail!("elevenlabs is not a completion model provider")
        }
    }
}

#[async_trait::async_trait]
impl VizierModelTrait for VizierModel {
    async fn completion(
        &self,
        message: Message,
        history: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)> {
        self.0.completion(message, history, tools).await
    }

    fn context_window(&self) -> Option<u64> {
        self.0.context_window()
    }
}

#[async_trait::async_trait]
pub trait VizierModelBuilder<Client>
where
    Client: rig_core::client::CompletionClient + Send + Sync,
{
    async fn init_client(resolved: &ResolvedProvider) -> Result<Client>;

    async fn build(
        resolved: &ResolvedProvider,
        agent_config: &AgentConfig,
    ) -> Result<VizierModelImpl<Client>> {
        let model = &agent_config.model;
        let client = Self::init_client(resolved).await?;
        let completion_model = client.completion_model(model);

        // Detect context window from model name
        let context_window = registry::detect_context_window(model);

        Ok(VizierModelImpl::<Client> {
            model: completion_model,
            max_tokens: agent_config.max_tokens,
            context_window,
        })
    }

    /// Build with an explicit context_window override (from agent config).
    async fn build_with_client_fn(
        resolved: &ResolvedProvider,
        agent_config: &AgentConfig,
        context_window_override: Option<u64>,
    ) -> Result<VizierModelImpl<Client>> {
        let model = &agent_config.model;
        let client = Self::init_client(resolved).await?;
        let completion_model = client.completion_model(model);

        // Config override > model name detection
        let context_window = context_window_override
            .or_else(|| registry::detect_context_window(model));

        Ok(VizierModelImpl::<Client> {
            model: completion_model,
            max_tokens: agent_config.max_tokens,
            context_window,
        })
    }
}

#[async_trait::async_trait]
pub trait VizierModelTrait {
    async fn completion(
        &self,
        message: Message,
        history: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)>;

    fn context_window(&self) -> Option<u64>;
}

pub struct VizierModelImpl<T>
where
    T: rig_core::client::CompletionClient,
{
    model: T::CompletionModel,
    max_tokens: Option<u64>,
    context_window: Option<u64>,
}

impl<T: rig_core::client::CompletionClient> VizierModelImpl<T> {
    /// Build a VizierModelImpl from a pre-constructed client with a known context window.
    pub fn build_with_client(
        client: &T,
        model_name: &str,
        max_tokens: Option<u64>,
        context_window: Option<u64>,
    ) -> Self {
        let model = client.completion_model(model_name);
        Self {
            model,
            max_tokens,
            context_window,
        }
    }
}

#[async_trait::async_trait]
impl<T: rig_core::client::CompletionClient> VizierModelTrait for VizierModelImpl<T> {
    async fn completion(
        &self,
        message: Message,
        history: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)> {
        let mut builder = self
            .model
            .completion_request(message)
            .messages(history)
            .tools(tools);

        if let Some(max) = self.max_tokens {
            builder = builder.max_tokens(max);
        }

        let request = builder.build();

        let response = self.model.completion(request).await?;

        Ok((response.message_id, response.choice, response.usage))
    }

    fn context_window(&self) -> Option<u64> {
        self.context_window
    }
}

