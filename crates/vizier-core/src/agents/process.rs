use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use crate::llm::types::Message;
use anyhow::Result;
use chrono::Utc;
use tokio::sync::watch;
use tokio::task::{AbortHandle, Id as TaskId, JoinHandle, JoinSet};

use crate::{
    agents::steps::{StopReason, TurnRef, seal_open_calls},
    agents::{
        agent::VizierAgent,
        hook::{
            VizierSessionHooks, debug::DebugHook, handover::HandoverSenderHook,
            thinking::ThinkingHook, tool_calls::ToolCallsHook,
        },
        skill::VizierSkills,
        tools::ToolContext,
    },
    dependencies::VizierDependencies,
    indexer::VizierIndexer,
    schema::{
        AgentConfig, AgentId, DreamStage, ErrorKind, SessionHistoryContent, VizierChannelId,
        VizierRequest, VizierRequestContent, VizierResponse, VizierResponseContent, VizierSession,
        VizierSessionDetail, dream_journal::DreamJournalEntry, history_entries_to_messages,
    },
    skill::invocation::{self, Invoked, Parsed},
    storage::{
        VizierStorage,
        dream_journal::DreamJournalStorage,
        history::{HistoryStorage, save_with_busy_retry},
        memory::MemoryStorage,
        scratch::ScratchStorage,
        session::SessionStorage,
        turn::TurnStorage,
    },
    transport::{DreamCommand, SessionEvent, SessionFrame, VizierRequestEnvelope, VizierTransport},
};

/// Requests waiting for the session's running job, each with where its response goes.
type SessionQueue = VecDeque<(
    VizierRequest,
    Option<flume::Sender<VizierResponse>>,
    TurnRef,
)>;

fn abort_session_silent(
    session: &VizierSession,
    turns: &mut SessionTurns,
    thinking_handles: &mut HashMap<VizierSession, Arc<JoinHandle<()>>>,
    session_queues: &mut HashMap<VizierSession, SessionQueue>,
    storage: &VizierStorage,
) -> bool {
    let had_in_flight = turns.abort(session);
    if let Some(handle) = thinking_handles.remove(session) {
        handle.abort();
    }
    let storage_clone = storage.clone();
    let session_clone = session.clone();
    tokio::spawn(async move {
        let _ = storage_clone
            .update_thinking_state(session_clone.0, session_clone.1, session_clone.2, false)
            .await;
    });
    session_queues.remove(session);

    had_in_flight
}

async fn abort_session_notify(
    session: &VizierSession,
    turns: &mut SessionTurns,
    thinking_handles: &mut HashMap<VizierSession, Arc<JoinHandle<()>>>,
    session_queues: &mut HashMap<VizierSession, SessionQueue>,
    storage: &VizierStorage,
    transport: &VizierTransport,
    response_tx: &Option<flume::Sender<VizierResponse>>,
) {
    // The aborted turn's own sender hears it too: Discord and Telegram send `/abort` with
    // no sender of their own, and only the turn's renderer can say "thinking aborted".
    let turn_tx = turns.response_tx(session);
    let had_in_flight =
        abort_session_silent(session, turns, thinking_handles, session_queues, storage);
    // A call the abort cut off gets its result now. The task may still be finishing a save
    // as it is dropped; a doubled result for one call is dropped again on replay.
    if had_in_flight {
        seal_open_calls(storage, transport, session, None, StopReason::Aborted).await;
    }
    // Nothing of the session is left to resume: the aborted turn and everything queued
    // behind it are dropped, so a restart does not bring them back.
    if let Err(e) = storage.cancel_session_turns(session).await {
        tracing::warn!(agent_id = %session.0, "could not cancel the session's journaled turns: {e}");
    }
    // The turn and its queue are gone, so a message sent straight after starts afresh.
    clear_idle_scratch(storage, session).await;
    if had_in_flight {
        for tx in [turn_tx.as_ref(), response_tx.as_ref()]
            .into_iter()
            .flatten()
        {
            let _ = tx
                .send_async(VizierResponse {
                    timestamp: chrono::Utc::now(),
                    content: crate::schema::VizierResponseContent::Abort,
                    attachments: vec![],
                    ..Default::default()
                })
                .await;
        }
    }
}

/// Serve one built agent's requests until `shutdown_rx` turns true or the request channel is
/// closed. Building the agent and registering its channel belong to the supervisor in
/// `agents/mod.rs`, which owns `recv` across restarts so requests wait rather than fail (#43).
pub async fn agent_process(
    agent: Arc<VizierAgent>,
    agent_id: AgentId,
    deps: VizierDependencies,
    agent_config: AgentConfig,
    indexer: Option<crate::indexer::VizierIndexer>,
    recv: flume::Receiver<VizierRequestEnvelope>,
    mut shutdown_rx: watch::Receiver<bool>,
    deleting: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    let mut turns = SessionTurns::default();
    let mut thinking_handles = HashMap::<VizierSession, Arc<JoinHandle<()>>>::new();
    let mut detail_tasks = JoinSet::new();
    let mut side_chats = JoinSet::new();

    let mut session_queues = HashMap::<VizierSession, SessionQueue>::new();
    let titling = TitlingSessions::default();
    let jobs = SessionJobs {
        agent: agent.clone(),
        agent_config: agent_config.clone(),
        deps: deps.clone(),
        indexer: indexer.clone(),
    };

    tracing::info!(agent_id = %agent_id, "agent process loop started");

    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                // A dropped sender means the supervisor is gone: stop as well, rather than
                // spin on a `changed()` that now returns at once.
                if changed.is_err() || *shutdown_rx.borrow() {
                    tracing::info!(agent_id = %agent_id, "shutdown signal received");
                    break;
                }
            }
            result = recv.recv_async() => {
                let Ok(envelope) = result else { break };
                let session = envelope.session;
                let request = envelope.request;
                let response_tx = envelope.response_tx;
                let turn = TurnRef { id: envelope.turn_id, resumed: envelope.resumed };
                tracing::trace!(agent_id = %session.0, channel = ?session.1, "incoming request");

                // A side chat must leave no trace (FR-006): it never creates or titles the
                // session's detail row, and never touches the conversation itself. The
                // on-the-record decision lives in one testable predicate (research R2).
                if records_session_activity(&request.content) {
                    detail_tasks.spawn(refresh_session_detail(
                        agent.clone(),
                        deps.storage.clone(),
                        session.clone(),
                        request.clone(),
                        titling.clone(),
                        deps.transport.clone(),
                    ));
                }

                // A side chat answers beside the conversation, never in its queue: a long turn
                // (a coding turn in a project chat) would otherwise hold it until it timed out.
                // It reads the history saved so far and leaves the running turn alone. Dropping
                // this set with the loop ends an unfinished one, so its sender sees the channel
                // close and reports the agent unavailable.
                if request.content.is_side_chat() {
                    side_chats.spawn(run_btw(
                        agent.clone(),
                        deps.storage.clone(),
                        session,
                        request,
                        response_tx,
                    ));
                    continue;
                }

                // Handle abort command
                if let VizierRequestContent::Command(ref cmd) = request.content {
                    if cmd == "abort" {
                        // Save command to history for display
                        let _ = deps.storage
                            .save_session_history(
                                session.clone(),
                                SessionHistoryContent::Command(cmd.clone()),
                            )
                            .await;

                        abort_session_notify(
                            &session,
                            &mut turns,
                            &mut thinking_handles,
                            &mut session_queues,
                            &deps.storage,
                            &deps.transport,
                            &response_tx,
                        )
                        .await;
                        continue;
                    }
                }

                // Handle dream command
                if let VizierRequestContent::Command(ref cmd) = request.content {
                    if cmd == "dream" {
                        // Send "Dream cycle started" response
                        if let Some(ref tx) = response_tx {
                            let _ = tx
                                .send_async(VizierResponse {
                                    timestamp: chrono::Utc::now(),
                                    content: VizierResponseContent::Message {
                                        content: "Dream cycle started.".to_string(),
                                        stats: None,
                                    },
                                    attachments: vec![],
                                    ..Default::default()
                                })
                                .await;
                        }
                        // Trigger dream via transport channel
                        let _ = deps
                            .transport
                            .send_dream_command(DreamCommand {
                                agent_id: agent_id.clone(),
                                cycle_id: None,
                            })
                            .await;
                        continue;
                    }
                }

                // A handover runs as a job in the session's queue rather than aborting it:
                // the turn in progress finishes first, and messages sent before or during
                // the handover carry on after it, reading the new checkpoint (#57).
                if is_handover_command(&request) && session_is_busy(&turns, &session_queues, &session) {
                    queue_handover(session_queues.entry(session.clone()).or_default(), (request, response_tx, turn));
                    continue;
                }

                // Queue message if a task is already running for this session
                if session_is_busy(&turns, &session_queues, &session) {
                    tracing::debug!(agent_id = %session.0, "queuing message while task in progress");
                    session_queues.entry(session.clone()).or_default().push_back((request, response_tx, turn));
                    continue;
                }

                start_session_job(
                    &jobs,
                    session,
                    request,
                    response_tx,
                    turn,
                    &mut turns,
                    &mut thinking_handles,
                );
            }
            // A turn ended — returned, panicked or was aborted. Learned from the task itself
            // ending, so a turn that never reaches its last line still releases its session.
            Some(finished) = turns.join_next(), if !turns.is_empty() => {
                // Done or panicked, the turn is over and its row goes. An aborted one was
                // cancelled by the `/abort` that stopped it.
                if finished.outcome != TurnOutcome::Cancelled
                    && let Some(id) = finished.turn.id
                    && let Err(e) = deps.storage.finish_turn(id).await
                {
                    tracing::warn!(agent_id = %agent_id, "could not close a turn's row: {e}");
                }
                // An aborted turn, or one a newer turn has replaced: `/abort` already reset
                // the session, and the queue belongs to whatever runs there now.
                if !finished.current {
                    if !session_is_busy(&turns, &session_queues, &finished.session) {
                        clear_idle_scratch(&deps.storage, &finished.session).await;
                    }
                    continue;
                }
                let session = finished.session;
                if finished.outcome == TurnOutcome::Panicked {
                    seal_open_calls(&deps.storage, &deps.transport, &session, None, StopReason::InternalError).await;
                }
                if finished.outcome != TurnOutcome::Done {
                    recover_failed_turn(&session, finished.outcome, finished.response_tx, &mut thinking_handles, &deps.storage);
                }

                if let Some((next_request, response_tx, next_turn)) = session_queues
                    .get_mut(&session)
                    .and_then(|queue| queue.pop_front())
                {
                    start_session_job(
                        &jobs,
                        session,
                        next_request,
                        response_tx,
                        next_turn,
                        &mut turns,
                        &mut thinking_handles,
                    );
                } else {
                    // Idle: nothing per-session needs keeping until it is next used (#49).
                    session_queues.remove(&session);
                    thinking_handles.remove(&session);
                    // Awaited here, not spawned: no turn of the session can start between
                    // "idle" and "cleared".
                    clear_idle_scratch(&deps.storage, &session).await;
                }
            }
            Some(_) = detail_tasks.join_next(), if !detail_tasks.is_empty() => {}
            Some(_) = side_chats.join_next(), if !side_chats.is_empty() => {}
        }
    }

    // This instance's turns stop with it, so none outlives an update or delete and carries
    // on with the old config, tools and MCP clients (#52).
    //
    // An update or restart resumes what it stopped on the next instance (`specs/023`), and
    // says nothing to the sender; a delete tells the sender, as before.
    let replacing = !deleting.load(std::sync::atomic::Ordering::SeqCst);
    let mut resumed_turns = Vec::new();
    for (session, response_tx, turn) in turns.stop_all(TURN_STOP_TIMEOUT).await {
        seal_open_calls(
            &deps.storage,
            &deps.transport,
            &session,
            None,
            StopReason::AgentRestarted,
        )
        .await;
        if replacing && turn.id.is_some() {
            if let Some(handle) = thinking_handles.remove(&session) {
                handle.abort();
            }
            resumed_turns.push((
                session,
                response_tx,
                TurnRef {
                    resumed: true,
                    ..turn
                },
            ));
            continue;
        }
        notify_turn_stopped(&session, response_tx, &mut thinking_handles, &deps.storage).await;
        // A session whose queue is handed on carries on in the next instance, scratch and all.
        if session_queues
            .get(&session)
            .is_none_or(|queue| queue.is_empty())
        {
            clear_idle_scratch(&deps.storage, &session).await;
        }
    }
    hand_on_queued(session_queues, resumed_turns, &deps).await;
    Ok(())
}

