use std::{str::FromStr, sync::Arc};

use chrono::Utc;
use croner::Cron;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::{
    agents::tools::{ToolContext, VizierTool},
    error::VizierError,
    schema::{AgentId, Requester, Task, TaskRunState, TaskSchedule},
    storage::{
        VizierStorage,
        task::TaskStorage,
        task_run::{TaskRunStorage, task_run_response},
    },
};

/// Who wanted the task, from what the agent declares when it schedules one.
///
/// The agent chooses between two variants rather than writing a free string, and there is no
/// account lookup — `requested_by` is the person as the channel they reached the agent on
/// knows them, because most people reaching an agent have no account at all.
fn resolve_requester(
    agent_id: &AgentId,
    on_own_initiative: bool,
    requested_by: Option<String>,
) -> Result<Requester, VizierError> {
    if on_own_initiative {
        return Ok(Requester::Agent(agent_id.clone()));
    }
    match requested_by {
        Some(identity) if !identity.trim().is_empty() => Ok(Requester::User(identity)),
        _ => Err(VizierError(
            "requested_by is required unless on_own_initiative is true".to_string(),
        )),
    }
}

/// `on_own_initiative` / `requested_by`, repeated in both scheduling tools' argument
/// structs because a `#[serde(flatten)]`ed struct renders as an `allOf` in the JSON schema
/// and not every provider accepts one in a tool definition.
const ON_OWN_INITIATIVE_DESC: &str = "Set true when you are setting this task up on your own initiative, with nobody having asked. Set false when someone asked you to.";
const REQUESTED_BY_DESC: &str = "Who asked for this task, as you know them (e.g. '@dani (DiscordId: 182...)'). Required unless on_own_initiative is true; it does not need to be an account.";

pub struct ScheduleOneTimeTask {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ScheduleOneTimeTaskArgs {
    #[schemars(description = "Title of the task")]
    title: String,

    #[schemars(description = "Instruction for the task")]
    instruction: String,

    #[schemars(description = ON_OWN_INITIATIVE_DESC)]
    on_own_initiative: bool,

    #[schemars(description = REQUESTED_BY_DESC)]
    requested_by: Option<String>,

    #[schemars(
        description = "Scheduled utc datetime of the task, in RFC3339 format (e.g., 2024-12-25T10:30:00Z)"
    )]
    schedule: String,

    #[schemars(description = "Optional slug for the task. If not provided, one will be generated from the title")]
    slug: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for ScheduleOneTimeTask {
    type Input = ScheduleOneTimeTaskArgs;
    type Output = String;

    fn name() -> String {
        "schedule_one_time_task".to_string()
    }

    fn description(&self) -> String {
        "Schedule a new one-time task at a specific date and time".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let utc_datetime = chrono::DateTime::parse_from_rfc3339(&args.schedule)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| {
                VizierError(
                    "Invalid datetime format. Use RFC3339 (e.g., 2024-12-25T10:30:00Z)".to_string(),
                )
            })?;

        let now = Utc::now();
        if utc_datetime < now {
            return Err(VizierError(
                "One-time task datetime must be in the future".to_string(),
            ));
        }

        let requester =
            resolve_requester(&self.agent_id, args.on_own_initiative, args.requested_by)?;

        let title = args.title.clone();
        self.storage
            .save_task(Task {
                slug: args.slug.clone().unwrap_or_else(|| slugify!(&args.title.clone())),
                requester,
                agent_id: self.agent_id.clone(),
                title: args.title,
                instruction: args.instruction,
                is_active: true,
                schedule: TaskSchedule::OneTimeTask(utc_datetime),
                last_executed_at: None,
                timestamp: chrono::Utc::now(),
            })
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(format!("Task '{}' scheduled for {}", title, args.schedule))
    }
}

