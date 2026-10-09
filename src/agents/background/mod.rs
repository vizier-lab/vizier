//! Background jobs (`specs/012-background-subagent-results/`): work an agent hands off with
//! `paralel_subtasks` or `delegate_agent`, which reports back to the conversation that
//! launched it as one `BackgroundReport` turn.
//!
//! Delivery is the same for every conversation. Nothing here branches on `VizierChannelId`:
//! a report's woken turn is republished onto the transport's session-event broadcast, and
//! whether anyone is listening (a WebUI socket) or not (everything else) is not this
//! module's concern.

use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::Utc;
use futures::{FutureExt, future::BoxFuture};
use parking_lot::Mutex;
use tokio::sync::watch;

use crate::{
    agents::tools::ToolContext,
    error::VizierError,
    schema::{
        AgentId, BackgroundJob, BackgroundJobId, BackgroundJobSnapshot, BackgroundPiece,
        BackgroundReport, Canceller, JobKind, JobState, PieceState, ReportEntry,
        VizierChannelId, VizierRequest, VizierRequestContent, VizierResponse,
        VizierResponseContent, VizierSession, background::new_job_id,
    },
    storage::{
        VizierStorage,
        background_job::{BackgroundJobStorage, piece_answer},
    },
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

pub mod report;

/// A turn woken by a background result counts as one level deeper than the turn that
/// launched the job. A turn at this depth may not launch another one.
pub const MAX_BACKGROUND_DEPTH: u8 = 3;

pub const DEFAULT_TIMEOUT_SECS: u64 = 600;
pub const MAX_TIMEOUT_SECS: u64 = 3600;

/// Waits between delivery attempts while the originating agent is not registered (it is
/// being respawned): about 30 seconds in all.
const DELIVERY_BACKOFF_MS: [u64; 6] = [500, 1_000, 2_000, 4_000, 8_000, 14_000];

const NO_ANSWER: &str = "the piece ended without an answer";
const AGENT_RESTARTED: &str = "the agent was restarted";

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

pub struct PieceSpec {
    pub executor_agent: AgentId,
    pub prompt: String,
}

/// Who may cancel a job: the agent that launched it (from any of its sessions), or a person
/// from the very conversation it was launched from.
pub enum CancelScope {
    Agent(AgentId),
    Origin(VizierSession),
}

impl CancelScope {
    fn allows(&self, job: &BackgroundJob) -> bool {
        match self {
            Self::Agent(agent) => &job.origin.0 == agent,
            Self::Origin(session) => &job.origin == session,
        }
    }
}

pub enum CancelOutcome {
    Cancelled {
        job: BackgroundJob,
        entries: Vec<ReportEntry>,
        nested_ids: Vec<BackgroundJobId>,
    },
    AlreadyFinished(BackgroundJob),
    NotFound,
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
        VizierResponseContent::Error { message, .. } => {
            Some(PieceOutcome::Failed(message.clone()))
        }
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

/// Read a piece's frames until the first terminal one.
///
/// A channel that closes before any terminal frame means the turn's sender was dropped
/// without answering, which is what happens when the executor's process is shut down
/// (an agent respawn): a turn that merely fails still sends an `Error` frame. So the
/// closed channel is reported as a restart (research D9).
async fn first_terminal(rx: &flume::Receiver<VizierResponse>) -> PieceOutcome {
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

/// Resolves once the job's cancel signal fires; never resolves otherwise.
async fn cancelled(mut rx: watch::Receiver<bool>) {
    loop {
        if *rx.borrow_and_update() {
            return;
        }
        if rx.changed().await.is_err() {
            // The sender is gone without signalling: the runner removed it on its own exit.
            std::future::pending::<()>().await;
        }
    }
}

#[derive(Clone)]
pub struct BackgroundJobs {
    storage: Arc<VizierStorage>,
    transport: VizierTransport,
    /// One cancel signal per live runner. It only *signals*; the guarded database transition
    /// decides whether a cancel took effect, so losing this map (a restart) cannot break the
    /// exactly-once rule.
    cancels: Arc<Mutex<HashMap<BackgroundJobId, watch::Sender<bool>>>>,
}

impl BackgroundJobs {
    pub fn new(storage: Arc<VizierStorage>, transport: VizierTransport) -> Self {
        Self {
            storage,
            transport,
            cancels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Open the job, dispatch its pieces and leave a runner awaiting them. Returns at once.
    pub async fn launch(
        &self,
        ctx: &ToolContext,
        kind: JobKind,
        pieces: Vec<PieceSpec>,
        timeout_secs: u64,
    ) -> crate::Result<BackgroundJob> {
        if ctx.background_depth >= MAX_BACKGROUND_DEPTH {
            return Err(VizierError(format!(
                "background nesting limit ({MAX_BACKGROUND_DEPTH}) reached: this turn was itself started by a background result"
            )));
        }

        let origin = ctx.session.clone();
        let now = Utc::now();
        let mut job = BackgroundJob {
            id: new_job_id(),
            kind,
            origin: origin.clone(),
            depth: ctx.background_depth,
            timeout_secs,
            created_at: now,
            finished_at: None,
            state: JobState::Running,
            cancelled_by: None,
            reason: None,
            pieces: pieces
                .into_iter()
                .enumerate()
                .map(|(ordinal, spec)| {
                    // Each piece gets a topic of its own, so its drill-down shows that piece
                    // alone (research D7).
                    let channel = match kind {
                        JobKind::Batch => VizierChannelId::Subagent,
                        JobKind::Delegation => VizierChannelId::InterAgent(vec![
                            origin.0.clone(),
                            spec.executor_agent.clone(),
                        ]),
                    };
                    BackgroundPiece {
                        ordinal: ordinal as u32,
                        prompt: spec.prompt,
                        session: VizierSession(
                            spec.executor_agent,
                            channel,
                            Some(uuid::Uuid::new_v4().to_string()),
                        ),
                        started_at: now,
                        finished_at: None,
                        state: PieceState::Running,
                        reason: None,
                    }
                })
                .collect(),
        };

        self.storage
            .open_background_job(job.clone())
            .await
            .map_err(|err| VizierError(format!("failed to record background job: {err}")))?;

        let mut receivers = Vec::with_capacity(job.pieces.len());
        for piece in job.pieces.iter_mut() {
            let (tx, rx) = flume::unbounded();
            let sent = self
                .transport
                .send_request(
                    piece.session.clone(),
                    VizierRequest {
                        timestamp: Utc::now(),
                        user: origin.0.clone(),
                        content: VizierRequestContent::Prompt(piece.prompt.clone()),
                        metadata: serde_json::json!({}),
                        background_depth: job.depth + 1,
                        ..Default::default()
                    },
                    Some(tx),
                )
                .await;

            match sent {
                Ok(()) => receivers.push(Some(rx)),
                Err(err) => {
                    // A piece that could not be handed off is failed right away (FR-014);
                    // the rest of the job carries on.
                    let reason = err.to_string();
                    let finished_at = Utc::now();
                    if let Err(e) = self
                        .storage
                        .close_background_piece(
                            &job.id,
                            piece.ordinal,
                            PieceState::Failed,
                            Some(reason.clone()),
                            finished_at,
                        )
                        .await
                    {
                        tracing::warn!("failed to close background piece {}#{}: {}", job.id, piece.ordinal, e);
                    }
                    piece.state = PieceState::Failed;
                    piece.reason = Some(reason);
                    piece.finished_at = Some(finished_at);
                    receivers.push(None);
                }
            }
        }

        let (cancel_tx, cancel_rx) = watch::channel(false);
        self.cancels.lock().insert(job.id.clone(), cancel_tx);

        self.publish(&job);

        let this = self.clone();
        let runner_job = job.clone();
        tokio::spawn(async move {
            let job_id = runner_job.id.clone();
            this.run_job(runner_job, receivers, cancel_rx).await;
            this.cancels.lock().remove(&job_id);
        });

        tracing::info!(
            "launched background {} {} with {} piece(s) from {}",
            kind.as_str(),
            job.id,
            job.pieces.len(),
            origin.to_slug()
        );

        Ok(job)
    }

    /// Await every piece, then report — unless a cancel wins first.
    async fn run_job(
        &self,
        job: BackgroundJob,
        receivers: Vec<Option<flume::Receiver<VizierResponse>>>,
        cancel_rx: watch::Receiver<bool>,
    ) {
        let limit = Duration::from_secs(job.timeout_secs);
        let job = &job;
        let pieces = job.pieces.iter().zip(receivers).map(|(piece, rx)| {
            let job_id = job.id.clone();
            async move {
                // Failed at dispatch: already closed, keep its reason.
                let Some(rx) = rx else {
                    return (
                        piece.state,
                        piece.reason.clone().unwrap_or_else(|| NO_ANSWER.to_string()),
                    );
                };

                let (state, text) = match tokio::time::timeout(limit, first_terminal(&rx)).await
                {
                    Ok(PieceOutcome::Answered(text)) => (PieceState::Answered, text),
                    Ok(PieceOutcome::Failed(reason)) => (PieceState::Failed, reason),
                    Err(_) => {
                        // Stop a runaway piece spending tokens; its late answer, if any, is
                        // discarded with the receiver (FR-013).
                        self.abort_piece(&piece.session, &job.origin.0).await;
                        (
                            PieceState::TimedOut,
                            format!("No answer within {}s", job.timeout_secs),
                        )
                    }
                };
                drop(rx);

                // An answered piece's text lives in its own session's history; only a piece
                // that did not answer has a reason worth storing.
                let reason = (state != PieceState::Answered).then(|| text.clone());
                if let Err(e) = self
                    .storage
                    .close_background_piece(&job_id, piece.ordinal, state, reason, Utc::now())
                    .await
                {
                    tracing::warn!("failed to close background piece {}#{}: {}", job_id, piece.ordinal, e);
                }
                self.publish_stored(&job_id).await;

                (state, text)
            }
        });

        let outcomes = tokio::select! {
            outcomes = futures::future::join_all(pieces) => outcomes,
            // `cancel` already closed the rows and aborted the pieces; dropping the piece
            // futures here drops their receivers, so nothing late is read.
            _ = cancelled(cancel_rx) => return,
        };

        // The exactly-once guard (FR-029): moving to `reporting` races a cancel on the same
        // `state = 'running'` predicate. Losing means the job was cancelled; deliver nothing.
        match self
            .storage
            .transition_background_job(
                &job.id,
                JobState::Running,
                JobState::Reporting,
                None,
                None,
                Utc::now(),
            )
            .await
        {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => {
                tracing::warn!("failed to move background job {} to reporting: {}", job.id, e);
                return;
            }
        }
        // Published before the report is handed over, so the tray hears `reporting` before
        // the woken turn's `thinking_start`.
        self.publish_stored(&job.id).await;

        let report = BackgroundReport {
            job_id: job.id.clone(),
            kind: job.kind,
            delegated_to: job.delegated_to(),
            entries: job
                .pieces
                .iter()
                .zip(outcomes)
                .map(|(piece, (state, text))| {
                    let (text, truncated) = report::truncate(&text, report::REPORT_TEXT_LIMIT);
                    ReportEntry {
                        ordinal: piece.ordinal,
                        prompt: piece.prompt.clone(),
                        state,
                        text,
                        truncated,
                    }
                })
                .collect(),
        };

        self.deliver(job, report).await;
    }

    /// Hand the report to the originating conversation as a turn of its own, retrying while
    /// the agent is not registered (being respawned).
    async fn deliver(&self, job: &BackgroundJob, report: BackgroundReport) {
        let request = VizierRequest {
            timestamp: Utc::now(),
            user: job.origin.0.clone(),
            content: VizierRequestContent::BackgroundReport(report),
            metadata: serde_json::json!({}),
            background_depth: job.depth + 1,
            ..Default::default()
        };

        let mut last_err = None;
        for attempt in 0..=DELIVERY_BACKOFF_MS.len() {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(DELIVERY_BACKOFF_MS[attempt - 1])).await;
            }
            let mut request = request.clone();
            request.timestamp = Utc::now();
            match self
                .transport
                .send_request(
                    job.origin.clone(),
                    request,
                    Some(self.broadcast_sender(job.origin.clone())),
                )
                .await
            {
                Ok(()) => {
                    if let Err(e) = self
                        .storage
                        .transition_background_job(
                            &job.id,
                            JobState::Reporting,
                            JobState::Reported,
                            None,
                            None,
                            Utc::now(),
                        )
                        .await
                    {
                        tracing::warn!("failed to mark background job {} reported: {}", job.id, e);
                    }
                    self.publish_stored(&job.id).await;
                    return;
                }
                Err(err) => last_err = Some(err.to_string()),
            }
        }

        let reason = format!(
            "report could not be delivered: {}",
            last_err.unwrap_or_else(|| "unknown error".into())
        );
        tracing::warn!("background job {}: {}", job.id, reason);
        if let Err(e) = self
            .storage
            .transition_background_job(
                &job.id,
                JobState::Reporting,
                JobState::Undelivered,
                None,
                Some(reason),
                Utc::now(),
            )
            .await
        {
            tracing::warn!("failed to mark background job {} undelivered: {}", job.id, e);
        }
        self.publish_stored(&job.id).await;
    }

    /// A response sender that republishes every frame of the woken turn onto the
    /// session-event broadcast (research D1).
    fn broadcast_sender(&self, session: VizierSession) -> flume::Sender<VizierResponse> {
        let (tx, rx) = flume::unbounded::<VizierResponse>();
        let transport = self.transport.clone();
        tokio::spawn(async move {
            while let Ok(frame) = rx.recv_async().await {
                transport.publish_session_event(SessionEvent {
                    session: session.clone(),
                    frame: SessionFrame::Response(frame),
                });
            }
        });
        tx
    }

    /// The existing `abort` command, sent to a piece's session.
    async fn abort_piece(&self, session: &VizierSession, origin_agent: &AgentId) {
        if let Err(e) = self
            .transport
            .send_request(
                session.clone(),
                VizierRequest {
                    timestamp: Utc::now(),
                    user: origin_agent.clone(),
                    content: VizierRequestContent::Command("abort".into()),
                    metadata: serde_json::json!({}),
                    ..Default::default()
                },
                None,
            )
            .await
        {
            tracing::debug!("could not abort background piece {}: {}", session.to_slug(), e);
        }
    }

    /// Cancel a running job: stop its pieces, cancel the jobs they launched, and return what
    /// had already been answered. Never sends a report.
    pub fn cancel<'a>(
        &'a self,
        job_id: &'a str,
        by: Canceller,
        reason: Option<String>,
        scope: CancelScope,
    ) -> BoxFuture<'a, crate::Result<CancelOutcome>> {
        async move {
            let Some(job) = self.load(job_id).await? else {
                return Ok(CancelOutcome::NotFound);
            };
            if !scope.allows(&job) {
                return Ok(CancelOutcome::NotFound);
            }

            let won = self
                .storage
                .transition_background_job(
                    job_id,
                    JobState::Running,
                    JobState::Cancelled,
                    Some(by.to_column()),
                    reason.clone(),
                    Utc::now(),
                )
                .await
                .map_err(|err| VizierError(format!("failed to cancel background job: {err}")))?;
            if !won {
                let job = self.load(job_id).await?.unwrap_or(job);
                return Ok(CancelOutcome::AlreadyFinished(job));
            }

            if let Some(signal) = self.cancels.lock().remove(job_id) {
                let _ = signal.send(true);
            }

            for piece in job.pieces.iter().filter(|p| p.state == PieceState::Running) {
                self.abort_piece(&piece.session, &job.origin.0).await;
            }

            // Cascade (FR-026): jobs launched from this job's pieces. Depth is capped, so the
            // recursion is bounded.
            let mut nested_ids = vec![];
            for piece in &job.pieces {
                let children = match self
                    .storage
                    .list_running_background_jobs(piece.session.clone())
                    .await
                {
                    Ok(children) => children,
                    Err(e) => {
                        tracing::warn!("failed to list nested jobs of {}: {}", job_id, e);
                        continue;
                    }
                };
                for child in children {
                    let outcome = self
                        .cancel(
                            &child.id,
                            by.clone(),
                            Some(format!("parent job {job_id} cancelled")),
                            CancelScope::Origin(piece.session.clone()),
                        )
                        .await;
                    if let Ok(CancelOutcome::Cancelled {
                        job: child,
                        nested_ids: grandchildren,
                        ..
                    }) = outcome
                    {
                        nested_ids.push(child.id);
                        nested_ids.extend(grandchildren);
                    }
                }
            }

            let job = self.load(job_id).await?.unwrap_or(job);
            let mut entries = Vec::with_capacity(job.pieces.len());
            for piece in &job.pieces {
                let text = match piece.state {
                    PieceState::Answered => piece_answer(&self.storage, &piece.session)
                        .await
                        .unwrap_or_default(),
                    PieceState::Cancelled => String::new(),
                    _ => piece.reason.clone().unwrap_or_default(),
                };
                let (text, truncated) = report::truncate(&text, report::REPORT_TEXT_LIMIT);
                entries.push(ReportEntry {
                    ordinal: piece.ordinal,
                    prompt: piece.prompt.clone(),
                    state: piece.state,
                    text,
                    truncated,
                });
            }

            self.publish(&job);
            tracing::info!("cancelled background job {} ({})", job_id, by.to_column());

            Ok(CancelOutcome::Cancelled {
                job,
                entries,
                nested_ids,
            })
        }
        .boxed()
    }

    async fn load(&self, job_id: &str) -> crate::Result<Option<BackgroundJob>> {
        self.storage
            .get_background_job(job_id)
            .await
            .map_err(|err| VizierError(format!("failed to read background job: {err}")))
    }

    fn publish(&self, job: &BackgroundJob) {
        self.transport.publish_session_event(SessionEvent {
            session: job.origin.clone(),
            frame: SessionFrame::Job(BackgroundJobSnapshot::from(job)),
        });
    }

    /// Publish the job as stored, which is what a reloading client would read.
    async fn publish_stored(&self, job_id: &str) {
        match self.load(job_id).await {
            Ok(Some(job)) => self.publish(&job),
            Ok(None) => {}
            Err(e) => tracing::warn!("{}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{ErrorKind, VizierAttachment, VizierAttachmentContent};

    fn audio() -> VizierAttachment {
        VizierAttachment {
            filename: "a.ogg".into(),
            content: VizierAttachmentContent::Url("x".into()),
        }
    }

    #[test]
    fn terminal_frames_map_to_piece_outcomes() {
        assert_eq!(
            classify(&VizierResponseContent::Message {
                content: "hi".into(),
                stats: None
            }),
            Some(PieceOutcome::Answered("hi".into()))
        );
        assert_eq!(
            classify(&VizierResponseContent::AudioReply(audio(), Some("said".into()), None)),
            Some(PieceOutcome::Answered("said".into()))
        );
        assert_eq!(
            classify(&VizierResponseContent::AudioReply(audio(), None, None)),
            Some(PieceOutcome::Failed(NO_ANSWER.into()))
        );
        assert_eq!(
            classify(&VizierResponseContent::Error {
                kind: ErrorKind::Completion,
                message: "boom".into()
            }),
            Some(PieceOutcome::Failed("boom".into()))
        );
        assert_eq!(
            classify(&VizierResponseContent::Abort),
            Some(PieceOutcome::Failed(NO_ANSWER.into()))
        );
        assert_eq!(
            classify(&VizierResponseContent::Empty),
            Some(PieceOutcome::Failed(NO_ANSWER.into()))
        );
    }

    #[test]
    fn mid_turn_frames_are_not_terminal() {
        for frame in [
            VizierResponseContent::ThinkingStart,
            VizierResponseContent::Thinking("hmm".into()),
            VizierResponseContent::ToolChoice {
                name: "t".into(),
                args: serde_json::json!({}),
            },
            VizierResponseContent::ToolResponse {
                response: serde_json::json!("ok"),
            },
            VizierResponseContent::Checkpoint { handover: None },
        ] {
            assert_eq!(classify(&frame), None, "{frame:?}");
        }
    }

    #[tokio::test]
    async fn a_closed_channel_without_an_answer_reads_as_a_restart() {
        let (tx, rx) = flume::unbounded::<VizierResponse>();
        tx.send(VizierResponse {
            timestamp: Utc::now(),
            content: VizierResponseContent::ThinkingStart,
            attachments: vec![],
        })
        .unwrap();
        drop(tx);
        assert_eq!(
            first_terminal(&rx).await,
            PieceOutcome::Failed(AGENT_RESTARTED.into())
        );
    }

    #[test]
    fn timeout_validation() {
        assert_eq!(validate_timeout(None).unwrap(), DEFAULT_TIMEOUT_SECS);
        assert_eq!(validate_timeout(Some(3)).unwrap(), 3);
        assert!(validate_timeout(Some(0)).is_err());
        assert!(validate_timeout(Some(3601)).is_err());
    }

    /// FR-012: a turn already at the depth cap may not launch.
    #[tokio::test]
    async fn the_depth_cap_refuses_a_launch() {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_background_job_schema(&conn).unwrap();
        let storage = VizierStorage::new(crate::storage::sqlite::SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(crate::storage::document::LocalDocumentStore::new(
                dir.path().to_path_buf(),
            )),
        ));
        let jobs = BackgroundJobs::new(Arc::new(storage), VizierTransport::new());
        let ctx = ToolContext {
            session: VizierSession("a".into(), VizierChannelId::Subagent, None),
            pending_attachments: Arc::new(tokio::sync::Mutex::new(vec![])),
            hooks: None,
            background_depth: MAX_BACKGROUND_DEPTH,
        };

        let err = jobs
            .launch(
                &ctx,
                JobKind::Batch,
                vec![PieceSpec {
                    executor_agent: "a".into(),
                    prompt: "x".into(),
                }],
                DEFAULT_TIMEOUT_SECS,
            )
            .await
            .unwrap_err();
        assert!(err.0.contains("background nesting limit (3) reached"), "{}", err.0);
    }
}
