use anyhow::Result;
use rig::{
    client::{CompletionClient, Nothing},
    providers::{deepseek, ollama, openrouter},
};

use crate::{
    agent::{agent::VizierAgentImpl, tools::VizierTools},
    dependencies::VizierDependencies,
    utils::{agent_workspace, init_workspace},
};

impl VizierAgentImpl<ollama::CompletionModel> {
    pub fn new(id: String, deps: VizierDependencies) -> Result<Self> {
        let agent_workspace = agent_workspace(&deps.config.workspace, &id);
        init_workspace(agent_workspace.clone());

        let agent_config = deps.config.agents.get(&id).unwrap();

        let client: ollama::Client = ollama::Client::builder()
            .base_url(deps.config.providers.ollama.clone().unwrap().base_url)
            .api_key(Nothing)
            .build()?;

        let boot = std::fs::read_to_string(std::path::PathBuf::from(format!(
            "{}/BOOT.md",
            agent_workspace
        )))?;

        let tool = VizierTools::new(id.clone(), deps.clone())?;

        let agent = client
            .agent(agent_config.model.clone())
            .name(&agent_config.name.clone())
            .preamble(&boot)
            .tool_server_handle(tool.handle)
            .default_max_turns(agent_config.turn_depth)
            .build();

        Ok(Self {
            id: id.clone(),
            agent,
            workspace: deps.config.workspace.clone(),
        })
    }
}

impl VizierAgentImpl<openrouter::CompletionModel> {
    pub fn new(id: String, deps: VizierDependencies) -> Result<Self> {
        let agent_workspace = agent_workspace(&deps.config.workspace, &id);
        init_workspace(agent_workspace.clone());

        let agent_config = deps.config.agents.get(&id).unwrap();

        let client: openrouter::Client =
            openrouter::Client::new(deps.config.providers.openrouter.clone().unwrap().api_key)?;

        let boot = std::fs::read_to_string(std::path::PathBuf::from(format!(
            "{}/BOOT.md",
            agent_workspace
        )))?;

        let tool = VizierTools::new(id.clone(), deps.clone())?;

        let agent = client
            .agent(agent_config.model.clone())
            .name(&agent_config.name)
            .preamble(&boot)
            .tool_server_handle(tool.handle)
            .default_max_turns(agent_config.turn_depth)
            .build();

        Ok(Self {
            id: id.clone(),
            agent,
            workspace: deps.config.workspace.clone(),
        })
    }
}

impl VizierAgentImpl<deepseek::CompletionModel> {
    pub fn new(id: String, deps: VizierDependencies) -> Result<Self> {
        let agent_workspace = agent_workspace(&deps.config.workspace, &id);
        init_workspace(agent_workspace.clone());

        let agent_config = deps.config.agents.get(&id).unwrap();

        let client: deepseek::Client =
            deepseek::Client::new(deps.config.providers.deepseek.clone().unwrap().api_key)?;

        let boot = std::fs::read_to_string(std::path::PathBuf::from(format!(
            "{}/BOOT.md",
            agent_workspace
        )))?;

        let tool = VizierTools::new(id.clone(), deps.clone())?;

        let agent = client
            .agent(agent_config.model.clone())
            .name(&agent_config.name)
            .preamble(&boot)
            .tool_server_handle(tool.handle)
            .default_max_turns(agent_config.turn_depth)
            .build();

        Ok(Self {
            id: id.clone(),
            agent,
            workspace: deps.config.workspace.clone(),
        })
    }
}
