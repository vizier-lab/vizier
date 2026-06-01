use std::sync::Arc;

use anyhow::Result;
use rig_core::{
    OneOrMany,
    completion::{CompletionModel, ToolDefinition, Usage},
    message::{AssistantContent, Message},
    providers::{anthropic, deepseek, gemini, llamafile, ollama, openai, openrouter, xiaomimimo},
};

use crate::{
    config::provider::ProviderVariant,
    dependencies::VizierDependencies,
    schema::{AgentConfig, AgentId, ProviderEntryConfig},
    storage::provider::ProviderStorage,
};

mod provider;

#[derive(Clone)]
pub struct VizierModel(Arc<Box<dyn VizierModelTrait + Sync + Send + 'static>>);

impl VizierModel {
    fn build<Model: VizierModelTrait + Sync + Send + 'static>(model: Model) -> Self {
        Self(Arc::new(Box::new(model)))
    }

    pub async fn new(
        agent_id: AgentId,
        deps: VizierDependencies,
        agent_config: &AgentConfig,
    ) -> Result<Self> {
        let provider_entry = deps
            .storage
            .get_provider(&agent_config.provider)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("provider {:?} not configured", agent_config.provider)
            })?;

        Ok(match agent_config.provider {
            ProviderVariant::ollama => Self::build(
                VizierModelImpl::<ollama::Client>::build(&provider_entry.config, agent_config)
                    .await?,
            ),
            ProviderVariant::openai => Self::build(
                VizierModelImpl::<openai::Client>::build(&provider_entry.config, agent_config)
                    .await?,
            ),
            ProviderVariant::anthropic => Self::build(
                VizierModelImpl::<anthropic::Client>::build(&provider_entry.config, agent_config)
                    .await?,
            ),
            ProviderVariant::openrouter => Self::build(
                VizierModelImpl::<openrouter::Client>::build(&provider_entry.config, agent_config)
                    .await?,
            ),
            ProviderVariant::gemini => Self::build(
                VizierModelImpl::<gemini::Client>::build(&provider_entry.config, agent_config)
                    .await?,
            ),
            ProviderVariant::deepseek => Self::build(
                VizierModelImpl::<deepseek::Client>::build(&provider_entry.config, agent_config)
                    .await?,
            ),
            ProviderVariant::mimo => Self::build(
                VizierModelImpl::<xiaomimimo::Client>::build(&provider_entry.config, agent_config)
                    .await?,
            ),
            ProviderVariant::llama_cpp => Self::build(
                VizierModelImpl::<llamafile::Client>::build(&provider_entry.config, agent_config)
                    .await?,
            ),
        })
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
}

#[async_trait::async_trait]
pub trait VizierModelBuilder<Client>
where
    Client: rig_core::client::CompletionClient + Send + Sync,
{
    async fn init_client(provider_config: &ProviderEntryConfig) -> Result<Client>;

    async fn build(
        provider_config: &ProviderEntryConfig,
        agent_config: &AgentConfig,
    ) -> Result<VizierModelImpl<Client>> {
        let model = &agent_config.model;

        let model = Self::init_client(provider_config)
            .await?
            .completion_model(model);

        Ok(VizierModelImpl::<Client> {
            model,
            max_tokens: agent_config.max_tokens,
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
}

pub struct VizierModelImpl<T>
where
    T: rig_core::client::CompletionClient,
{
    model: T::CompletionModel,
    max_tokens: Option<u64>,
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
}
