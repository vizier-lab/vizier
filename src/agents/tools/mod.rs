use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use rig::{
    completion::ToolDefinition,
    tool::server::{ToolServer, ToolServerHandle},
    tools::ThinkTool,
};

use crate::{
    agents::tools::{
        brave_search::{BraveSearch, NewsOnlySearch, WebOnlySearch},
        consult::{ConsultAgent, DelegateAgent},
        discord::new_discord_tools,
        document::init_document_tools,
        notify::{
            DiscordDmPrimaryUser, NotifyPrimaryUser, TelegramDmPrimaryUser, WebUiNotifyPrimaryUser,
        },
        scheduler::{ScheduleCronTask, ScheduleOneTimeTask},
        shell::ShellExec,
        skill::CreateSkill,
        subtasks::SubtasksTool,
        telegram::new_telegram_tools,
        vector_memory::init_vector_memory,
        workspace::{
            AgentDocument, HeartbeatDocument, IdentDocument, ReadPrimaryDocument,
            WritePrimaryDocument,
        },
    },
    dependencies::VizierDependencies,
    error::VizierError,
    mcp::{VizierMcp, VizierMcpClient},
    schema::AgentId,
    utils::agent_workspace,
};

#[cfg(feature = "python")]
mod python;

#[cfg(feature = "python")]
use crate::agents::tools::python::PythonInterpreter;

mod brave_search;
mod consult;
mod discord;
mod document;
mod notify;
mod scheduler;
mod shell;
mod skill;
mod subtasks;
mod telegram;
mod vector_memory;
mod workspace;

#[derive(Clone)]
pub struct VizierTools {
    pub handle: ToolServerHandle,
    pub mcp: HashMap<String, Arc<VizierMcp>>,
}

impl VizierTools {
    pub async fn tools(&self) -> Result<Vec<ToolDefinition>> {
        let mut res = vec![];

        for tool in self.handle.get_tool_defs(None).await? {
            res.push(tool);
        }

        for (key, mcp) in &self.mcp {
            res.extend(mcp.tools().await?.iter().map(|tool| ToolDefinition {
                name: format!("mcp_{}__{}", key.clone(), tool.name.clone()),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            }));
        }

        Ok(res)
    }

    pub async fn call(&self, function_name: String, params: String) -> Result<String> {
        // mcp calls
        if function_name.starts_with("mcp_") {
            let (server, function_name) = function_name.split_once("__").unwrap();
            let server = server.replace("mcp_", "");

            let res = self
                .mcp
                .get(&server)
                .ok_or(VizierError("mcp not found".into()))?
                .call(function_name.to_string(), serde_json::from_str(&params)?)
                .await?;

            return Ok(serde_json::to_string(&res)?);
        }

        let res = self.handle.call_tool(&function_name, &params).await?;
        Ok(res)
    }
}

