use anyhow::Result;
use rig::tool::server::{ToolServer, ToolServerHandle};

use crate::{
    agent::{
        exec::ExecCliFromWorkspace,
        tools::{
            brave_search::{BraveSearch, NewsOnlySearch, WebOnlySearch},
            discord::new_discord_tools,
            python::PythonInterpreter,
            scheduler::{ScheduleCronTask, ScheduleOneTimeTask},
            vector_memory::init_vector_memory,
            workspace::{AgentDocument, IdentDocument, WritePrimaryDocument},
        },
    },
    dependencies::VizierDependencies,
    schema::AgentId,
    utils::agent_workspace,
};

mod brave_search;
mod discord;
mod python;
mod scheduler;
mod vector_memory;
mod workspace;

#[derive(Clone)]
pub struct VizierTools {
    pub handle: ToolServerHandle,
}

impl VizierTools {
    pub async fn new(agent_id: AgentId, deps: VizierDependencies) -> Result<Self> {
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
            ))
            .tool(ScheduleOneTimeTask {
                agent_id: agent_id.clone(),
                db: deps.database.clone(),
            })
            .tool(ScheduleCronTask {
                agent_id: agent_id.clone(),
                db: deps.database.clone(),
            });

        if agent_config.tools.cli_access {
            if tool_config.dangerously_enable_cli_access {
                let exec_cli_from_workspace = ExecCliFromWorkspace(agent_workspace.clone());
                tool_server_builder = tool_server_builder.tool(exec_cli_from_workspace);
            }
        }

        if agent_config.tools.python_interpreter {
            let mut python_interpreter =
                PythonInterpreter::new(format!("{agent_workspace}/workdir"));

            if agent_config.tools.discord.is_programatically_enabled() {
                if let Some(discord) = &deps.config.channels.discord {
                    if let Some(discord) =
                        discord.iter().find(|discord| discord.agent_id == agent_id)
                    {
                        let (send_message, react_message, get_message) =
                            new_discord_tools(discord.token.clone());
                        python_interpreter = python_interpreter
                            .tool(send_message)
                            .tool(react_message)
                            .tool(get_message);
                    }
                }
            }

            if agent_config.tools.brave_search.is_programatically_enabled() {
                if let Some(brave_search) = tool_config.brave_search.clone() {
                    python_interpreter = python_interpreter
                        .tool(BraveSearch::<WebOnlySearch>::new(&brave_search))
                        .tool(BraveSearch::<NewsOnlySearch>::new(&brave_search));
                }
            }

            if agent_config.tools.vector_memory.enabled
                && !agent_config.tools.vector_memory.programmatic_tool_call
            {
                if let Some(vector_memory) = tool_config.vector_memory.clone() {
                    let (read_memory, write_memory) = init_vector_memory(
                        agent_id.clone(),
                        workspace.clone(),
                        vector_memory,
                        deps.clone(),
                    )?;

                    tool_server_builder = tool_server_builder.tool(read_memory).tool(write_memory);
                }
            }

            let python_tool_docs = python_interpreter.generate_docs_tool().await;
            tool_server_builder = tool_server_builder.tool(python_interpreter);
            tool_server_builder = tool_server_builder.tool(python_tool_docs);
        }

        if agent_config.tools.discord.enabled && !agent_config.tools.discord.programmatic_tool_call
        {
            if let Some(discord) = &deps.config.channels.discord {
                if let Some(discord) = discord.iter().find(|discord| discord.agent_id == agent_id) {
                    let (send_message, react_message, get_message) =
                        new_discord_tools(discord.token.clone());
                    tool_server_builder = tool_server_builder
                        .tool(send_message)
                        .tool(react_message)
                        .tool(get_message);
                }
            }
        }

        if agent_config.tools.brave_search.enabled
            && !agent_config.tools.brave_search.programmatic_tool_call
        {
            if let Some(brave_search) = tool_config.brave_search {
                tool_server_builder = tool_server_builder
                    .tool(BraveSearch::<WebOnlySearch>::new(&brave_search))
                    .tool(BraveSearch::<NewsOnlySearch>::new(&brave_search));
            }
        }

        if agent_config.tools.vector_memory.enabled
            && !agent_config.tools.vector_memory.programmatic_tool_call
        {
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

        let tool_server = tool_server_builder.run();

        Ok(Self {
            handle: tool_server,
        })
    }
}

