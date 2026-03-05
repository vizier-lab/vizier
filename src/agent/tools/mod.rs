use anyhow::Result;
use rig::tool::server::{ToolServer, ToolServerHandle};

use crate::{
    agent::{
        exec::ExecCliFromWorkspace,
        tools::{
            brave_search::{BraveSearch, NewsOnlySearch, WebOnlySearch},
            vector_memory::init_vector_memory,
            workspace::{AgentDocument, IdentDocument, UserDocument, WritePrimaryDocument},
        },
    },
    config::ToolsConfig,
    dependencies::VizierDependencies,
};

mod brave_search;
mod vector_memory;
mod workspace;

#[derive(Clone)]
pub struct VizierTools {
    pub handle: ToolServerHandle,
    pub turn_depth: u32,
    pub workspace: String,
}

impl VizierTools {
    pub async fn new(
        workspace: String,
        config: ToolsConfig,
        deps: VizierDependencies,
    ) -> Result<Self> {
        let mut tool_server_builder = ToolServer::new()
            .tool(WritePrimaryDocument::<AgentDocument>::new(
                workspace.clone(),
            ))
            .tool(WritePrimaryDocument::<IdentDocument>::new(
                workspace.clone(),
            ))
            .tool(WritePrimaryDocument::<UserDocument>::new(workspace.clone()));

        if let Some(brave_search) = config.brave_search {
            tool_server_builder = tool_server_builder
                .tool(BraveSearch::<WebOnlySearch>::new(&brave_search))
                .tool(BraveSearch::<NewsOnlySearch>::new(&brave_search));
        }

        if let Some(vector_memory) = config.vector_memory {
            let (read_memory, write_memory) =
                init_vector_memory(workspace.clone(), vector_memory, deps).await?;

            tool_server_builder = tool_server_builder.tool(read_memory).tool(write_memory);
        }

        if config.dangerously_enable_cli_access {
            let exec_cli_from_workspace = ExecCliFromWorkspace(workspace.clone());
            tool_server_builder = tool_server_builder.tool(exec_cli_from_workspace);
        }

        let tool_server = tool_server_builder.run();

        Ok(Self {
            workspace,
            turn_depth: config.turn_depth,
            handle: tool_server,
        })
    }
}
