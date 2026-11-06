use crate::time::Timestamp;
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use anyhow::Result;
use chrono::{TimeZone, Local};
use croner::Cron;

use crate::{
    dependencies::VizierDependencies,
    runtime::Gate,
    schema::{
        Task, TaskRunState, TaskSchedule, VizierChannelId, VizierRequest, VizierResponse,
        VizierResponseContent, VizierSession,
    },
    storage::{task::TaskStorage, task_run::TaskRunStorage},
};

mod dream;

use dream::DreamScheduler;

pub struct VizierScheduler {
    /// Dream triggers from the bus, subscribed in `new` so none sent after startup is missed.
    dream_commands: crate::transport::DreamCommands,
    deps: VizierDependencies,
    dream_scheduler: DreamScheduler,
    /// Whether this node owns control. `vizier run --role all` asks [`AlwaysYes`], so nothing
    /// changes there; a Fleet node asks its lease (#166).
    ///
    /// [`AlwaysYes`]: crate::runtime::AlwaysYes
    gate: Arc<dyn Gate>,
}

impl VizierScheduler {
    pub async fn new(deps: VizierDependencies, gate: Arc<dyn Gate>) -> Result<VizierScheduler> {
        let dream_scheduler = DreamScheduler::new(deps.clone());
        let dream_commands = deps.transport.serve_dream_commands().await?;
        Ok(VizierScheduler {
            dream_commands,
            deps,
            dream_scheduler,
            gate,
        })
    }

