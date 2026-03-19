use std::{fs, path::PathBuf, sync::Arc};

use anyhow::Result;
use chrono::Utc;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use rig::{
    OneOrMany,
    agent::{Agent, HookAction, PromptHook},
    client::CompletionClient,
    completion::{Chat, Completion},
    message::{AssistantContent, Message, ToolResultContent, UserContent},
    providers::{deepseek, ollama, openrouter},
};

use crate::{
    agent::{
        agent_impl::{provider::VizierAgentTrait, system_prompt::user::primary_user_md},
        hook::{VizierAgentHook, thinking::ThinkingHook},
        memory::SessionMemories,
    },
    config::{provider::ProviderVariant, user::UserConfig},
    dependencies::VizierDependencies,
    schema::{VizierRequest, VizierSession},
    utils::agent_workspace,
};

mod provider;
mod system_prompt;

#[derive(Clone)]
pub enum VizierAgent {
    Ollama(VizierAgentImpl<ollama::Client>),
    OpenRouter(VizierAgentImpl<openrouter::Client>),
    Deepseek(VizierAgentImpl<deepseek::Client>),
}

impl VizierAgent {
    pub async fn new(deps: &VizierDependencies, session: VizierSession) -> Result<VizierAgent> {
        let agent_config = deps.config.agents.get(&session.0.clone()).unwrap();
        let agent = match &agent_config.provider {
            ProviderVariant::openrouter => VizierAgent::OpenRouter(
                VizierAgentImpl::<openrouter::Client>::new(session.clone(), deps.clone()).await?,
            ),

            ProviderVariant::deepseek => VizierAgent::Deepseek(
                VizierAgentImpl::<deepseek::Client>::new(session.clone(), deps.clone()).await?,
            ),

            ProviderVariant::ollama => VizierAgent::Ollama(
                VizierAgentImpl::<ollama::Client>::new(session.clone(), deps.clone()).await?,
            ),
        };

        Ok(agent)
    }

    pub async fn prompt(&self, req: VizierRequest) -> Result<String> {
        let response = match self {
            Self::Ollama(agent) => agent.prompt(req).await,
            Self::OpenRouter(agent) => agent.prompt(req).await,
            Self::Deepseek(agent) => agent.prompt(req).await,
        }?;

        Ok(response)
    }

    pub async fn chat(&self, req: VizierRequest, memory: &SessionMemories) -> Result<String> {
        let response = match self {
            Self::Ollama(agent) => agent.chat(req, memory).await,
            Self::OpenRouter(agent) => agent.chat(req, memory).await,
            Self::Deepseek(agent) => agent.chat(req, memory).await,
        }?;

        Ok(response)
    }

    pub async fn silent_read(&self, req: VizierRequest, memory: &SessionMemories) -> Result<()> {
        let _ = match self {
            Self::Ollama(agent) => agent.chat(req, memory).await,
            Self::OpenRouter(agent) => agent.chat(req, memory).await,
            Self::Deepseek(agent) => agent.chat(req, memory).await,
        }?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct VizierAgentImpl<Client: CompletionClient> {
    #[allow(unused)]
    id: String,
    agent: Agent<Client::CompletionModel>,

    hooks: Vec<Arc<Box<dyn VizierAgentHook>>>,

    workspace: String,
    primary_user: UserConfig,

    silent_read_initiative_chance: f32,
}

impl<Client: CompletionClient> VizierAgentImpl<Client> {
    pub async fn prepare_system_prompts(&self) -> Vec<Message> {
        let agent_workspace = agent_workspace(&self.workspace, &self.id);

        let agent_md = read_md_file(agent_workspace.clone(), "AGENT.md".into());
        let ident_md = read_md_file(agent_workspace.clone(), "IDENT.md".into());

        let res = vec![
            Message::user(agent_md),
            Message::user(primary_user_md(&self.primary_user)),
            Message::user(ident_md),
        ];

        res
    }

    pub async fn prompt(&self, req: VizierRequest) -> Result<String> {
        let history = self.prepare_system_prompts().await;

        let response = self
            .agent
            .chat(format!("{}", req.to_prompt()?,), history)
            .await?;

        Ok(response)
    }

    pub async fn chat(&self, req: VizierRequest, memory: &SessionMemories) -> Result<String> {
        let mut rng = StdRng::seed_from_u64(Utc::now().timestamp() as u64);
        let initiative_factor = rng.random_range(0_f32..=1_f32);

        if req.is_silent_read && initiative_factor > self.silent_read_initiative_chance {
            return Ok("".into());
        }

        if req.is_task {
            return self.prompt(req).await;
        }

        let mut history = self.prepare_system_prompts().await;
        history.extend(memory.recall_as_messages());

        let mut req = req;
        for hook in &self.hooks {
            req = hook.on_request(req).await?;
        }

        let output: String;
        let mut message = Message::user(format!("{}", req.to_prompt()?,));
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
                for hook in &self.hooks {
                    (function_name, args) = hook.on_tool_call(function_name, args).await?;
                }

                let mut tool_res = match self
                    .agent
                    .tool_server_handle
                    .call_tool(&function_name, &args)
                    .await
                {
                    Err(err) => err.to_string(),
                    Ok(s) => s,
                };
                for hook in &self.hooks {
                    tool_res = hook.on_tool_response(tool_res).await?;
                }

                let content = ToolResultContent::from_tool_output(tool_res);
                tool_responses.push(if let Some(call_id) = &call.call_id {
                    UserContent::tool_result_with_call_id(call.id.clone(), call_id.clone(), content)
                } else {
                    UserContent::tool_result(call.id.clone(), content)
                });
            }

            message = Message::User {
                content: OneOrMany::many(tool_responses).unwrap(),
            }
        }

        let mut response = output;
        for hook in &self.hooks {
            response = hook.on_response(response).await?;
        }

        Ok(response)
    }
}

fn read_md_file(workspace: String, file: String) -> String {
    let path = PathBuf::from(format!("{}/{}", workspace, file));

    fs::read_to_string(path).unwrap()
}
