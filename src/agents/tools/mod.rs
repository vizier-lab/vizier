use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

use anyhow::Result;
use chrono::{DateTime, Utc};
use rig_core::completion::ToolDefinition;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

use crate::{
    agents::mcp::McpServer,
    agents::tools::{
        brave_search::{BraveSearch, NewsOnlySearch, WebOnlySearch},
        background_jobs::{CancelBackgroundJob, ListBackgroundJobs},
        consult::{ConsultAgent, DelegateAgent},
        discord::new_discord_tools,
        dream_journal::ReadDreamJournal,
        fetch::FetchWebpage,
        http_client::HttpClient,
        python::{DescribeToolFunction, ExecutePython, ListToolFunctions},
scheduler::{
    DeleteTask, GetTaskDetail, GetTaskRunDetail, ListTask, ListTaskRuns, ScheduleCronTask,
    ScheduleOneTimeTask,
},
        read_image::ReadImageFile,
        session_files::{ListSessionFiles, ReadDocumentFile, SendAttachment},
        shell::ShellExec,
        skill::{
            CreateSkill, DeleteSkill, ExecuteSkillResource, GetSkillDetails, ListSkills,
            ReadSkillResource, UpdateSkill, UseSkill,
        },
        subtasks::SubtasksTool,
        telegram::new_telegram_tools,
        think::ThinkTool,
        tts::TtsGenerate,
        stt::SttTranscribe,
        image_gen::ImageGenerate,
        vector_memory::{RecallSettings, SEARCH_THRESHOLD, init_vector_memory},
        webui::{ListWebuiTopics, SendWebuiMessage},
        workspace::{ReadCore, WriteCore},
    },
    config::provider::ProviderVariant,
    dependencies::VizierDependencies,
    error::VizierError,
    indexer::VizierIndexer,
    sandbox::docs::python_identifier,
    schema::{AgentId, VizierAttachment, VizierResponse, VizierSession},
    storage::{VizierStorage, agent::AgentStorage},
    utils::agent_workspace,
};

mod background_jobs;
mod brave_search;
mod consult;
mod discord;
mod dream_journal;
mod fetch;
mod http_client;
mod python;
mod read_image;
mod scheduler;
mod session_files;
mod shell;
mod skill;
mod subtasks;
mod telegram;
mod think;
pub mod tts;
pub mod stt;
mod image_gen;
mod vector_memory;
mod webui;
mod workspace;

type VizierToolDef = Arc<Box<dyn VizierToolDyn + Send + Sync + 'static>>;

/// What a script-callable tool is for, so an agent can list one area at a time
/// instead of every tool. Declaration order is display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolCategory {
    Memory,
    Identity,
    Scheduling,
    Agents,
    Skills,
    Files,
    Web,
    Audio,
    Media,
    Discord,
    Telegram,
    Webui,
    Shell,
}

impl ToolCategory {
    pub const ALL: [ToolCategory; 13] = [
        Self::Memory,
        Self::Identity,
        Self::Scheduling,
        Self::Agents,
        Self::Skills,
        Self::Files,
        Self::Web,
        Self::Audio,
        Self::Media,
        Self::Discord,
        Self::Telegram,
        Self::Webui,
        Self::Shell,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Identity => "identity",
            Self::Scheduling => "scheduling",
            Self::Agents => "agents",
            Self::Skills => "skills",
            Self::Files => "files",
            Self::Web => "web",
            Self::Audio => "audio",
            Self::Media => "media",
            Self::Discord => "discord",
            Self::Telegram => "telegram",
            Self::Webui => "webui",
            Self::Shell => "shell",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Memory => "Search, read, write and organise long-term memory",
            Self::Identity => "Read and update your CORE identity document",
            Self::Scheduling => "Schedule one-time and recurring tasks and check their results",
            Self::Agents => {
                "Consult or delegate to other agents, run parallel subtasks, manage background jobs"
            }
            Self::Skills => "Create, find and use reusable skills",
            Self::Files => "Read files attached to this conversation and send attachments",
            Self::Web => "Search the web and news, fetch pages, make HTTP requests",
            Self::Audio => "Generate speech from text and transcribe audio",
            Self::Media => "Generate images",
            Self::Discord => {
                "Send, read and react to Discord messages; look up guild, channel and member info"
            }
            Self::Telegram => "Send, read and react to Telegram messages",
            Self::Webui => "Message WebUI users and list their topics",
            Self::Shell => "Run shell commands",
        }
    }
}