pub struct ScheduleCronTask {
    pub db: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ScheduleCronTaskArgs {
    #[schemars(description = "Title of the task")]
    title: String,

    #[schemars(description = r#"
        Instruction for the recurring task.
        to avoid recursively making a task. avoid mentioning the recurring rule.
        for examples:
        **Don't**: tell a joke every minutes
        **Do**: tell a joke
    "#)]
    instruction: String,

    #[schemars(description = ON_OWN_INITIATIVE_DESC)]
    on_own_initiative: bool,

    #[schemars(description = REQUESTED_BY_DESC)]
    requested_by: Option<String>,

    #[schemars(
        description = "Recurring pattern for the task, following the the standard cron expression"
    )]
    cron: String,

    #[schemars(description = "Optional slug for the task. If not provided, one will be generated from the title")]
    slug: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for ScheduleCronTask {
    type Input = ScheduleCronTaskArgs;
    type Output = String;

    fn name() -> String {
        "schedule_cron_task".to_string()
    }

    fn description(&self) -> String {
        "Schedule a new recurring task using a cron expression".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if args.cron.trim().is_empty() {
            return Err(VizierError("Cron expression cannot be empty".to_string()));
        }

        match Cron::from_str(&args.cron) {
            Ok(_) => {}
            Err(e) => {
                return Err(VizierError(format!("Invalid cron expression: {}", e)));
            }
        }

        let requester =
            resolve_requester(&self.agent_id, args.on_own_initiative, args.requested_by)?;

        let title = args.title.clone();
        let cron = args.cron.clone();
        self.db
            .save_task(Task {
                slug: args.slug.clone().unwrap_or_else(|| slugify!(&args.title.clone())),
                requester,
                agent_id: self.agent_id.clone(),
                title: args.title,
                instruction: args.instruction,
                is_active: true,
                schedule: TaskSchedule::CronTask(args.cron),
                last_executed_at: Some(chrono::Utc::now()),
                timestamp: chrono::Utc::now(),
            })
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(format!("Task '{}' scheduled with cron '{}'", title, cron))
    }
}

pub struct ListTask {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListTaskArgs {
    #[schemars(description = "Filter by active status (optional)")]
    is_active: Option<bool>,
}

#[async_trait::async_trait]
impl VizierTool for ListTask {
    type Input = ListTaskArgs;
    type Output = Vec<Task>;

    fn name() -> String {
        "list_task".to_string()
    }

    fn description(&self) -> String {
        "List all tasks for the agent".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let tasks = self
            .storage
            .get_task_list(Some(self.agent_id.clone()), args.is_active)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(tasks)
    }
}

pub struct DeleteTask {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DeleteTaskArgs {
    #[schemars(description = "Slug of the task to delete")]
    slug: String,
}

#[async_trait::async_trait]
impl VizierTool for DeleteTask {
    type Input = DeleteTaskArgs;
    type Output = String;

    fn name() -> String {
        "delete_task".to_string()
    }

    fn description(&self) -> String {
        "Delete a task by its slug".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let slug = args.slug.clone();
        self.storage
            .delete_task(self.agent_id.clone(), args.slug)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(format!("Task '{}' deleted", slug))
    }
}

pub struct GetTaskDetail {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetTaskDetailArgs {
    #[schemars(description = "Slug of the task to get")]
    slug: String,
}

#[async_trait::async_trait]
impl VizierTool for GetTaskDetail {
    type Input = GetTaskDetailArgs;
    type Output = Option<Task>;

    fn name() -> String {
        "get_task_detail".to_string()
    }

    fn description(&self) -> String {
        "Get details of a specific task by its slug".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let task = self
            .storage
            .get_task(self.agent_id.clone(), args.slug)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(task)
    }
}

/// `list_task_runs` / `get_task_run_detail` bounds.
///
/// The split is the idiom the memory tools already establish — `memory_search` returns
/// addressed passages, `memory_read` returns one whole document by address — and it exists
/// for the same reason here: one call must not be able to spend the whole context.
const DEFAULT_RUN_LIMIT: usize = 10;
const MAX_RUN_LIMIT: usize = 50;
/// An agent reading its own history must not be able to crowd out its current work.
const MAX_RESPONSE_BYTES: usize = 8_000;

pub struct ListTaskRuns {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListTaskRunsArgs {
    #[schemars(description = "Slug of one of your own tasks")]
    slug: String,

    #[schemars(description = "How many runs to return. Defaults to 10, capped at 50.")]
    limit: Option<usize>,

    #[schemars(
        description = "The run_id of the oldest run you have already seen (RFC3339), to page further back"
    )]
    before: Option<String>,
}

/// One run as an agent sees it listed: when it ran and how it ended, never what it said.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskRunSummary {
    /// The run's address, which `get_task_run_detail` takes.
    run_id: chrono::DateTime<Utc>,
    ran_at: chrono::DateTime<Utc>,
    state: TaskRunState,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListTaskRunsOutput {
    runs: Vec<TaskRunSummary>,
    has_more: bool,
}

#[async_trait::async_trait]
impl VizierTool for ListTaskRuns {
    type Input = ListTaskRunsArgs;
    type Output = ListTaskRunsOutput;

