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
    schema::{AgentConfig, AgentId, ProviderEntryConfig, Quantization},
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
        if agent_config.provider == ProviderVariant::mistralrs {
            return Ok(Self::build(
                MistralRsModel::new(agent_config, &deps.config.workspace).await?,
            ));
        }

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
            ProviderVariant::mistralrs => unreachable!(),
            ProviderVariant::elevenlabs => {
                anyhow::bail!("elevenlabs is not a completion model provider")
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

        // mistralrs doesn't need a provider entry
        if provider == ProviderVariant::mistralrs {
            return Ok(Self::build(
                MistralRsModel::new(&override_config, &deps.config.workspace).await?,
            ));
        }

        let provider_entry = deps
            .storage
            .get_provider(&provider)
            .await?
            .ok_or_else(|| anyhow::anyhow!("provider {:?} not configured", provider))?;

        Ok(match override_config.provider {
            ProviderVariant::ollama => Self::build(
                VizierModelImpl::<ollama::Client>::build(&provider_entry.config, &override_config)
                    .await?,
            ),
            ProviderVariant::openai => Self::build(
                VizierModelImpl::<openai::Client>::build(&provider_entry.config, &override_config)
                    .await?,
            ),
            ProviderVariant::anthropic => Self::build(
                VizierModelImpl::<anthropic::Client>::build(
                    &provider_entry.config,
                    &override_config,
                )
                .await?,
            ),
            ProviderVariant::openrouter => Self::build(
                VizierModelImpl::<openrouter::Client>::build(
                    &provider_entry.config,
                    &override_config,
                )
                .await?,
            ),
            ProviderVariant::gemini => Self::build(
                VizierModelImpl::<gemini::Client>::build(&provider_entry.config, &override_config)
                    .await?,
            ),
            ProviderVariant::deepseek => Self::build(
                VizierModelImpl::<deepseek::Client>::build(
                    &provider_entry.config,
                    &override_config,
                )
                .await?,
            ),
            ProviderVariant::mimo => Self::build(
                VizierModelImpl::<xiaomimimo::Client>::build(
                    &provider_entry.config,
                    &override_config,
                )
                .await?,
            ),
            ProviderVariant::llama_cpp => Self::build(
                VizierModelImpl::<llamafile::Client>::build(
                    &provider_entry.config,
                    &override_config,
                )
                .await?,
            ),
            ProviderVariant::mistralrs => unreachable!(),
            ProviderVariant::elevenlabs => {
                anyhow::bail!("elevenlabs is not a completion model provider")
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

pub struct MistralRsModel {
    model: mistralrs::Model,
    max_tokens: Option<u64>,
}

impl MistralRsModel {
    pub async fn new(agent_config: &AgentConfig, workspace: &str) -> Result<Self> {
        use mistralrs::{IsqBits, IsqType, ModelBuilder};

        let quantization = agent_config.quantization.as_ref();
        let cache_dir =
            crate::utils::mistralrs::mistralrs_model_dir(workspace, &agent_config.model);

        let mut builder = ModelBuilder::new(&agent_config.model).from_hf_cache_path(cache_dir);

        builder = match quantization {
            Some(Quantization::Auto4) => builder.with_auto_isq(IsqBits::Four),
            Some(Quantization::Auto8) => builder.with_auto_isq(IsqBits::Eight),
            Some(Quantization::Q4_0) => builder.with_isq(IsqType::Q4_0),
            Some(Quantization::Q4_1) => builder.with_isq(IsqType::Q4_1),
            Some(Quantization::Q4K) => builder.with_isq(IsqType::Q4K),
            Some(Quantization::Q5_0) => builder.with_isq(IsqType::Q5_0),
            Some(Quantization::Q5_1) => builder.with_isq(IsqType::Q5_1),
            Some(Quantization::Q5K) => builder.with_isq(IsqType::Q5K),
            Some(Quantization::Q6K) => builder.with_isq(IsqType::Q6K),
            Some(Quantization::Q8_0) => builder.with_isq(IsqType::Q8_0),
            Some(Quantization::Q8_1) => builder.with_isq(IsqType::Q8_1),
            Some(Quantization::Hqq4) => builder.with_isq(IsqType::HQQ4),
            Some(Quantization::Hqq8) => builder.with_isq(IsqType::HQQ8),
            Some(Quantization::Fp8) => builder.with_isq(IsqType::F8E4M3),
            None => builder,
        };

        let model = builder.build().await?;

        Ok(Self {
            model,
            max_tokens: agent_config.max_tokens,
        })
    }
}

#[async_trait::async_trait]
impl VizierModelTrait for MistralRsModel {
    async fn completion(
        &self,
        message: Message,
        history: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)> {
        use mistralrs::{Function, RequestBuilder, TextMessageRole, Tool, ToolChoice, ToolType};

        let mut request = RequestBuilder::new();

        for msg in &history {
            match msg {
                Message::System { content } => {
                    request = request.add_message(TextMessageRole::System, content.clone());
                }
                Message::User { content } => {
                    let text = content
                        .iter()
                        .filter_map(|c| match c {
                            rig_core::message::UserContent::Text(t) => Some(t.text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    request = request.add_message(TextMessageRole::User, text);
                }
                Message::Assistant { content, .. } => {
                    let text = content
                        .iter()
                        .filter_map(|c| match c {
                            AssistantContent::Text(t) => Some(t.text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    request = request.add_message(TextMessageRole::Assistant, text);
                }
            }
        }

        match &message {
            Message::System { content } => {
                request = request.add_message(TextMessageRole::System, content.clone());
            }
            Message::User { content } => {
                let text = content
                    .iter()
                    .filter_map(|c| match c {
                        rig_core::message::UserContent::Text(t) => Some(t.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                request = request.add_message(TextMessageRole::User, text);
            }
            Message::Assistant { content, .. } => {
                let text = content
                    .iter()
                    .filter_map(|c| match c {
                        AssistantContent::Text(t) => Some(t.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                request = request.add_message(TextMessageRole::Assistant, text);
            }
        }

        if !tools.is_empty() {
            let mistral_tools: Vec<Tool> = tools
                .iter()
                .map(|t| {
                    let params: Option<std::collections::HashMap<String, serde_json::Value>> =
                        serde_json::from_value(t.parameters.clone()).ok();
                    Tool {
                        tp: ToolType::Function,
                        function: Function {
                            description: Some(t.description.clone()),
                            name: t.name.clone(),
                            parameters: params,
                        },
                    }
                })
                .collect();
            request = request
                .set_tools(mistral_tools)
                .set_tool_choice(ToolChoice::Auto);
        }

        if let Some(max) = self.max_tokens {
            use mistralrs::SamplingParams;
            let params = SamplingParams {
                max_len: Some(max as usize),
                ..SamplingParams::deterministic()
            };
            request = request.set_sampling(params);
        }

        let response = self.model.send_chat_request(request).await?;

        let choice = &response.choices[0];
        let mut contents = Vec::new();

        if let Some(text) = &choice.message.content {
            if !text.is_empty() {
                contents.push(AssistantContent::Text(rig_core::message::Text {
                    text: text.clone(),
                    additional_params: None,
                }));
            }
        }

        if let Some(tool_calls) = &choice.message.tool_calls {
            for tc in tool_calls {
                contents.push(AssistantContent::ToolCall(rig_core::message::ToolCall {
                    id: tc.id.clone(),
                    call_id: None,
                    function: rig_core::message::ToolFunction {
                        name: tc.function.name.clone(),
                        arguments: serde_json::from_str(&tc.function.arguments).unwrap_or_default(),
                    },
                    signature: None,
                    additional_params: None,
                }));
            }
        }

        let usage = Usage {
            input_tokens: response.usage.prompt_tokens as u64,
            output_tokens: response.usage.completion_tokens as u64,
            total_tokens: response.usage.total_tokens as u64,
            cached_input_tokens: 0,
            cache_creation_input_tokens: 0,
            reasoning_tokens: 0,
            tool_use_prompt_tokens: 0,
        };

        let one_or_many = if contents.is_empty() {
            OneOrMany::one(AssistantContent::Text(rig_core::message::Text {
                text: String::new(),
                additional_params: None,
            }))
        } else if contents.len() == 1 {
            OneOrMany::one(contents.into_iter().next().unwrap())
        } else {
            OneOrMany::many(contents)?
        };

        Ok((None, one_or_many, usage))
    }
}
