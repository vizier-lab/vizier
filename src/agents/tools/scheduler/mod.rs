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
}

#[async_trait::async_trait]
impl VizierTool for ScheduleOneTimeTask {
    type Input = ScheduleOneTimeTaskArgs;
    type Output = ();

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

        let response = self
            .storage
            .save_task(Task {
                slug: slugify!(&args.title.clone()),
                user: args.user,
                agent_id: self.agent_id.clone(),
                title: args.title,
                instruction: args.instruction,
                is_active: true,
                schedule: TaskSchedule::OneTimeTask(utc_datetime),
                last_executed_at: None,
                timestamp: chrono::Utc::now(),
            })
            .await;

        if let Err(err) = response {
            return Err(VizierError(err.to_string()));
        }

        Ok(())
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
}

#[async_trait::async_trait]
impl VizierTool for ScheduleCronTask {
    type Input = ScheduleCronTaskArgs;
    type Output = ();

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

        let response = self
            .db
            .save_task(Task {
                slug: slugify!(&args.title.clone()),
                user: args.user,
                agent_id: self.agent_id.clone(),
                title: args.title,
                instruction: args.instruction,
                is_active: true,
                schedule: TaskSchedule::CronTask(args.cron),
                last_executed_at: Some(chrono::Utc::now()),
                timestamp: chrono::Utc::now(),
            })
            .await;

        if let Err(err) = response {
            return Err(VizierError(err.to_string()));
        }

        Ok(())
    }
}
