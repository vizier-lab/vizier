use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::Instrument;

use anyhow::Result;
use chrono::{DateTime, Utc};
use crate::llm::types::ToolDefinition;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

use crate::{
    agents::mcp::McpServer,
    agents::tools::{
        background_jobs::{CancelBackgroundJob, ListBackgroundJobs},
        consult::{ConsultAgent, DelegateAgent},
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
        think::ThinkTool,
        tts::TtsGenerate,
        stt::SttTranscribe,
        image_gen::ImageGenerate,
        vector_memory::{RecallSettings, SEARCH_THRESHOLD, init_vector_memory},
        webui::{ListWebuiTopics, SendWebuiMessage},
        workspace::{ReadCore, WriteCore},
    },
    dependencies::VizierDependencies,
    error::VizierError,
    indexer::VizierIndexer,
    sandbox::docs::python_identifier,
    schema::{AgentId, VizierAttachment, VizierResponse, VizierSession},
    storage::{VizierStorage, agent::AgentStorage},
    utils::agent_workspace,
};

mod background_jobs;
pub mod brave_search;
mod consult;
pub mod discord;
mod dream_journal;
mod fetch;
mod http_client;
mod python;
mod read_image;
mod scheduler;
pub mod self_integrations;
mod session_files;
mod shell;
mod skill;
mod subtasks;
pub mod telegram;
mod think;
pub mod tts;
pub mod stt;
mod image_gen;
mod vector_memory;
mod webui;
mod workspace;

pub type VizierToolDef = Arc<Box<dyn VizierToolDyn + Send + Sync + 'static>>;

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
    Integrations,
}

