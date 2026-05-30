use std::sync::Arc;

use anyhow::Result;
use rig::{
    OneOrMany,
    completion::{CompletionModel, ToolDefinition, Usage},
    message::{AssistantContent, Message},
    providers::{anthropic, deepseek, gemini, ollama, openai, openrouter},
};

use crate::{
    config::provider::ProviderVariant, dependencies::VizierDependencies,
    schema::{AgentConfig, AgentId},
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
        Ok(match agent_config.provider {
            ProviderVariant::ollama => {
                Self::build(VizierModelImpl::<ollama::Client>::build(agent_id, deps, agent_config).await?)
            }
            ProviderVariant::openai => {
                Self::build(VizierModelImpl::<openai::Client>::build(agent_id, deps, agent_config).await?)
            }
            ProviderVariant::anthropic => {
                Self::build(VizierModelImpl::<anthropic::Client>::build(agent_id, deps, agent_config).await?)
            }
            ProviderVariant::openrouter => {
                Self::build(VizierModelImpl::<openrouter::Client>::build(agent_id, deps, agent_config).await?)
            }
            ProviderVariant::gemini => {
                Self::build(VizierModelImpl::<gemini::Client>::build(agent_id, deps, agent_config).await?)
            }
            ProviderVariant::deepseek => {
                Self::build(VizierModelImpl::<deepseek::Client>::build(agent_id, deps, agent_config).await?)
            }
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
    Client: rig::client::CompletionClient + Send + Sync,
{
    async fn init_client(agent_id: AgentId, deps: VizierDependencies) -> Result<Client>;

    async fn build(
        agent_id: AgentId,
        deps: VizierDependencies,
        agent_config: &AgentConfig,
    ) -> Result<VizierModelImpl<Client>> {
        let model = &agent_config.model;

        let model = Self::init_client(agent_id, deps.clone())
            .await?
            .completion_model(model);

        Ok(VizierModelImpl::<Client>(model))
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

pub struct VizierModelImpl<T>(T::CompletionModel)
where
    T: rig::client::CompletionClient;

#[async_trait::async_trait]
impl<T: rig::client::CompletionClient> VizierModelTrait for VizierModelImpl<T> {
    async fn completion(
        &self,
        message: Message,
        history: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)> {
        let request = self
            .0
            .completion_request(message)
            .messages(history)
            .tools(tools)
            .build();

        let response = self.0.completion(request).await?;

        Ok((response.message_id, response.choice, response.usage))
    }
}