impl VizierTools {
    pub async fn new(agent_id: AgentId, deps: VizierDependencies) -> Result<Self> {
        let agent_config = deps.config.agents.get(&agent_id).unwrap();
        let tool_config = deps.config.tools.clone();
        let workspace = deps.config.workspace.clone();
        let agent_workspace_path = agent_workspace(&workspace, &agent_id);
        let agent_workspace = agent_workspace_path.to_string_lossy().to_string();

        let mut tool_server_builder = ToolServer::new()
            .tool(ThinkTool)
            .tool(WritePrimaryDocument::<AgentDocument>::new(
                agent_workspace.clone(),
            ))
            .tool(WritePrimaryDocument::<IdentDocument>::new(
                agent_workspace.clone(),
            ))
            .tool(WritePrimaryDocument::<HeartbeatDocument>::new(
                agent_workspace.clone(),
            ))
            .tool(ReadPrimaryDocument::<HeartbeatDocument>::new(
                agent_workspace.clone(),
            ))
            .tool(ScheduleOneTimeTask {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            })
            .tool(ScheduleCronTask {
                agent_id: agent_id.clone(),
                db: deps.storage.clone(),
            })
            .tool(ConsultAgent::new(agent_id.clone(), deps.clone()))
            .tool(DelegateAgent::new(agent_id.clone(), deps.clone()))
            .tool(SubtasksTool::new(agent_id.clone(), deps.clone()))
            .tool(CreateSkill::new(agent_id.clone(), deps.clone()));

        if agent_config.documents.len() > 0 {
            tool_server_builder =
                tool_server_builder.tool(init_document_tools(agent_id.clone(), deps.clone())?);
        }

        if agent_config.tools.shell_access {
            tool_server_builder = tool_server_builder.tool(ShellExec(deps.shell.clone()));
        }

        #[cfg(feature = "python")]
        if agent_config.tools.python_interpreter {
            let mut python_interpreter =
                PythonInterpreter::new(format!("{agent_workspace}/workdir"));

            if agent_config.tools.discord.is_programatically_enabled() {
                if let Some(discord) = &deps.config.channels.discord {
                    if let Some((_, discord)) = discord.iter().find(|(id, _)| **id == agent_id) {
                        let token = discord.token.clone();

                        let (send_message, react_message, get_message) =
                            new_discord_tools(token.clone());
                        python_interpreter = python_interpreter
                            .tool(send_message)
                            .tool(react_message)
                            .tool(get_message);
                    }
                }
            }

            if agent_config.tools.telegram.is_programatically_enabled() {
                if let Some(telegram) = &deps.config.channels.telegram {
                    if let Some((_, telegram)) = telegram.iter().find(|(id, _)| **id == agent_id) {
                        let bot_token = telegram.token.clone();

                        let (send_message, react_message, get_message) =
                            new_telegram_tools(bot_token.clone());
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
                if let Some(_) = deps.config.embedding {
                    let (read_memory, write_memory) =
                        init_vector_memory(agent_id.clone(), deps.clone())?;

                    python_interpreter = python_interpreter.tool(read_memory).tool(write_memory);
                }
            }

            if agent_config
                .tools
                .notify_primary_user
                .is_programatically_enabled()
            {
                python_interpreter = python_interpreter
                    .tool(DiscordDmPrimaryUser::new(deps.config.clone()))
                    .tool(TelegramDmPrimaryUser::new(deps.config.clone()))
                    .tool(WebUiNotifyPrimaryUser::new(
                        agent_id.clone(),
                        deps.transport.clone(),
                    ))
                    .tool(NotifyPrimaryUser::new(
                        deps.config.clone(),
                        agent_id.clone(),
                        deps.transport.clone(),
                    ));
            }

            let python_tool_docs = python_interpreter.generate_docs_tool().await;
            tool_server_builder = tool_server_builder.tool(python_interpreter);
            tool_server_builder = tool_server_builder.tool(python_tool_docs);
        }

        if agent_config.tools.discord.enabled && !agent_config.tools.discord.programmatic_tool_call
        {
            if let Some(discord) = &deps.config.channels.discord {
                if let Some((_, discord)) = discord.iter().find(|(id, _)| **id == agent_id) {
                    let token = discord.token.clone();

                    let (send_message, react_message, get_message) = new_discord_tools(token);
                    tool_server_builder = tool_server_builder
                        .tool(send_message)
                        .tool(react_message)
                        .tool(get_message);
                }
            }
        }

        if agent_config.tools.telegram.enabled
            && !agent_config.tools.telegram.programmatic_tool_call
        {
            if let Some(telegram) = &deps.config.channels.telegram {
                if let Some((_, telegram)) = telegram.iter().find(|(id, _)| **id == agent_id) {
                    let bot_token = telegram.token.clone();

                    let (send_message, react_message, get_message) = new_telegram_tools(bot_token);
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
            if let Some(_) = deps.config.embedding {
                let (read_memory, write_memory) =
                    init_vector_memory(agent_id.clone(), deps.clone())?;

                tool_server_builder = tool_server_builder.tool(read_memory).tool(write_memory);
            }
        }

        tool_server_builder = tool_server_builder
            .tool(DiscordDmPrimaryUser::new(deps.config.clone()))
            .tool(TelegramDmPrimaryUser::new(deps.config.clone()))
            .tool(WebUiNotifyPrimaryUser::new(
                agent_id.clone(),
                deps.transport.clone(),
            ))
            .tool(NotifyPrimaryUser::new(
                deps.config.clone(),
                agent_id.clone(),
                deps.transport.clone(),
            ));

        let tool_server = tool_server_builder.run();

        let mut mcp = HashMap::new();
        for m in &agent_config.tools.mcp_servers {
            if let Some(client) = deps.mcp_clients.clients.get(m) {
                mcp.insert(m.clone(), client.clone());
            }
        }

        Ok(Self {
            handle: tool_server,
            mcp,
        })
    }
}
