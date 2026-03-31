use anyhow::Result;
use std::collections::HashMap;
use tokio::task::JoinHandle;

use crate::agents::process::agent_process;
use crate::dependencies::VizierDependencies;
use crate::schema::AgentId;

pub mod agent;
pub mod hook;
pub mod process;
pub mod skill;
pub mod tools;

pub struct VizierAgents {
    deps: VizierDependencies,
    agents: HashMap<AgentId, JoinHandle<()>>,
}

impl VizierAgents {
    pub async fn new(deps: VizierDependencies) -> Result<Self> {
        let mut agents = HashMap::new();
        for (agent_id, _) in deps.config.agents.iter() {
            let agent_id = agent_id.clone();
            let deps = deps.clone();
            agents.insert(
                agent_id.clone(),
                tokio::spawn(async move {
                    let _ = agent_process(agent_id.clone(), deps.clone()).await;
                }),
            );
        }

        Ok(Self { deps, agents })
    }

    // TODO: add runtime agent management
    pub async fn run(&mut self) -> Result<()> {
        loop {}
    }
}
