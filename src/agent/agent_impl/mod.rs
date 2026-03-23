use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use chrono::Utc;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use rig::{
    OneOrMany,
    agent::Agent,
    client::CompletionClient,
    completion::Completion,
    message::{AssistantContent, Message, ToolResultContent, UserContent},
    providers::{anthropic, deepseek, gemini, ollama, openai, openrouter},
};
use tokio::time::{Instant, timeout};

use crate::{
    agent::{
        agent_impl::{provider::VizierAgentTrait, system_prompt::user::primary_user_md},
        hook::{VizierSessionHook, VizierSessionHooks},
        memory::SessionMemories,
    },
    config::{provider::ProviderVariant, user::UserConfig},
    dependencies::VizierDependencies,
    schema::{VizierRequest, VizierResponse, VizierResponseStats},
    utils::agent_workspace,
};

mod provider;
mod system_prompt;

#[derive(Clone)]
pub enum VizierAgent {
    Ollama(VizierAgentImpl<ollama::Client>),
    OpenRouter(VizierAgentImpl<openrouter::Client>),
    Deepseek(VizierAgentImpl<deepseek::Client>),
    Anthropic(VizierAgentImpl<anthropic::Client>),
    OpenAI(VizierAgentImpl<openai::Client>),
    Gemini(VizierAgentImpl<gemini::Client>),
}

impl VizierAgent {
    pub async fn new(agent_id: String, deps: &VizierDependencies) -> Result<VizierAgent> {
        let agent_config = deps.config.agents.get(&agent_id.clone()).unwrap();
        let agent = match &agent_config.provider {
            ProviderVariant::openrouter => VizierAgent::OpenRouter(
                VizierAgentImpl::<openrouter::Client>::build(agent_id.clone(), deps.clone())
                    .await?,
            ),
            ProviderVariant::deepseek => VizierAgent::Deepseek(
                VizierAgentImpl::<deepseek::Client>::build(agent_id.clone(), deps.clone()).await?,
            ),
            ProviderVariant::ollama => VizierAgent::Ollama(
                VizierAgentImpl::<ollama::Client>::build(agent_id.clone(), deps.clone()).await?,
            ),
            ProviderVariant::anthropic => VizierAgent::Anthropic(
                VizierAgentImpl::<anthropic::Client>::build(agent_id.clone(), deps.clone()).await?,
            ),
            ProviderVariant::openai => VizierAgent::OpenAI(
                VizierAgentImpl::<openai::Client>::build(agent_id.clone(), deps.clone()).await?,
            ),
            ProviderVariant::gemini => VizierAgent::Gemini(
                VizierAgentImpl::<gemini::Client>::build(agent_id.clone(), deps.clone()).await?,
            ),
        };

        Ok(agent)
    }

    pub async fn prompt(
        &self,
        req: VizierRequest,
        hooks: Arc<VizierSessionHooks>,
    ) -> Result<VizierResponse> {
        let response = match self {
            Self::Ollama(agent) => agent.prompt(req, hooks).await,
            Self::OpenRouter(agent) => agent.prompt(req, hooks).await,
            Self::Deepseek(agent) => agent.prompt(req, hooks).await,
            Self::Anthropic(agent) => agent.prompt(req, hooks).await,
            Self::OpenAI(agent) => agent.prompt(req, hooks).await,
            Self::Gemini(agent) => agent.prompt(req, hooks).await,
        }?;

        Ok(response)
    }