#[derive(Clone)]
pub struct ToolContext {
    pub session: VizierSession,
    pub pending_attachments: Arc<Mutex<Vec<VizierAttachment>>>,
    /// The session's hooks, so tools that dispatch other tools (the Python
    /// sandbox bridge) run them through the same hook pipeline as direct calls.
    pub hooks: Option<Arc<crate::agents::hook::VizierSessionHooks>>,
    /// The turn's `VizierRequest::background_depth`, which `BackgroundJobs::launch` checks
    /// against `MAX_BACKGROUND_DEPTH`. 0 outside an ordinary chat turn.
    pub background_depth: u8,
}

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

    pub fn get_tool(&self, function_name: String) -> Result<VizierToolDef> {
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
        ctx: &ToolContext,
    ) -> Result<serde_json::Value, VizierError> {
        let _span = tracing::info_span!("tool_call", function = %function_name).entered();
        tracing::trace!(function = %function_name, "tool call started");
        let tool = self
            .tools
            .get(&function_name)
            .ok_or(VizierError(format!("{function_name} does not exists")))?;

        let output = tool.tool_call(args, ctx).await?;
        tracing::trace!(function = %function_name, "tool call completed");
        Ok(serde_json::from_str(&output).map_err(|err| VizierError(err.to_string()))?)
    }
}

/// Name-based dispatch over the agent's script-callable tools: every native tool
/// with a category, plus the MCP servers' tools.
#[derive(Clone)]
pub struct ToolRouter {
    pub default_toolset: VizierToolSet,
    pub user_toolset: VizierToolSet,
    pub mcp: HashMap<String, Arc<McpServer>>,
}

/// What a catalogue name dispatches to.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolTarget {
    Native(String),
    /// `tool` is the server's own name for it.
    Mcp { server: String, tool: String },
}

/// One script-callable function: the single source for its name, category and dispatch.
#[derive(Clone, Debug)]
pub struct CatalogueEntry {
    pub name: String,
    pub category: String,
    pub target: ToolTarget,
    pub definition: ToolDefinition,
}

/// A category line of the system prompt.
#[derive(Clone, Debug, PartialEq)]
pub struct CategorySummary {
    pub name: String,
    pub description: String,
}

#[derive(Clone)]
pub struct VizierTools {
    pub router: ToolRouter,
    /// The only tools the model calls directly: `think`, `execute_python` and the docs tools.
    pub sandbox_toolset: VizierToolSet,
    /// Fixed for the instance's lifetime, so the system prompt that lists them stays
    /// byte-identical across turns.
    pub categories: Vec<CategorySummary>,
}

#[async_trait::async_trait]
pub trait VizierToolDyn {
    fn tool_name(&self) -> String;

    fn category(&self) -> Option<ToolCategory>;

    fn tool_def(&self) -> ToolDefinition;

    fn description(&self) -> String;

    fn input_schema(&self) -> serde_json::Value;

    fn output_schema(&self) -> serde_json::Value;

    async fn tool_call(&self, args: String, ctx: &ToolContext) -> Result<String, VizierError>;
}

#[async_trait::async_trait]
impl<Tool: VizierTool + Sync + Send> VizierToolDyn for Tool {
    fn tool_name(&self) -> String {
        Self::name()
    }

    fn category(&self) -> Option<ToolCategory> {
        Self::category()
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

    async fn tool_call(&self, args: String, ctx: &ToolContext) -> Result<String, VizierError> {
        let input = serde_json::from_str(&args).map_err(|err| VizierError(err.to_string()))?;
        let output = self.call(input, ctx).await?;

        serde_json::to_string(&output).map_err(|err| VizierError(err.to_string()))
    }
}

#[async_trait::async_trait]
pub trait VizierTool {
    type Input: JsonSchema + for<'a> Deserialize<'a> + Serialize;
    type Output: JsonSchema + for<'a> Deserialize<'a> + Serialize;

    fn name() -> String;

    /// The category a script lists this tool under; `None` only for the tools the model
    /// calls directly and the dream-only journal tools, which no script can reach.
    fn category() -> Option<ToolCategory>;

    fn input_schema() -> serde_json::Value {
        serde_json::to_value(schema_for!(<Self as VizierTool>::Input)).unwrap()
    }

    fn output_schema() -> serde_json::Value {
        serde_json::to_value(schema_for!(<Self as VizierTool>::Output)).unwrap()
    }

    fn description(&self) -> String;

