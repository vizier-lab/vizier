//! Blocking sub-agent runs (`parallel_subagents`): dispatch several pieces, wait for all of
//! them, and return their results in the order the caller supplied them.
//!
//! There is no background variant any more (spec 058): an agent that wants the fan-out to run
//! while it carries on calls `parallel_subagents` inside a detached `execute_python`. A run has
//! no row of its own; its only durable trace is the history of each piece's own conversation.

use std::{sync::Arc, time::Duration};

use futures::{FutureExt, future::BoxFuture, future::join_all};
use parking_lot::Mutex;
use schemars::JsonSchema;
use serde::Serialize;
use tokio::time::Instant;

use crate::{
    agents::{detached::check_depth, tools::ToolContext},
    error::VizierError,
    schema::{
        AgentId, VizierChannelId, VizierRequest, VizierRequestContent, VizierResponseContent,
        VizierSession,
    },
    transport::VizierTransport,
};

/// Time reserved for the caller to receive a result before the outer tool timeout cuts
/// the call off.
pub const GRACE: Duration = Duration::from_secs(2);

/// The default per-task limit of `parallel_subagents`.
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;
/// The longest per-task limit.
pub const MAX_TIMEOUT_SECS: u64 = 3600;

/// A result's text is cut at this many characters.
pub const RESULT_TEXT_LIMIT: usize = 4000;

const TRUNCATION_MARKER: &str = " … [truncated]";
const NO_ANSWER: &str = "the piece ended without an answer";
/// A piece whose reply stream closed without an answer: its agent's process went away.
pub const AGENT_RESTARTED: &str = "the agent was restarted";

/// How one piece ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubagentState {
    Answered,
    Failed,
    TimedOut,
}

/// One result from a blocking subagent run.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SubagentResult {
    /// The zero-based position of the task in the input list.
    pub task: u32,
    /// How this piece ended.
    pub state: SubagentState,
    /// The answer, or the reason there is none.
    pub text: String,
    /// Whether `text` was cut at the limit.
    pub truncated: bool,
    /// The piece's conversation topic, where its full history remains inspectable.
    pub topic: String,
}

/// The `timeout_secs` tool argument: 1..=3600, default 600.
pub fn validate_timeout(timeout_secs: Option<u64>) -> crate::Result<u64> {
    match timeout_secs {
        None => Ok(DEFAULT_TIMEOUT_SECS),
        Some(secs) if (1..=MAX_TIMEOUT_SECS).contains(&secs) => Ok(secs),
        Some(_) => Err(VizierError(format!(
            "timeout_secs must be between 1 and {MAX_TIMEOUT_SECS}"
        ))),
    }
}

/// Cut `text` to `limit` characters, marking the cut. Returns whether it cut.
pub fn truncate(text: &str, limit: usize) -> (String, bool) {
    match text.char_indices().nth(limit) {
        Some((end, _)) => (format!("{}{TRUNCATION_MARKER}", &text[..end]), true),
        None => (text.to_string(), false),
    }
}

/// How a piece ended, from its first terminal frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PieceOutcome {
    Answered(String),
    Failed(String),
}

/// Map one frame of a piece's turn to how the piece ended, or `None` for a mid-turn frame.
pub fn classify(content: &VizierResponseContent) -> Option<PieceOutcome> {
    match content {
        VizierResponseContent::Message { content, .. } => {
            Some(PieceOutcome::Answered(content.clone()))
        }
        VizierResponseContent::AudioReply(_, Some(text), _) => {
            Some(PieceOutcome::Answered(text.clone()))
        }
        VizierResponseContent::AudioReply(_, None, _) => {
            Some(PieceOutcome::Failed(NO_ANSWER.to_string()))
        }
        VizierResponseContent::Error { message, .. } => Some(PieceOutcome::Failed(message.clone())),
        VizierResponseContent::Abort | VizierResponseContent::Empty => {
            Some(PieceOutcome::Failed(NO_ANSWER.to_string()))
        }
        VizierResponseContent::ThinkingStart
        | VizierResponseContent::Thinking(_)
        | VizierResponseContent::ToolChoice { .. }
        | VizierResponseContent::ToolResponse { .. }
        | VizierResponseContent::Checkpoint { .. } => None,
    }
}

