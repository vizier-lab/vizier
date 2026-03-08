use std::{fs, path::PathBuf};

use anyhow::Result;
use chrono::Utc;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use rig::{
    agent::Agent,
    completion::{Chat, CompletionModel},
    message::Message,
    providers::{deepseek, ollama, openrouter},
};

use crate::{
    agent::{agent_impl::system_prompt::user::primary_user_md, memory::SessionMemories},
    config::{provider::ProviderVariant, user::UserConfig},
    dependencies::VizierDependencies,
    transport::VizierRequest,
    utils::agent_workspace,
};

mod provider;
mod system_prompt;

#[derive(Clone)]
pub enum VizierAgent {
    Ollama(VizierAgentImpl<ollama::CompletionModel>),
    OpenRouter(VizierAgentImpl<openrouter::CompletionModel>),
    Deepseek(VizierAgentImpl<deepseek::CompletionModel>),
}

impl VizierAgent {
    pub fn new(deps: &VizierDependencies, id: String) -> Result<VizierAgent> {
        let agent_config = deps.config.agents.get(&id).unwrap();
        let agent = match &agent_config.provider {
            ProviderVariant::openrouter => VizierAgent::OpenRouter(VizierAgentImpl::<
                openrouter::CompletionModel,
            >::new(
                id.clone(), deps.clone()
            )?),

            ProviderVariant::deepseek => VizierAgent::Deepseek(VizierAgentImpl::<
                deepseek::CompletionModel,
            >::new(
                id.clone(), deps.clone()
            )?),

            ProviderVariant::ollama => VizierAgent::Ollama(VizierAgentImpl::<
                ollama::CompletionModel,
            >::new(
                id.clone(), deps.clone()
            )?),
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
pub struct VizierAgentImpl<T: CompletionModel> {
    #[allow(unused)]
    id: String,
    agent: Agent<T>,

    workspace: String,
    primary_user: UserConfig,

    silent_read_initiative_chance: f32,
}

impl<T: CompletionModel> VizierAgentImpl<T> {
    pub async fn prompt(&self, req: VizierRequest) -> Result<String> {
        let agent_workspace = agent_workspace(&self.workspace, &self.id);

        let agent_md = read_md_file(agent_workspace.clone(), "AGENT.md".into());
        let ident_md = read_md_file(agent_workspace.clone(), "IDENT.md".into());

        let history = vec![
            Message::user(agent_md),
            Message::user(primary_user_md(&self.primary_user)),
            Message::user(ident_md),
        ];

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

        let agent_workspace = agent_workspace(&self.workspace, &self.id);

        let agent_md = read_md_file(agent_workspace.clone(), "AGENT.md".into());
        let ident_md = read_md_file(agent_workspace.clone(), "IDENT.md".into());

        let mut history = vec![
            Message::user(agent_md),
            Message::user(primary_user_md(&self.primary_user)),
            Message::user(ident_md),
        ];

        history.extend(memory.recall_as_messages());

        let response = self
            .agent
            .chat(format!("{}", req.to_prompt()?,), history)
            .await?;

        Ok(response)
    }
}

fn read_md_file(workspace: String, file: String) -> String {
    let path = PathBuf::from(format!("{}/{}", workspace, file));

    fs::read_to_string(path).unwrap()
}
