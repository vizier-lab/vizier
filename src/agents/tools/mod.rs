use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::Utc;
use rig::completion::ToolDefinition;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{
        brave_search::{BraveSearch, NewsOnlySearch, WebOnlySearch},
        consult::{ConsultAgent, DelegateAgent},
        discord::new_discord_tools,
        fetch::FetchWebpage,
        http_client::HttpClient,
        notify::{
            DiscordDmPrimaryUser, NotifyPrimaryUser, TelegramDmPrimaryUser, WebUiNotifyPrimaryUser,
        },
        ptc::ProgramaticSandbox,
        scheduler::{DeleteTask, GetTaskDetail, ListTask, ScheduleCronTask, ScheduleOneTimeTask},
        shared_document::init_shared_document_tools,
        shell::ShellExec,
        skill::CreateSkill,
        subtasks::SubtasksTool,
        telegram::new_telegram_tools,
        think::ThinkTool,
        vector_memory::init_vector_memory,
        workspace::{
            AgentDocument, HeartbeatDocument, IdentDocument, ReadPrimaryDocument,
            WritePrimaryDocument,
        },
    },
    dependencies::VizierDependencies,
    error::VizierError,
    mcp::{VizierMcp, VizierMcpClient},
    schema::{AgentId, VizierResponse},
    utils::agent_workspace,
};

mod brave_search;
mod consult;
mod discord;
mod fetch;
mod http_client;
mod notify;
mod ptc;
mod scheduler;
mod shared_document;
mod shell;
mod skill;
mod subtasks;
mod telegram;
mod think;
mod vector_memory;
mod workspace;

type VizierToolDef = Arc<Box<dyn VizierToolDyn + Send + Sync + 'static>>;

#[derive(Clone)]
pub struct VizierToolSet {
    pub tools: HashMap<String, Arc<Box<dyn VizierToolDyn + Send + Sync + 'static>>>,
}

impl VizierToolSet {
    fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    fn tool<Tool: VizierToolDyn + Sync + Send + 'static>(mut self, tool: Tool) -> Self {
        self.tools
            .insert(tool.tool_name(), Arc::new(Box::new(tool)));

        self
    }

    fn get_tool(&self, function_name: String) -> Result<VizierToolDef> {
        let tool = self
            .tools
            .get(&function_name)
            .ok_or(VizierError(format!("{function_name} does not exists")))?;

        Ok(tool.clone())
    }

    async fn call(
        &self,
        function_name: String,
        args: String,
    ) -> Result<serde_json::Value, VizierError> {
        let tool = self
            .tools
            .get(&function_name)
            .ok_or(VizierError(format!("{function_name} does not exists")))?;

        let output = tool.tool_call(args).await?;
        Ok(serde_json::from_str(&output).map_err(|err| VizierError(err.to_string()))?)
    }
}

#[derive(Clone)]
pub struct VizierTools {
    pub default_toolset: VizierToolSet,
    pub user_toolset: VizierToolSet,
    pub mcp: HashMap<String, Arc<VizierMcp>>,
}

#[async_trait::async_trait]
pub trait VizierToolDyn {
    fn tool_name(&self) -> String;

    fn tool_def(&self) -> ToolDefinition;

    fn description(&self) -> String;

    fn input_schema(&self) -> serde_json::Value;

    fn output_schema(&self) -> serde_json::Value;

    async fn tool_call(&self, args: String) -> Result<String, VizierError>;
}

#[async_trait::async_trait]
impl<Tool: VizierTool + Sync + Send> VizierToolDyn for Tool {
    fn tool_name(&self) -> String {
        Self::name()
    }

    fn tool_def(&self) -> ToolDefinition {
        ToolDefinition {
            name: Self::name(),
            description: self.description(),
            parameters: Self::input_schema(),
        }
    }

    fn description(&self) -> String {
        <Self as VizierTool>::description(self)
    }

    fn input_schema(&self) -> serde_json::Value {
        Self::input_schema()
    }

    fn output_schema(&self) -> serde_json::Value {
        Self::output_schema()
    }

    async fn tool_call(&self, args: String) -> Result<String, VizierError> {
        let input = serde_json::from_str(&args).map_err(|err| VizierError(err.to_string()))?;
        let output = self.call(input).await?;

        serde_json::to_string(&output).map_err(|err| VizierError(err.to_string()))
    }
}

#[async_trait::async_trait]
pub trait VizierTool {
    type Input: JsonSchema + for<'a> Deserialize<'a> + Serialize;
    type Output: JsonSchema + for<'a> Deserialize<'a> + Serialize;

    fn name() -> String;

    fn input_schema() -> serde_json::Value {
        serde_json::to_value(schema_for!(<Self as VizierTool>::Input)).unwrap()
    }

    fn output_schema() -> serde_json::Value {
        serde_json::to_value(schema_for!(<Self as VizierTool>::Output)).unwrap()
    }

    fn description(&self) -> String;

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError>;
}

