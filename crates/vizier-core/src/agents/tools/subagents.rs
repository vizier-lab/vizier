use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::{
        subagents::validate_timeout,
        tools::{ToolCategory, ToolContext, VizierTool},
    },
    dependencies::VizierDependencies,
    error::VizierError,
};

/// One independent task for a subagent run.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SubagentTask {
    pub prompt: String,
}

/// The input of `parallel_subagents`.
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

const PARALLEL_DESCRIPTION: &str = "Run several independent tasks in parallel as subagents and wait for all of them. Returns one result per task, in the order given: {\"results\": [{\"task\", \"state\", \"text\", \"truncated\", \"topic\"}]}, with `state` `answered`, `failed` or `timed_out`. The wait counts against this call's time limit: tasks still running when it is reached are stopped and returned as `timed_out`. For long work, call it inside execute_python with detached=true: the script then runs in the background and its result arrives later.";

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
            .subagents
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
    fn the_description_points_long_work_at_a_detached_script() {
        assert!(PARALLEL_DESCRIPTION.contains("wait"));
        assert!(PARALLEL_DESCRIPTION.contains("detached=true"));
    }
}
