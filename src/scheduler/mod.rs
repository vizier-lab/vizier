use std::{collections::HashMap, str::FromStr, time::Duration};

use anyhow::Result;
use chrono::{DateTime, Utc};
use croner::Cron;

use crate::{
    dependencies::VizierDependencies,
    schema::{Task, TaskSchedule, VizierChannelId, VizierRequest, VizierSession},
    storage::task::TaskStorage,
};

pub struct VizierScheduler {
    deps: VizierDependencies,
}

impl VizierScheduler {
    pub async fn new(deps: VizierDependencies) -> Result<VizierScheduler> {
        Ok(VizierScheduler { deps })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut schedules: HashMap<(DateTime<Utc>, String), Task> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let now = Utc::now();

            // fetch tasks
            let tasks = self.deps.storage.get_task_list(None, Some(true)).await?;

            for task in tasks.iter() {
                match &task.schedule {
                    TaskSchedule::OneTimeTask(schedule) => {
                        schedules.insert((*schedule, task.slug.clone()), task.clone())
                    }
                    TaskSchedule::CronTask(cron) => {
                        let cron = Cron::from_str(&cron).unwrap();
                        let schedule =
                            cron.find_next_occurrence(&task.last_executed_at.unwrap_or(now), true)?;

                        schedules.insert((schedule, task.slug.clone()), task.clone())
                    }
                };
            }

            // handle task
            let mut to_be_run = vec![];
            let lookup = schedules.clone();
            for ((schedule, slug), _) in &lookup {
                if *schedule <= now {
                    if let Some(task) = schedules.remove(&(*schedule, slug.clone())) {
                        to_be_run.push(task);
                    }
                }
            }

            for task in to_be_run {
                if let &TaskSchedule::OneTimeTask(_) = &task.schedule {
                    let _ = self
                        .deps
                        .storage
                        .delete_task(task.agent_id.clone(), task.slug.clone())
                        .await;
                }

                if let &TaskSchedule::CronTask(_) = &task.schedule {
                    let mut updated_task = task.clone();
                    updated_task.last_executed_at = Some(now);
                    let _ = self.deps.storage.save_task(updated_task).await;
                }

                let _ = self
                    .deps
                    .transport
                    .send_request(
                        VizierSession(
                            task.agent_id,
                            VizierChannelId::Task(task.slug.clone(), now.clone()),
                            None,
                        ),
                        VizierRequest {
                            user: task.user,
                            content: crate::schema::VizierRequestContent::Task(task.instruction),
                            metadata: serde_json::json!({
                                "timestamp": now,
                            }),
                        },
                    )
                    .await;
            }
        }
    }
}
