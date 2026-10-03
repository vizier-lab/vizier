use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use anyhow::Result;
use chrono::{DateTime, Utc};
use rig_core::completion::ToolDefinition;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

use crate::{
    agents::mcp::{VizierMcp, VizierMcpClient},
    agents::tools::{
        brave_search::{BraveSearch, NewsOnlySearch, WebOnlySearch},
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
    schema::{AgentId, VizierAttachment, VizierResponse, VizierSession},
    storage::{VizierStorage, agent::AgentStorage},
    utils::agent_workspace,
};

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

#[derive(Clone)]
pub struct ToolContext {
    pub session: VizierSession,
    pub pending_attachments: Arc<Mutex<Vec<VizierAttachment>>>,
    /// The session's hooks, so tools that dispatch other tools (the Python
    /// sandbox bridge) run them through the same hook pipeline as direct calls.
    pub hooks: Option<Arc<crate::agents::hook::VizierSessionHooks>>,
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

/// How an agent's tools are offered to the model, derived from `tools.python`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolExposure {
    /// Sandbox off: regular tools only.
    Direct,
    /// Sandbox on, code mode off: regular tools plus `execute_python`.
    SandboxAdditive,
    /// Code mode on: only `think`, `execute_python` and the docs tools are direct;
    /// every other tool is reachable from scripts only.
    CodeModeExclusive,
}

impl ToolExposure {
    fn from_config(agent_id: &AgentId, python: &crate::schema::PythonSandboxConfig) -> Self {
        match (python.enabled, python.code_mode) {
            (false, false) => Self::Direct,
            (false, true) => {
                tracing::warn!(
                    "agent {agent_id} has tools.python.code_mode without tools.python.enabled; treating code mode as off"
                );
                Self::SandboxAdditive
            }
            (true, false) => Self::SandboxAdditive,
            (true, true) => Self::CodeModeExclusive,
        }
    }
}

/// Name-based dispatch over the agent's regular tools. Shared by direct calls
/// (`VizierTools::call`) and calls made from Python scripts.
#[derive(Clone)]
pub struct ToolRouter {
    pub default_toolset: VizierToolSet,
    pub user_toolset: VizierToolSet,
    pub mcp: HashMap<String, Arc<VizierMcp>>,
}

#[derive(Clone)]
pub struct VizierTools {
    pub router: ToolRouter,
    pub sandbox_toolset: VizierToolSet,
    pub exposure: ToolExposure,
}

#[async_trait::async_trait]
pub trait VizierToolDyn {
    fn tool_name(&self) -> String;

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
    })
}

/// Tools the model may call directly while code mode is on.
const CODE_MODE_DIRECT_TOOLS: &[&str] = &[
    "think",
    "execute_python",
    "list_tool_functions",
    "describe_tool_function",
];

/// Whether `name` is offered to (and callable by) the model under `exposure`.
fn exposed_directly(exposure: ToolExposure, name: &str) -> bool {
    match exposure {
        ToolExposure::Direct | ToolExposure::SandboxAdditive => true,
        ToolExposure::CodeModeExclusive => CODE_MODE_DIRECT_TOOLS.contains(&name),
    }
}