/// Queues a handover ahead of the queued messages and behind any handover already waiting.
fn queue_handover(
    queue: &mut SessionQueue,
    item: (
        VizierRequest,
        Option<flume::Sender<VizierResponse>>,
        TurnRef,
    ),
) {
    let position = queue
        .iter()
        .position(|(queued, _, _)| !is_handover_command(queued))
        .unwrap_or(queue.len());
    queue.insert(position, item);
}

/// A session with nothing running or queued loses its scratch files (`specs/020-scratchpad-tools`
/// research R4). A failure is logged: the leftovers go at the next idle or at startup.
async fn clear_idle_scratch(storage: &VizierStorage, session: &VizierSession) {
    if let Err(e) = storage.clear_scratch(session).await {
        tracing::warn!(agent_id = %session.0, channel = ?session.1, "failed to clear scratch files: {e}");
    }
}

/// How long a stopping agent waits for its aborted turns to end.
const TURN_STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// A turn cut short because its agent stopped: the session stops showing as thinking,
/// and whoever sent the request hears why there is no answer.
async fn notify_turn_stopped(
    session: &VizierSession,
    response_tx: Option<flume::Sender<VizierResponse>>,
    thinking_handles: &mut HashMap<VizierSession, Arc<JoinHandle<()>>>,
    storage: &VizierStorage,
) {
    if let Some(handle) = thinking_handles.remove(session) {
        handle.abort();
    }
    let _ = storage
        .update_thinking_state(
            session.0.clone(),
            session.1.clone(),
            session.2.clone(),
            false,
        )
        .await;
    if let Some(tx) = response_tx {
        let _ = tx
            .send_async(VizierResponse {
                timestamp: chrono::Utc::now(),
                content: VizierResponseContent::Error {
                    kind: ErrorKind::AgentUnavailable,
                    message: "The agent was updated or stopped while working on this, so the \
                              turn was cut short. Send it again to retry."
                        .to_string(),
                },
                attachments: vec![],
                ..Default::default()
            })
            .await;
    }
}

/// Requests still queued when the agent stops go back on its request channel, which the
/// next instance inherits (an update, a restart). With no agent left to take them (a
/// delete), their senders are told so.
async fn hand_on_queued(
    session_queues: HashMap<VizierSession, SessionQueue>,
    resumed_turns: Vec<(
        VizierSession,
        Option<flume::Sender<VizierResponse>>,
        TurnRef,
    )>,
    deps: &VizierDependencies,
) {
    // Turns that were running go first, ahead of their session's queue, on their own sender.
    for (session, response_tx, turn) in resumed_turns {
        let request = match turn.id {
            Some(id) => deps
                .storage
                .pending_turns(&session.0)
                .await
                .ok()
                .and_then(|rows| rows.into_iter().find(|row| row.id == id))
                .map(|row| row.request),
            None => None,
        };
        let Some(request) = request else { continue };
        if let Err(e) = deps
            .transport
            .resend(VizierRequestEnvelope {
                session: session.clone(),
                request,
                response_tx,
                turn_id: turn.id,
                resumed: true,
            })
            .await
        {
            tracing::debug!(agent_id = %session.0, "could not hand a running turn on: {e}");
        }
    }
    for (session, queue) in session_queues {
        for (request, response_tx, turn) in queue {
            if let Err(e) = deps
                .transport
                .resend(VizierRequestEnvelope {
                    session: session.clone(),
                    request,
                    response_tx: response_tx.clone(),
                    turn_id: turn.id,
                    resumed: turn.resumed,
                })
                .await
            {
                tracing::debug!(agent_id = %session.0, "dropping a queued request: {e}");
                if let Some(tx) = response_tx {
                    let _ = tx
                        .send_async(VizierResponse {
                            timestamp: chrono::Utc::now(),
                            content: VizierResponseContent::Error {
                                kind: ErrorKind::AgentUnavailable,
                                message: "The agent was removed before it got to this.".to_string(),
                            },
                            attachments: vec![],
                            ..Default::default()
                        })
                        .await;
                }
            }
        }
    }
}

/// Sessions with a title being generated, so a burst of messages (a busy group chat) starts
/// one generation rather than one each. Holds only generations in flight.
type TitlingSessions = Arc<std::sync::Mutex<std::collections::HashSet<VizierSession>>>;

/// A session's claim on title generation, released however the generation ends.
struct TitlingClaim {
    titling: TitlingSessions,
    session: VizierSession,
}

impl TitlingClaim {
    fn take(titling: &TitlingSessions, session: &VizierSession) -> Option<Self> {
        let mut in_flight = titling
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        in_flight.insert(session.clone()).then(|| Self {
            titling: titling.clone(),
            session: session.clone(),
        })
    }
}

impl Drop for TitlingClaim {
    fn drop(&mut self) {
        self.titling
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.session);
    }
}

/// Keeps the session's detail in step with an arriving request: created, titled with the
/// topic slug, if it does not exist yet; given a generated title once it has enough requests.
///
/// Both decisions read storage, never a count kept in memory: such a count grew with every
/// session ever seen and restarted from zero with the process, so the first message after a
/// restart re-created the detail and reset its title to the slug (#49).
async fn refresh_session_detail(
    agent: Arc<VizierAgent>,
    storage: Arc<VizierStorage>,
    session: VizierSession,
    request: VizierRequest,
    titling: TitlingSessions,
    transport: VizierTransport,
) {
    let VizierSession(agent_id, channel, topic) = session.clone();
    let slug_title = topic.clone().unwrap_or("DEFAULT".to_string());
    let titled_session = session.clone();

    let existing = match storage
        .get_session_detail_by_topic(agent_id.clone(), channel.clone(), topic.clone())
        .await
    {
        Ok(existing) => existing,
        Err(err) => {
            tracing::warn!(agent_id = %agent_id, "failed to read session detail: {err}");
            return;
        }
    };
    // A session without a detail row is new: record it, then title it from this first request.
    let existing = match existing {
        Some(existing) => existing,
        None => {
            let detail = VizierSessionDetail {
                agent_id: agent_id.clone(),
                channel: channel.clone(),
                topic: topic.clone(),
                title: slug_title.clone(),
                is_thinking: true,
            };
            let _ = storage.save_session_detail(detail.clone()).await;
            detail
        }
    };

    // Titled already, by generation or by someone renaming it. A project session created empty
    // carries the placeholder until its first request.
    if existing.title != slug_title && existing.title != crate::schema::NEW_SESSION_TITLE {
        return;
    }
    let Some(_claim) = TitlingClaim::take(&titling, &session) else {
        return;
    };
    let Ok(request_prompt) = request.to_prompt() else {
        return;
    };

    let prompt = format!(
        r#"summarize (don't execute) the prompt below into a 60 character title:
"{}"

**only response the summarize title**"#,
        request_prompt
    );
    let ctx = ToolContext {
        loaded_images: Default::default(),
        recalled: Default::default(),
        client: None,
        session,
        pending_attachments: Arc::new(Mutex::new(vec![])),
        hooks: None,
        background_depth: 0,
        deadline: None,
        dream_sources: vec![],
        secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
        project: None,
    };
    let Ok((title, _, _, _)) = agent
        .prompt(
            Message::User(vec![crate::llm::types::Part::Text(prompt)]),
            vec![],
            0,
            None,
            false,
            &ctx,
        )
        .await
    else {
        return;
    };

    let mut title = title;
    title.truncate(60);
    if title.starts_with('"') {
        title.remove(0);
    }
    if title.ends_with('"') {
        title.pop();
    }

    let detail = VizierSessionDetail {
        agent_id,
        channel,
        topic,
        title: title.clone(),
        is_thinking: existing.is_thinking,
    };
    if storage.update_session_detail(detail).await.is_ok() {
        transport.publish_session_event(SessionEvent {
            session: titled_session,
            frame: SessionFrame::Title { title },
        });
    }
}

