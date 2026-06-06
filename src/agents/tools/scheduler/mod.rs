use std::{str::FromStr, sync::Arc};

use chrono::Utc;
use croner::Cron;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::{
    agents::tools::VizierTool,
    error::VizierError,
    schema::{AgentId, Task, TaskSchedule},
    storage::{VizierStorage, task::TaskStorage},
};

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

    #[schemars(description = "user who request the task")]
    user: String,

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

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
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

        let title = args.title.clone();
        self.storage
            .save_task(Task {
                slug: args.slug.clone().unwrap_or_else(|| slugify!(&args.title.clone())),
                user: args.user,
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

    #[schemars(description = "User who request the task")]
    user: String,

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

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        if args.cron.trim().is_empty() {
            return Err(VizierError("Cron expression cannot be empty".to_string()));
        }

        match Cron::from_str(&args.cron) {
            Ok(_) => {}
            Err(e) => {
                return Err(VizierError(format!("Invalid cron expression: {}", e)));
            }
        }

        let title = args.title.clone();
        let cron = args.cron.clone();
        self.db
            .save_task(Task {
                slug: args.slug.clone().unwrap_or_else(|| slugify!(&args.title.clone())),
                user: args.user,
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

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
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

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
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

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let task = self
            .storage
            .get_task(self.agent_id.clone(), args.slug)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(task)
    }
}
