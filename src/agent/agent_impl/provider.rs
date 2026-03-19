use std::{env, sync::Arc};

use anyhow::Result;
use rig::{
    client::Nothing,
    providers::{deepseek, ollama, openrouter},
};

use crate::{
    agent::{
        agent_impl::{
            VizierAgentImpl,
            system_prompt::{boot::boot_md, init_workspace},
        },
        hook::{VizierAgentHook, thinking::ThinkingHook},
        tools::VizierTools,
    },
    dependencies::VizierDependencies,
    schema::VizierSession,
    utils::{self, agent_workspace},
};

#[async_trait::async_trait]
pub trait VizierAgentTrait<Client>
where
    Self: Sized,
    Client: rig::client::CompletionClient + Send + Sync,
{
    async fn init_client(session: VizierSession, deps: VizierDependencies) -> Result<Client>;

    async fn new(
        session: VizierSession,
        deps: VizierDependencies,
    ) -> Result<VizierAgentImpl<Client>> {
        let client = Self::init_client(session.clone(), deps.clone()).await?;

        let agent_config = deps.config.agents.get(&session.0).unwrap();

        let boot = boot_md(agent_config);
        let path = agent_workspace(&deps.config.workspace.clone(), &session.0);
        init_workspace(path);

        let tool = VizierTools::new(session.0.clone(), deps.clone()).await?;

        let agent = client
            .agent(agent_config.model.clone())
            .name(&agent_config.name.clone())
            .preamble(&boot)
            .tool_server_handle(tool.handle)
            .default_max_turns(agent_config.turn_depth)
            .build();

        let mut hooks: Vec<Arc<Box<dyn VizierAgentHook>>> = vec![];

        if let Some(true) = agent_config.show_thinking {
            hooks.push(Arc::new(Box::new(ThinkingHook::new(
                deps.transport.clone(),
                session.clone(),
            ))));
        }

        Ok(VizierAgentImpl::<Client> {
            id: session.0.clone(),
            agent,
            hooks,
            workspace: deps.config.workspace.clone(),
            primary_user: deps.config.primary_user.clone(),
            silent_read_initiative_chance: agent_config.silent_read_initiative_chance,
        })
    }
}

#[async_trait::async_trait]
impl VizierAgentTrait<ollama::Client> for VizierAgentImpl<ollama::Client> {
    async fn init_client(
        session: VizierSession,
        deps: VizierDependencies,
    ) -> Result<ollama::Client> {
        let agent_config = deps.config.agents.get(&session.0).unwrap();
        let base_url = deps.config.providers.ollama.clone().unwrap().base_url;

        utils::ollama::ollama_pull_model(&base_url, &agent_config.model).await?;

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
        _session: VizierSession,
        deps: VizierDependencies,
    ) -> Result<openrouter::Client> {
        let client: openrouter::Client = openrouter::Client::new(
            env::var("OPENROUTER_API_KEY")
                .unwrap_or_else(|_| deps.config.providers.openrouter.clone().unwrap().api_key),
        )?;

        Ok(client)
    }
}

#[async_trait::async_trait]
impl VizierAgentTrait<deepseek::Client> for VizierAgentImpl<deepseek::Client> {
    async fn init_client(
        _session: VizierSession,
        deps: VizierDependencies,
    ) -> Result<deepseek::Client> {
        let client: deepseek::Client = deepseek::Client::new(
            env::var("DEEPSEEK_API_KEY")
                .unwrap_or_else(|_| deps.config.providers.deepseek.clone().unwrap().api_key),
        )?;

        Ok(client)
    }
}