impl ToolRouter {
    pub async fn definitions(&self) -> Result<Vec<ToolDefinition>> {
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

    pub async fn call(
        &self,
        function_name: String,
        params: String,
        ctx: &ToolContext,
    ) -> Result<VizierResponse> {
        // mcp calls
        if function_name.starts_with("mcp_") {
            if let Some((server, function_name)) = function_name.split_once("__") {
                let server = server.replace("mcp_", "");

                let res = self
                    .mcp
                    .get(&server)
                    .ok_or(VizierError("mcp not found".into()))?
                    .call(function_name.to_string(), serde_json::from_str(&params)?)
                    .await?;

                return Ok(res);
            }
        }

        if let Ok(tool) = self.default_toolset.get_tool(function_name.clone()) {
            let output = tool.tool_call(params.clone(), ctx).await?;
            return tool_output_to_response(&output);
        }

        if let Ok(tool) = self.user_toolset.get_tool(function_name.clone()) {
            let output = tool.tool_call(params.clone(), ctx).await?;
            return tool_output_to_response(&output);
        }

        Err(VizierError(format!("{} not found", function_name)).into())
    }
}

impl VizierTools {
    pub async fn tools(&self) -> Result<Vec<ToolDefinition>> {
        let mut defs = match self.exposure {
            ToolExposure::CodeModeExclusive => self
                .router
                .default_toolset
                .tools
                .values()
                .chain(self.router.user_toolset.tools.values())
                .filter(|tool| exposed_directly(self.exposure, &tool.tool_name()))
                .map(|tool| tool.tool_def())
                .collect(),
            _ => self.router.definitions().await?,
        };
        defs.extend(self.sandbox_toolset.tools.values().map(|tool| tool.tool_def()));
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
        if let Ok(tool) = self.sandbox_toolset.get_tool(function_name.clone()) {
            let output = tool.tool_call(params, ctx).await?;
            return tool_output_to_response(&output);
        }

        if !exposed_directly(self.exposure, &function_name) {
            return Err(VizierError(format!(
                "{function_name} is not exposed directly while code mode is on; call it from a script via execute_python (see list_tool_functions)"
            ))
            .into());
        }

        self.router.call(function_name, params, ctx).await
    }

    const DREAM_TOOL_NAMES: &'static [&'static str] = &[
        // Memory (8)
        "memory_read",
        "memory_write",
        "memory_list",
        "memory_detail",
        "memory_follow",
        "memory_graph",
        "memory_delete",
        "memory_delete_bundle",
        // Workspace (2)
        "WRITE_CORE",
        "READ_CORE",
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
            .tool(ThinkTool)
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
            .tool(DelegateAgent::new(
                agent_id.clone(),
                other_agents.clone(),
                deps.transport.clone(),
            ))
            .tool(SubtasksTool::new(agent_id.clone(), deps.clone()))
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

        let mut mcp = HashMap::new();
        for (name, mcp_config) in &agent_config.tools.mcp_servers {
            match mcp_config.to_client().await {
                Ok(client) => {
                    mcp.insert(name.clone(), Arc::new(client));
                }
                Err(e) => {
                    tracing::error!(
                        "failed to create MCP client '{}' for agent {}: {}",
                        name,
                        agent_id,
                        e
                    );
                }
            }
        }

        let router = ToolRouter {
            default_toolset,
            user_toolset,
            mcp,
        };
        let exposure = ToolExposure::from_config(&agent_id, &agent_config.tools.python);

        let mut sandbox_toolset = VizierToolSet::new();
        if exposure != ToolExposure::Direct {
            let code_mode = exposure == ToolExposure::CodeModeExclusive;
            let limits = crate::sandbox::SandboxLimits {
                timeout: *agent_config.tools.timeout,
                tools_enabled: code_mode,
            };
            sandbox_toolset = sandbox_toolset.tool(ExecutePython::new(
                router.clone(),
                limits,
                &agent_config.tools.timeout.to_string(),
            ));
            if code_mode {
                sandbox_toolset = sandbox_toolset
                    .tool(ListToolFunctions::new(router.clone()))
                    .tool(DescribeToolFunction::new(router.clone()));
            }
        }

        Ok(Self {
            router,
            sandbox_toolset,
            exposure,
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

    #[test]
    fn code_mode_exposes_only_the_sandbox_tools_directly() {
        for exposure in [ToolExposure::Direct, ToolExposure::SandboxAdditive] {
            assert!(exposed_directly(exposure, "memory_read"));
            assert!(exposed_directly(exposure, "mcp_gh__create_issue"));
        }
        for name in ["think", "execute_python", "list_tool_functions", "describe_tool_function"] {
            assert!(exposed_directly(ToolExposure::CodeModeExclusive, name));
        }
        for name in ["memory_read", "READ_CORE", "mcp_gh__create_issue"] {
            assert!(!exposed_directly(ToolExposure::CodeModeExclusive, name));
        }
    }
}