/// What every job started on a session needs, shared by the two places one starts: a
/// request arriving at an idle session, and the next queued one when a job completes.
struct SessionJobs {
    agent: Arc<VizierAgent>,
    agent_config: AgentConfig,
    deps: VizierDependencies,
    indexer: Option<VizierIndexer>,
}

/// How a session's turn ended, as seen from its task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnOutcome {
    Done,
    Panicked,
    Cancelled,
}

/// A turn whose task has ended, with where its response was going.
struct FinishedTurn {
    session: VizierSession,
    response_tx: Option<flume::Sender<VizierResponse>>,
    turn: TurnRef,
    outcome: TurnOutcome,
    /// Still the session's running turn when it ended. False when `/abort` stopped it, or
    /// a newer turn was started in its place: the session is no longer this turn's to hand on.
    current: bool,
}

/// The sessions' running turns. A turn is known to have ended because its task ended,
/// not because it reported so on its way out — a turn that panics never gets there, and
/// would otherwise hold its session (and every message queued behind it) forever (#46).
#[derive(Default)]
struct SessionTurns {
    tasks: JoinSet<()>,
    running: HashMap<VizierSession, (TaskId, AbortHandle)>,
    spawned: HashMap<
        TaskId,
        (
            VizierSession,
            Option<flume::Sender<VizierResponse>>,
            TurnRef,
        ),
    >,
}

impl SessionTurns {
    fn spawn<F>(
        &mut self,
        session: VizierSession,
        response_tx: Option<flume::Sender<VizierResponse>>,
        turn_ref: TurnRef,
        turn: F,
    ) where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let handle = self.tasks.spawn(turn);
        let id = handle.id();
        self.spawned
            .insert(id, (session.clone(), response_tx, turn_ref));
        self.running.insert(session, (id, handle));
    }

    /// Where the session's running turn sends its response, if it has a turn and a sender.
    fn response_tx(&self, session: &VizierSession) -> Option<flume::Sender<VizierResponse>> {
        let (id, _) = self.running.get(session)?;
        self.spawned.get(id)?.1.clone()
    }

    fn is_running(&self, session: &VizierSession) -> bool {
        self.running.contains_key(session)
    }

    fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Stops the session's running turn. True if there was one. The session is free at
    /// once; the stopped task still comes out of `join_next`, as not `current`.
    fn abort(&mut self, session: &VizierSession) -> bool {
        match self.running.remove(session) {
            Some((_, handle)) => {
                handle.abort();
                true
            }
            None => false,
        }
    }

    /// Cancel-safe: the only await is `JoinSet::join_next_with_id`, which is.
    async fn join_next(&mut self) -> Option<FinishedTurn> {
        loop {
            let (id, outcome) = match self.tasks.join_next_with_id().await? {
                Ok((id, ())) => (id, TurnOutcome::Done),
                Err(err) if err.is_panic() => (err.id(), TurnOutcome::Panicked),
                Err(err) => (err.id(), TurnOutcome::Cancelled),
            };
            let Some((session, response_tx, turn)) = self.spawned.remove(&id) else {
                continue;
            };
            let current = self
                .running
                .get(&session)
                .is_some_and(|(running, _)| *running == id);
            if current {
                self.running.remove(&session);
            }
            return Some(FinishedTurn {
                session,
                response_tx,
                turn,
                outcome,
                current,
            });
        }
    }

    /// Stops every turn and waits up to `timeout` for their tasks to end. Returns each
    /// session that had a running turn, with where its response was going; turns already
    /// aborted (not running) told their sender when they were.
    async fn stop_all(
        &mut self,
        timeout: Duration,
    ) -> Vec<(
        VizierSession,
        Option<flume::Sender<VizierResponse>>,
        TurnRef,
    )> {
        let stopped = self
            .running
            .drain()
            .filter_map(|(_, (id, _))| self.spawned.remove(&id))
            .collect();
        self.spawned.clear();
        self.tasks.abort_all();
        let drained = tokio::time::timeout(timeout, async {
            while self.tasks.join_next().await.is_some() {}
        })
        .await;
        if drained.is_err() {
            tracing::warn!("turns did not stop in time; leaving them aborted");
        }
        stopped
    }
}

/// What a turn does on its way out, done for it when it never got there: the session
/// stops showing as thinking, and whoever sent the request hears that it failed.
fn recover_failed_turn(
    session: &VizierSession,
    outcome: TurnOutcome,
    response_tx: Option<flume::Sender<VizierResponse>>,
    thinking_handles: &mut HashMap<VizierSession, Arc<JoinHandle<()>>>,
    storage: &VizierStorage,
) {
    if outcome == TurnOutcome::Panicked {
        tracing::error!(agent_id = %session.0, channel = ?session.1, "turn panicked; releasing its session");
    }
    if let Some(handle) = thinking_handles.remove(session) {
        handle.abort();
    }
    let storage = storage.clone();
    let session = session.clone();
    tokio::spawn(async move {
        let _ = storage
            .update_thinking_state(session.0, session.1, session.2, false)
            .await;
        if outcome == TurnOutcome::Panicked
            && let Some(tx) = response_tx
        {
            let message = "The turn failed with an internal error and was stopped.".to_string();
            let _ = tx
                .send_async(VizierResponse {
                    timestamp: chrono::Utc::now(),
                    content: VizierResponseContent::Error {
                        kind: ErrorKind::classify(&message),
                        message,
                    },
                    attachments: vec![],
                    ..Default::default()
                })
                .await;
        }
    });
}

/// `/checkpoint` and `/lobotomy`: commands that write a new checkpoint for the session.
fn is_handover_command(request: &VizierRequest) -> bool {
    matches!(&request.content, VizierRequestContent::Command(cmd) if cmd == "checkpoint" || cmd == "lobotomy")
}

/// Whether handling this request records anything about its conversation: history, session
/// detail, the thinking flag. Only the side chat is excluded (research R2) — every other
/// kind is an on-the-record turn. One predicate, so both gates and the tests read one rule.
fn records_session_activity(content: &VizierRequestContent) -> bool {
    !matches!(content, VizierRequestContent::Btw(_))
}

/// A job is running, or one has finished and the queue it leaves behind has not been
/// picked up yet: its completion is still on its way, and starting a request now would
/// jump ahead of the queue. Every turn's end reaches the loop through `SessionTurns`,
/// however it ended, so a non-empty queue is always picked up.
fn session_is_busy(
    turns: &SessionTurns,
    session_queues: &HashMap<VizierSession, SessionQueue>,
    session: &VizierSession,
) -> bool {
    turns.is_running(session)
        || session_queues
            .get(session)
            .is_some_and(|queue| !queue.is_empty())
}

/// Runs one request as the session's job. Registered in `turns`, so anything that
/// arrives meanwhile queues behind it, `/abort` stops it, and its end starts the next
/// queued request.
fn start_session_job(
    jobs: &SessionJobs,
    session: VizierSession,
    request: VizierRequest,
    response_tx: Option<flume::Sender<VizierResponse>>,
    turn: TurnRef,
    turns: &mut SessionTurns,
    thinking_handles: &mut HashMap<VizierSession, Arc<JoinHandle<()>>>,
) {
    // handle thinking
    if let Some(handle) = thinking_handles.get(&session) {
        handle.abort();
    }
    let thinking_response_tx = response_tx.clone();
    let thinking_request = request.clone();
    let thinking_handle = Arc::new(tokio::spawn(async move {
        if matches!(
            thinking_request.content,
            VizierRequestContent::Chat(_)
                | VizierRequestContent::AudioChat(_, _)
                | VizierRequestContent::BackgroundReport(_)
                | VizierRequestContent::ClientEvent(_)
        ) {
            if let Some(ref tx) = thinking_response_tx {
                let _ = tx
                    .send_async(VizierResponse {
                        timestamp: chrono::Utc::now(),
                        content: crate::schema::VizierResponseContent::ThinkingStart,
                        attachments: vec![],
                        ..Default::default()
                    })
                    .await;
            }
        }
    }));
    thinking_handles.insert(session.clone(), thinking_handle.clone());

    let agent = jobs.agent.clone();
    let agent_config = jobs.agent_config.clone();
    let deps = jobs.deps.clone();
    let storage = deps.storage.clone();
    let indexer = jobs.indexer.clone();
    let turn_session = session.clone();
    let turn_response_tx = response_tx.clone();
    turns.spawn(turn_session, turn_response_tx, turn, async move {
        if let Some(id) = turn.id
            && let Err(e) = storage.start_turn(id).await
        {
            tracing::warn!("could not mark a turn running: {e}");
        }
        // Set is_thinking = true. A side chat must not make the conversation look busy
        // with a turn that never appears in it (research R2).
        let records_activity = records_session_activity(&request.content);
        if records_activity {
            let _ = storage
                .update_thinking_state(
                    session.0.clone(),
                    session.1.clone(),
                    session.2.clone(),
                    true,
                )
                .await;
        }

        let result = match &request.content {
            VizierRequestContent::Command(cmd) if cmd == "checkpoint" => {
                run_checkpoint(&agent, &storage, &session, &response_tx).await;
                Ok(())
            }
            VizierRequestContent::Command(cmd) if cmd == "lobotomy" => {
                run_lobotomy(&storage, &session, &response_tx).await;
                Ok(())
            }
            // A side chat is answered by `run_btw`, which never saves anything: no history,
            // no session detail, no turn journal, no session events (research R1/R2).
            VizierRequestContent::Btw(_) => {
                run_btw(agent.clone(), storage.clone(), session.clone(), request.clone(), response_tx.clone()).await;
                Ok(())
            }
            _ => {
                handle_request(
                    agent.clone(),
                    agent_config.clone(),
                    session.clone(),
                    request.clone(),
                    response_tx.clone(),
                    storage.clone(),
                    indexer.clone(),
                    &deps,
                    turn,
                )
                .await
            }
        };

        if let Err(err) = result {
            tracing::error!("{}", err);
            if let Some(ref tx) = response_tx {
                let err_str = err.to_string();
                let _ = tx
                    .send_async(VizierResponse {
                        timestamp: chrono::Utc::now(),
                        content: VizierResponseContent::Error {
                            kind: ErrorKind::classify(&err_str),
                            message: err_str,
                        },
                        attachments: vec![],
                        ..Default::default()
                    })
                    .await;
            }
        }

        thinking_handle.abort();

        // Set is_thinking = false (skipped for a side chat, as above).
        if records_activity {
            let _ = storage
                .update_thinking_state(
                    session.0.clone(),
                    session.1.clone(),
                    session.2.clone(),
                    false,
                )
                .await;
        }
    });
}