    async fn call(&self, args: Self::Input, ctx: &ToolContext)
    -> Result<Self::Output, VizierError>;
}

/// A tool's serialised output as a `VizierResponse`: tools that already answer
/// with one pass through, everything else is wrapped as a `ToolResponse`.
fn tool_output_to_response(output: &str) -> Result<VizierResponse> {
    let res = serde_json::from_str::<serde_json::Value>(output)?;

    if let Ok(vizier_response) = serde_json::from_value(res.clone()) {
        return Ok(vizier_response);
    }

    Ok(VizierResponse {
        timestamp: Utc::now(),
        content: crate::schema::VizierResponseContent::ToolResponse { response: res },
        attachments: vec![],
        ..Default::default()
    })
}

/// An MCP server's category: its key, unless that is a built-in category's name.
fn mcp_category_name(server: &str) -> String {
    if ToolCategory::ALL.iter().any(|category| category.name() == server) {
        format!("{server}_mcp")
    } else {
        server.to_string()
    }
}

/// Names for MCP tools (`(server, tool)` pairs), aligned with `mcp`. Native names are
/// reserved first, so an MCP tool can never take one; MCP tools are then named in
/// `(server, tool)` order and a taken name gets `_2`, `_3`, …, so the result does not
/// depend on the order servers connected in.
fn assign_names(native: &[&str], mcp: &[(&str, &str)]) -> Vec<String> {
    let mut taken: HashSet<String> = native.iter().map(|name| python_identifier(name)).collect();
    let mut order: Vec<usize> = (0..mcp.len()).collect();
    order.sort_by_key(|&i| mcp[i]);

    let mut names = vec![String::new(); mcp.len()];
    for i in order {
        let (server, tool) = mcp[i];
        let base = python_identifier(&format!("{server}_{tool}"));
        let mut name = base.clone();
        let mut n = 2;
        while !taken.insert(name.clone()) {
            name = format!("{base}_{n}");
            n += 1;
        }
        names[i] = name;
    }
    names
}

/// Built-in categories in declaration order, then one per MCP server sorted by name.
fn summarise_categories<'a>(
    present: impl IntoIterator<Item = ToolCategory>,
    servers: impl IntoIterator<Item = &'a String>,
) -> Vec<CategorySummary> {
    let present: BTreeSet<ToolCategory> = present.into_iter().collect();
    let mut mcp: Vec<CategorySummary> = servers
        .into_iter()
        .map(|server| CategorySummary {
            name: mcp_category_name(server),
            description: format!("Tools from the {server} MCP server"),
        })
        .collect();
    mcp.sort_by(|a, b| a.name.cmp(&b.name));

    present
        .into_iter()
        .map(|category| CategorySummary {
            name: category.name().into(),
            description: category.description().into(),
        })
        .chain(mcp)
        .collect()
}

/// The agent's categories, from its registered tools and configured MCP servers — never
/// live listings, so a server that is down keeps its line.
fn category_summaries(router: &ToolRouter) -> Vec<CategorySummary> {
    summarise_categories(
        router.native_tools().filter_map(|tool| tool.category()),
        router.mcp.keys(),
    )
}

impl ToolRouter {
    fn native_tools(&self) -> impl Iterator<Item = &VizierToolDef> {
        self.default_toolset
            .tools
            .values()
            .chain(self.user_toolset.tools.values())
    }

    /// Every script-callable function, sorted by category (system-prompt order) then name.
    pub async fn catalogue(&self) -> Vec<CatalogueEntry> {
        let mut entries: Vec<CatalogueEntry> = self
            .native_tools()
            .filter_map(|tool| {
                Some(CatalogueEntry {
                    name: python_identifier(&tool.tool_name()),
                    category: tool.category()?.name().into(),
                    target: ToolTarget::Native(tool.tool_name()),
                    definition: tool.tool_def(),
                })
            })
            .collect();

        // Concurrently, so one slow server costs one listing timeout, not one per server. A
        // server that can't list leaves its tools out of this turn and never fails it (#53).
        let listings = futures::future::join_all(
            self.mcp
                .iter()
                .map(|(key, server)| async move { (key.clone(), server.tools().await) }),
        )
        .await;
        let mcp: Vec<(String, ToolDefinition)> = listings
            .into_iter()
            .flat_map(|(key, tools)| tools.into_iter().map(move |tool| (key.clone(), tool)))
            .collect();

        let native: Vec<String> = self.native_tools().map(|tool| tool.tool_name()).collect();
        let native: Vec<&str> = native.iter().map(String::as_str).collect();
        let pairs: Vec<(&str, &str)> = mcp
            .iter()
            .map(|(server, tool)| (server.as_str(), tool.name.as_str()))
            .collect();
        let names = assign_names(&native, &pairs);

        entries.extend(mcp.into_iter().zip(names).map(|((server, definition), name)| {
            CatalogueEntry {
                name,
                category: mcp_category_name(&server),
                target: ToolTarget::Mcp {
                    server,
                    tool: definition.name.clone(),
                },
                definition,
            }
        }));

        let rank = |entry: &CatalogueEntry| match entry.target {
            ToolTarget::Native(_) => ToolCategory::ALL
                .iter()
                .position(|category| category.name() == entry.category)
                .unwrap_or(ToolCategory::ALL.len()),
            ToolTarget::Mcp { .. } => ToolCategory::ALL.len(),
        };
        entries.sort_by(|a, b| {
            (rank(a), &a.category, &a.name).cmp(&(rank(b), &b.category, &b.name))
        });
        entries
    }