    /// Closes the runs a restart interrupted that a pending turn resumed: each waits for its
    /// session's turn to end and records how it did, as `fire` does. It keeps the run's row
    /// `running`, which is what stops `tick_tasks` from firing the task on top of it.
    async fn recover(&self) {
        let runs = match self.deps.storage.open_task_runs().await {
            Ok(runs) => runs,
            Err(e) => {
                tracing::warn!("could not read open task runs to recover: {e}");
                return;
            }
        };
        for run in runs {
            let session = crate::storage::task_run::task_run_session(&run);
            let deps = self.deps.clone();
            tracing::info!(task = %run.task_slug, "recovering a task run");
            tokio::spawn(async move {
                use crate::agents::background::PieceOutcome;
                let state = match deps.transport.await_session_outcome(&session, run.ran_at).await {
                    PieceOutcome::Answered(_) => TaskRunState::Answered,
                    PieceOutcome::Failed(_) => TaskRunState::NoResponse,
                };
                if let Err(e) = deps.storage.close_task_run(run.id, state, crate::time::now()).await {
                    tracing::error!("Failed to record the end of a run of task '{}': {}", run.task_slug, e);
                } else {
                    // A resumed run that has now ended is news, as any other run's end is.
                    let title = deps
                        .storage
                        .get_task(run.agent_id.clone(), run.task_slug.clone())
                        .await
                        .ok()
                        .flatten()
                        .map_or_else(|| run.task_slug.clone(), |task| task.title);
                    deps.notifications
                        .task_run_ended(&run.agent_id, &run.task_slug, &title, run.id, state, crate::time::now())
                        .await;
                }
            });
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        if self.gate.may_run() {
            self.recover().await;
        }
        let mut schedules: HashMap<(Timestamp, String), Task> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_mins(1));
        // Startup already purged, so the first daily purge is a day from now.
        let mut last_purge = crate::time::now();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // A closed gate is "not now", never "stop": a standby waits for the lease
                    // and takes over on a later tick (#159), so the subsystem keeps running.
                    if !self.gate.may_run() {
                        tracing::debug!("the control gate is closed; skipping this tick");
                        continue;
                    }
                    // Task scheduling
                    if let Err(e) = self.tick_tasks(&mut schedules).await {
                        tracing::error!("Task scheduler error: {}", e);
                    }
                    // Dream cron scheduling
                    if let Err(e) = self.dream_scheduler.tick().await {
                        tracing::error!("Dream scheduler error: {}", e);
                    }
                    // Notification retention (spec 029): once a day from this tick.
                    if crate::time::now() - last_purge >= chrono::Duration::days(1) {
                        last_purge = crate::time::now();
                        let cutoff = crate::time::now()
                            - chrono::Duration::days(crate::storage::notification::RETENTION_DAYS);
                        match self.deps.notifications.purge_before(cutoff).await {
                            Ok(0) => {}
                            Ok(purged) => tracing::info!("purged {} notification(s) past retention", purged),
                            Err(e) => tracing::warn!("failed to purge old notifications: {}", e),
                        }
                    }
                }
                cmd = self.dream_commands.recv() => {
                    if !self.gate.may_run() {
                        tracing::debug!("the control gate is closed; ignoring a dream command");
                        continue;
                    }
                    match cmd {
                        Some(cmd) => {
                            tracing::info!("Received dream command for agent '{}'", cmd.agent_id);
                            if let Err(e) = self.dream_scheduler.trigger_dream(&cmd.agent_id).await {
                                tracing::error!("Dream trigger error for '{}': {}", cmd.agent_id, e);
                            }
                        }
                        None => {
                            return Err(anyhow::anyhow!("the dream command subscription closed"));
                        }
                    }
                }
            }
        }
    }

    /// The same predicate `DreamScheduler` uses to recognise the end of a turn, and the one every
    /// other reader of a turn's frames uses (`VizierResponse::ends_turn`). It includes `Error`,
    /// because a turn that failed the way a scheduled task fails on an ended session is over: the
    /// run closes on that frame instead of waiting for a channel that may never close (FR-017).
    fn is_final_response(response: &VizierResponse) -> bool {
        response.ends_turn()
    }

    /// Is a run of this task already in flight?
    ///
    /// Answered by the `running` row in `task_run`, not by in-memory state. The `HashSet`
    /// this replaced was inserted and removed within one loop iteration, so it never
    /// actually spanned a run; and now that runs are awaited in spawned tasks, in-memory
    /// state would also have to be shared back across them and would still be lost on
    /// restart. The row survives both, and the startup sweep is what keeps a row left open
    /// by a stopped process from blocking the task forever.
    ///
    /// A run resumed after a restart keeps its `running` row until `recover` closes it
    /// (`specs/023-persist-turn-activity`), so this is also what stops `tick_tasks` from
    /// firing a missed schedule on top of it: the task runs once, not twice.
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
        schedules: &mut HashMap<(Timestamp, String), Task>,
    ) -> Result<()> {
        let now = crate::time::now();

        let tasks = match self.deps.storage.get_task_list(None, Some(true)).await {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Failed to fetch task list: {}", e);
                return Ok(());
            }
        };

        schedules.clear();

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
    async fn fire(&self, task: Task, now: Timestamp) {
        let task_slug = task.slug.clone();
        let agent_id = task.agent_id.clone();
        let title = task.title.clone();

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
        let ran_at = match Local.timestamp_opt(now.timestamp(), 0).single() {
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

        let (response_tx, response_rx) = match self.deps.transport.reply_channel().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::error!("Failed to open a reply stream for task '{}': {}", task_slug, e);
                self.close_run(run_id, &agent_id, &task_slug, &title, TaskRunState::NoResponse)
                    .await;
                return;
            }
        };

        let dispatched = self
            .deps
            .transport
            .send_request(
                session,
                VizierRequest {
                    timestamp: now,
                    user: task.requester.to_frontmatter(),
                    content: crate::schema::VizierRequestContent::Unattended(task.instruction),
                    // The run's time is the request's own `timestamp`, which the agent sees as `sent_at`.
                    metadata: serde_json::json!({}),
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
            self.close_run(run_id, &agent_id, &task_slug, &title, TaskRunState::NoResponse)
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
                .close_task_run(run_id, state, crate::time::now())
                .await
            {
                tracing::error!(
                    "Failed to record the end of a run of task '{}': {}",
                    task_slug,
                    e
                );
            } else {
                deps.notifications
                    .task_run_ended(&agent_id, &task_slug, &title, run_id, state, crate::time::now())
                    .await;
            }
        });
    }

    async fn close_run(
        &self,
        run_id: Option<i64>,
        agent_id: &str,
        task_slug: &str,
        title: &str,
        state: TaskRunState,
    ) {
        let Some(run_id) = run_id else {
            return;
        };
        if let Err(e) = self
            .deps
            .storage
            .close_task_run(run_id, state, crate::time::now())
            .await
        {
            tracing::error!(
                "Failed to record the end of a run of task '{}': {}",
                task_slug,
                e
            );
        } else {
            self.deps
                .notifications
                .task_run_ended(agent_id, task_slug, title, run_id, state, crate::time::now())
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::schema::{ErrorKind, VizierResponse, VizierResponseContent};
    use crate::config::VizierConfig;

    fn frame(content: VizierResponseContent) -> VizierResponse {
        VizierResponse {
            content,
            ..Default::default()
        }
    }

    /// FR-017: a scheduled task whose turn ended on an ended session closes on that frame. The
    /// sender is deliberately kept alive — an agent still holding the channel — so a predicate
    /// that ignored `Error` would keep the run open until the timeout instead of ending it.
    #[tokio::test]
    async fn a_scheduled_turn_that_failed_end_session_closes_the_run() {
        let (tx, rx) = flume::unbounded::<VizierResponse>();
        tx.send(frame(VizierResponseContent::ThinkingStart)).unwrap();
        tx.send(frame(VizierResponseContent::Error {
            kind: ErrorKind::Completion,
            message: crate::llm::sso::SsoError::SessionExpired.to_string(),
        }))
        .unwrap();

        let ended = tokio::time::timeout(Duration::from_secs(1), async {
            while let Ok(response) = rx.recv_async().await {
                if VizierScheduler::is_final_response(&response) {
                    return true;
                }
            }
            false
        })
        .await
        .expect("the run must end on the failure, not wait for the channel to close");

        assert!(ended);
        drop(tx);
    }

    #[tokio::test]
    async fn deleted_task_is_not_fired_from_the_schedule_cache() {
        let dir = tempfile::tempdir().unwrap();
        let deps = VizierDependencies::new(VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let scheduler = VizierScheduler::new(deps.clone(), Arc::new(crate::runtime::AlwaysYes))
            .await
            .unwrap();
        let now = crate::time::now();
        let task = Task {
            slug: "deleted".into(),
            requester: crate::schema::Requester::Agent("agent".into()),
            agent_id: "agent".into(),
            title: "Deleted task".into(),
            instruction: "must not run".into(),
            is_active: true,
            schedule: TaskSchedule::OneTimeTask(now),
            last_executed_at: None,
            timestamp: now,
        };
        let mut schedules = HashMap::from([((now, task.slug.clone()), task)]);

        scheduler.tick_tasks(&mut schedules).await.unwrap();

        let runs = deps
            .storage
            .list_task_runs("agent".into(), "deleted".into(), None, None, 10)
            .await
            .unwrap();
        assert!(
            runs.is_empty(),
            "a deleted task was fired from stale scheduler state"
        );
    }

    /// Spec 052: a cron expression means the clock of the zone it is evaluated in. The scheduler
    /// hands `croner` a `Timestamp`, so the zone is the instance's; these pin the behaviour over
    /// real zones through a function that is generic over the zone, because a test cannot change
    /// the process's own.
    fn occurrences<T: chrono::TimeZone>(expression: &str, from: chrono::DateTime<T>, n: usize) -> Vec<String>
    where
        T::Offset: std::fmt::Display,
    {
        let cron = Cron::from_str(expression).unwrap();
        let mut at = from;
        let mut out = Vec::new();
        for _ in 0..n {
            at = cron.find_next_occurrence(&at, false).unwrap();
            out.push(at.to_rfc3339());
        }
        out
    }

    fn at_in<T: chrono::TimeZone>(tz: &T, text: &str) -> chrono::DateTime<T> {
        crate::time::parse_moment_in(text, tz).unwrap()
    }

    #[test]
    fn a_daily_nine_is_nine_on_the_local_clock() {
        use chrono_tz::Asia::Jakarta;
        assert_eq!(
            occurrences("0 9 * * *", at_in(&Jakarta, "2026-11-06T10:00"), 2),
            ["2026-11-07T09:00:00+07:00", "2026-11-08T09:00:00+07:00"]
        );
    }

    #[test]
    fn a_job_in_the_spring_gap_fires_once_at_the_first_valid_time() {
        use chrono_tz::America::New_York;
        // 02:30 does not exist on 2030-03-10: the clocks jump from 02:00 to 03:00.
        let runs = occurrences("30 2 * * *", at_in(&New_York, "2030-03-09T12:00"), 3);
        assert_eq!(runs[0], "2030-03-10T03:00:00-04:00", "{runs:?}");
        assert!(runs[1].starts_with("2030-03-11T02:30"), "once in the gap, then back to 02:30: {runs:?}");
        assert!(runs[2].starts_with("2030-03-12T02:30"), "{runs:?}");
    }

    #[test]
    fn a_job_in_the_autumn_overlap_fires_once() {
        use chrono_tz::America::New_York;
        // 01:30 happens twice on 2030-11-03.
        let runs = occurrences("30 1 * * *", at_in(&New_York, "2030-11-02T12:00"), 3);
        assert_eq!(runs.iter().filter(|r| r.starts_with("2030-11-03")).count(), 1, "{runs:?}");
        assert!(runs[1].starts_with("2030-11-04T01:30"), "{runs:?}");
    }

    #[test]
    fn the_same_expression_is_a_different_moment_in_a_different_zone() {
        use chrono_tz::{America::New_York, Asia::Jakarta};
        let jakarta = Cron::from_str("0 9 * * *").unwrap().find_next_occurrence(&at_in(&Jakarta, "2026-11-06T10:00"), false).unwrap();
        let new_york = Cron::from_str("0 9 * * *").unwrap().find_next_occurrence(&at_in(&New_York, "2026-11-06T10:00"), false).unwrap();
        assert_eq!(jakarta.with_timezone(&chrono::Utc).to_rfc3339(), "2026-11-07T02:00:00+00:00");
        assert_eq!(new_york.with_timezone(&chrono::Utc).to_rfc3339(), "2026-11-07T14:00:00+00:00");
    }

    /// FR-019: after an upgrade the next occurrence is found from the last run. A task whose last
    /// run was yesterday and whose local time today has passed comes due at once, exactly one
    /// catch-up; once it has run, `last_executed_at` moves to now and it follows the local schedule.
    #[test]
    fn an_upgrade_costs_at_most_one_catch_up() {
        use chrono_tz::Asia::Jakarta;
        let cron = Cron::from_str("0 9 * * *").unwrap();
        let last = at_in(&Jakarta, "2026-11-05T09:00");
        let now = at_in(&Jakarta, "2026-11-06T14:00");
        let due = cron.find_next_occurrence(&last, false).unwrap();
        assert!(due <= now, "today's 09:00 has passed, so it is due: {due}");
        let after = cron.find_next_occurrence(&now, false).unwrap();
        assert_eq!(after.to_rfc3339(), "2026-11-07T09:00:00+07:00", "then tomorrow, not again today");
    }

    /// On a UTC instance nothing moves: the same expression gives the same moments as before.
    #[test]
    fn on_a_utc_instance_nothing_moves() {
        let utc = chrono::Utc;
        assert_eq!(
            occurrences("0 9 * * *", at_in(&utc, "2026-11-06T10:00"), 1),
            ["2026-11-07T09:00:00+00:00"]
        );
    }
}