/// Read a piece's frames until the first terminal one. A stream that closes before one means
/// the turn's sender was dropped without answering: its process was shut down.
async fn first_terminal(rx: &crate::transport::ResponseStream) -> PieceOutcome {
    loop {
        match rx.recv_async().await {
            Ok(frame) => {
                if let Some(outcome) = classify(&frame.content) {
                    return outcome;
                }
            }
            Err(_) => return PieceOutcome::Failed(AGENT_RESTARTED.to_string()),
        }
    }
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

/// The launching turn's project, for a subagent piece (spec 041 FR-007a). A piece of a piece
/// gets none: only a person's own project chat is the source of a scope.
fn project_scope(ctx: &ToolContext) -> Option<crate::schema::ProjectScope> {
    let project = ctx.project.as_ref()?;
    let owner = ctx.session.1.web_user()?;
    Some(crate::schema::ProjectScope {
        project_id: project.project_id.clone(),
        owner: owner.to_string(),
        topic: ctx.session.2.clone().unwrap_or_default(),
        mode: project.mode,
    })
}

/// Runs blocking sub-agent fan-outs. Cheap to clone.
#[derive(Clone)]
pub struct Subagents {
    transport: VizierTransport,
}

impl Subagents {
    pub fn new(transport: VizierTransport) -> Self {
        Self { transport }
    }

    /// Run all prompts as fresh subagent conversations and wait for every one of them.
    /// The pieces are transient requests, so a restart cannot resume them, and a dropped call
    /// aborts the ones still running.
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
        let scope = project_scope(ctx);
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
        let mut guard = WaitGuard::new(self.clone(), unfinished.clone());

        let dispatches = join_all(sessions.iter().zip(prompts.iter()).map(|(session, prompt)| {
            self.dispatch_piece(
                session,
                prompt,
                &origin,
                ctx.background_depth + 1,
                scope.as_ref(),
                ctx.parent_detached_id.clone(),
            )
        }))
        .await;

        let mut waits: Vec<BoxFuture<'_, (SubagentState, String)>> =
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
                    waits.push(async move { (SubagentState::Failed, err.to_string()) }.boxed());
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

    async fn dispatch_piece(
        &self,
        session: &VizierSession,
        prompt: &str,
        origin: &AgentId,
        depth: u8,
        scope: Option<&crate::schema::ProjectScope>,
        parent_detached_id: Option<String>,
    ) -> crate::Result<crate::transport::ResponseStream> {
        let (tx, rx) = self.transport.reply_channel().await?;
        self.transport
            .send_request(
                session.clone(),
                VizierRequest {
                    timestamp: crate::time::now(),
                    user: origin.clone(),
                    content: VizierRequestContent::Prompt(prompt.to_string()),
                    metadata: serde_json::json!({}),
                    background_depth: depth,
                    project_scope: scope.cloned(),
                    transient: true,
                    parent_detached_id,
                    ..Default::default()
                },
                Some(tx),
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;
        Ok(rx)
    }

    /// Wait for one dispatched piece, stopping it if it runs past its limit.
    async fn await_piece(
        &self,
        rx: &crate::transport::ResponseStream,
        limit: Duration,
        session: &VizierSession,
        origin: &AgentId,
    ) -> (SubagentState, String) {
        match tokio::time::timeout(limit, first_terminal(rx)).await {
            Ok(PieceOutcome::Answered(text)) => (SubagentState::Answered, text),
            Ok(PieceOutcome::Failed(reason)) => (SubagentState::Failed, reason),
            Err(_) => {
                // Stop a runaway piece spending tokens; its late answer, if any, is
                // discarded with the receiver.
                self.abort_piece(session, origin).await;
                (
                    SubagentState::TimedOut,
                    format!("No answer within {}s", limit.as_secs()),
                )
            }
        }
    }

    /// The existing `abort` command, sent to a piece's session.
    async fn abort_piece(&self, session: &VizierSession, origin: &AgentId) {
        if let Err(e) = self
            .transport
            .send_request(
                session.clone(),
                VizierRequest {
                    timestamp: crate::time::now(),
                    user: origin.clone(),
                    content: VizierRequestContent::Command("abort".into()),
                    metadata: serde_json::json!({}),
                    ..Default::default()
                },
                None,
            )
            .await
        {
            tracing::debug!("could not abort subagent piece {}: {}", session.to_slug(), e);
        }
    }
}

/// The pieces that a dropped blocking call must stop.
struct WaitGuard {
    runner: Subagents,
    unfinished: Arc<Mutex<Vec<(VizierSession, AgentId)>>>,
    armed: bool,
}

impl WaitGuard {
    fn new(runner: Subagents, unfinished: Arc<Mutex<Vec<(VizierSession, AgentId)>>>) -> Self {
        Self { runner, unfinished, armed: true }
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
        let runner = self.runner.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn(async move {
                    for (session, origin) in unfinished {
                        runner.abort_piece(&session, &origin).await;
                    }
                });
            }
            Err(err) => tracing::warn!("could not stop unfinished subagents: {err}"),
        }
    }
}

