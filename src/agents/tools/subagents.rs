use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::{
        background::{PieceSpec, validate_timeout},
        tools::{ToolCategory, ToolContext, VizierTool},
    },
    dependencies::VizierDependencies,
    error::VizierError,
    schema::JobKind,
};

/// One independent task for a subagent run.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SubagentTask {
    pub prompt: String,
}

/// The input shared by the blocking and background subagent tools.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SubagentsArgs {
    #[schemars(description = "the tasks to run, at least one")]
    pub tasks: Vec<SubagentTask>,
    #[schemars(
        description = "[optional] time limit for each task, in seconds, from 1 to 3600 (default 600)"
    )]
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Validate the shared input and return the per-task timeout in seconds.
fn validate(args: &SubagentsArgs) -> crate::Result<u64> {
    if args.tasks.is_empty() || args.tasks.iter().any(|task| task.prompt.trim().is_empty()) {
        return Err(VizierError("tasks must not be empty".into()));
    }
    validate_timeout(args.timeout_secs)
}

const BACKGROUND_DESCRIPTION: &str = "Run several independent tasks in parallel, in the background. This call returns immediately with a job id; it does NOT return the results. When every task has finished, you will receive one message in this same conversation, marked as a background report, listing each task's result in the order given. Do not wait, poll, or claim the work is done before that report arrives. Tell the person what you have started if they are waiting.";

const PARALLEL_DESCRIPTION: &str = "Run several independent tasks in parallel as subagents and wait for all of them. Returns one result per task, in the order given: {\"results\": [{\"task\", \"state\", \"text\", \"truncated\", \"topic\"}]}, with `state` `answered`, `failed` or `timed_out`. Use it when you need the results for your current reply and the tasks are quick. The wait counts against this tool call's time limit: tasks still running when it is reached are stopped and returned as `timed_out`. For long work, use `background_subagents` instead.";

/// The non-blocking fan-out tool. It keeps the behaviour of the old batch tool under
/// its new name.
pub struct BackgroundSubagentsTool {
    deps: VizierDependencies,
}

impl BackgroundSubagentsTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for BackgroundSubagentsTool {
    type Input = SubagentsArgs;
    type Output = String;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Agents)
    }

    fn name() -> String {
        "background_subagents".to_string()
    }

    fn description(&self) -> String {
        BACKGROUND_DESCRIPTION.into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let timeout_secs = validate(&args)?;

        let count = args.tasks.len();
        let pieces = args
            .tasks
            .into_iter()
            .map(|task| PieceSpec {
                executor_agent: ctx.session.0.clone(),
                prompt: task.prompt,
            })
            .collect();

        let job = self
            .deps
            .background_jobs
            .launch(ctx, JobKind::Batch, pieces, timeout_secs)
            .await?;

        Ok(format!(
            "Started background batch {} with {} task{}. Results will arrive as a background report in this conversation.",
            job.id,
            count,
            if count == 1 { "" } else { "s" }
        ))
    }
}

/// The blocking fan-out tool: it waits for all pieces and returns their results in the
/// same turn.
pub struct ParallelSubagentsTool {
    deps: VizierDependencies,
}

impl ParallelSubagentsTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for ParallelSubagentsTool {
    type Input = SubagentsArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Agents)
    }

    fn name() -> String {
        "parallel_subagents".to_string()
    }

    fn description(&self) -> String {
        PARALLEL_DESCRIPTION.into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let timeout_secs = validate(&args)?;
        let prompts = args.tasks.into_iter().map(|task| task.prompt).collect();

        let results = self
            .deps
            .background_jobs
            .run_and_wait(ctx, prompts, Duration::from_secs(timeout_secs))
            .await?;

        Ok(serde_json::json!({ "results": results }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(tasks: &[&str], timeout_secs: Option<u64>) -> SubagentsArgs {
        SubagentsArgs {
            tasks: tasks
                .iter()
                .map(|prompt| SubagentTask {
                    prompt: (*prompt).to_string(),
                })
                .collect(),
            timeout_secs,
        }
    }

    #[test]
    fn an_empty_task_list_is_refused() {
        let error = validate(&args(&[], None)).unwrap_err();
        assert_eq!(error.0, "tasks must not be empty");
    }

    #[test]
    fn a_blank_prompt_is_refused() {
        let error = validate(&args(&["  \n\t"], None)).unwrap_err();
        assert_eq!(error.0, "tasks must not be empty");
    }

    #[test]
    fn an_out_of_range_timeout_is_refused() {
        let error = validate(&args(&["hello"], Some(0))).unwrap_err();
        assert_eq!(error.0, "timeout_secs must be between 1 and 3600");
        let error = validate(&args(&["hello"], Some(3601))).unwrap_err();
        assert_eq!(error.0, "timeout_secs must be between 1 and 3600");
    }

    #[test]
    fn the_default_timeout_is_used_when_omitted() {
        assert_eq!(validate(&args(&["hello"], None)).unwrap(), 600);
        assert_eq!(validate(&args(&["hello"], Some(3))).unwrap(), 3);
    }

    #[test]
    fn the_descriptions_distinguish_waiting_from_background_work() {
        assert!(PARALLEL_DESCRIPTION.contains("background_subagents"));
        assert!(PARALLEL_DESCRIPTION.contains("wait"));
        assert!(BACKGROUND_DESCRIPTION.contains("returns immediately"));
    }
}