async fn send_text(response_tx: &Option<flume::Sender<VizierResponse>>, content: String) {
    if let Some(tx) = response_tx {
        let _ = tx
            .send_async(VizierResponse {
                timestamp: chrono::Utc::now(),
                content: VizierResponseContent::Message {
                    content,
                    stats: None,
                },
                attachments: vec![],
                ..Default::default()
            })
            .await;
    }
}

async fn send_checkpoint(
    response_tx: &Option<flume::Sender<VizierResponse>>,
    handover: Option<String>,
) {
    if let Some(tx) = response_tx {
        let _ = tx
            .send_async(VizierResponse {
                timestamp: chrono::Utc::now(),
                content: VizierResponseContent::Checkpoint { handover },
                attachments: vec![],
                ..Default::default()
            })
            .await;
    }
}

/// `/checkpoint`: summarises the session so far into a handover and saves it as a checkpoint.
async fn run_checkpoint(
    agent: &VizierAgent,
    storage: &VizierStorage,
    session: &VizierSession,
    response_tx: &Option<flume::Sender<VizierResponse>>,
) {
    // Saved when the handover starts, not when the command arrived, so it lands after the
    // turn it waited for.
    let _ = storage
        .save_session_history(
            session.clone(),
            SessionHistoryContent::Command("checkpoint".to_string()),
        )
        .await;

    let history = match storage
        .list_session_history(session.clone(), None, None, None)
        .await
    {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Failed to get session history for checkpoint: {}", e);
            send_text(
                response_tx,
                "Failed to create checkpoint: could not retrieve history.".to_string(),
            )
            .await;
            return;
        }
    };

    let messages = history_entries_to_messages(&history);
    let reactions = crate::agents::agent::system_prompt::reactions::reaction_digest(&history, 0);
    let ctx = ToolContext {
        loaded_images: Default::default(),
        recalled: Default::default(),
        client: None,
        session: session.clone(),
        pending_attachments: Arc::new(Mutex::new(vec![])),
        hooks: None,
        background_depth: 0,
        deadline: None,
        dream_sources: vec![],
        secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
        // The handover is a summary, not a project turn; the rebuilt prompt gets the section.
        project: None,
    };

    // Generate handover
    let handover = match agent
        .generate_handover_message(&messages, reactions.as_deref(), &ctx)
        .await
    {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Failed to generate handover: {}", e);
            send_text(response_tx, format!("Failed to create checkpoint: {}", e)).await;
            return;
        }
    };

    // Save checkpoint
    if let Err(e) = storage
        .save_checkpoint(session.clone(), handover.clone())
        .await
    {
        tracing::error!("Failed to save checkpoint: {}", e);
        send_text(response_tx, format!("Failed to save checkpoint: {}", e)).await;
        return;
    }

    send_checkpoint(response_tx, handover).await;
    tracing::info!("Manual checkpoint created for session {:?}", session);
}

/// One `/btw` ask: load the host conversation up to this request's timestamp, hand it to
/// the agent's one tool-less completion, and send the answer or the classified error. It
/// never writes to storage — even a failure to load history is answered as an error rather
/// than saved (`specs/033-btw-side-chat/contracts/agent-side-chat.md`).
async fn run_btw(
    agent: Arc<VizierAgent>,
    storage: Arc<VizierStorage>,
    session: VizierSession,
    request: VizierRequest,
    response_tx: Option<flume::Sender<VizierResponse>>,
) {
    let answer = match storage
        .list_session_history_until_checkpoint(session.clone(), Some(request.timestamp))
        .await
    {
        Ok((history, handover)) => match agent.side_chat(request, session, history, handover).await
        {
            Ok(reply) => reply,
            Err(e) => btw_error_response(e.to_string()),
        },
        Err(e) => btw_error_response(e.to_string()),
    };
    if let Some(tx) = response_tx {
        let _ = tx.send_async(answer).await;
    }
}

/// A side-chat failure sent on the response channel: never saved, only classified.
fn btw_error_response(message: String) -> VizierResponse {
    VizierResponse {
        timestamp: chrono::Utc::now(),
        content: VizierResponseContent::Error {
            kind: ErrorKind::classify(&message),
            message,
        },
        attachments: vec![],
        ..Default::default()
    }
}

/// `/lobotomy`: a checkpoint with no handover, so the session carries nothing over.
async fn run_lobotomy(
    storage: &VizierStorage,
    session: &VizierSession,
    response_tx: &Option<flume::Sender<VizierResponse>>,
) {
    let _ = storage
        .save_session_history(
            session.clone(),
            SessionHistoryContent::Command("lobotomy".to_string()),
        )
        .await;

    // Save checkpoint with no handover
    if let Err(e) = storage.save_checkpoint(session.clone(), None).await {
        tracing::error!("Failed to save lobotomy checkpoint: {}", e);
        send_text(response_tx, format!("Failed to create lobotomy: {}", e)).await;
        return;
    }

    send_checkpoint(response_tx, None).await;
    tracing::info!("Lobotomy created for session {:?}", session);
}

/// Whether a message is usable as a retrieval query on its own (FR-029).
///
/// An embedding of "ok" or "do that one" does not encode what the agent is being asked about — it
/// encodes acknowledgement, or a pointer to something only the previous turns name. Retrieving
/// against it returns whatever happens to sit nearest in the vector space, and substantive
/// passages presented as relevant are worse than no passages at all. So this declines *before* a
/// relevance query is issued, which also means it is not paid for.
///
/// Widening the query to a window of recent conversation would likely beat any threshold tuning,
/// because "do that one" only becomes answerable with the previous turns included. That changes
/// what the query *is*, and is deliberately left to its own feature.
fn is_usable_query(message: &str) -> bool {
    let trimmed = message.trim();
    if trimmed.chars().count() < 12 {
        return false;
    }

    let words: Vec<String> = trimmed
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() < 3 {
        return false;
    }

    // Purely referential: every word is either an acknowledgement, a filler, or a pointer with no
    // content of its own. One substantive word is enough to make the message worth a query.
    const EMPTY: &[&str] = &[
        "ok", "okay", "k", "kk", "sure", "yes", "yeah", "yep", "no", "nope", "thanks", "thank",
        "you", "ty", "thx", "cheers", "please", "pls", "do", "did", "does", "done", "that", "this",
        "those", "these", "it", "them", "one", "ones", "the", "a", "an", "and", "or", "but", "so",
        "then", "now", "again", "too", "also", "just", "go", "ahead", "sounds", "good", "great",
        "nice", "cool", "fine", "right", "sorry", "hi", "hey", "hello", "lol", "haha", "hmm", "oh",
        "ah", "yo", "wait", "stop", "continue", "next", "more", "same", "first", "second", "last",
        "my", "your", "i", "me", "we", "us", "let", "lets", "can", "could", "would", "will",
        "should", "is", "are", "was", "were", "be", "been", "to", "for", "of", "on", "in", "at",
        "with", "up", "out", "all", "any", "some", "what", "how", "why",
    ];
    words.iter().any(|w| !EMPTY.contains(&w.as_str()))
}

/// Retrieve the related-memory passages for one turn, within that request kind's budget.
///
/// Returns an empty vector for every reason it should: no indexer, a zero budget, an unusable
/// query, nothing clearing the threshold, or a failed lookup. The turn proceeds without the block
/// in all of those cases and never fails because of them (FR-027).
#[allow(clippy::too_many_arguments)]
async fn retrieve_auto_context(
    storage: &VizierStorage,
    indexer: Option<&VizierIndexer>,
    agent_config: &AgentConfig,
    agent_id: &str,
    prompt: &str,
    budget: usize,
    kind: &str,
) -> Vec<crate::schema::MemoryPassageResult> {
    let Some(idx) = indexer else {
        return Vec::new();
    };
    // A zero budget is the default on the SilentRead path, which fires for every observed channel
    // message — so this returns quietly rather than logging per message.
    if budget == 0 {
        return Vec::new();
    }
    if !is_usable_query(prompt) {
        tracing::debug!(
            agent_id,
            kind,
            "message is not a usable retrieval query; skipping automatic context"
        );
        return Vec::new();
    }

    let cfg = &agent_config.auto_context;
    let passages = match storage
        .query_memory(
            agent_id.to_string(),
            None,
            prompt.to_string(),
            budget,
            cfg.threshold,
            cfg.per_document,
            idx,
            &agent_config.chunking,
        )
        .await
    {
        Ok(passages) => passages,
        Err(e) => {
            // FR-027: a failed assembly costs the block, never the turn.
            tracing::warn!(
                agent_id,
                kind,
                "automatic context lookup failed; proceeding without it: {e}"
            );
            return Vec::new();
        }
    };

    if passages.is_empty() {
        // FR-026: the section is omitted entirely rather than filled with weak matches. How often
        // this branch is taken is what decides whether the feature saves tokens or spends them.
        tracing::debug!(
            agent_id,
            kind,
            threshold = cfg.threshold,
            "no passage cleared the automatic-context threshold; no block injected"
        );
        return Vec::new();
    }

    let fitted = fit_to_size_cap(passages, cfg.size_cap);
    tracing::debug!(
        agent_id,
        kind,
        passages = fitted.len(),
        bytes = fitted.iter().map(|p| p.text.len()).sum::<usize>(),
        threshold = cfg.threshold,
        "injected automatic memory context"
    );
    fitted
}