/// Turn one terminal outcome into the stable, task-aligned result shape.
fn result_from_outcome(
    task: usize,
    (state, text): (SubagentState, String),
    topic: &str,
) -> SubagentResult {
    let (text, truncated) = truncate(&text, RESULT_TEXT_LIMIT);
    SubagentResult {
        task: task as u32,
        state,
        text,
        truncated,
        topic: topic.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::VizierResponse;

    #[test]
    fn no_deadline_uses_the_per_task_limit() {
        let now = Instant::now();
        assert_eq!(wait_limit(Duration::from_secs(30), None, now), Ok(Duration::from_secs(30)));
    }

    #[test]
    fn the_deadline_wins_when_it_is_earlier() {
        let now = Instant::now();
        let limit = wait_limit(Duration::from_secs(30), Some(now + Duration::from_secs(10)), now).unwrap();
        assert_eq!(limit, Duration::from_secs(8));
    }

    #[test]
    fn the_per_task_limit_wins_when_it_is_earlier() {
        let now = Instant::now();
        let limit = wait_limit(Duration::from_secs(5), Some(now + Duration::from_secs(30)), now).unwrap();
        assert_eq!(limit, Duration::from_secs(5));
    }

    #[test]
    fn too_little_time_is_refused() {
        let now = Instant::now();
        let remaining =
            wait_limit(Duration::from_secs(30), Some(now + Duration::from_secs(2)), now).unwrap_err();
        assert_eq!(remaining, Duration::from_secs(2));
    }

    #[test]
    fn outcomes_keep_their_input_order_and_task_numbers() {
        let outcomes = vec![
            (SubagentState::Answered, "one".into()),
            (SubagentState::Failed, "two".into()),
            (SubagentState::TimedOut, "three".into()),
        ];
        let results: Vec<_> = outcomes
            .into_iter()
            .enumerate()
            .map(|(task, outcome)| result_from_outcome(task, outcome, "topic"))
            .collect();
        assert_eq!(results.iter().map(|r| r.task).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(results[2].state, SubagentState::TimedOut);
        assert_eq!(serde_json::to_value(results[2].state).unwrap(), "timed_out");
        assert!(results.iter().all(|result| result.topic == "topic"));
    }

    #[test]
    fn long_text_is_cut_with_a_marker() {
        let long = "x".repeat(RESULT_TEXT_LIMIT + 10);
        let (cut, truncated) = truncate(&long, RESULT_TEXT_LIMIT);
        assert!(truncated);
        assert!(cut.ends_with(TRUNCATION_MARKER));
        assert_eq!(truncate("short", RESULT_TEXT_LIMIT), ("short".into(), false));
    }

    #[test]
    fn timeout_validation() {
        assert_eq!(validate_timeout(None).unwrap(), DEFAULT_TIMEOUT_SECS);
        assert_eq!(validate_timeout(Some(3)).unwrap(), 3);
        assert!(validate_timeout(Some(0)).is_err());
        assert!(validate_timeout(Some(3601)).is_err());
    }

    #[test]
    fn mid_turn_frames_are_not_terminal() {
        for frame in [
            VizierResponseContent::ThinkingStart,
            VizierResponseContent::Thinking("hmm".into()),
            VizierResponseContent::ToolChoice { name: "t".into(), args: serde_json::json!({}) },
            VizierResponseContent::ToolResponse { response: serde_json::json!("ok") },
            VizierResponseContent::Checkpoint { handover: None },
        ] {
            assert_eq!(classify(&frame), None, "{frame:?}");
        }
    }

    #[tokio::test]
    async fn a_closed_channel_without_an_answer_reads_as_a_restart() {
        let (tx, rx) = crate::transport::standalone_reply_channel().await;
        tx.send(VizierResponse {
            timestamp: crate::time::now(),
            content: VizierResponseContent::ThinkingStart,
            attachments: vec![],
            ..Default::default()
        })
        .unwrap();
        drop(tx);
        assert_eq!(first_terminal(&rx).await, PieceOutcome::Failed(AGENT_RESTARTED.into()));
    }
}