    fn name() -> String {
        "list_task_runs".to_string()
    }

    fn description(&self) -> String {
        "List past runs of one of your own tasks, newest first. Returns when each ran and how \
         it ended, not what it said — use `get_task_run_detail` with a run's address to read \
         its report."
            .into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let before = parse_run_id(args.before.as_deref())?;
        let limit = args
            .limit
            .unwrap_or(DEFAULT_RUN_LIMIT)
            .clamp(1, MAX_RUN_LIMIT);

        // One past the page, so `has_more` comes from the same query.
        let mut runs = self
            .storage
            // Scoped to the owning agent, as every other tool in this module is: another
            // agent's task cannot be addressed at all, which makes the scope structural
            // rather than a check that could be forgotten.
            .list_task_runs(self.agent_id.clone(), args.slug, before, None, limit + 1)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let has_more = runs.len() > limit;
        runs.truncate(limit);

        Ok(ListTaskRunsOutput {
            // Every state is listed, `no_response` included: a task that has been failing
            // for a week must be visible as such, not quietly omitted.
            runs: runs
                .iter()
                .map(|run| TaskRunSummary {
                    run_id: run.ran_at,
                    ran_at: run.ran_at,
                    state: run.state,
                })
                .collect(),
            has_more,
        })
    }
}

pub struct GetTaskRunDetail {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetTaskRunDetailArgs {
    #[schemars(description = "Slug of one of your own tasks")]
    slug: String,

    #[schemars(description = "The run's address, as list_task_runs gives it (RFC3339)")]
    run_id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskRunDetail {
    run_id: chrono::DateTime<Utc>,
    ran_at: chrono::DateTime<Utc>,
    state: TaskRunState,
    /// `null` when the run's state is not `answered`.
    response: Option<String>,
    truncated: bool,
}

#[async_trait::async_trait]
impl VizierTool for GetTaskRunDetail {
    type Input = GetTaskRunDetailArgs;
    type Output = TaskRunDetail;

    fn name() -> String {
        "get_task_run_detail".to_string()
    }

    fn description(&self) -> String {
        "Read what one run of your own task reported. Takes a run address from \
         `list_task_runs`. Returns the report only — no reasoning and no tool activity."
            .into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let ran_at = parse_run_id(Some(&args.run_id))?.ok_or_else(|| {
            VizierError("run_id is required, in RFC3339 format".to_string())
        })?;

        let run = self
            .storage
            .get_task_run(self.agent_id.clone(), args.slug.clone(), ran_at)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| {
                VizierError(format!(
                    "no run of task '{}' at {}",
                    args.slug, args.run_id
                ))
            })?;

        // Only the final report. The agent's own past reasoning is the bulk of the text and
        // the least use to it, so the trail is never returned here.
        let response = task_run_response(&self.storage, &run).await;

        let (response, truncated) = match response {
            Some(text) if text.len() > MAX_RESPONSE_BYTES => {
                // On a char boundary, so a multi-byte character cannot be cut in half.
                let mut cut = MAX_RESPONSE_BYTES;
                while cut > 0 && !text.is_char_boundary(cut) {
                    cut -= 1;
                }
                (Some(text[..cut].to_string()), true)
            }
            other => (other, false),
        };

        Ok(TaskRunDetail {
            run_id: run.ran_at,
            ran_at: run.ran_at,
            state: run.state,
            response,
            truncated,
        })
    }
}

/// A run's address as the listing gives it back.
fn parse_run_id(value: Option<&str>) -> Result<Option<chrono::DateTime<Utc>>, VizierError> {
    match value {
        None => Ok(None),
        Some(raw) if raw.trim().is_empty() => Ok(None),
        Some(raw) => chrono::DateTime::parse_from_rfc3339(raw)
            .map(|dt| Some(dt.with_timezone(&Utc)))
            .map_err(|_| {
                VizierError(format!(
                    "'{raw}' is not an RFC3339 timestamp; use a run_id from list_task_runs"
                ))
            }),
    }
}
