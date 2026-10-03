use std::{collections::HashMap, str::FromStr, time::Duration};

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use croner::Cron;

use crate::{
    dependencies::VizierDependencies,
    schema::{
        Task, TaskRunState, TaskSchedule, VizierChannelId, VizierRequest, VizierResponse,
        VizierResponseContent, VizierSession,
    },
    storage::{task::TaskStorage, task_run::TaskRunStorage},
};

mod dream;

use dream::DreamScheduler;

pub struct VizierScheduler {
    deps: VizierDependencies,
    dream_scheduler: DreamScheduler,
}

impl VizierScheduler {
    pub async fn new(deps: VizierDependencies) -> Result<VizierScheduler> {
        let dream_scheduler = DreamScheduler::new(deps.clone());
        Ok(VizierScheduler {
            deps,
            dream_scheduler,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut schedules: HashMap<(DateTime<Utc>, String), Task> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_mins(1));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Task scheduling
                    if let Err(e) = self.tick_tasks(&mut schedules).await {
                        tracing::error!("Task scheduler error: {}", e);
                    }
                    // Dream cron scheduling
                    if let Err(e) = self.dream_scheduler.tick().await {
                        tracing::error!("Dream scheduler error: {}", e);
                    }
                }
                cmd = self.deps.transport.recv_dream_command() => {
                    match cmd {
                        Ok(cmd) => {
                            tracing::info!("Received dream command for agent '{}'", cmd.agent_id);
                            if let Err(e) = self.dream_scheduler.trigger_dream(&cmd.agent_id).await {
                                tracing::error!("Dream trigger error for '{}': {}", cmd.agent_id, e);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Dream command channel error: {}", e);
                        }
                    }
                }
            }
        }
    }

    /// The same predicate `DreamScheduler` uses to recognise the end of a turn.
    fn is_final_response(response: &VizierResponse) -> bool {
        matches!(
            response.content,
            VizierResponseContent::Message { .. }
                | VizierResponseContent::Abort
                | VizierResponseContent::Empty
        )
    }

    /// Is a run of this task already in flight?
    ///
    /// Answered by the `running` row in `task_run`, not by in-memory state. The `HashSet`
    /// this replaced was inserted and removed within one loop iteration, so it never
    /// actually spanned a run; and now that runs are awaited in spawned tasks, in-memory
    /// state would also have to be shared back across them and would still be lost on
    /// restart. The row survives both, and the startup sweep is what keeps a row left open
    /// by a stopped process from blocking the task forever.
    async fn is_running(&self, task: &Task) -> bool {
        match self
            .deps
            .storage
            .running_task_run(task.agent_id.clone(), task.slug.clone())
            .await
        {
            Ok(run) => run.is_some(),
            // A failed lookup must not silently stop a task from ever firing, so the task
            // is treated as free. The cost of being wrong is an overlapping run; the cost
            // of the other choice is a task that never runs again.
            Err(e) => {
                tracing::warn!(
                    "failed to check whether task '{}' is already running: {}",
                    task.slug,
                    e
                );
                false
            }
        }
    }

    async fn tick_tasks(
        &self,
        schedules: &mut HashMap<(DateTime<Utc>, String), Task>,
    ) -> Result<()> {
        let now = Utc::now();

        let tasks = match self.deps.storage.get_task_list(None, Some(true)).await {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Failed to fetch task list: {}", e);
                return Ok(());
            }
        };

        for task in tasks.iter() {
            if self.is_running(task).await {
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
        for (schedule, slug) in lookup.keys() {
            if *schedule <= now
                && let Some(task) = schedules.remove(&(*schedule, slug.clone()))
            {
                if self.is_running(&task).await {
                    continue;
                }
                to_be_run.push(task);
            }
        }

        for task in to_be_run {
            self.fire(task, now).await;
        }

        Ok(())
    }

    /// Record the firing, dispatch it, and close the run from the response channel.
    ///
    /// A recording failure is logged and swallowed — it must never abort the run, which is
    /// the work the person actually asked for. The cost is a run missing from the history,
    /// not a task that did not happen.
    async fn fire(&self, task: Task, now: DateTime<Utc>) {
        let task_slug = task.slug.clone();
        let agent_id = task.agent_id.clone();

        if let &TaskSchedule::OneTimeTask(_) = &task.schedule {
            // Deactivated, not deleted. Deleting it before dispatch is what left nothing to
            // hang a result on — and `get_task_list(None, Some(true))` already filters on
            // `is_active`, so this is equally effective at stopping a re-pick.
            let mut fired = task.clone();
            fired.is_active = false;
            fired.last_executed_at = Some(now);
            if let Err(e) = self.deps.storage.save_task(fired).await {
                tracing::error!("Failed to deactivate one-time task '{}': {}", task_slug, e);
            }
        }

        if let &TaskSchedule::CronTask(_) = &task.schedule {
            let mut updated_task = task.clone();
            updated_task.last_executed_at = Some(now);
            if let Err(e) = self.deps.storage.save_task(updated_task).await {
                tracing::error!("Failed to update cron task '{}': {}", task_slug, e);
            }
        }

        // The run's address is a whole-second time, so it round-trips through the session
        // slug and back to a `run_id` a caller can name.
        let ran_at = match Utc.timestamp_opt(now.timestamp(), 0).single() {
            Some(ran_at) => ran_at,
            None => now,
        };
        let session = VizierSession(
            agent_id.clone(),
            VizierChannelId::Task(task_slug.clone(), ran_at),
            None,
        );

        let run_id = match self
            .deps
            .storage
            .open_task_run(
                agent_id.clone(),
                task_slug.clone(),
                ran_at,
                session.to_slug(),
            )
            .await
        {
            Ok(id) => Some(id),
            Err(e) => {
                tracing::error!(
                    "Failed to record the start of a run of task '{}': {}",
                    task_slug,
                    e
                );
                None
            }
        };

        let (response_tx, response_rx) = flume::unbounded::<VizierResponse>();

        let dispatched = self
            .deps
            .transport
            .send_request(
                session,
                VizierRequest {
                    timestamp: now,
                    user: task.requester.to_frontmatter(),
                    content: crate::schema::VizierRequestContent::Unattended(task.instruction),
                    metadata: serde_json::json!({
                        "timestamp": now,
                    }),
                    // What turns the frontmatter's `sender` from a person's name into
                    // `scheduler`, and carries the slug and `requested_by` with it.
                    scheduled_task: Some(task_slug.clone()),

                    ..Default::default()
                },
                Some(response_tx),
            )
            .await;

        if let Err(e) = dispatched {
            // `send_request` returns `Err` when the agent is not registered, which is the
            // "agent unreachable" case: the run opens and closes in the same tick, so the
            // firing is visible rather than silently absent.
            tracing::error!("Failed to send request for task '{}': {}", task_slug, e);
            self.close_run(run_id, &task_slug, TaskRunState::NoResponse)
                .await;
            return;
        }

        let Some(run_id) = run_id else {
            // The run is dispatched and will do its work; there is just no row to close.
            return;
        };

        let deps = self.deps.clone();
        tokio::spawn(async move {
            let mut answered = false;
            while let Ok(response) = response_rx.recv_async().await {
                if Self::is_final_response(&response) {
                    answered = matches!(response.content, VizierResponseContent::Message { .. });
                    break;
                }
            }

            let state = if answered {
                TaskRunState::Answered
            } else {
                // The channel closed without a message, or the turn ended in `Abort`/`Empty`.
                // Either way the run produced nothing a person can read, and that has to be
                // visible as such rather than looking like a task that never ran.
                TaskRunState::NoResponse
            };

            if let Err(e) = deps
                .storage
                .close_task_run(run_id, state, Utc::now())
                .await
            {
                tracing::error!(
                    "Failed to record the end of a run of task '{}': {}",
                    task_slug,
                    e
                );
            }
        });
    }

    async fn close_run(&self, run_id: Option<i64>, task_slug: &str, state: TaskRunState) {
        let Some(run_id) = run_id else {
            return;
        };
        if let Err(e) = self
            .deps
            .storage
            .close_task_run(run_id, state, Utc::now())
            .await
        {
            tracing::error!(
                "Failed to record the end of a run of task '{}': {}",
                task_slug,
                e
            );
        }
    }
}
