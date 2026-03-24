use anyhow::Result;
use rig::{
    client::Nothing,
    providers::{anthropic, deepseek, gemini, ollama, openai, openrouter},
};

use crate::{
    agent::{
        agent_impl::{
            VizierAgentImpl,
            system_prompt::{boot::boot_md, init_workspace},
        },
        tools::VizierTools,
    },
    dependencies::VizierDependencies,
    utils::agent_workspace,
};

#[async_trait::async_trait]
pub trait VizierAgentTrait<Client>
where
    Self: Sized,
    Client: rig::client::CompletionClient + Send + Sync,
{
    async fn init_client(agent_id: String, deps: VizierDependencies) -> Result<Client>;

    async fn build(agent_id: String, deps: VizierDependencies) -> Result<VizierAgentImpl<Client>> {
        let client = Self::init_client(agent_id.clone(), deps.clone()).await?;

        let agent_config = deps.config.agents.get(&agent_id).unwrap();

        let boot = boot_md();
        let path = agent_workspace(&deps.config.workspace.clone(), &agent_id);
        init_workspace(path);

        let tool = VizierTools::new(agent_id.clone(), deps.clone()).await?;

        let agent = client
            .agent(agent_config.model.clone())
            .name(&agent_config.name.clone())
            .preamble(&boot)
            .tool_server_handle(tool.handle)
            .default_max_turns(agent_config.thinking_depth)
            .build();

        Ok(VizierAgentImpl::<Client> {
            id: agent_id.clone(),
            agent,
            system_prompt: agent_config
                .system_prompt
                .clone()
                .unwrap_or("You are a helpful assistant".to_string()),
            workspace: deps.config.workspace.clone(),
            primary_user: deps.config.primary_user.clone(),
            silent_read_initiative_chance: agent_config.silent_read_initiative_chance,
            prompt_timeout: agent_config.prompt_timeout.into(),
            tool_call_timeout: agent_config.tools.timeout.into(),
        })
    }
}

#[async_trait::async_trait]
impl VizierAgentTrait<ollama::Client> for VizierAgentImpl<ollama::Client> {
    async fn init_client(_agent_id: String, deps: VizierDependencies) -> Result<ollama::Client> {
        let base_url = deps.config.providers.ollama.clone().unwrap().base_url;

        let client: ollama::Client = ollama::Client::builder()
            .base_url(base_url)
            .api_key(Nothing)
            .build()?;

        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierAgentTrait<openrouter::Client> for VizierAgentImpl<openrouter::Client> {
    async fn init_client(
        _agent_id: String,
        deps: VizierDependencies,
    ) -> Result<openrouter::Client> {
        let client: openrouter::Client =
            openrouter::Client::new(deps.config.providers.openrouter.clone().unwrap().api_key)?;

        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierAgentTrait<deepseek::Client> for VizierAgentImpl<deepseek::Client> {
    async fn init_client(_agent_id: String, deps: VizierDependencies) -> Result<deepseek::Client> {
        let client: deepseek::Client =
            deepseek::Client::new(deps.config.providers.deepseek.clone().unwrap().api_key)?;

        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierAgentTrait<anthropic::Client> for VizierAgentImpl<anthropic::Client> {
    async fn init_client(_agent_id: String, deps: VizierDependencies) -> Result<anthropic::Client> {
        let client: anthropic::Client =
            anthropic::Client::new(deps.config.providers.anthropic.clone().unwrap().api_key)?;

        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierAgentTrait<openai::Client> for VizierAgentImpl<openai::Client> {
    async fn init_client(_agent_id: String, deps: VizierDependencies) -> Result<openai::Client> {
        let client: openai::Client =
            if let Some(base_url) = deps.config.providers.openai.clone().unwrap().base_url {
                let api_key = deps.config.providers.openai.clone().unwrap().api_key;
                openai::Client::builder()
                    .base_url(base_url)
                    .api_key(api_key)
                    .build()?
            } else {
                openai::Client::new(deps.config.providers.openai.clone().unwrap().api_key)?
            };

        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierAgentTrait<gemini::Client> for VizierAgentImpl<gemini::Client> {
    async fn init_client(_agent_id: String, deps: VizierDependencies) -> Result<gemini::Client> {
        let client: gemini::Client =
            gemini::Client::new(deps.config.providers.gemini.clone().unwrap().api_key)?;

        Ok(client)
    }
}