    pub async fn call(
        &self,
        function_name: String,
        params: String,
        ctx: &ToolContext,
    ) -> Result<VizierResponse> {
        if let Ok(tool) = self.default_toolset.get_tool(function_name.clone()) {
            let output = tool.tool_call(params.clone(), ctx).await?;
            return tool_output_to_response(&output);
        }

        if let Ok(tool) = self.user_toolset.get_tool(function_name.clone()) {
            let output = tool.tool_call(params.clone(), ctx).await?;
            return tool_output_to_response(&output);
        }

        let target = self
            .catalogue()
            .await
            .into_iter()
            .find(|entry| entry.name == function_name)
            .map(|entry| entry.target);

        match target {
            Some(ToolTarget::Native(tool)) => {
                let tool = self
                    .default_toolset
                    .get_tool(tool.clone())
                    .or_else(|_| self.user_toolset.get_tool(tool))?;
                let output = tool.tool_call(params, ctx).await?;
                tool_output_to_response(&output)
            }
            Some(ToolTarget::Mcp { server, tool }) => self
                .mcp
                .get(&server)
                .ok_or(VizierError("mcp not found".into()))?
                .call(tool, serde_json::from_str(&params)?)
                .await,
            None => Err(VizierError(format!("{} not found", function_name)).into()),
        }
    }
}

impl VizierTools {
    pub async fn tools(&self) -> Result<Vec<ToolDefinition>> {
        let mut defs: Vec<ToolDefinition> = self
            .sandbox_toolset
            .tools
            .values()
            .map(|tool| tool.tool_def())
            .collect();
        // Toolsets are HashMaps, whose iteration order changes across process restarts and agent
        // respawns; a stable order keeps the tool list a cacheable prompt prefix.
        defs.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(defs)
    }

    pub async fn call(
        &self,
        function_name: String,
        params: String,
        ctx: &ToolContext,
    ) -> Result<VizierResponse> {
        let tool = self.sandbox_toolset.get_tool(function_name.clone()).map_err(|_| {
            VizierError(format!(
                "{function_name} is not called directly; call it from an execute_python script (see list_tool_functions)"
            ))
        })?;
        let output = tool.tool_call(params, ctx).await?;
        tool_output_to_response(&output)
    }

    const DREAM_TOOL_NAMES: &'static [&'static str] = &[
        // Memory (8)
        "memory_read",
        "memory_write",
        "memory_list",
        "memory_search",
        "memory_follow",
        "memory_graph",
        "memory_delete",
        "memory_delete_bundle",
        // Workspace (2)
        "write_core",
        "read_core",
        // Scheduler (6)
        "schedule_one_time_task",
        "schedule_cron_task",
        "list_task",
        "delete_task",
        // `is_non_user_channel` excludes `task__` from the sessions the dream cycle
        // reflects on, so these two are the only route by which a task's outcome reaches
        // memory at all. Paired with `list_task`, the cycle can walk from "what tasks do I
        // have" to "how have they been going".
        "list_task_runs",
        "get_task_run_detail",
        // Skills (5)
        "create_skill",
        "update_skill",
        "list_skills",
        "get_skill_details",
        "use_skill",
    ];

    pub async fn dream_tools(
        &self,
        agent_id: AgentId,
        storage: Arc<VizierStorage>,
    ) -> Result<Vec<ToolDefinition>> {
        let mut tools = vec![];

        // Filter default_toolset for dream-relevant tools
        for name in Self::DREAM_TOOL_NAMES {
            if let Ok(tool) = self.router.default_toolset.get_tool(name.to_string()) {
                tools.push(tool.tool_def());
            }
        }

        // Add read_dream_journal for API browsing
        let read_journal = ReadDreamJournal { agent_id, storage };
        let read_def = <ReadDreamJournal as VizierToolDyn>::tool_def(&read_journal);
        tools.push(read_def);

        Ok(tools)
    }

    pub async fn dream_call(
        &self,
        function_name: String,
        params: String,
        agent_id: &AgentId,
        storage: &Arc<VizierStorage>,
        ctx: &ToolContext,
    ) -> Result<VizierResponse> {
        // Handle dream journal tools
        match function_name.as_str() {
            "read_dream_journal" => {
                let tool = ReadDreamJournal {
                    agent_id: agent_id.clone(),
                    storage: storage.clone(),
                };
                let output = tool.tool_call(params, ctx).await?;
                let res = serde_json::from_str(&output)?;
                return Ok(VizierResponse {
                    timestamp: Utc::now(),
                    content: crate::schema::VizierResponseContent::ToolResponse { response: res },
                    attachments: vec![],
                    ..Default::default()
                });
            }
            _ => {}
        }

        // Delegate to default_toolset for memory, workspace, scheduler, skill tools
        self.router.call(function_name, params, ctx).await
    }
}