/// Record whether the agent searched memory anyway after automatic context was injected (FR-032).
///
/// This is the hit/miss signal the threshold and budget are tuned against. A *miss* — the block was
/// injected and the agent still had to search — means the passages did not answer the question, so
/// the turn paid for them twice. A *hit* means the follow-up round trip was displaced, which is the
/// only way this feature nets out, since five passages cost roughly 15x the ten titles they
/// replaced. Logged rather than stored: the replay harness that derives the threshold (task T051)
/// reads these, and a table would be a second source of truth for a number nothing serves at
/// runtime.
async fn record_context_followup(
    storage: &VizierStorage,
    session: &VizierSession,
    agent_id: &str,
    since: chrono::DateTime<Utc>,
    injected: usize,
) {
    let Ok(history) = storage
        .list_session_by_time_window(session.clone(), Some(since), None)
        .await
    else {
        return;
    };

    let searched = history.iter().any(|entry| {
        matches!(
            &entry.content,
            SessionHistoryContent::ToolCall { name, .. }
                if name == "memory_search" || name == "memory_read" || name == "memory_follow"
        )
    });

    tracing::info!(
        agent_id,
        injected_passages = injected,
        searched_anyway = searched,
        outcome = if searched { "miss" } else { "hit" },
        "automatic memory context follow-up"
    );
}

/// Spend the total size budget in rank order, stopping at the first passage that does not fit
/// (FR-022, FR-028).
///
/// No partial passage at the tail: a half-sentence costs tokens and tells the agent nothing. The
/// one exception is a single passage larger than the whole budget — dropping it silently would
/// hide the best match, so it is truncated and says so, with its address intact.
fn fit_to_size_cap(
    passages: Vec<crate::schema::MemoryPassageResult>,
    size_cap: usize,
) -> Vec<crate::schema::MemoryPassageResult> {
    let mut out = Vec::new();
    let mut spent = 0usize;

    for mut passage in passages {
        let len = passage.text.len();
        if spent + len <= size_cap {
            spent += len;
            out.push(passage);
            continue;
        }
        if out.is_empty() && size_cap > 0 {
            // The top-ranked passage alone overruns the budget. Truncate on a character boundary
            // and mark it, so the agent knows to read the document if it needs the rest.
            let mut end = size_cap.min(passage.text.len());
            while end > 0 && !passage.text.is_char_boundary(end) {
                end -= 1;
            }
            passage.text.truncate(end);
            passage.truncated = true;
            out.push(passage);
        }
        break;
    }

    out
}

