//! Blocking subagent runs: dispatch several pieces, wait for all of them, and return
//! their results in the order the caller supplied them.
//!
//! This is deliberately not a background job. A blocking run has no job row, no tray
//! entry and no report; its only durable trace is the history of each piece's own
//! conversation.

use std::{sync::Arc, time::Duration};

use futures::{FutureExt, future::BoxFuture, future::join_all};
use parking_lot::Mutex;
use schemars::JsonSchema;
use serde::Serialize;
use tokio::time::Instant;

use crate::{
    agents::{
        background::{BackgroundJobs, check_depth, report},
        tools::ToolContext,
    },
    error::VizierError,
    schema::{AgentId, PieceState, VizierChannelId, VizierSession},
};

/// Time reserved for the caller to receive a result before the outer tool timeout cuts
/// the call off.
pub const GRACE: Duration = Duration::from_secs(2);

/// One result from a blocking subagent run.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SubagentResult {
    /// The zero-based position of the task in the input list.
    pub task: u32,
    /// The terminal state of this piece.
    pub state: PieceState,
    /// The answer, or the reason there is none.
    pub text: String,
    /// Whether `text` was cut at the report limit.
    pub truncated: bool,
    /// The piece's conversation topic, where its full history remains inspectable.
    pub topic: String,
}

/// How long a blocking run may wait for its pieces.
///
/// With no outer deadline, the per-task limit is the only limit. With one, leave a
/// grace period for the result to travel back to the script. If less than one second
/// plus the grace period remains, there is not enough time to make a useful call.
fn wait_limit(
    per_task: Duration,
    deadline: Option<Instant>,
    now: Instant,
) -> Result<Duration, Duration> {
    let Some(deadline) = deadline else {
        return Ok(per_task);
    };

    let remaining = deadline.saturating_duration_since(now);
    if remaining < GRACE + Duration::from_secs(1) {
        return Err(remaining);
    }
    Ok(per_task.min(remaining - GRACE))
}

/// The pieces that a dropped blocking call must stop.
struct WaitGuard {
    jobs: BackgroundJobs,
    origin: AgentId,
    unfinished: Arc<Mutex<Vec<(VizierSession, AgentId)>>>,
    armed: bool,
}

impl WaitGuard {
    fn new(
        jobs: BackgroundJobs,
        origin: AgentId,
        unfinished: Arc<Mutex<Vec<(VizierSession, AgentId)>>>,
    ) -> Self {
        Self {
            jobs,
            origin,
            unfinished,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for WaitGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }

        let unfinished = std::mem::take(&mut *self.unfinished.lock());
        if unfinished.is_empty() {
            return;
        }

        let jobs = self.jobs.clone();
        let origin = self.origin.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn(async move {
                    for (session, origin) in unfinished {
                        jobs.abort_piece(&session, &origin).await;
                    }
                });
            }
            Err(err) => {
                tracing::warn!("could not stop unfinished blocking subagents: {err}");
            }
        }
    }
}

/// Turn one terminal outcome into the stable, task-aligned result shape.
fn result_from_outcome(
    task: usize,
    (state, text): (PieceState, String),
    topic: &str,
) -> SubagentResult {
    let (text, truncated) = report::truncate(&text, report::REPORT_TEXT_LIMIT);
    SubagentResult {
        task: task as u32,
        state,
        text,
        truncated,
        topic: topic.to_string(),
    }
}

