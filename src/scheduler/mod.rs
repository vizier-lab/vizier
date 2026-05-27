use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    time::Duration,
};

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
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
        let mut running: HashSet<(String, String)> = HashSet::new();
        let mut interval = tokio::time::interval(Duration::from_mins(1));
        loop {
            interval.tick().await;
            let now = Utc::now();

            let tasks = match self.deps.storage.get_task_list(None, Some(true)).await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to fetch task list: {}", e);
                    continue;
                }
            };

            for task in tasks.iter() {
                if running.contains(&(task.agent_id.clone(), task.slug.clone())) {
                    continue;
                }

                match &task.schedule {
                    TaskSchedule::OneTimeTask(schedule) => {
                        schedules.insert((*schedule, task.slug.clone()), task.clone());
                    }
                    TaskSchedule::CronTask(cron_str) => {
                        let cron = match Cron::from_str(cron_str) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::warn!(
                                    "Invalid cron expression for task '{}': {}",
                                    task.slug,
                                    e
                                );
                                continue;
                            }
                        };
                        let schedule = match cron
                            .find_next_occurrence(&task.last_executed_at.unwrap_or(now), true)
                        {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to find next occurrence for cron task '{}': {}",
                                    task.slug,
                                    e
                                );
                                continue;
                            }
                        };
                        schedules.insert((schedule, task.slug.clone()), task.clone());
                    }
                };
            }

            let mut to_be_run = vec![];
            let lookup = schedules.clone();
            for ((schedule, slug), _) in &lookup {
                if *schedule <= now {
                    if let Some(task) = schedules.remove(&(*schedule, slug.clone())) {
                        if running.contains(&(task.agent_id.clone(), task.slug.clone())) {
                            continue;
                        }
                        to_be_run.push(task);
                    }
                }
            }

            for task in to_be_run {
                running.insert((task.agent_id.clone(), task.slug.clone()));

                let task_slug = task.slug.clone();
                let agent_id = task.agent_id.clone();

                if let &TaskSchedule::OneTimeTask(_) = &task.schedule {
                    if let Err(e) = self
                        .deps
                        .storage
                        .delete_task(task.agent_id.clone(), task.slug.clone())
                        .await
                    {
                        tracing::error!("Failed to delete one-time task '{}': {}", task_slug, e);
                    }
                }

                if let &TaskSchedule::CronTask(_) = &task.schedule {
                    let mut updated_task = task.clone();
                    updated_task.last_executed_at = Some(now);
                    if let Err(e) = self.deps.storage.save_task(updated_task).await {
                        tracing::error!("Failed to update cron task '{}': {}", task_slug, e);
                    }
                }

                let now_second = Utc.timestamp_opt(now.timestamp(), 0).unwrap();

                if let Err(e) = self
                    .deps
                    .transport
                    .send_request(
                        VizierSession(
                            agent_id,
                            VizierChannelId::Task(task_slug.clone(), now_second),
                            None,
                        ),
                        VizierRequest {
                            timestamp: now,
                            user: task.user,
                            content: crate::schema::VizierRequestContent::Task(task.instruction),
                            metadata: serde_json::json!({
                                "timestamp": now,
                            }),

                            ..Default::default()
                        },
                    )
                    .await
                {
                    tracing::error!("Failed to send request for task '{}': {}", task_slug, e);
                }

                running.remove(&(task.agent_id, task_slug));
            }
        }
    }
}