    pub async fn chat(
        &self,
        req: VizierRequest,
        memory: &SessionMemories,
        hooks: Arc<VizierSessionHooks>,
    ) -> Result<VizierResponse> {
        let response = match self {
            Self::Ollama(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::OpenRouter(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::Deepseek(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::Anthropic(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::OpenAI(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::Gemini(agent) => agent.chat(req, Some(memory), hooks).await,
        }?;

        Ok(response)
    }

    pub async fn silent_read(
        &self,
        req: VizierRequest,
        memory: &SessionMemories,
        hooks: Arc<VizierSessionHooks>,
    ) -> Result<()> {
        let _ = match self {
            Self::Ollama(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::OpenRouter(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::Deepseek(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::Anthropic(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::OpenAI(agent) => agent.chat(req, Some(memory), hooks).await,
            Self::Gemini(agent) => agent.chat(req, Some(memory), hooks).await,
        }?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct VizierAgentImpl<Client: CompletionClient> {
    #[allow(unused)]
    id: String,
    agent: Agent<Client::CompletionModel>,
    system_prompt: String,
    workspace: String,
    primary_user: UserConfig,
    silent_read_initiative_chance: f32,

    prompt_timeout: Duration,
    tool_call_timeout: Duration,
}

impl<Client: CompletionClient> VizierAgentImpl<Client> {
    pub async fn prepare_system_prompts(&self) -> Vec<Message> {
        let agent_workspace = agent_workspace(&self.workspace, &self.id);

        let agent_md = read_md_file(agent_workspace.clone(), "AGENT.md".into());
        let ident_md = read_md_file(agent_workspace.clone(), "IDENTITY.md".into());

        let res = vec![
            Message::system(self.system_prompt.clone()),
            Message::system(primary_user_md(&self.primary_user)),
            Message::system(agent_md),
            Message::system(ident_md),
        ];

        res
    }

    pub async fn prompt(
        &self,
        req: VizierRequest,
        hooks: Arc<VizierSessionHooks>,
    ) -> Result<VizierResponse> {
        let response = self.chat(req, None, hooks).await?;

        Ok(response)
    }

    pub async fn chat(
        &self,
        req: VizierRequest,
        memory: Option<&SessionMemories>,
        hooks: Arc<VizierSessionHooks>,
    ) -> Result<VizierResponse> {
        timeout(self.prompt_timeout, async {
            let mut rng = StdRng::seed_from_u64(Utc::now().timestamp() as u64);
            let initiative_factor = rng.random_range(0_f32..=1_f32);

            let mut history = self.prepare_system_prompts().await;
            if let Some(memory) = memory {
                history.extend(memory.recall_as_messages());
            }

            let mut req = req;
            req = hooks.on_request(req).await?;

            if req.is_silent_read && initiative_factor > self.silent_read_initiative_chance {
                return Ok(VizierResponse::Empty);
            }

            let output: String;
            let mut message = Message::user(format!("{}", req.to_prompt()?,));

            let start = Instant::now();

            loop {
                let response = self
                    .agent
                    .completion(message.clone(), history.clone())
                    .await?
                    .send()
                    .await?;

                history.push(message);

                history.push(Message::Assistant {
                    id: response.message_id.clone(),
                    content: response.choice.clone(),
                });

                let (tool_calls, others): (Vec<_>, Vec<_>) = response
                    .choice
                    .iter()
                    .partition(|choice| matches!(choice, AssistantContent::ToolCall(_)));

                if tool_calls.is_empty() {
                    output = others
                        .iter()
                        .filter_map(|item| {
                            if let AssistantContent::Text(text) = item {
                                Some(text.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    break;
                }

                let mut tool_responses = vec![];
                for call in tool_calls.iter().filter_map(|item| {
                    if let AssistantContent::ToolCall(call) = item {
                        Some(call)
                    } else {
                        None
                    }
                }) {
                    let (mut function_name, mut args) = (
                        call.function.name.to_string(),
                        serde_json::to_string(&call.function.arguments).unwrap(),
                    );
                    (function_name, args) = hooks.on_tool_call(function_name, args).await?;

                    let mut tool_res = match timeout(
                        self.tool_call_timeout,
                        self.agent
                            .tool_server_handle
                            .call_tool(&function_name, &args),
                    )
                    .await?
                    {
                        Err(err) => err.to_string(),
                        Ok(s) => s,
                    };
                    tool_res = hooks.on_tool_response(tool_res).await?;

                    let content = ToolResultContent::from_tool_output(tool_res);
                    tool_responses.push(if let Some(call_id) = &call.call_id {
                        UserContent::tool_result_with_call_id(
                            call.id.clone(),
                            call_id.clone(),
                            content,
                        )
                    } else {
                        UserContent::tool_result(call.id.clone(), content)
                    });
                }

                message = Message::User {
                    content: OneOrMany::many(tool_responses).unwrap(),
                }
            }

            let mut response = VizierResponse::Message {
                content: output.clone(),
                stats: Some(VizierResponseStats {
                    duration: start.elapsed(),
                }),
            };
            response = hooks.on_response(response).await?;

            Ok(response)
        })
        .await?
    }
}

fn read_md_file(workspace: String, file: String) -> String {
    let path = PathBuf::from(format!("{}/{}", workspace, file));

    fs::read_to_string(path).unwrap()
}
