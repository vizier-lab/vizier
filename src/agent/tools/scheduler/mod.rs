use std::sync::Arc;

use chrono::Utc;
use rig::{completion::ToolDefinition, tool::Tool};
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::{
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

    #[schemars(description = "Scheduled utc datetime of the task, in timestamp secs format")]
    schedule: DateTime,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Clone)]
struct DateTime {
    #[schemars(description = "year")]
    year: i32,
    #[schemars(description = "month")]
    month: u32,
    #[schemars(description = "day")]
    day: u32,
    #[schemars(description = "hour")]
    hour: u32,
    #[schemars(description = "minute")]
    minute: u32,
    #[schemars(description = "second")]
    second: u32,
}

impl DateTime {
    /// Converts to an Optional NaiveDateTime (returns None if input is invalid)
    pub fn to_chrono(&self) -> Option<chrono::NaiveDateTime> {
        let date = chrono::NaiveDate::from_ymd_opt(self.year, self.month, self.day)?;
        let time = chrono::NaiveTime::from_hms_opt(self.hour, self.minute, self.second)?;

        Some(chrono::NaiveDateTime::new(date, time))
    }

    /// Converts to a UTC DateTime
    pub fn to_utc(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.to_chrono()
            .map(|naive| chrono::DateTime::from_naive_utc_and_offset(naive, Utc))
    }
}

impl Tool for ScheduleOneTimeTask
where
    Self: Sync + Send,
{
    const NAME: &'static str = "schedule_one_time_task";
    type Error = VizierError;
    type Args = ScheduleOneTimeTaskArgs;
    type Output = ();

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!("Schedule a new one-time task at a specific date and time"),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        log::info!("schedule_task {} {:?}", args.title, args.schedule.clone());

        let response = self
            .storage
            .save_task(Task {
                slug: slugify!(&args.title.clone()),
                user: args.user,
                agent_id: self.agent_id.clone(),
                title: args.title,
                instruction: args.instruction,
                is_active: true,
                schedule: TaskSchedule::OneTimeTask(args.schedule.to_utc().unwrap()),
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

impl Tool for ScheduleCronTask
where
    Self: Sync + Send,
{
    const NAME: &'static str = "schedule_cron_task";
    type Error = VizierError;
    type Args = ScheduleCronTaskArgs;
    type Output = ();

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!("Schedule a new recurring task using a cron expression"),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        log::info!(
            "schedule_task {} {} {:?}",
            args.title,
            args.instruction,
            args.cron.clone()
        );

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