impl VizierTools {
    pub async fn new(
        agent_id: AgentId,
        deps: VizierDependencies,
        agent_config: &crate::schema::AgentConfig,
        indexer: Option<crate::indexer::VizierIndexer>,
        stt: Option<Arc<crate::stt::VizierStt>>,
        tts: Option<Arc<crate::tts::VizierTts>>,
        image_gen: Option<Arc<crate::image_generation::VizierImageGen>>,
    ) -> Result<Self> {
        let workspace = deps.config.workspace.clone();
        let agent_workspace_path = agent_workspace(&workspace, &agent_id);
        let agent_workspace = agent_workspace_path.to_string_lossy().to_string();

        let other_agents: HashMap<String, crate::schema::AgentConfig> = deps
            .storage
            .list_agents()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|(aid, _)| aid != &agent_id)
            .collect();

        let mut default_toolset = VizierToolSet::new();
        let mut user_toolset = VizierToolSet::new();

        default_toolset = default_toolset
            .tool(WriteCore::new(agent_id.clone(), deps.storage.clone()))
            .tool(ReadCore::new(agent_id.clone(), deps.storage.clone()))
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
            .tool(ListTaskRuns {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            })
            .tool(GetTaskRunDetail {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            })
            .tool(ConsultAgent::new(
                agent_id.clone(),
                other_agents.clone(),
                deps.transport.clone(),
            ))
            .tool(DelegateAgent::new(other_agents.clone(), deps.clone()))
            .tool(SubtasksTool::new(deps.clone()))
            .tool(ListBackgroundJobs::new(deps.clone()))
            .tool(CancelBackgroundJob::new(deps.clone()))
            .tool(CreateSkill::new(agent_id.clone(), deps.clone(), indexer.clone()))
            .tool(UpdateSkill::new(deps.clone(), indexer.clone()))
            .tool(DeleteSkill::new(deps.clone(), indexer.clone()))
            .tool(ListSkills::new(agent_id.clone(), deps.clone()))
            .tool(GetSkillDetails::new(agent_id.clone(), deps.clone()))
            .tool(UseSkill::new(agent_id.clone(), deps.clone()))
            .tool(ReadSkillResource::new(Some(agent_id.clone()), deps.clone()))
            .tool(ExecuteSkillResource::new(
                Some(agent_id.clone()),
                deps.clone(),
            ))
            .tool(ListSessionFiles {
                storage: deps.storage.clone(),
            })
            .tool(ReadDocumentFile {
                storage: deps.storage.clone(),
                file_manager: deps.file_manager.clone(),
            })
            .tool(ReadImageFile {
                storage: deps.storage.clone(),
                file_manager: deps.file_manager.clone(),
                vision: build_read_image_vision(&deps, agent_config).await,
            })
            .tool(SendAttachment {
                storage: deps.storage.clone(),
            });

        if let Some(ref shell_config) = agent_config.tools.shell {
            match crate::agents::shell::VizierShell::new(shell_config).await {
                Ok(shell) => {
                    default_toolset = default_toolset.tool(ShellExec(Arc::new(shell)));
                }
                Err(e) => {
                    tracing::error!("failed to create shell for agent {}: {}", agent_id, e);
                }
            }
        }

        if agent_config.tools.discord.enabled {
            if let Some(token) = &agent_config.discord_token {
                let (send_message, react_message, get_message, guild_info, channel_info, member_info) =
                    new_discord_tools(token.clone(), agent_id.clone(), deps.storage.clone());
                default_toolset = default_toolset
                    .tool(send_message)
                    .tool(react_message)
                    .tool(get_message)
                    .tool(guild_info)
                    .tool(channel_info)
                    .tool(member_info);
            }
        }

        if agent_config.tools.telegram.enabled {
            if let Some(token) = &agent_config.telegram_token {
                let (send_message, react_message, get_message) = new_telegram_tools(token.clone(), agent_id.clone(), deps.storage.clone());
                default_toolset = default_toolset
                    .tool(send_message)
                    .tool(react_message)
                    .tool(get_message);
            }
        }

        user_toolset = user_toolset
            .tool(SendWebuiMessage {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            })
            .tool(ListWebuiTopics {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            });