impl BackgroundJobs {
    /// Run all prompts as fresh subagent conversations and wait for every one of them.
    ///
    /// The run is not a background job: no storage row is written, nothing is published
    /// to the session-event broadcast, and no report is delivered later. The pieces are
    /// transient requests, so a restart cannot resume them.
    pub async fn run_and_wait(
        &self,
        ctx: &ToolContext,
        prompts: Vec<String>,
        per_task: Duration,
    ) -> crate::Result<Vec<SubagentResult>> {
        check_depth(ctx)?;

        let limit = wait_limit(per_task, ctx.deadline, Instant::now()).map_err(|remaining| {
            VizierError(format!(
                "not enough time left in this tool call ({}s); call it earlier in the script or from a new execute_python",
                remaining.as_secs_f64().ceil() as u64
            ))
        })?;

        let origin = ctx.session.0.clone();
        let sessions: Vec<VizierSession> = prompts
            .iter()
            .map(|_| {
                VizierSession(
                    origin.clone(),
                    VizierChannelId::Subagent,
                    Some(uuid::Uuid::new_v4().to_string()),
                )
            })
            .collect();

        // Register every session before dispatching. If the call is dropped while a
        // dispatch is in flight, the guard still knows which piece to abort.
        let unfinished = Arc::new(Mutex::new(
            sessions
                .iter()
                .map(|session| (session.clone(), origin.clone()))
                .collect(),
        ));
        let mut guard = WaitGuard::new(self.clone(), origin.clone(), unfinished.clone());

        let dispatches = join_all(
            sessions
                .iter()
                .zip(prompts.iter())
                .map(|(session, prompt)| {
                    self.dispatch_piece(session, prompt, &origin, ctx.background_depth + 1, true)
                }),
        )
        .await;

        let mut waits: Vec<BoxFuture<'_, (PieceState, String)>> =
            Vec::with_capacity(sessions.len());
        for (session, dispatch) in sessions.iter().zip(dispatches) {
            match dispatch {
                Ok(rx) => {
                    let session = session.clone();
                    let origin = origin.clone();
                    let unfinished = unfinished.clone();
                    waits.push(
                        async move {
                            let outcome = self.await_piece(&rx, limit, &session, &origin).await;
                            unfinished
                                .lock()
                                .retain(|(unfinished, _)| unfinished != &session);
                            outcome
                        }
                        .boxed(),
                    );
                }
                Err(err) => {
                    unfinished
                        .lock()
                        .retain(|(unfinished, _)| unfinished != session);
                    waits.push(async move { (PieceState::Failed, err.to_string()) }.boxed());
                }
            }
        }

        let outcomes = join_all(waits).await;
        guard.disarm();

        Ok(outcomes
            .into_iter()
            .enumerate()
            .map(|(task, outcome)| {
                let topic = sessions[task].2.as_deref().unwrap_or_default();
                result_from_outcome(task, outcome, topic)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;

    #[test]
    fn no_deadline_uses_the_per_task_limit() {
        let now = Instant::now();
        assert_eq!(
            wait_limit(Duration::from_secs(30), None, now),
            Ok(Duration::from_secs(30))
        );
    }

    #[test]
    fn the_deadline_wins_when_it_is_earlier() {
        let now = Instant::now();
        let limit = wait_limit(
            Duration::from_secs(30),
            Some(now + Duration::from_secs(10)),
            now,
        )
        .unwrap();
        assert_eq!(limit, Duration::from_secs(8));
    }

    #[test]
    fn the_per_task_limit_wins_when_it_is_earlier() {
        let now = Instant::now();
        let limit = wait_limit(
            Duration::from_secs(5),
            Some(now + Duration::from_secs(30)),
            now,
        )
        .unwrap();
        assert_eq!(limit, Duration::from_secs(5));
    }

    #[test]
    fn too_little_time_is_refused() {
        let now = Instant::now();
        let remaining = wait_limit(
            Duration::from_secs(30),
            Some(now + Duration::from_secs(2)),
            now,
        )
        .unwrap_err();
        assert_eq!(remaining, Duration::from_secs(2));
    }

    #[test]
    fn outcomes_keep_their_input_order_and_task_numbers() {
        let outcomes = vec![
            (PieceState::Answered, "one".into()),
            (PieceState::Failed, "two".into()),
            (PieceState::TimedOut, "three".into()),
        ];
        let results: Vec<_> = outcomes
            .into_iter()
            .enumerate()
            .map(|(task, outcome)| result_from_outcome(task, outcome, "topic"))
            .collect();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].task, 0);
        assert_eq!(results[0].state, PieceState::Answered);
        assert_eq!(results[1].task, 1);
        assert_eq!(results[1].state, PieceState::Failed);
        assert_eq!(results[2].task, 2);
        assert_eq!(results[2].state, PieceState::TimedOut);
        assert!(results.iter().all(|result| result.topic == "topic"));
    }
}