impl ToolCategory {
    pub const ALL: [ToolCategory; 14] = [
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
        Self::Integrations,
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
            Self::Integrations => "integrations",
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
            Self::Integrations => "Manage your own MCP servers, addons and channel connections",
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
    /// Images a tool loaded for the model to see (`read_image_file`), however it was called —
    /// directly, or from an `execute_python` script. The agent loop adds them to the
    /// conversation after the round's tool results (FR-033).
    pub loaded_images: Arc<Mutex<Vec<VizierAttachment>>>,
}

#[derive(Clone, Default)]
pub struct VizierToolSet {
    pub tools: HashMap<String, Arc<Box<dyn VizierToolDyn + Send + Sync + 'static>>>,
}

impl VizierToolSet {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn tool<Tool: VizierToolDyn + Sync + Send + 'static>(mut self, tool: Tool) -> Self {
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

/// The GenAI `execute_tool` span. `call_id` is the model's tool call id: a call made from a
/// script has none. Arguments and result are content, recorded only when the GenAI content
/// capture is on (`crate::llm::telemetry::capture_content`).
fn execute_tool_span(function_name: &str, args: &str, call_id: Option<&str>) -> tracing::Span {
    let span = tracing::info_span!(
        "execute_tool",
        otel.name = %format!("execute_tool {function_name}"),
        otel.status_code = tracing::field::Empty,
        gen_ai.operation.name = "execute_tool",
        gen_ai.tool.name = %function_name,
        gen_ai.tool.type = "function",
        gen_ai.tool.call.id = call_id,
        gen_ai.tool.call.arguments = tracing::field::Empty,
        gen_ai.tool.call.result = tracing::field::Empty,
        error.type = tracing::field::Empty,
    );
    if crate::llm::telemetry::capture_content() {
        span.record("gen_ai.tool.call.arguments", args);
    }
    span
}

fn record_tool_result(span: &tracing::Span, result: &Result<VizierResponse>) {
    let capture = crate::llm::telemetry::capture_content();
    match result {
        Ok(response) => {
            if capture {
                // What the model is given back, without the response envelope.
                let content = match &response.content {
                    crate::schema::VizierResponseContent::ToolResponse { response } => response.to_string(),
                    other => serde_json::to_string(other).unwrap_or_default(),
                };
                span.record("gen_ai.tool.call.result", content);
            }
        }
        Err(err) => {
            span.record("otel.status_code", "ERROR");
            span.record("error.type", "tool_error");
            if capture {
                span.record("gen_ai.tool.call.result", err.to_string());
            }
        }
    }
}

/// Run one tool call the model asked for under its `execute_tool` span, cut off at `limit`.
pub async fn traced_tool_call(
    function_name: &str,
    call_id: &str,
    args: &str,
    limit: std::time::Duration,
    call: impl std::future::Future<Output = Result<VizierResponse>>,
) -> std::result::Result<Result<VizierResponse>, tokio::time::error::Elapsed> {
    let span = execute_tool_span(function_name, args, Some(call_id));
    let outcome = tokio::time::timeout(limit, call).instrument(span.clone()).await;
    match &outcome {
        Ok(result) => record_tool_result(&span, result),
        Err(_) => {
            span.record("otel.status_code", "ERROR");
            span.record("error.type", "timeout");
        }
    }
    outcome
}

/// Name-based dispatch over the agent's script-callable tools: every native tool
/// with a category, the addons' tools, plus the MCP servers' tools.
#[derive(Clone)]
pub struct ToolRouter {
    pub default_toolset: VizierToolSet,
    pub user_toolset: VizierToolSet,
    /// Native tools contributed by the agent's channel connections, and its self-management
    /// tools while any family is granted (specs/017-agent-self-integrations research R5).
    pub integration_toolset: VizierToolSet,
    /// Each attached addon's tools, listed under the addon id as their category.
    pub addons: BTreeMap<String, AddonTools>,
    pub mcp: HashMap<String, Arc<McpServer>>,
    /// Category names an MCP server may not take: every registered addon id, attached or not.
    pub reserved: Arc<Vec<String>>,
}

/// One addon's tools and the description its category line carries.
#[derive(Clone)]
pub struct AddonTools {
    pub description: String,
    pub toolset: VizierToolSet,
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
    /// Changes only when the composed tool set does, so the system prompt that lists them
    /// stays byte-identical across turns in between.
    pub categories: Vec<CategorySummary>,
    /// The integration layer this snapshot was composed from, kept for status and for the
    /// next refresh to reuse unchanged MCP connections.
    pub integrations: Arc<crate::integrations::IntegrationTools>,
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

/// An MCP server's category: its key, unless that is a built-in category's or an addon's name.
fn mcp_category_name(server: &str, reserved: &[String]) -> String {
    if ToolCategory::ALL.iter().any(|category| category.name() == server)
        || reserved.iter().any(|name| name == server)
    {
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

/// Built-in categories in declaration order, then one per addon and one per MCP server, each
/// sorted by name.
fn summarise_categories<'a>(
    present: impl IntoIterator<Item = ToolCategory>,
    addons: impl IntoIterator<Item = (&'a String, &'a String)>,
    servers: impl IntoIterator<Item = &'a String>,
    reserved: &[String],
) -> Vec<CategorySummary> {
    let present: BTreeSet<ToolCategory> = present.into_iter().collect();
    let mut addons: Vec<CategorySummary> = addons
        .into_iter()
        .map(|(name, description)| CategorySummary {
            name: name.clone(),
            description: description.clone(),
        })
        .collect();
    addons.sort_by(|a, b| a.name.cmp(&b.name));
    let mut mcp: Vec<CategorySummary> = servers
        .into_iter()
        .map(|server| CategorySummary {
            name: mcp_category_name(server, reserved),
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
        .chain(addons)
        .chain(mcp)
        .collect()
}

/// The agent's categories, from its registered tools, attached addons and configured MCP
/// servers — never live listings, so a server that is down keeps its line.
fn category_summaries(router: &ToolRouter) -> Vec<CategorySummary> {
    summarise_categories(
        router.native_tools().filter_map(|tool| tool.category()),
        router
            .addons
            .iter()
            .filter(|(_, addon)| !addon.toolset.tools.is_empty())
            .map(|(name, addon)| (name, &addon.description)),
        router.mcp.keys(),
        &router.reserved,
    )
}

impl ToolRouter {
    /// Native tools listed under their own category.
    fn native_tools(&self) -> impl Iterator<Item = &VizierToolDef> {
        self.default_toolset
            .tools
            .values()
            .chain(self.user_toolset.tools.values())
            .chain(self.integration_toolset.tools.values())
    }

    /// Addon tools, each with the addon id it is listed under.
    fn addon_tools(&self) -> impl Iterator<Item = (&String, &VizierToolDef)> {
        self.addons
            .iter()
            .flat_map(|(id, addon)| addon.toolset.tools.values().map(move |tool| (id, tool)))
    }

    /// A native or addon tool by its own name.
    fn get_native(&self, name: &str) -> Option<VizierToolDef> {
        self.native_tools()
            .chain(self.addon_tools().map(|(_, tool)| tool))
            .find(|tool| tool.tool_name() == name)
            .cloned()
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
        entries.extend(self.addon_tools().map(|(addon, tool)| CatalogueEntry {
            name: python_identifier(&tool.tool_name()),
            category: addon.clone(),
            target: ToolTarget::Native(tool.tool_name()),
            definition: tool.tool_def(),
        }));

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

        let native: Vec<String> = self
            .native_tools()
            .chain(self.addon_tools().map(|(_, tool)| tool))
            .map(|tool| tool.tool_name())
            .collect();
        let native: Vec<&str> = native.iter().map(String::as_str).collect();
        let pairs: Vec<(&str, &str)> = mcp
            .iter()
            .map(|(server, tool)| (server.as_str(), tool.name.as_str()))
            .collect();
        let names = assign_names(&native, &pairs);

        entries.extend(mcp.into_iter().zip(names).map(|((server, definition), name)| {
            CatalogueEntry {
                name,
                category: mcp_category_name(&server, &self.reserved),
                target: ToolTarget::Mcp {
                    server,
                    tool: definition.name.clone(),
                },
                definition,
            }
        }));

        // Built-ins in declaration order, then addons, then MCP servers.
        let rank = |entry: &CatalogueEntry| match entry.target {
            ToolTarget::Native(_) => ToolCategory::ALL
                .iter()
                .position(|category| category.name() == entry.category)
                .unwrap_or(ToolCategory::ALL.len()),
            ToolTarget::Mcp { .. } => ToolCategory::ALL.len() + 1,
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
        let span = execute_tool_span(&function_name, &params, None);
        let result = self.dispatch(function_name, params, ctx).instrument(span.clone()).await;
        record_tool_result(&span, &result);
        result
    }

    async fn dispatch(
        &self,
        function_name: String,
        params: String,
        ctx: &ToolContext,
    ) -> Result<VizierResponse> {
        if let Some(tool) = self.get_native(&function_name) {
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
                    .get_native(&tool)
                    .ok_or_else(|| VizierError(format!("{tool} does not exists")))?;
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

        // Delegate to default_toolset for memory, workspace, scheduler, skill tools. The dream
        // loop already opened this call's span, so the router's own is skipped.
        self.router.dispatch(function_name, params, ctx).await
    }
}

/// Everything about an agent's tools that is not an integration: built once per instance, and
/// expensive (the shell container, STT/TTS clients, the indexer). Integration changes compose a
/// new [`VizierTools`] over it without rebuilding it (research R2).
pub struct BaseTools {
    pub default_toolset: VizierToolSet,
    pub user_toolset: VizierToolSet,
    limits: crate::sandbox::SandboxLimits,
    timeout: String,
    reserved: Arc<Vec<String>>,
}

impl BaseTools {
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

        user_toolset = user_toolset
            .tool(SendWebuiMessage {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            })
            .tool(ListWebuiTopics {
                agent_id: agent_id.clone(),
                storage: deps.storage.clone(),
            });

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

        let limits = crate::sandbox::SandboxLimits {
            timeout: *agent_config.tools.timeout,
            tools_enabled: true,
        };

        Ok(Self {
            default_toolset,
            user_toolset,
            limits,
            timeout: agent_config.tools.timeout.to_string(),
            reserved: Arc::new(deps.integration_kinds.addon_ids()),
        })
    }
}

#[cfg(test)]
impl BaseTools {
    /// No tools at all, for tests of the integration layer.
    pub fn empty_for_tests(reserved: Vec<String>) -> Self {
        Self {
            default_toolset: VizierToolSet::new(),
            user_toolset: VizierToolSet::new(),
            limits: crate::sandbox::SandboxLimits {
                timeout: std::time::Duration::from_secs(30),
                tools_enabled: true,
            },
            timeout: "30s".into(),
            reserved: Arc::new(reserved),
        }
    }
}

/// The agent's tools: `base` plus the integration layer, with the sandbox tools and the
/// category list built over the result. Cheap (`Arc` clones and one category pass), so an
/// integration change composes a new snapshot instead of restarting the agent.
pub fn compose(base: &BaseTools, integrations: Arc<crate::integrations::IntegrationTools>) -> VizierTools {
    let mut integration_toolset = integrations.native.clone();
    integration_toolset
        .tools
        .extend(integrations.self_management.tools.clone());
    let router = ToolRouter {
        default_toolset: base.default_toolset.clone(),
        user_toolset: base.user_toolset.clone(),
        integration_toolset,
        addons: integrations.addons.clone(),
        mcp: integrations.mcp.clone(),
        reserved: base.reserved.clone(),
    };
    let categories = category_summaries(&router);

    let sandbox_toolset = VizierToolSet::new()
        .tool(ThinkTool)
        .tool(ExecutePython::new(router.clone(), base.limits, &base.timeout))
        .tool(ListToolFunctions::new(router.clone(), categories.clone()))
        .tool(DescribeToolFunction::new(router.clone()));

    VizierTools {
        router,
        sandbox_toolset,
        categories,
        integrations,
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
        // FR-033: no vision model of its own, so images go to the main model as
        // attachments. `llm::prepare` replaces them with a note for a model that can't see
        // images, and agent health warns about that case.
        _ => return None,
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
        brave_search::{BraveSearch, NewsOnlySearch, WebOnlySearch},
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
            entry::<self_integrations::ListIntegrationKinds>(),
            entry::<self_integrations::ListIntegrations>(),
            entry::<self_integrations::CreateIntegration>(),
            entry::<self_integrations::UpdateIntegration>(),
            entry::<self_integrations::DeleteIntegration>(),
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
            ("list_integration_kinds", Integrations),
            ("list_integrations", Integrations),
            ("create_integration", Integrations),
            ("update_integration", Integrations),
            ("delete_integration", Integrations),
        ];
        // Listed under their addon's id rather than a built-in category (research R12).
        const ADDON: &[(&str, &str)] = &[
            ("brave_search_web", "brave_search"),
            ("brave_search_news", "brave_search"),
        ];
        const UNCATEGORISED: &[&str] = &[
            "brave_search_web",
            "brave_search_news",
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

        for (name, addon) in ADDON {
            assert!(name.starts_with(&format!("{addon}_")), "{name} is not namespaced");
            assert!(tools.iter().any(|(n, c)| n == name && c.is_none()));
        }
    }

    #[test]
    fn self_management_tools_never_reach_the_dream_cycle() {
        for name in [
            "list_integration_kinds",
            "list_integrations",
            "create_integration",
            "update_integration",
            "delete_integration",
        ] {
            assert!(!VizierTools::DREAM_TOOL_NAMES.contains(&name), "{name} is a dream tool");
        }
    }

    #[test]
    fn no_integrations_category_without_its_tools() {
        let present = [ToolCategory::Memory, ToolCategory::Shell];
        let names: Vec<String> = summarise_categories(present, [], &[], &[])
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert!(!names.iter().any(|n| n == "integrations"));
        let names: Vec<String> =
            summarise_categories([ToolCategory::Integrations], [], &[], &[])
                .into_iter()
                .map(|c| c.name)
                .collect();
        assert_eq!(names, ["integrations"]);
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
        let (addon, description) = ("brave_search".to_string(), "Search the web".to_string());
        let summaries = summarise_categories(present, [(&addon, &description)], &servers, &[]);
        let names: Vec<&str> = summaries.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "memory", "identity", "scheduling", "agents", "skills", "files", "webui",
                "brave_search", "a", "github", "memory_mcp"
            ]
        );
        assert_eq!(summaries[7].description, "Search the web");
        assert_eq!(summaries[10].description, "Tools from the memory MCP server");
        assert_eq!(
            summaries,
            summarise_categories(present, [(&addon, &description)], &servers, &[])
        );
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
        assert_eq!(mcp_category_name("memory", &[]), "memory_mcp");
        assert_eq!(mcp_category_name("discord", &[]), "discord_mcp");
        assert_eq!(mcp_category_name("github", &[]), "github");
    }

    #[test]
    fn a_server_named_like_an_addon_gets_a_suffix_and_cannot_take_its_tools() {
        let reserved = vec!["brave_search".to_string()];
        assert_eq!(mcp_category_name("brave_search", &reserved), "brave_search_mcp");
        assert_eq!(
            assign_names(&["brave_search_web"], &[("brave_search", "web")]),
            ["brave_search_web_2"]
        );
    }
}