        if agent_config.tools.brave_search.enabled {
            let per_agent = &agent_config.tools.brave_search.settings;

            if let (Some(key), Some(ss)) = (per_agent.api_key.as_ref(), per_agent.safesearch) {
                let cfg = crate::config::tools::BraveSearchConfig {
                    api_key: key.clone(),
                    safesearch: ss,
                };
                user_toolset = user_toolset
                    .tool(BraveSearch::<WebOnlySearch>::new(&cfg))
                    .tool(BraveSearch::<NewsOnlySearch>::new(&cfg));
            }
        }

        if agent_config.tools.fetch.enabled {
            user_toolset = user_toolset.tool(FetchWebpage);
        }

        if agent_config.tools.http_client.enabled {
            user_toolset = user_toolset.tool(HttpClient);
        }

        if let Some(idx) = indexer.clone() {
            let (
                search_memory,
                write_memory,
                list_memory,
                read_memory,
                follow_memory,
                graph_memory,
                delete_memory,
                delete_memory_bundle,
            ) = init_vector_memory(
                agent_id.clone(),
                deps.storage.clone(),
                idx,
                RecallSettings {
                    chunking: agent_config.chunking.clone(),
                    search_limit: 10,
                    search_threshold: SEARCH_THRESHOLD,
                    per_document: agent_config.auto_context.per_document,
                },
            )?;

            default_toolset = default_toolset
                .tool(search_memory)
                .tool(write_memory)
                .tool(list_memory)
                .tool(read_memory)
                .tool(follow_memory)
                .tool(graph_memory)
                .tool(delete_memory)
                .tool(delete_memory_bundle);
        }

        if let Some(tts) = tts {
            let voice = agent_config
                .tools
                .tts
                .settings
                .voice
                .clone()
                .unwrap_or_else(|| {
                    agent_config
                        .tools
                        .tts
                        .settings
                        .provider
                        .default_voice()
                        .into()
                });
            let speed = agent_config.tools.tts.settings.speed.unwrap_or(1.0);
            user_toolset = user_toolset.tool(TtsGenerate {
                tts,
                storage: deps.storage.clone(),
                file_manager: deps.file_manager.clone(),
                voice,
                speed,
            });
        }

        if let Some(stt) = stt {
            let language = agent_config.tools.stt.settings.language.clone();
            user_toolset = user_toolset.tool(SttTranscribe {
                stt,
                storage: deps.storage.clone(),
                file_manager: deps.file_manager.clone(),
                language,
            });
        }

        if let Some(image_gen) = image_gen {
            let default_size = agent_config.tools.image_gen.settings.size.clone();
            user_toolset = user_toolset.tool(ImageGenerate {
                image_gen,
                storage: deps.storage.clone(),
                file_manager: deps.file_manager.clone(),
                default_size,
            });
        }

        // Connected in the background: a slow or broken server must not hold up the agent's
        // start, and one that fails to connect is retried on later turns (#53).
        let mut mcp = HashMap::new();
        for (name, mcp_config) in &agent_config.tools.mcp_servers {
            let server = McpServer::new(name.clone(), mcp_config.clone());
            server.warm_up();
            mcp.insert(name.clone(), server);
        }

        let router = ToolRouter {
            default_toolset,
            user_toolset,
            mcp,
        };
        let categories = category_summaries(&router);

        let limits = crate::sandbox::SandboxLimits {
            timeout: *agent_config.tools.timeout,
            tools_enabled: true,
        };
        let sandbox_toolset = VizierToolSet::new()
            .tool(ThinkTool)
            .tool(ExecutePython::new(
                router.clone(),
                limits,
                &agent_config.tools.timeout.to_string(),
            ))
            .tool(ListToolFunctions::new(router.clone(), categories.clone()))
            .tool(DescribeToolFunction::new(router.clone()));

        Ok(Self {
            router,
            sandbox_toolset,
            categories,
        })
    }
}

