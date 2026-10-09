use serde::{Deserialize, Serialize};

use crate::{
    agents::{
        background::{PieceSpec, validate_timeout},
        tools::{ToolContext, VizierTool},
    },
    dependencies::VizierDependencies,
    error::VizierError,
    schema::JobKind,
};

pub struct SubtasksTool {
    deps: VizierDependencies,
}

impl SubtasksTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Task {
    prompt: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SubtasksArgs {
    #[schemars(description = "the tasks to run, at least one")]
    tasks: Vec<Task>,
    #[schemars(
        description = "[optional] time limit for each task, in seconds, from 1 to 3600 (default 600)"
    )]
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[async_trait::async_trait]
impl VizierTool for SubtasksTool {
    type Input = SubtasksArgs;
    type Output = String;

    fn name() -> String {
        // Misspelt, and kept: tool names are the dispatch key.
        "paralel_subtasks".to_string()
    }

    fn description(&self) -> String {
        "Run several independent tasks in parallel, in the background. This call returns immediately with a job id; it does NOT return the results. When every task has finished, you will receive one message in this same conversation, marked as a background report, listing each task's result in the order given. Do not wait, poll, or claim the work is done before that report arrives. Tell the person what you have started if they are waiting.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if args.tasks.is_empty() || args.tasks.iter().any(|t| t.prompt.trim().is_empty()) {
            return Err(VizierError("tasks must not be empty".into()));
        }
        let timeout_secs = validate_timeout(args.timeout_secs)?;

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