pub async fn handle_request(
    agent: Arc<VizierAgent>,
    agent_config: AgentConfig,
    session: VizierSession,
    mut request: VizierRequest,
    response_tx: Option<flume::Sender<VizierResponse>>,
    storage: Arc<VizierStorage>,
    indexer: Option<crate::indexer::VizierIndexer>,
    deps: &VizierDependencies,
    turn: TurnRef,
) -> Result<()> {
    let mut hooks = VizierSessionHooks::new().hook(DebugHook(session.clone()));

    if let Some(ref tx) = response_tx {
        hooks = hooks.hook(ThinkingHook::new(tx.clone(), session.clone()));
    }

    if let Some(ref tx) = response_tx {
        hooks = hooks.hook(ToolCallsHook::new(tx.clone(), session.clone()));
    }

    // Register HandoverSenderHook if response_tx is available
    if let Some(ref tx) = response_tx {
        hooks = hooks.hook(HandoverSenderHook::new(tx.clone(), session.clone()));
    }

    let hooks = Arc::new(hooks);

    match &request.content {
        VizierRequestContent::Chat(_)
        | VizierRequestContent::AudioChat(_, _)
        | VizierRequestContent::BackgroundReport(_)
        | VizierRequestContent::ClientEvent(_) => {
            // `/skills`, `/<skill> …` and `/skill <name> …` (spec 021): every channel delivers
            // them as `Chat`, so this is the one place they are told apart.
            // The list as typed rides the request's history entry; the resolved contents never
            // are stored (FR-019).
            if !request.mentions.is_empty() {
                set_metadata(
                    &mut request.metadata,
                    "mentions",
                    serde_json::json!(request.mentions.clone()),
                );
            }
            // A person's `/name` resolves the same scopes the tools do: the project's own skills
            // first, then the agent's, then the global ones (FR-036). For a non-project chat this
            // costs one pointer check and no storage read.
            let skills = {
                let cwd = crate::projects::context::skills_cwd(&deps, &session, &request).await;
                crate::skill::scopes::SkillScopes::for_turn(
                    &deps.config.workspace,
                    &session.0,
                    cwd.as_deref(),
                )
            };

            let mut invoked = None;
            if let VizierRequestContent::Chat(text) = &request.content {
                match invocation::parse(text) {
                    Parsed::Plain => {}
                    Parsed::List => {
                        let reply = list_reply(&skills);
                        return answer_directly(&storage, &session, request, &response_tx, reply)
                            .await;
                    }
                    Parsed::Invoke { name, input } => match skills.find(&name) {
                        Some(found) if found.skill.user_invocable => {
                            set_metadata(
                                &mut request.metadata,
                                "invoked_skill",
                                found.skill.name.clone().into(),
                            );
                            invoked = Some(Invoked { skill: found.skill, input });
                        }
                        Some(found) => {
                            let reply = format!(
                                "`{}` is not runnable by name. Ask me to use it instead.",
                                found.skill.name
                            );
                            return answer_directly(
                                &storage,
                                &session,
                                request,
                                &response_tx,
                                reply,
                            )
                            .await;
                        }
                        None => {
                            let reply = not_found_reply(&skills, &name);
                            return answer_directly(
                                &storage,
                                &session,
                                request,
                                &response_tx,
                                reply,
                            )
                            .await;
                        }
                    },
                }
            }

            let prompt = match &request.content {
                // Retrieval looks for what the skill is being asked to do, not the command.
                VizierRequestContent::Chat(_) if let Some(invoked) = &invoked => {
                    if invoked.input.is_empty() {
                        invoked.skill.description.clone()
                    } else {
                        invoked.input.clone()
                    }
                }
                VizierRequestContent::Chat(p) => p.clone(),
                VizierRequestContent::ClientEvent(event) => event.text.clone(),
                VizierRequestContent::AudioChat(_, Some(text)) => text.clone(),
                VizierRequestContent::AudioChat(_, None) => "[Voice message]".to_string(),
                VizierRequestContent::BackgroundReport(report) => report.to_string(),
                _ => unreachable!(),
            };
            // A report is a poor retrieval query — it is mostly other turns' answers — so it
            // gets no automatic passages. Skill recommendation still sees its text.
            let passage_budget = match &request.content {
                VizierRequestContent::BackgroundReport(_) => 0,
                _ => agent_config.auto_context.chat_passages,
            };
            let (history, checkpoint_handover) = storage
                .list_session_history_until_checkpoint(
                    session.clone(),
                    Some(request.timestamp.clone()),
                )
                .await?;

            let memory = retrieve_auto_context(
                &storage,
                indexer.as_ref(),
                &agent_config,
                &session.0,
                &prompt,
                passage_budget,
                "chat",
            )
            .await;
            let injected_context = (!memory.is_empty()).then_some(memory.len());
            let turn_start = Utc::now();
            let skills = agent.recommend_skills(&prompt).await.unwrap_or_default();
            // A WebUI reply is news for the person whose conversation it is. A woken
            // background report already has its own job notification, so it is left out.
            let notifies_chat = session.1.web_user().is_some()
                && matches!(
                    &request.content,
                    VizierRequestContent::Chat(_)
                        | VizierRequestContent::AudioChat(..)
                        | VizierRequestContent::ClientEvent(_)
                );
            let res = agent
                .chat(
                    request,
                    session.clone(),
                    history,
                    memory,
                    skills,
                    invoked,
                    Some(hooks),
                    checkpoint_handover,
                    turn,
                )
                .await?;
            // FR-032: the hit/miss signal the threshold is tuned against. Whether the agent went
            // on to search memory anyway is what says if the injected block was any use.
            if let Some(injected) = injected_context {
                let agent = session.0.clone();
                record_context_followup(&storage, &session, &agent, turn_start, injected).await;
            }
            if notifies_chat
                && matches!(
                    res.content,
                    VizierResponseContent::Message { .. } | VizierResponseContent::AudioReply(..)
                )
                && let Some(uid) = &res.history_uid
            {
                deps.notifications
                    .chat_replied(&session, uid, res.timestamp)
                    .await;
            }
            if let Some(ref tx) = response_tx {
                let _ = tx.send_async(res).await;
            }
        }
        VizierRequestContent::SilentRead(_) => {
            let prompt = match &request.content {
                VizierRequestContent::SilentRead(p) => p.clone(),
                _ => unreachable!(),
            };
            let (history, checkpoint_handover) = storage
                .list_session_history_until_checkpoint(
                    session.clone(),
                    Some(request.timestamp.clone()),
                )
                .await?;
            // This path fires for every non-mention message in a Discord guild channel and every
            // Telegram group message, so its cost scales with channel traffic rather than with
            // conversation volume. It gets its own budget, defaulting to zero (FR-030).
            let memory = retrieve_auto_context(
                &storage,
                indexer.as_ref(),
                &agent_config,
                &session.0,
                &prompt,
                agent_config.auto_context.silent_read_passages,
                "silent_read",
            )
            .await;
            let injected_context = (!memory.is_empty()).then_some(memory.len());
            let turn_start = Utc::now();
            let skills = agent.recommend_skills(&prompt).await.unwrap_or_default();
            let res = agent
                .chat(
                    request,
                    session.clone(),
                    history,
                    memory,
                    skills,
                    None,
                    Some(hooks),
                    checkpoint_handover,
                    turn,
                )
                .await?;
            if let Some(injected) = injected_context {
                let agent = session.0.clone();
                record_context_followup(&storage, &session, &agent, turn_start, injected).await;
            }
            if let Some(ref tx) = response_tx {
                let _ = tx.send_async(res).await;
            }
        }
        VizierRequestContent::Prompt(_)
        | VizierRequestContent::AudioPrompt(_, _)
        | VizierRequestContent::Unattended(_) => {
            let res = match &session.1 {
                VizierChannelId::Dream(dream_session, stage) => {
                    let dream_start = Utc::now();
                    match stage {
                        DreamStage::Extraction => {
                            let end = request.timestamp;
                            let session_history = storage
                                .list_session_by_time_window(
                                    *dream_session.clone(),
                                    None,
                                    Some(end),
                                )
                                .await?;

                            // Skip empty sessions — send Abort
                            if session_history.is_empty() {
                                if let Some(ref tx) = response_tx {
                                    let _ = tx
                                        .send_async(VizierResponse {
                                            timestamp: end,
                                            content: VizierResponseContent::Abort,
                                            attachments: vec![],
                                            ..Default::default()
                                        })
                                        .await;
                                }
                                return Ok(());
                            }

                            let cycle_id = request
                                .metadata
                                .get("dream_cycle_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let session_context = dream_session.to_slug();

                            let response = agent
                                .dream_chat(
                                    request,
                                    session.clone(),
                                    session_history,
                                    Some(hooks.clone()),
                                    deps,
                                )
                                .await?;

                            // Save extraction as dream journal entry
                            save_dream_entry(
                                &deps.storage,
                                &session.0,
                                &cycle_id,
                                vec![session_context.clone()],
                                Some(session_context),
                                &agent_config,
                                dream_start,
                                DreamStage::Extraction,
                                &response,
                            )
                            .await;

                            response
                        }
                        DreamStage::Consolidation => {
                            let cycle_id = request
                                .metadata
                                .get("dream_cycle_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let source_sessions: Vec<String> = serde_json::from_value(
                                request
                                    .metadata
                                    .get("source_sessions")
                                    .cloned()
                                    .unwrap_or(serde_json::json!([])),
                            )
                            .unwrap_or_default();

                            let response = agent
                                .dream_chat(
                                    request,
                                    session.clone(),
                                    vec![],
                                    Some(hooks.clone()),
                                    deps,
                                )
                                .await?;

                            // Save consolidation as dream journal entry
                            save_dream_entry(
                                &deps.storage,
                                &session.0,
                                &cycle_id,
                                source_sessions,
                                None,
                                &agent_config,
                                dream_start,
                                DreamStage::Consolidation,
                                &response,
                            )
                            .await;

                            response
                        }
                    }
                }
                _ => {
                    agent
                        .chat(
                            request,
                            session.clone(),
                            vec![],
                            vec![],
                            vec![],
                            None,
                            Some(hooks),
                            None,
                            turn,
                        )
                        .await?
                }
            };

            if let Some(ref tx) = response_tx {
                let _ = tx.send_async(res).await;
            }
        }
        VizierRequestContent::Command(cmd) => {
            tracing::warn!("unhandled command: {}", cmd);
        }
        // A side chat is answered by `run_btw` in `start_session_job`, never here — this arm
        // is defensive: a refactor that bypasses that dispatch fails loudly instead of saving
        // anything (specs/033-btw-side-chat, research R1).
        VizierRequestContent::Btw(_) => {
            return Err(anyhow::Error::msg("side chats are answered by run_btw"));
        }
    }

    Ok(())
}

/// Answer a message without a model turn (`/skills`, an unknown skill): the request and the
/// reply are saved like any exchange, so they show on reload, and no usage is recorded.
async fn answer_directly(
    storage: &Arc<VizierStorage>,
    session: &VizierSession,
    mut request: VizierRequest,
    response_tx: &Option<flume::Sender<VizierResponse>>,
    content: String,
) -> Result<()> {
    request.attachments.clear();
    if let Err(e) =
        save_with_busy_retry(storage, session, SessionHistoryContent::Request(request)).await
    {
        tracing::error!(agent_id = %session.0, "failed to save session history: {e}");
    }
    let mut response = VizierResponse {
        timestamp: Utc::now(),
        content: VizierResponseContent::Message {
            content,
            stats: None,
        },
        ..Default::default()
    };
    match save_with_busy_retry(
        storage,
        session,
        SessionHistoryContent::Response(response.clone()),
    )
    .await
    {
        Ok(uid) => response.history_uid = Some(uid),
        Err(e) => tracing::error!(agent_id = %session.0, "failed to save session history: {e}"),
    }
    if let Some(tx) = response_tx {
        let _ = tx.send_async(response).await;
    }
    Ok(())
}

fn set_metadata(metadata: &mut serde_json::Value, key: &str, value: serde_json::Value) {
    if !metadata.is_object() {
        *metadata = serde_json::json!({});
    }
    if let Some(object) = metadata.as_object_mut() {
        object.insert(key.to_string(), value);
    }
}

/// The `/skills` reply: every scope, one line per skill, the project's own first.
fn list_reply(scopes: &crate::skill::scopes::SkillScopes) -> String {
    let found = scopes.discover();
    let available = scopes.available();
    if available.is_empty() {
        return "No skills are available to this agent.".to_string();
    }
    let mut reply = available
        .iter()
        .map(|scoped| {
            let hint = if scoped.skill.user_invocable {
                format!("`/{}`", scoped.skill.name)
            } else {
                scoped.skill.name.clone()
            };
            format!("{hint} — {}", scoped.skill.description)
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !found.skipped.is_empty() {
        reply.push_str(&format!(
            "\n\n{} skill folder(s) could not be read: {}",
            found.skipped.len(),
            found
                .skipped
                .iter()
                .map(|skipped| skipped.folder.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    reply
}

/// The reply to `/<name>` when no skill has that name.
fn not_found_reply(scopes: &crate::skill::scopes::SkillScopes, name: &str) -> String {
    let known: Vec<String> = scopes
        .available()
        .into_iter()
        .map(|scoped| scoped.skill.name)
        .collect();
    let mut reply = format!("No skill named \"{name}\".");
    let close = crate::sandbox::docs::did_you_mean(name, &known);
    if !close.is_empty() {
        reply.push_str(&format!(" Did you mean: {}?", close.join(", ")));
    }
    reply.push_str(" Send /skills to list them.");
    reply
}

async fn save_dream_entry(
    storage: &Arc<VizierStorage>,
    agent_id: &str,
    cycle_id: &str,
    source_sessions: Vec<String>,
    session_context: Option<String>,
    agent_config: &AgentConfig,
    start_time: chrono::DateTime<Utc>,
    stage: DreamStage,
    response: &VizierResponse,
) {
    let content = match &response.content {
        VizierResponseContent::Message { content, .. } => content.clone(),
        _ => return,
    };

    if content.is_empty() {
        return;
    }

    let now = Utc::now();
    let duration_ms = (now - start_time).num_milliseconds().max(0) as u64;

    let entry = DreamJournalEntry {
        id: uuid::Uuid::new_v4().to_string(),
        dream_cycle_id: cycle_id.to_string(),
        agent_id: agent_id.to_string(),
        timestamp: now,
        stage,
        source_sessions,
        session_context,
        content,
        duration_ms: Some(duration_ms),
        provider_used: Some(format!("{:?}", agent_config.provider)),
        model_used: Some(agent_config.model.clone()),
    };

    if let Err(e) = storage.save_dream_entry(entry).await {
        tracing::error!(
            "Failed to save dream journal entry for '{}': {}",
            agent_id,
            e
        );
    }
}

#[cfg(test)]
mod auto_context_tests {
    use super::*;
    use crate::schema::MemoryPassageResult;

    fn passage(text: &str) -> MemoryPassageResult {
        MemoryPassageResult {
            bundle: "work".into(),
            path: "ops/deploys".into(),
            title: "Deployment practice".into(),
            ordinal: 3,
            ordinal_end: 3,
            line_start: 48,
            line_end: 71,
            text: text.into(),
            score: 0.8,
            truncated: false,
        }
    }

    // ---- FR-029: the query-usability gate ----

    #[test]
    fn purely_referential_messages_do_not_trigger_retrieval() {
        for message in [
            "ok",
            "okay",
            "thanks",
            "thank you",
            "do that one",
            "yes please do that",
            "sounds good to me",
            "ok cool thanks",
            "",
            "   ",
            "k",
        ] {
            assert!(
                !is_usable_query(message),
                "{message:?} is not a usable retrieval query"
            );
        }
    }

    #[test]
    fn substantive_messages_do_trigger_retrieval() {
        for message in [
            "when are our deployment windows",
            "what did we decide about the rollback procedure",
            "remind me who owns the billing service",
            "do that one for the staging cluster instead",
        ] {
            assert!(is_usable_query(message), "{message:?} is a usable query");
        }
    }

    /// The gate is about content, not politeness: one substantive word is enough. "do that one for
    /// the staging cluster" is referential *and* usable, because "staging cluster" is something to
    /// retrieve against.
    #[test]
    fn one_substantive_word_is_enough_to_clear_the_gate() {
        assert!(!is_usable_query("ok do that one"));
        assert!(is_usable_query("ok do the migration"));
    }

    // ---- FR-022, FR-028: the total size cap ----

    #[test]
    fn passages_are_taken_in_rank_order_until_the_budget_runs_out() {
        let passages = vec![passage(&"a".repeat(100)), passage(&"b".repeat(100))];
        let out = fit_to_size_cap(passages, 250);
        assert_eq!(out.len(), 2, "both fit inside the cap");
        assert!(out.iter().all(|p| !p.truncated));
    }

    #[test]
    fn the_remainder_is_omitted_rather_than_cut_in_half() {
        let passages = vec![
            passage(&"a".repeat(100)),
            passage(&"b".repeat(100)),
            passage(&"c".repeat(100)),
        ];
        let out = fit_to_size_cap(passages, 250);
        assert_eq!(out.len(), 2, "the third is dropped whole, not trimmed");
        assert!(
            out.iter().all(|p| p.text.len() == 100 && !p.truncated),
            "no partial passage at the tail"
        );
    }

    /// FR-028: dropping the top-ranked passage silently would hide the best match, so it is
    /// truncated and says so, with its address intact.
    #[test]
    fn a_single_oversized_passage_is_truncated_and_marked_rather_than_dropped() {
        let out = fit_to_size_cap(vec![passage(&"x".repeat(5000))], 1000);
        assert_eq!(out.len(), 1, "it is included, not dropped");
        assert!(out[0].truncated, "and it says it was truncated");
        assert_eq!(out[0].text.len(), 1000);
        assert_eq!(out[0].bundle, "work", "its address survives truncation");
        assert_eq!(out[0].path, "ops/deploys");
        assert_eq!(out[0].ordinal, 3);
    }

    #[test]
    fn truncation_lands_on_a_character_boundary() {
        // Every char is 3 bytes, so a byte-indexed truncate would split one.
        let out = fit_to_size_cap(vec![passage(&"日".repeat(500))], 1000);
        assert_eq!(out.len(), 1);
        assert!(out[0].truncated);
        assert_eq!(out[0].text.len() % 3, 0, "did not split a multi-byte char");
        assert!(out[0].text.chars().all(|c| c == '日'));
    }

    #[test]
    fn a_zero_cap_yields_nothing_rather_than_an_empty_truncated_passage() {
        assert!(fit_to_size_cap(vec![passage("anything")], 0).is_empty());
    }

    // ---- FR-021, FR-026, FR-031: rendering ----

    #[test]
    fn the_memory_section_is_omitted_entirely_when_nothing_qualified() {
        let rendered =
            crate::agents::agent::system_prompt::context::context_md(&[], &[], None, None);
        assert!(
            !rendered.contains("Related Memories"),
            "FR-026: no empty heading, no weakly-related filler: {rendered}"
        );
    }

    #[test]
    fn a_rendered_passage_carries_its_address_and_is_labelled_as_data() {
        let rendered = crate::agents::agent::system_prompt::context::context_md(
            &[passage("Deploys go out Tuesday.")],
            &[],
            None,
            None,
        );
        assert!(rendered.contains(r#"<memory bundle="work" path="ops/deploys" passage="3">"#));
        assert!(rendered.contains("Deploys go out Tuesday."));
        assert!(rendered.contains("</memory>"));
        assert!(
            rendered.contains("not instruction"),
            "FR-031: stated to be reference material, not instruction"
        );
        assert!(
            rendered.contains("memory_read"),
            "FR-021: says how to reach the whole document"
        );
    }

    #[test]
    fn a_merged_passage_renders_its_ordinal_range() {
        let mut p = passage("merged text");
        p.ordinal_end = 5;
        let rendered =
            crate::agents::agent::system_prompt::context::context_md(&[p], &[], None, None);
        assert!(rendered.contains(r#"passage="3-5""#), "{rendered}");
    }

    #[test]
    fn a_truncated_passage_says_so_in_the_rendered_block() {
        let mut p = passage("cut short");
        p.truncated = true;
        let rendered =
            crate::agents::agent::system_prompt::context::context_md(&[p], &[], None, None);
        assert!(rendered.contains(r#"truncated="true""#), "{rendered}");
        assert!(rendered.contains("truncated to fit"), "{rendered}");
    }
}

#[cfg(test)]
mod session_turns_tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::oneshot;

    fn session(topic: &str) -> VizierSession {
        VizierSession("agent".into(), VizierChannelId::System, Some(topic.into()))
    }

    async fn next(turns: &mut SessionTurns) -> FinishedTurn {
        tokio::time::timeout(Duration::from_secs(5), turns.join_next())
            .await
            .expect("a turn ended")
            .expect("the set was not empty")
    }

    #[tokio::test]
    async fn a_turn_that_returns_frees_its_session() {
        let mut turns = SessionTurns::default();
        turns.spawn(session("a"), None, TurnRef::default(), async {});
        assert!(turns.is_running(&session("a")));

        let finished = next(&mut turns).await;
        assert_eq!(finished.outcome, TurnOutcome::Done);
        assert!(finished.current);
        assert!(!turns.is_running(&session("a")));
        assert!(turns.is_empty());
    }

    /// #46: the session is released and its sender is handed back, even though the turn
    /// never reached its last line.
    #[tokio::test]
    async fn a_panicking_turn_still_frees_its_session_and_returns_its_sender() {
        let mut turns = SessionTurns::default();
        let (tx, _rx) = flume::unbounded();
        turns.spawn(session("a"), Some(tx), TurnRef::default(), async {
            panic!("turn blew up")
        });

        let finished = next(&mut turns).await;
        assert_eq!(finished.outcome, TurnOutcome::Panicked);
        assert!(
            finished.current,
            "the loop must pick up the queue behind it"
        );
        assert_eq!(finished.session, session("a"));
        assert!(!turns.is_running(&session("a")));

        assert!(
            finished.response_tx.is_some(),
            "so the sender can be told it failed"
        );
    }

    #[tokio::test]
    async fn an_aborted_turn_frees_its_session_at_once_and_ends_as_not_current() {
        let mut turns = SessionTurns::default();
        turns.spawn(
            session("a"),
            None,
            TurnRef::default(),
            std::future::pending(),
        );

        assert!(turns.abort(&session("a")));
        assert!(
            !turns.is_running(&session("a")),
            "a new request may start right away"
        );
        assert!(!turns.abort(&session("a")), "nothing left to abort");

        let finished = next(&mut turns).await;
        assert_eq!(finished.outcome, TurnOutcome::Cancelled);
        assert!(!finished.current);
    }

    /// An aborted turn ends after a newer one has started in its session. Its end must not
    /// release the session, or the next queued message would run beside the newer turn.
    #[tokio::test]
    async fn a_stale_turn_ending_leaves_the_newer_turn_running() {
        let mut turns = SessionTurns::default();
        let (release_old, old_released) = oneshot::channel::<()>();
        turns.spawn(session("a"), None, TurnRef::default(), async {
            let _ = old_released.await;
            panic!("old turn");
        });
        // Abort only marks it; replace it as the session's turn before it is joined.
        turns.abort(&session("a"));
        turns.spawn(
            session("a"),
            None,
            TurnRef::default(),
            std::future::pending(),
        );
        drop(release_old);

        let finished = next(&mut turns).await;
        assert!(!finished.current);
        assert!(
            turns.is_running(&session("a")),
            "the newer turn still holds the session"
        );
    }

    /// #52: a stopping agent's turns end with it, and only the running ones are handed back
    /// to be told why (an aborted turn's sender already heard).
    #[tokio::test]
    async fn stop_all_ends_every_turn_and_returns_the_running_ones() {
        let mut turns = SessionTurns::default();
        let (tx, _rx) = flume::unbounded();
        let (dropped_tx, dropped) = oneshot::channel::<()>();
        turns.spawn(session("a"), Some(tx), TurnRef::default(), async move {
            let _keep = dropped_tx;
            std::future::pending::<()>().await
        });
        turns.spawn(
            session("b"),
            None,
            TurnRef::default(),
            std::future::pending(),
        );
        turns.abort(&session("b"));

        let stopped = turns.stop_all(Duration::from_secs(5)).await;
        assert_eq!(stopped.len(), 1);
        assert_eq!(stopped[0].0, session("a"));
        assert!(stopped[0].1.is_some());
        assert!(turns.is_empty(), "every task has ended");
        assert!(
            dropped.await.is_err(),
            "the turn's future was dropped, not detached"
        );
    }

    #[tokio::test]
    async fn sessions_are_tracked_independently() {
        let mut turns = SessionTurns::default();
        turns.spawn(session("a"), None, TurnRef::default(), async {
            panic!("a fails")
        });
        turns.spawn(
            session("b"),
            None,
            TurnRef::default(),
            std::future::pending(),
        );

        let finished = next(&mut turns).await;
        assert_eq!(finished.session, session("a"));
        assert!(!turns.is_running(&session("a")));
        assert!(turns.is_running(&session("b")));
    }

    #[test]
    fn handovers_re_sent_in_order_keep_their_place_ahead_of_messages() {
        let req = |content| VizierRequest {
            content,
            ..Default::default()
        };
        let chat = |t: &str| req(VizierRequestContent::Chat(t.into()));
        let handover = || req(VizierRequestContent::Command("checkpoint".into()));
        let mut queue = SessionQueue::new();
        // Re-sent in acceptance order: message A, handover H, message B.
        for item in [chat("A"), handover(), chat("B")] {
            if is_handover_command(&item) {
                queue_handover(&mut queue, (item, None, TurnRef::default()));
            } else {
                queue.push_back((item, None, TurnRef::default()));
            }
        }
        let order: Vec<_> = queue
            .iter()
            .map(|(r, _, _)| is_handover_command(r))
            .collect();
        assert_eq!(order, vec![true, false, false]);
    }

    #[test]
    fn a_queue_behind_a_finished_turn_still_counts_as_busy_until_picked_up() {
        let turns = SessionTurns::default();
        let mut queues = HashMap::<VizierSession, SessionQueue>::new();
        assert!(!session_is_busy(&turns, &queues, &session("a")));

        queues.entry(session("a")).or_default().push_back((
            VizierRequest {
                content: VizierRequestContent::Chat("queued".into()),
                ..Default::default()
            },
            None,
            TurnRef::default(),
        ));
        assert!(session_is_busy(&turns, &queues, &session("a")));
    }

    #[test]
    fn one_title_generation_per_session_at_a_time() {
        let titling = TitlingSessions::default();
        let first = TitlingClaim::take(&titling, &session("a"));
        assert!(first.is_some());
        assert!(
            TitlingClaim::take(&titling, &session("a")).is_none(),
            "a burst starts one"
        );
        assert!(
            TitlingClaim::take(&titling, &session("b")).is_some(),
            "other sessions are independent"
        );

        drop(first);
        assert!(
            TitlingClaim::take(&titling, &session("a")).is_some(),
            "released when it ends"
        );
        assert!(
            titling.lock().unwrap().is_empty(),
            "nothing is kept once generations end"
        );
    }
}

#[cfg(test)]
mod btw_tests {
    use super::*;
    use crate::agents::agent::model::VizierModel;
    use crate::config::VizierConfig;
    use crate::dependencies::VizierDependencies;
    use crate::llm::{
        AssistantPart, CompletionModel, CompletionRequest, CompletionResponse, LlmError, ToolChoice,
    };
    use crate::tests_support::agent_config;
    use crate::storage::usage::UsageStorage;
    use crate::schema::{BackgroundReport, ClientEvent, SideChat};
    use crate::schema::background::JobKind;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Answers every completion with the next scripted side, so a side chat runs without a
    /// network or a channel. Records the tool choice each call was given.
    struct ScriptedSideChat {
        replies: std::sync::Mutex<Vec<Option<Result<AssistantPart, LlmError>>>>,
        calls: AtomicU32,
        tool_choices: std::sync::Mutex<Vec<Option<ToolChoice>>>,
    }

    impl ScriptedSideChat {
        fn text(parts: Vec<AssistantPart>) -> Arc<Self> {
            let replies: Vec<std::option::Option<std::result::Result<AssistantPart, LlmError>>> =
                parts.into_iter().map(|p| Some(Ok(p))).collect();
            Arc::new(Self {
                replies: replies.into(),
                calls: AtomicU32::new(0),
                tool_choices: vec![].into(),
            })
        }

        fn tool_call() -> Arc<Self> {
            Arc::new(Self {
                replies: vec![Some(Ok(AssistantPart::ToolCall {
                    id: "call-1".into(),
                    name: "list_tool_functions".into(),
                    arguments: serde_json::json!({}),
                    provider_data: None,
                }))]
                .into(),
                calls: AtomicU32::new(0),
                tool_choices: vec![].into(),
            })
        }

        fn failing() -> Arc<Self> {
            Arc::new(Self {
                replies: vec![Some(Err(LlmError::Http {
                    status: 500,
                    retry_after: None,
                    body: "boom".into(),
                }))]
                .into(),
                calls: AtomicU32::new(0),
                tool_choices: vec![].into(),
            })
        }

        fn tool_choices(&self) -> Vec<Option<ToolChoice>> {
            self.tool_choices.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl CompletionModel for ScriptedSideChat {
        async fn complete(
            &self,
            req: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.tool_choices.lock().unwrap().push(req.tool_choice);
            self.replies
                .lock()
                .unwrap()
                .remove(0)
                .map(|next| {
                    next.map(|part| CompletionResponse {
                        id: None,
                        content: vec![part],
                        usage: crate::llm::Usage {
                            input_tokens: 10,
                            output_tokens: 5,
                            total_tokens: 15,
                            ..Default::default()
                        },
                        model: Some("scripted-served".into()),
                        finish_reason: None,
                    })
                })
                .unwrap_or_else(|| Err(LlmError::Decode("no scripted reply left".into())))
        }

        fn context_window(&self) -> Option<u64> {
            None
        }
    }

    fn btw_session() -> VizierSession {
        VizierSession(
            "btw-agent".into(),
            VizierChannelId::HTTP("btw-user".into(), "btw-topic".into()),
            None,
        )
    }

    fn btw_request(question: &str) -> VizierRequest {
        VizierRequest {
            content: VizierRequestContent::Btw(crate::schema::SideChat {
                question: question.into(),
                turns: vec![],
            }),
            ..Default::default()
        }
    }

    /// One temp workspace with a full `VizierAgent` on a scripted model, and seeded host
    /// history, so `run_btw` can be driven end to end.
    async fn setup(
        model: Arc<ScriptedSideChat>,
    ) -> (
        Arc<VizierAgent>,
        Arc<VizierStorage>,
        VizierSession,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let config = VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        let deps = VizierDependencies::new(config).await.unwrap();
        let agent_id = "btw-agent";
        let session = btw_session();
        let storage = deps.storage.clone();
        storage
            .save_session_history(
                session.clone(),
                SessionHistoryContent::Request(btw_request("ask first")),
            )
            .await
            .unwrap();
        storage
            .save_session_history(
                session.clone(),
                SessionHistoryContent::Response(VizierResponse::default()),
            )
            .await
            .unwrap();
        let mut agent =
            VizierAgent::new(agent_id.to_string(), &deps, &agent_config(agent_id), None)
                .await
                .unwrap();
        let scripted = VizierModel::scripted(
            agent_id.to_string(),
            storage.clone(),
            model.clone(),
            "scripted",
        );
        agent.swap_model_for_test(scripted);
        (Arc::new(agent), storage, session, dir)
    }

    async fn history_uids(storage: &VizierStorage, session: &VizierSession) -> Vec<String> {
        storage
            .list_session_history(session.clone(), None, None, None)
            .await
            .unwrap()
            .into_iter()
            .map(|h| h.uid)
            .collect()
    }

    async fn usage_event_count(storage: &VizierStorage, agent_id: &str) -> u64 {
        storage
            .usage_groups_since(
                &[agent_id.to_string()],
                chrono::DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            )
            .await
            .unwrap()
            .iter()
            .map(|g| g.calls)
            .sum()
    }

    async fn detail_at(
        storage: &VizierStorage,
        session: &VizierSession,
    ) -> Option<VizierSessionDetail> {
        storage
            .get_session_detail_by_topic(
                session.0.clone(),
                session.1.clone(),
                session.2.clone(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_side_chat_answers_and_leaves_no_trace() {
        let (agent, storage, session, _dir) =
            setup(ScriptedSideChat::text(vec![AssistantPart::Text("aside answer".into())])).await;

        let (tx, rx) = flume::bounded(1);
        run_btw(
            agent.clone(),
            storage.clone(),
            session.clone(),
            btw_request("what is up"),
            Some(tx),
        )
        .await;

        let reply = rx.recv_async().await.unwrap();
        let VizierResponseContent::Message { content, stats } = reply.content else {
            panic!("expected a message reply, got {:?}", reply.content);
        };
        assert_eq!(content, "aside answer");
        assert!(reply.history_uid.is_none(), "a side chat reply is never saved");
        assert!(reply.attachments.is_empty());
        let stats = stats.unwrap();
        assert_eq!(stats.provider.map(|p| p.to_string()).as_deref(), Some("dummyplug"));
        assert_eq!(stats.model.as_deref(), Some("scripted"));

        // Nothing in history: before and after, the same two entries by uid.
        assert_eq!(history_uids(&storage, &session).await.len(), 2);

        let detail = detail_at(&storage, &session).await;
        assert!(detail.is_none(), "a side chat never creates or updates session detail");

        let pending = storage.pending_turns("btw-agent").await.unwrap();
        assert!(pending.is_empty(), "a side chat never journals a turn");
    }

    #[tokio::test]
    async fn a_tool_call_answer_becomes_the_no_tools_notice() {
        let (agent, storage, session, _dir) = setup(ScriptedSideChat::tool_call()).await;

        let (tx, rx) = flume::bounded(1);
        run_btw(
            agent,
            storage.clone(),
            session.clone(),
            btw_request("do something"),
            Some(tx),
        )
        .await;

        let reply = rx.recv_async().await.unwrap();
        let VizierResponseContent::Message { content, .. } = reply.content else {
            panic!("expected a message reply, got {:?}", reply.content);
        };
        assert_eq!(content, crate::agents::agent::system_prompt::side_chat::SIDE_CHAT_NO_TOOLS);
    }

    #[tokio::test]
    async fn a_model_failure_is_answered_as_an_error_without_storage_writes() {
        let (agent, storage, session, _dir) = setup(ScriptedSideChat::failing()).await;

        let detail_before = detail_at(&storage, &session).await;

        let (tx, rx) = flume::bounded(1);
        run_btw(
            agent,
            storage.clone(),
            session.clone(),
            btw_request("hi"),
            Some(tx),
        )
        .await;

        let reply = rx.recv_async().await.unwrap();
        let VizierResponseContent::Error { kind: _, message } = reply.content else {
            panic!("expected an error reply, got {:?}", reply.content);
        };
        assert!(!message.is_empty());
        assert!(reply.history_uid.is_none());

        let detail_after = detail_at(&storage, &session).await;
        assert_eq!(detail_before.is_none(), detail_after.is_none());
        assert_eq!(history_uids(&storage, &session).await.len(), 2);
    }

    #[tokio::test]
    async fn exactly_one_usage_event_per_side_chat() {
        let (agent, storage, session, _dir) =
            setup(ScriptedSideChat::text(vec![AssistantPart::Text("aside answer".into())])).await;

        let (tx, rx) = flume::bounded(1);
        run_btw(
            agent,
            storage.clone(),
            session.clone(),
            btw_request("hi"),
            Some(tx),
        )
        .await;
        let _ = rx.recv_async().await.unwrap();

        // The usage event is written from a spawned task; give it a moment to land.
        for _ in 0..50 {
            if usage_event_count(&storage, "btw-agent").await == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(usage_event_count(&storage, "btw-agent").await, 1);
    }

    #[tokio::test]
    async fn the_side_chat_completion_is_tool_less() {
        let model = ScriptedSideChat::text(vec![AssistantPart::Text("aside answer".into())]);
        let (agent, storage, session, _dir) = setup(model.clone()).await;

        let (tx, rx) = flume::bounded(1);
        run_btw(
            agent,
            storage.clone(),
            session.clone(),
            btw_request("hi"),
            Some(tx),
        )
        .await;
        let _ = rx.recv_async().await.unwrap();

        let choices = model.tool_choices();
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0], Some(ToolChoice::None));
    }

    #[test]
    fn only_a_side_chat_has_no_session_activity() {
        let activity: Vec<VizierRequestContent> = vec![
            VizierRequestContent::Chat("hi".into()),
            VizierRequestContent::Prompt("hi".into()),
            VizierRequestContent::SilentRead("hi".into()),
            VizierRequestContent::Unattended("hi".into()),
            VizierRequestContent::Command("checkpoint".into()),
            VizierRequestContent::BackgroundReport(BackgroundReport {
                job_id: "job-1".into(),
                kind: JobKind::Batch,
                delegated_to: None,
                entries: vec![],
            }),
            VizierRequestContent::ClientEvent(ClientEvent {
                text: "click".into(),
                data: None,
                source_call_id: None,
            }),
        ];
        for content in &activity {
            assert!(
                records_session_activity(content),
                "expected session activity for {content:?}"
            );
        }
        assert!(!records_session_activity(&VizierRequestContent::Btw(SideChat {
            question: "aside".into(),
            turns: vec![],
        })));
    }
}