async fn build_read_image_vision(
    deps: &VizierDependencies,
    agent_config: &crate::schema::AgentConfig,
) -> Option<crate::agents::agent::model::VizierModel> {
    if !agent_config.tools.read_image.enabled {
        return None;
    }

    let provider = agent_config.tools.read_image.settings.provider.clone();
    let model = agent_config.tools.read_image.settings.model.clone();

    let (provider, model) = match (provider, model) {
        (Some(p), Some(m)) if !m.trim().is_empty() => (p, m),
        _ => {
            tracing::warn!(
                "read_image.enabled is true for agent {} but provider/model not set; \
                falling back to attachment-injection flow",
                agent_config.name
            );
            return None;
        }
    };

    match crate::agents::agent::model::VizierModel::new_with_override(
        deps,
        agent_config,
        Some((provider.clone(), model.clone())),
    )
    .await
    {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::error!(
                "failed to build read_image vision model for agent {} ({:?}:{}): {}",
                agent_config.name,
                provider,
                model,
                e
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::tools::{
        discord::{
            GetDiscordChannelInfo, GetDiscordGuildInfo, GetDiscordMemberInfo, GetDiscordMessage,
            ReactDiscordMessage, SendDiscordMessage,
        },
        dream_journal::WriteDreamJournal,
        telegram::{GetTelegramMessage, ReactTelegramMessage, SendTelegramMessage},
        vector_memory::{
            MemoryDelete, MemoryDeleteBundle, MemoryFollow, MemoryGraphTool, MemoryList,
            MemoryRead, MemorySearch, MemoryWrite,
        },
    };

    fn entry<T: VizierTool>() -> (String, Option<ToolCategory>) {
        (T::name(), T::category())
    }

    /// data-model.md's tool → category table, read off the tools themselves.
    fn every_tool() -> Vec<(String, Option<ToolCategory>)> {
        vec![
            entry::<MemorySearch>(),
            entry::<MemoryRead>(),
            entry::<MemoryWrite>(),
            entry::<MemoryList>(),
            entry::<MemoryFollow>(),
            entry::<MemoryGraphTool>(),
            entry::<MemoryDelete>(),
            entry::<MemoryDeleteBundle>(),
            entry::<ReadCore>(),
            entry::<WriteCore>(),
            entry::<ScheduleOneTimeTask>(),
            entry::<ScheduleCronTask>(),
            entry::<ListTask>(),
            entry::<GetTaskDetail>(),
            entry::<DeleteTask>(),
            entry::<ListTaskRuns>(),
            entry::<GetTaskRunDetail>(),
            entry::<ConsultAgent>(),
            entry::<DelegateAgent>(),
            entry::<SubtasksTool>(),
            entry::<ListBackgroundJobs>(),
            entry::<CancelBackgroundJob>(),
            entry::<CreateSkill>(),
            entry::<UpdateSkill>(),
            entry::<DeleteSkill>(),
            entry::<ListSkills>(),
            entry::<GetSkillDetails>(),
            entry::<UseSkill>(),
            entry::<ReadSkillResource>(),
            entry::<ExecuteSkillResource>(),
            entry::<ListSessionFiles>(),
            entry::<ReadDocumentFile>(),
            entry::<ReadImageFile>(),
            entry::<SendAttachment>(),
            entry::<BraveSearch<WebOnlySearch>>(),
            entry::<BraveSearch<NewsOnlySearch>>(),
            entry::<FetchWebpage>(),
            entry::<HttpClient>(),
            entry::<TtsGenerate>(),
            entry::<SttTranscribe>(),
            entry::<ImageGenerate>(),
            entry::<SendDiscordMessage>(),
            entry::<ReactDiscordMessage>(),
            entry::<GetDiscordMessage>(),
            entry::<GetDiscordGuildInfo>(),
            entry::<GetDiscordChannelInfo>(),
            entry::<GetDiscordMemberInfo>(),
            entry::<SendTelegramMessage>(),
            entry::<ReactTelegramMessage>(),
            entry::<GetTelegramMessage>(),
            entry::<SendWebuiMessage>(),
            entry::<ListWebuiTopics>(),
            entry::<ShellExec>(),
            entry::<ThinkTool>(),
            entry::<ExecutePython>(),
            entry::<ListToolFunctions>(),
            entry::<DescribeToolFunction>(),
            entry::<ReadDreamJournal>(),
            entry::<WriteDreamJournal>(),
        ]
    }

    #[test]
    fn every_tool_is_in_its_category() {
        use ToolCategory::*;
        const EXPECTED: &[(&str, ToolCategory)] = &[
            ("memory_search", Memory),
            ("memory_read", Memory),
            ("memory_write", Memory),
            ("memory_list", Memory),
            ("memory_follow", Memory),
            ("memory_graph", Memory),
            ("memory_delete", Memory),
            ("memory_delete_bundle", Memory),
            ("read_core", Identity),
            ("write_core", Identity),
            ("schedule_one_time_task", Scheduling),
            ("schedule_cron_task", Scheduling),
            ("list_task", Scheduling),
            ("get_task_detail", Scheduling),
            ("delete_task", Scheduling),
            ("list_task_runs", Scheduling),
            ("get_task_run_detail", Scheduling),
            ("consult_agent", Agents),
            ("delegate_agent", Agents),
            ("paralel_subtasks", Agents),
            ("list_background_jobs", Agents),
            ("cancel_background_job", Agents),
            ("create_skill", Skills),
            ("update_skill", Skills),
            ("delete_skill", Skills),
            ("list_skills", Skills),
            ("get_skill_details", Skills),
            ("use_skill", Skills),
            ("read_skill_resource", Skills),
            ("execute_skill_resource", Skills),
            ("list_session_files", Files),
            ("read_document_file", Files),
            ("read_image_file", Files),
            ("send_attachment", Files),
            ("web_search", Web),
            ("news_search", Web),
            ("fetch", Web),
            ("http_client", Web),
            ("tts_generate", Audio),
            ("stt_transcribe", Audio),
            ("image_generate", Media),
            ("discord_send_message", Discord),
            ("discord_react_message", Discord),
            ("discord_get_message_by_id", Discord),
            ("discord_get_guild_info", Discord),
            ("discord_get_channel_info", Discord),
            ("discord_get_member_info", Discord),
            ("telegram_send_message", Telegram),
            ("telegram_react_message", Telegram),
            ("telegram_get_message_by_id", Telegram),
            ("webui_send_message", Webui),
            ("webui_list_topics", Webui),
            ("shell_exec", Shell),
        ];
        const UNCATEGORISED: &[&str] = &[
            "think",
            "execute_python",
            "list_tool_functions",
            "describe_tool_function",
            "read_dream_journal",
            "write_dream_journal",
        ];

        let tools = every_tool();
        let categorised: Vec<(&str, ToolCategory)> = tools
            .iter()
            .filter_map(|(name, category)| Some((name.as_str(), (*category)?)))
            .collect();
        let mut expected = EXPECTED.to_vec();
        let mut actual = categorised.clone();
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected);

        let mut uncategorised: Vec<&str> = tools
            .iter()
            .filter(|(_, category)| category.is_none())
            .map(|(name, _)| name.as_str())
            .collect();
        let mut expected_none = UNCATEGORISED.to_vec();
        uncategorised.sort();
        expected_none.sort();
        assert_eq!(uncategorised, expected_none);

        for category in ToolCategory::ALL {
            assert!(
                EXPECTED.iter().any(|(_, c)| *c == category),
                "{category:?} has no tool"
            );
        }
    }

    #[test]
    fn every_dream_tool_name_is_a_real_tool() {
        let names: Vec<String> = every_tool().into_iter().map(|(name, _)| name).collect();
        for name in VizierTools::DREAM_TOOL_NAMES {
            assert!(names.iter().any(|n| n == name), "{name} is not a tool");
        }
    }

    #[test]
    fn categories_list_built_ins_in_order_then_mcp_servers_by_name() {
        use ToolCategory::*;
        let servers: Vec<String> = ["memory", "github", "a"].iter().map(|s| s.to_string()).collect();
        // No Discord, Telegram, TTS, STT, image or shell tools registered.
        let present = [Webui, Memory, Agents, Identity, Memory, Skills, Files, Scheduling];
        let summaries = summarise_categories(present, &servers);
        let names: Vec<&str> = summaries.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "memory", "identity", "scheduling", "agents", "skills", "files", "webui", "a",
                "github", "memory_mcp"
            ]
        );
        assert_eq!(summaries[9].description, "Tools from the memory MCP server");
        assert_eq!(summaries, summarise_categories(present, &servers));
    }

    #[test]
    fn mcp_tools_are_named_server_then_tool() {
        assert_eq!(assign_names(&[], &[("github", "create_issue")]), ["github_create_issue"]);
        assert_eq!(assign_names(&[], &[("my-srv", "do-thing")]), ["my_srv_do_thing"]);
    }

    #[test]
    fn native_names_win_and_collisions_are_suffixed_in_server_order() {
        assert_eq!(
            assign_names(&["memory_search"], &[("memory", "search")]),
            ["memory_search_2"]
        );
        assert_eq!(
            assign_names(&[], &[("a", "b_c"), ("a_b", "c")]),
            ["a_b_c", "a_b_c_2"]
        );
        assert_eq!(
            assign_names(&[], &[("a_b", "c"), ("a", "b_c")]),
            ["a_b_c_2", "a_b_c"]
        );
    }

    #[test]
    fn no_mcp_name_keeps_the_old_prefix() {
        let names = assign_names(&["read_core"], &[("github", "create_issue"), ("fs", "read_file")]);
        assert!(names.iter().all(|name| !name.starts_with("mcp_")));
        assert!(!names.contains(&"mcp_github__create_issue".to_string()));
    }

    #[test]
    fn a_server_named_like_a_built_in_category_gets_a_suffix() {
        assert_eq!(mcp_category_name("memory"), "memory_mcp");
        assert_eq!(mcp_category_name("discord"), "discord_mcp");
        assert_eq!(mcp_category_name("github"), "github");
    }
}
