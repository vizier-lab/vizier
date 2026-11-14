//! The `subagents` tools (spec 059, contracts/subagent-tools.md): the main agent hires
//! temporary workers, waits for them, answers their questions and stops them; a worker asks its
//! main agent with `ask_main_agent`. Every refusal is a `status` result, never an exception.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::{
        subagents::{ASK_TOOL, Until, is_worker, may_hire, validate_timeout},
        tools::{ToolCategory, ToolContext, VizierTool},
    },
    dependencies::VizierDependencies,
    error::VizierError,
    schema::{Subagent, SubagentState},
};

fn summaries(workers: &[Subagent], full: bool) -> Vec<serde_json::Value> {
    workers
        .iter()
        .map(|w| serde_json::to_value(w.summary(full)).unwrap_or_default())
        .collect()
}

fn summary(worker: &Subagent, full: bool) -> serde_json::Value {
    serde_json::to_value(worker.summary(full)).unwrap_or_default()
}

/// `invalid_input` as data, so a script can branch on it.
fn invalid(err: VizierError) -> serde_json::Value {
    serde_json::json!({ "status": "invalid_input", "message": err.0 })
}

/// Shared by the six management tools: offered only below `subagents.max_depth` (FR-018).
fn offered_to_hirer(deps: &VizierDependencies, ctx: &ToolContext) -> bool {
    may_hire(ctx, deps.config.subagents.max_depth)
}

// --- spawn_subagents -------------------------------------------------------------------------

/// One task for one worker.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SpawnTask {
    #[schemars(description = "everything the worker needs: the goal, the context you already have, and the shape of the answer you want. The worker knows nothing else")]
    pub task: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SpawnArgs {
    #[schemars(description = "the tasks, one worker each, at least one")]
    pub tasks: Vec<SpawnTask>,
    #[schemars(description = "[optional] each worker's time limit in seconds, 1 to 3600 (default 600); time spent waiting for your reply does not count")]
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

pub struct SpawnSubagentsTool {
    deps: VizierDependencies,
}

impl SpawnSubagentsTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

const SPAWN_DESCRIPTION: &str = "Hire temporary workers, one per task, and return their handles at once without waiting: {\"group\", \"subagents\": [{\"id\", \"position\", \"state\": \"running\", ...}]} in the order given. A worker is you, working on one task: it acts with your tools and memory, cannot talk to the person, and reports back to you. It can ask you a question (it then reads `asking`); answer with reply_subagent. Workers keep running when this turn ends; results you have not waited for arrive later as a `[Subagent update]` message. Use wait_subagents to collect them.";

#[async_trait::async_trait]
impl VizierTool for SpawnSubagentsTool {
    type Input = SpawnArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Subagents)
    }

    fn name() -> String {
        "spawn_subagents".to_string()
    }

    fn description(&self) -> String {
        SPAWN_DESCRIPTION.into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_hirer(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let timeout_secs = match validate_timeout(args.timeout_secs) {
            Ok(secs) => secs,
            Err(e) => return Ok(invalid(e)),
        };
        let tasks = args.tasks.into_iter().map(|t| t.task).collect();
        match self.deps.subagents.spawn(ctx, tasks, timeout_secs).await {
            Ok(workers) => Ok(serde_json::json!({
                "group": workers.first().map(|w| w.group_id.clone()),
                "subagents": summaries(&workers, false),
            })),
            Err(e) if e.0.contains("must not be empty") || e.0.contains("the limit is") => Ok(invalid(e)),
            Err(e) => Err(e),
        }
    }
}

// --- wait_subagents --------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaitUntil {
    Any,
    #[default]
    All,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct WaitArgs {
    #[schemars(description = "the workers to wait for")]
    pub ids: Vec<String>,
    #[schemars(description = "[optional] `all` (default): until every one has settled; `any`: until one has. A worker that is asking you counts as settled")]
    #[serde(default)]
    pub until: WaitUntil,
}

pub struct WaitSubagentsTool {
    deps: VizierDependencies,
}

impl WaitSubagentsTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for WaitSubagentsTool {
    type Input = WaitArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Subagents)
    }

    fn name() -> String {
        "wait_subagents".to_string()
    }

    fn description(&self) -> String {
        "Wait until one (`until: \"any\"`) or every (`until: \"all\"`, the default) given worker has settled: answered, failed, timed out, stopped, or asking you a question. Returns {\"met\", \"subagents\": [...]}; `met` is false when this call's time ran out first, and the workers keep running. Answer an asking worker with reply_subagent, then wait again. What this returns is not delivered to you again later.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_hirer(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if args.ids.is_empty() {
            return Ok(invalid(VizierError("ids must not be empty".into())));
        }
        let until = match args.until {
            WaitUntil::Any => Until::Any,
            WaitUntil::All => Until::All,
        };
        Ok(match self.deps.subagents.wait(ctx, &args.ids, until).await? {
            Ok(waited) => serde_json::json!({
                "met": waited.met,
                "subagents": summaries(&waited.workers, false),
            }),
            Err(refusal) => refusal.to_json(),
        })
    }
}

// --- reply_subagent --------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReplyArgs {
    #[schemars(description = "the worker that asked")]
    pub id: String,
    #[schemars(description = "your answer to its question")]
    pub text: String,
}

pub struct ReplySubagentTool {
    deps: VizierDependencies,
}

