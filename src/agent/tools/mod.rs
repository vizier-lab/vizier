use anyhow::Result;
use rig::tool::server::{ToolServer, ToolServerHandle};

use crate::{
    agent::{
        exec::ExecCliFromWorkspace,
        session::AgentId,
        tools::{
            brave_search::{BraveSearch, NewsOnlySearch, WebOnlySearch},
            vector_memory::init_vector_memory,
            workspace::{AgentDocument, IdentDocument, WritePrimaryDocument},
        },
    },
    dependencies::VizierDependencies,
    utils::agent_workspace,
};

mod brave_search;
mod vector_memory;
mod workspace;

#[derive(Clone)]
pub struct VizierTools {
    pub handle: ToolServerHandle,
}

impl VizierTools {
    pub fn new(agent_id: AgentId, deps: VizierDependencies) -> Result<Self> {
        let agent_config = deps.config.agents.get(&agent_id).unwrap();
        let tool_config = deps.config.tools.clone();
        let workspace = deps.config.workspace.clone();
        let agent_workspace = agent_workspace(&workspace, &agent_id);

        let mut tool_server_builder = ToolServer::new()
            .tool(WritePrimaryDocument::<AgentDocument>::new(
                agent_workspace.clone(),
            ))
            .tool(WritePrimaryDocument::<IdentDocument>::new(
                agent_workspace.clone(),
            ));

        if agent_config.tools.enable_brave_search {
            if let Some(brave_search) = tool_config.brave_search {
                tool_server_builder = tool_server_builder
                    .tool(BraveSearch::<WebOnlySearch>::new(&brave_search))
                    .tool(BraveSearch::<NewsOnlySearch>::new(&brave_search));
            }
        }

        if agent_config.tools.enable_vector_memory {
            if let Some(vector_memory) = tool_config.vector_memory {
                let (read_memory, write_memory) = init_vector_memory(
                    agent_id.clone(),
                    workspace.clone(),
                    vector_memory,
                    deps.clone(),
                )?;

                tool_server_builder = tool_server_builder.tool(read_memory).tool(write_memory);
            }
        }

        if agent_config.tools.enable_cli_access {
            if tool_config.dangerously_enable_cli_access {
                let exec_cli_from_workspace = ExecCliFromWorkspace(agent_workspace.clone());
                tool_server_builder = tool_server_builder.tool(exec_cli_from_workspace);
            }
        }

        let tool_server = tool_server_builder.run();

        Ok(Self {
            handle: tool_server,
        })
    }
}