impl VizierTools {
    pub async fn tools(&self) -> Result<Vec<ToolDefinition>> {
        let mut res = vec![];

        for (_, tool) in self.default_toolset.tools.iter() {
            res.push(tool.tool_def());
        }

        for (_, tool) in self.user_toolset.tools.iter() {
            res.push(tool.tool_def());
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

    pub async fn call(&self, function_name: String, params: String) -> Result<VizierResponse> {
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

            return Ok(VizierResponse {
                timestamp: Utc::now(),
                content: crate::schema::VizierResponseContent::ToolResponse { response: res },
            });
        }

        if let Ok(tool) = self.default_toolset.get_tool(function_name.clone()) {
            let output = tool.tool_call(params.clone()).await?;
            let res = serde_json::from_str::<serde_json::Value>(&output)?;

            if let Ok(vizier_response) = serde_json::from_value(res.clone()) {
                return Ok(vizier_response);
            }

            return Ok(VizierResponse {
                timestamp: Utc::now(),
                content: crate::schema::VizierResponseContent::ToolResponse { response: res },
            });
        }

        if let Ok(tool) = self.user_toolset.get_tool(function_name.clone()) {
            let output = tool.tool_call(params.clone()).await?;
            let res = serde_json::from_str::<serde_json::Value>(&output)?;

            if let Ok(vizier_response) = serde_json::from_value(res.clone()) {
                return Ok(vizier_response);
            }

            return Ok(VizierResponse {
                timestamp: Utc::now(),
                content: crate::schema::VizierResponseContent::ToolResponse { response: res },
            });
        }

        Err(VizierError(format!("{} not found", function_name)).into())
    }
}

impl VizierTools {
    pub async fn new(agent_id: AgentId, deps: VizierDependencies) -> Result<Self> {
        let agent_config = deps.config.agents.get(&agent_id).unwrap();
        let tool_config = deps.config.tools.clone();
        let workspace = deps.config.workspace.clone();
        let agent_workspace_path = agent_workspace(&workspace, &agent_id);
        let agent_workspace = agent_workspace_path.to_string_lossy().to_string();

        let mut default_toolset = VizierToolSet::new();
        let mut user_toolset = VizierToolSet::new();

        default_toolset = default_toolset
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
            .tool(ListTask {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            })
            .tool(DeleteTask {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            })
            .tool(GetTaskDetail {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            })
            .tool(ConsultAgent::new(agent_id.clone(), deps.clone()))
            .tool(DelegateAgent::new(agent_id.clone(), deps.clone()))
            .tool(SubtasksTool::new(agent_id.clone(), deps.clone()))
            .tool(CreateSkill::new(agent_id.clone(), deps.clone()));

        if agent_config.tools.shell_access {
            default_toolset = default_toolset.tool(ShellExec(deps.shell.clone()));
        }

        default_toolset = default_toolset
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

        if agent_config.tools.discord.enabled {
            if let Some(discord) = &deps.config.channels.discord {
                if let Some((_, discord)) = discord.iter().find(|(id, _)| **id == agent_id) {
                    let token = discord.token.clone();

                    let (send_message, react_message, get_message) = new_discord_tools(token);
                    default_toolset = default_toolset
                        .tool(send_message)
                        .tool(react_message)
                        .tool(get_message);
                }
            }
        }

        if agent_config.tools.telegram.enabled {
            if let Some(telegram) = &deps.config.channels.telegram {
                if let Some((_, telegram)) = telegram.iter().find(|(id, _)| **id == agent_id) {
                    let bot_token = telegram.token.clone();

                    let (send_message, react_message, get_message) = new_telegram_tools(bot_token);
                    default_toolset = default_toolset
                        .tool(send_message)
                        .tool(react_message)
                        .tool(get_message);
                }
            }
        }

        if agent_config.tools.brave_search.enabled {
            if let Some(brave_search) = tool_config.brave_search {
                user_toolset = user_toolset
                    .tool(BraveSearch::<WebOnlySearch>::new(&brave_search))
                    .tool(BraveSearch::<NewsOnlySearch>::new(&brave_search));
            }
        }

        if agent_config.tools.fetch.enabled {
            user_toolset = user_toolset.tool(FetchWebpage);
        }

        if agent_config.tools.http_client.enabled {
            user_toolset = user_toolset.tool(HttpClient);
        }

        if agent_config.tools.vector_memory.enabled {
            if let Some(_) = deps.config.embedding {
                let (read_memory, write_memory, list_memory, detail_memory) =
                    init_vector_memory(agent_id.clone(), deps.clone())?;

                user_toolset = user_toolset
                    .tool(read_memory)
                    .tool(write_memory)
                    .tool(list_memory)
                    .tool(detail_memory);
            }
        }

        let (shared_doc_read, shared_doc_write, shared_doc_get, shared_doc_list) =
            init_shared_document_tools(agent_id.clone(), deps.clone())?;
        user_toolset = user_toolset
            .tool(shared_doc_read)
            .tool(shared_doc_write)
            .tool(shared_doc_get)
            .tool(shared_doc_list);

        let mut mcp = HashMap::new();
        for m in &agent_config.tools.mcp_servers {
            if let Some(client) = deps.mcp_clients.clients.get(m) {
                mcp.insert(m.clone(), client.clone());
            }
        }

        if agent_config.tools.programmatic_sandbox {
            let ptc_toolset = VizierToolSet::new().tool(ProgramaticSandbox {
                tools: Arc::new(user_toolset),
            });
            let tools = Self {
                default_toolset: default_toolset.clone(),
                user_toolset: ptc_toolset,
                mcp: mcp.clone(),
            };
            return Ok(tools);
        }

        let tools = Self {
            default_toolset: default_toolset.clone(),
            user_toolset: user_toolset.clone(),
            mcp: mcp.clone(),
        };
        Ok(tools)
    }
}