impl ReplySubagentTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for ReplySubagentTool {
    type Input = ReplyArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Subagents)
    }

    fn name() -> String {
        "reply_subagent".to_string()
    }

    fn description(&self) -> String {
        "Answer a worker that is asking you a question. It continues from where it stopped, in its own conversation. Returns {\"subagent\"} (now running); refused with status `not_asking` when it is not asking.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_hirer(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        match self.deps.subagents.reply(ctx, &args.id, &args.text).await {
            Ok(Ok(worker)) => Ok(serde_json::json!({ "subagent": summary(&worker, false) })),
            Ok(Err(refusal)) => Ok(refusal.to_json()),
            Err(e) if e.0.contains("must not be empty") || e.0.contains("the limit is") => Ok(invalid(e)),
            Err(e) => Err(e),
        }
    }
}

// --- stop_subagent ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct WorkerArgs {
    #[schemars(description = "the worker's id (`s_…`)")]
    pub id: String,
}

pub struct StopSubagentTool {
    deps: VizierDependencies,
}

impl StopSubagentTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for StopSubagentTool {
    type Input = WorkerArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Subagents)
    }

    fn name() -> String {
        "stop_subagent".to_string()
    }

    fn description(&self) -> String {
        "Stop one running or asking worker; the others keep going. Returns {\"subagent\"} (stopped). A worker that already finished is returned as it is with status `already_finished`. A stopped worker is never delivered to you.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_hirer(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(match self.deps.subagents.stop(ctx, &args.id).await? {
            Ok((worker, true)) => serde_json::json!({ "subagent": summary(&worker, false) }),
            Ok((worker, false)) => serde_json::json!({
                "status": "already_finished",
                "subagent": summary(&worker, false),
            }),
            Err(refusal) => refusal.to_json(),
        })
    }
}

// --- list_subagents / get_subagent -----------------------------------------------------------

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct ListArgs {}

pub struct ListSubagentsTool {
    deps: VizierDependencies,
}

impl ListSubagentsTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for ListSubagentsTool {
    type Input = ListArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Subagents)
    }

    fn name() -> String {
        "list_subagents".to_string()
    }

    fn description(&self) -> String {
        "The workers you hired in this conversation: running and asking first, then the last 50 finished ones. Returns {\"subagents\": [...]}. Listing does not count as reading a result.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_hirer(&self.deps, ctx)
    }

    async fn call(&self, _args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let workers = self.deps.subagents.list(ctx).await?;
        Ok(serde_json::json!({ "subagents": summaries(&workers, false) }))
    }
}

pub struct GetSubagentTool {
    deps: VizierDependencies,
}

impl GetSubagentTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for GetSubagentTool {
    type Input = WorkerArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Subagents)
    }

    fn name() -> String {
        "get_subagent".to_string()
    }

    fn description(&self) -> String {
        "One worker with its whole task and result: {\"subagent\"}. Its open question or result counts as read, so it is not delivered to you again.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_hirer(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(match self.deps.subagents.get(ctx, &args.id).await? {
            Ok(worker) => serde_json::json!({ "subagent": summary(&worker, true) }),
            Err(refusal) => refusal.to_json(),
        })
    }
}

// --- ask_main_agent (workers only) -----------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AskArgs {
    #[schemars(description = "one clear question only your main agent can answer")]
    pub question: String,
}

pub struct AskMainAgentTool {
    deps: VizierDependencies,
}

impl AskMainAgentTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for AskMainAgentTool {
    type Input = AskArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Subagents)
    }

    fn name() -> String {
        ASK_TOOL.to_string()
    }

    fn description(&self) -> String {
        "Ask the agent you work for one question when you cannot go on without its decision. Your step ends here; its answer arrives as your next message and you continue from where you stopped. Do not ask what you can reasonably decide yourself.".into()
    }

    /// Offered only in a worker's own conversation.
    fn available(&self, ctx: &ToolContext) -> bool {
        is_worker(ctx)
    }

    fn ends_turn() -> bool {
        true
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        // A refusal is an error here, not a status: a successful call ends the turn, and a
        // question that was not recorded must not.
        let asked = self.deps.subagents.ask(ctx, &args.question).await?;
        debug_assert_eq!(asked.state, SubagentState::Asking);
        Ok(serde_json::json!({ "status": "asked" }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spawn_description_tells_the_agent_what_a_worker_is() {
        assert!(SPAWN_DESCRIPTION.contains("without waiting"));
        assert!(SPAWN_DESCRIPTION.contains("reply_subagent"));
        assert!(SPAWN_DESCRIPTION.contains("[Subagent update]"));
        assert!(!SPAWN_DESCRIPTION.contains("parallel_subagents"));
    }

    #[test]
    fn wait_defaults_to_all() {
        let args: WaitArgs = serde_json::from_value(serde_json::json!({"ids": ["s_1"]})).unwrap();
        assert!(matches!(args.until, WaitUntil::All));
        let args: WaitArgs =
            serde_json::from_value(serde_json::json!({"ids": ["s_1"], "until": "any"})).unwrap();
        assert!(matches!(args.until, WaitUntil::Any));
    }

    #[test]
    fn only_ask_main_agent_ends_the_turn() {
        assert!(<AskMainAgentTool as VizierTool>::ends_turn());
        assert!(!<WaitSubagentsTool as VizierTool>::ends_turn());
        assert!(!<SpawnSubagentsTool as VizierTool>::ends_turn());
    }
}
