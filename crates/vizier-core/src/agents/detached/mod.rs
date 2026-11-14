//! Detached scripts (`specs/058-detached-tool-calls/`): `execute_python` with `detached: true`
//! runs the script in its own task while the agent carries on, and delivers its outcome back to
//! the conversation exactly once.
//!
//! Exactly once is a database fact, not a property of this module: every transition out of
//! `running` is one guarded update (`DetachedScriptStorage`), and delivery is one claim that
//! sets `delivered_at`. The agent's inner loop claims at each round boundary; a script that
//! ends while no turn runs pokes the conversation with an empty `DetachedDelivery` request,
//! whose handler claims too and runs a turn only if it got rows. Nothing here branches on the
//! kind of conversation beyond refusing the ones nobody is waiting on.

use std::{collections::HashMap, sync::Arc, time::Duration};

use futures::future::BoxFuture;
use parking_lot::Mutex;
use tokio::sync::{mpsc, watch};
use tracing::Instrument;

use crate::{
    agents::tools::ToolContext,
    error::VizierError,
    notifications::Notifications,
    sandbox::{ExecutionReport, ToolInvocationRecord},
    schema::{
        AgentId, Canceller, DetachedScript, DetachedScriptId, DetachedScriptState, VizierAttachment,
        VizierChannelId, VizierRequest, VizierRequestContent, VizierSession,
        detached::{
            DEFAULT_TIMEOUT_SECS, MAX_ARGUMENT_PREVIEW_CHARS, MAX_BACKGROUND_DEPTH,
            MAX_INTENT_CHARS, MAX_OUTCOME_BYTES, MAX_TIMEOUT_SECS, new_detached_id,
        },
    },
    storage::{
        VizierStorage,
        detached_script::{CancelOutcome, DetachedScriptStorage},
        session_file::SessionFileStorage,
    },
    transport::{SessionEvent, SessionFrame, VizierTransport},
    utils::get_mime_type,
};

pub mod delivery;

pub use delivery::{DELIVERY_HEADING, render_delivery};

/// Told the invocation records so far each time a nested call finishes.
pub type ProgressFn = Arc<dyn Fn(&[ToolInvocationRecord]) + Send + Sync>;

/// Runs the script once, given the detached context and the progress callback. Built by
/// `execute_python` so this module never reaches into the sandbox tool.
pub type ScriptRunner = Box<
    dyn FnOnce(ToolContext, ProgressFn) -> BoxFuture<'static, (ExecutionReport, Vec<VizierAttachment>)>
        + Send,
>;

/// The outcome stored for a script whose time limit was reached while it waited.
fn timed_out_reason(secs: u64) -> String {
    format!("the script's time limit ({secs}s) was reached")
}

/// Progress is written at most this often.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(1);

/// Waits between delivery pokes while the agent is not registered (being respawned).
const POKE_BACKOFF_MS: [u64; 6] = [500, 1_000, 2_000, 4_000, 8_000, 14_000];

/// The `timeout_secs` argument of a detached `execute_python`: 1..=3600, default 600. An inline
/// call ignores it (models fill in optional fields they do not use), so only a detached call
/// is checked.
pub fn validate_timeout(detached: bool, timeout_secs: Option<u64>) -> crate::Result<u64> {
    match (detached, timeout_secs) {
        (false, _) | (true, None) => Ok(DEFAULT_TIMEOUT_SECS),
        (true, Some(secs)) if (1..=MAX_TIMEOUT_SECS).contains(&secs) => Ok(secs),
        (true, Some(_)) => Err(VizierError(format!(
            "timeout_secs must be between 1 and {MAX_TIMEOUT_SECS}"
        ))),
    }
}

/// Refuse work started from a turn already this deep in background work.
pub fn check_depth(ctx: &ToolContext) -> crate::Result<()> {
    if ctx.background_depth >= MAX_BACKGROUND_DEPTH {
        return Err(VizierError(format!(
            "nesting limit ({MAX_BACKGROUND_DEPTH}) reached: this turn was itself started by sub-agent or background work"
        )));
    }
    Ok(())
}

/// Why a launch is refused before any row is written (contracts/detached-flag.md §2).
pub fn check_launch(ctx: &ToolContext) -> crate::Result<()> {
    if ctx.session.is_scheduled_task() || matches!(ctx.session.1, VizierChannelId::Dream(..)) {
        return Err(VizierError(
            "cannot detach here: nobody is waiting to be told. A scheduled run or a dream \
             cycle ends with its turn, so run the script inline."
                .into(),
        ));
    }
    check_depth(ctx)
}

/// How a live task was told to stop. Whoever stops it has already written the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Run,
    Cancelled,
    Interrupted,
}

struct Live {
    agent_id: AgentId,
    stop: watch::Sender<Stop>,
}

struct Inner {
    storage: Arc<VizierStorage>,
    transport: VizierTransport,
    notifications: Notifications,
    /// The tasks running in this process, by script id.
    live: Mutex<HashMap<DetachedScriptId, Live>>,
}

/// Launches, stops and delivers detached scripts. One per process, on `VizierDependencies`.
#[derive(Clone)]
pub struct DetachedScripts {
    inner: Arc<Inner>,
}

/// What a launch needs besides the context.
pub struct Launch {
    pub intent: String,
    pub code: String,
    pub timeout_secs: u64,
}

impl DetachedScripts {
    pub fn new(
        storage: Arc<VizierStorage>,
        transport: VizierTransport,
        notifications: Notifications,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                storage,
                transport,
                notifications,
                live: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Record the script and start it. Returns the stored row; the caller hands its id back
    /// to the model as the handle.
    pub async fn launch(
        &self,
        ctx: &ToolContext,
        launch: Launch,
        run: ScriptRunner,
    ) -> crate::Result<DetachedScript> {
        check_launch(ctx)?;
        let redactor = &ctx.secret_redactor;
        let intent: String = redactor.text(launch.intent.trim()).chars().take(MAX_INTENT_CHARS).collect();
        let script = DetachedScript {
            id: new_detached_id(),
            agent_id: ctx.session.0.clone(),
            session: ctx.session.clone(),
            parent_id: ctx.parent_detached_id.clone(),
            depth: ctx.background_depth,
            intent,
            state: DetachedScriptState::Running,
            timeout_secs: launch.timeout_secs,
            code: redactor.text(&launch.code),
            tool_calls: serde_json::json!([]),
            created_at: crate::time::now(),
            finished_at: None,
            outcome: None,
            delivered_at: None,
            cancelled_by: None,
        };
        self.inner
            .storage
            .open_detached_script(script.clone())
            .await
            .map_err(|e| VizierError(format!("could not record the detached script: {e}")))?;
        self.inner.publish(&script);

        let task_ctx = detached_context(ctx, &script);
        let (stop, stopped) = watch::channel(Stop::Run);
        self.inner.live.lock().insert(
            script.id.clone(),
            Live { agent_id: script.agent_id.clone(), stop },
        );

        let span = tracing::info_span!(
            "detached_script",
            otel.name = %format!("detached_script {}", script.id),
            vizier.detached.id = %script.id,
            vizier.detached.timeout_secs = script.timeout_secs,
            vizier.detached.depth = script.depth,
        );
        let inner = self.inner.clone();
        let running = script.clone();
        tokio::spawn(
            async move { inner.run(running, task_ctx, run, stopped).await }.instrument(span),
        );
        Ok(script)
    }

    /// Cancel a running script and every running script launched beneath it. Nothing is
    /// delivered for any of them.
    pub async fn cancel(&self, id: &str, by: Canceller) -> crate::Result<CancelOutcome> {
        let outcome = self
            .inner
            .storage
            .cancel_detached_script(id, by, crate::time::now())
            .await
            .map_err(|e| VizierError(format!("could not cancel the detached script: {e}")))?;
        if let CancelOutcome::Cancelled(ids) = &outcome {
            for id in ids {
                self.inner.signal(id, Stop::Cancelled);
                self.inner.publish_stored(id).await;
            }
        }
        Ok(outcome)
    }

    /// The agent's instance is stopping for an update or a restart: its scripts in this
    /// process are interrupted, and their conversations are told once the next instance runs
    /// (the poke waits in the agent's queue).
    pub async fn interrupt_agent(&self, agent_id: &str) {
        let ids = self.inner.live_of(agent_id);
        if ids.is_empty() {
            return;
        }
        let moved = match self.inner.storage.interrupt_running_scripts(agent_id, None, Some(&ids)).await {
            Ok(moved) => moved,
            Err(e) => {
                tracing::warn!(agent_id, "could not interrupt detached scripts: {e}");
                vec![]
            }
        };
        for id in &ids {
            self.inner.signal(id, Stop::Interrupted);
        }
        self.inner.settle_interrupted(moved).await;
    }

    /// The agent is being deleted: stop its scripts and forget them.
    pub async fn delete_agent(&self, agent_id: &str) {
        for id in self.inner.live_of(agent_id) {
            self.inner.signal(&id, Stop::Interrupted);
        }
        if let Err(e) = self.inner.storage.delete_agent_scripts(agent_id).await {
            tracing::warn!(agent_id, "could not delete detached scripts: {e}");
        }
    }

    /// A conversation is being deleted: stop its running scripts and forget them all.
    pub async fn delete_session(&self, session: &VizierSession) {
        match self.inner.storage.delete_session_scripts(session).await {
            Ok(running) => {
                for id in running {
                    self.inner.signal(&id, Stop::Interrupted);
                }
            }
            Err(e) => tracing::warn!("could not delete a conversation's detached scripts: {e}"),
        }
    }

    /// Recovery when this process claims `agent:{agent_id}` (spec 057): scripts left running
    /// under an older owner are interrupted, and every conversation with something to deliver
    /// is poked once.
    pub async fn recover_agent(&self, agent_id: &AgentId, below: u64) {
        match self.inner.storage.interrupt_running_scripts(agent_id, Some(below), None).await {
            Ok(moved) => {
                for script in &moved {
                    self.inner.publish(script);
                    self.inner.notifications.detached_finished(script).await;
                }
            }
            Err(e) => tracing::warn!(agent_id, "could not recover detached scripts: {e}"),
        }
        match self.inner.storage.deliverable_script_sessions(agent_id).await {
            Ok(sessions) => {
                for session in sessions {
                    tokio::spawn(self.inner.clone().poke(session, 0));
                }
            }
            Err(e) => tracing::warn!(agent_id, "could not list undelivered detached scripts: {e}"),
        }
    }

    /// Take this conversation's finished scripts for delivery. Whoever calls first gets them.
    pub async fn claim(&self, session: &VizierSession) -> Vec<DetachedScript> {
        match self.inner.storage.claim_deliverable_scripts(session).await {
            Ok(scripts) => {
                for script in &scripts {
                    self.inner.publish(script);
                }
                scripts
            }
            Err(e) => {
                tracing::warn!(session = %session.to_slug(), "could not claim detached scripts: {e}");
                vec![]
            }
        }
    }

    /// How many scripts of this process are running (tests).
    pub fn live_count(&self) -> usize {
        self.inner.live.lock().len()
    }
}

/// The context a detached script runs in: one level deeper, nobody to ask, no client, no turn
/// to end, and its own containers so nothing it collects lands on the launching turn.
fn detached_context(ctx: &ToolContext, script: &DetachedScript) -> ToolContext {
    let mut task = ctx.clone();
    task.background_depth = ctx.background_depth.saturating_add(1);
    task.interactive = false;
    task.client = None;
    task.detached = true;
    task.parent_detached_id = Some(script.id.clone());
    task.pending_attachments = Default::default();
    task.loaded_images = Default::default();
    task.recalled = Default::default();
    task.deadline = Some(tokio::time::Instant::now() + Duration::from_secs(script.timeout_secs));
    task
}

impl Inner {
    fn live_of(&self, agent_id: &str) -> Vec<DetachedScriptId> {
        self.live
            .lock()
            .iter()
            .filter(|(_, live)| live.agent_id == agent_id)
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn signal(&self, id: &str, why: Stop) {
        if let Some(live) = self.live.lock().get(id) {
            live.stop.send_replace(why);
        }
    }

    fn publish(&self, script: &DetachedScript) {
        self.transport.publish_session_event(SessionEvent {
            session: script.session.clone(),
            frame: SessionFrame::DetachedScript(script.snapshot()),
        });
    }

    async fn publish_stored(&self, id: &str) {
        match self.storage.get_detached_script(id).await {
            Ok(Some(script)) => self.publish(&script),
            Ok(None) => {}
            Err(e) => tracing::warn!("could not read detached script {id}: {e}"),
        }
    }

    async fn settle_interrupted(self: &Arc<Self>, moved: Vec<DetachedScript>) {
        let mut sessions: Vec<(VizierSession, u8)> = vec![];
        for script in &moved {
            self.publish(script);
            self.notifications.detached_finished(script).await;
            if !sessions.iter().any(|(s, _)| s == &script.session) {
                sessions.push((script.session.clone(), script.depth));
            }
        }
        // Spawned: the agent may be between instances, and the poke waits for the next one.
        for (session, depth) in sessions {
            tokio::spawn(self.clone().poke(session, depth));
        }
    }

    /// The task of one script.
    async fn run(
        self: Arc<Self>,
        script: DetachedScript,
        ctx: ToolContext,
        run: ScriptRunner,
        mut stop: watch::Receiver<Stop>,
    ) {
        let id = script.id.clone();
        let latest: Arc<Mutex<Vec<ToolInvocationRecord>>> = Default::default();
        let (wake, woken) = mpsc::unbounded_channel::<()>();
        let progress: ProgressFn = {
            let latest = latest.clone();
            Arc::new(move |records: &[ToolInvocationRecord]| {
                *latest.lock() = records.to_vec();
                let _ = wake.send(());
            })
        };
        let writer = tokio::spawn(self.clone().write_progress(script.clone(), latest.clone(), woken));

        let limit = Duration::from_secs(script.timeout_secs);
        let outcome = tokio::select! {
            ran = tokio::time::timeout(limit, run(ctx.clone(), progress)) => Some(ran),
            _ = stopped(&mut stop) => None,
        };
        writer.abort();
        self.live.lock().remove(&id);

        let (state, outcome, tool_calls) = match outcome {
            // Cancelled or interrupted: whoever stopped it has written the row.
            None => return,
            Some(Err(_)) => (
                DetachedScriptState::TimedOut,
                serde_json::Value::String(timed_out_reason(script.timeout_secs)),
                stored_calls(&latest.lock()),
            ),
            Some(Ok((report, attachments))) => {
                let files = self.save_files(&script.session, &attachments).await;
                let state = match &report.error {
                    None if report.ok => DetachedScriptState::Succeeded,
                    Some(err) if err.limit.as_deref() == Some("timeout") => DetachedScriptState::TimedOut,
                    _ => DetachedScriptState::Failed,
                };
                let calls = stored_calls(&report.tool_calls);
                (state, bounded_outcome(&report, &files), calls)
            }
        };

        let closed = match self
            .storage
            .close_detached_script(&id, state, outcome, tool_calls, crate::time::now())
            .await
        {
            Ok(closed) => closed,
            Err(e) => {
                // A newer owner holds the agent: it recovers the row.
                tracing::warn!(script = %id, "could not record the end of a detached script: {e}");
                return;
            }
        };
        if !closed {
            // A cancel or an interrupt won the race.
            return;
        }
        let Ok(Some(stored)) = self.storage.get_detached_script(&id).await else {
            return;
        };
        self.publish(&stored);
        self.notifications.detached_finished(&stored).await;
        self.clone().poke(stored.session.clone(), stored.depth).await;
    }

    /// Write the latest records at most once a second. A row that no longer reads `running`
    /// (cancelled or interrupted by another process) stops the task.
    async fn write_progress(
        self: Arc<Self>,
        script: DetachedScript,
        latest: Arc<Mutex<Vec<ToolInvocationRecord>>>,
        mut woken: mpsc::UnboundedReceiver<()>,
    ) {
        let mut written = 0usize;
        while woken.recv().await.is_some() {
            while woken.try_recv().is_ok() {}
            let records = latest.lock().clone();
            match self.storage.record_detached_progress(&script.id, stored_calls(&records)).await {
                Ok(Some(DetachedScriptState::Running)) => {}
                Ok(Some(DetachedScriptState::Cancelled)) => {
                    self.signal(&script.id, Stop::Cancelled);
                    return;
                }
                Ok(_) => {
                    self.signal(&script.id, Stop::Interrupted);
                    return;
                }
                Err(e) => {
                    tracing::warn!(script = %script.id, "could not record detached progress: {e}");
                    if crate::storage::fence::is_ownership_lost(&e) {
                        self.signal(&script.id, Stop::Interrupted);
                        return;
                    }
                }
            }
            if records.len() != written {
                written = records.len();
                let mut snapshot = script.clone();
                snapshot.tool_calls = stored_calls(&records);
                self.publish(&snapshot);
            }
            tokio::time::sleep(PROGRESS_INTERVAL).await;
        }
    }

    /// Files the script produced become session files of its conversation, as a turn's tool
    /// attachments do. Returns their names for the delivery.
    async fn save_files(&self, session: &VizierSession, attachments: &[VizierAttachment]) -> Vec<String> {
        let mut names = vec![];
        for attachment in attachments {
            let saved = async {
                let content = self.transport.send_file_resolve(attachment.clone()).await?;
                let record = self
                    .transport
                    .send_file_upload(attachment.filename.clone(), content)
                    .await?;
                self.storage
                    .save_session_file(
                        session,
                        &attachment.filename,
                        &get_mime_type(&attachment.filename),
                        record.size,
                        &record.id,
                    )
                    .await?;
                anyhow::Ok(())
            }
            .await;
            match saved {
                Ok(()) => names.push(attachment.filename.clone()),
                Err(e) => tracing::warn!(file = %attachment.filename, "could not save a detached script's file: {e}"),
            }
        }
        names
    }

    /// Tell the conversation something may be ready. The handler claims; an empty claim runs
    /// no turn, so a poke that loses to a round-boundary claim costs nothing.
    async fn poke(self: Arc<Self>, session: VizierSession, depth: u8) {
        let request = VizierRequest {
            timestamp: crate::time::now(),
            user: session.0.clone(),
            content: VizierRequestContent::DetachedDelivery(String::new()),
            metadata: serde_json::json!({}),
            background_depth: depth.saturating_add(1),
            ..Default::default()
        };
        let mut last_err = None;
        for attempt in 0..=POKE_BACKOFF_MS.len() {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(POKE_BACKOFF_MS[attempt - 1])).await;
            }
            let mut request = request.clone();
            request.timestamp = crate::time::now();
            let replies = self.transport.broadcast_sender(session.clone()).await.ok();
            match self.transport.send_request(session.clone(), request, replies).await {
                Ok(()) => return,
                Err(e) => last_err = Some(e.to_string()),
            }
        }
        // The row stays deliverable: the next round boundary or the next recovery takes it.
        tracing::warn!(
            session = %session.to_slug(),
            "could not poke a conversation about finished background work: {}",
            last_err.unwrap_or_default()
        );
    }
}

async fn stopped(stop: &mut watch::Receiver<Stop>) {
    while *stop.borrow_and_update() == Stop::Run {
        if stop.changed().await.is_err() {
            // The sender lives in `live` until the task removes it: never reached while running.
            std::future::pending::<()>().await;
        }
    }
}

/// The stored form of the calls so far: argument previews cut to 500 characters each, the
/// list cut at 32 KiB. Already redacted by the bridge.
fn stored_calls(records: &[ToolInvocationRecord]) -> serde_json::Value {
    let mut out = vec![];
    let mut size = 0usize;
    for record in records {
        let mut value = serde_json::to_value(record).unwrap_or_default();
        if let Some(arguments) = value.get_mut("arguments") {
            preview(arguments);
        }
        size += value.to_string().len();
        if size > MAX_OUTCOME_BYTES {
            break;
        }
        out.push(value);
    }
    serde_json::Value::Array(out)
}

fn preview(arguments: &mut serde_json::Value) {
    let cut = |text: &str| -> String {
        if text.chars().count() <= MAX_ARGUMENT_PREVIEW_CHARS {
            text.to_string()
        } else {
            let head: String = text.chars().take(MAX_ARGUMENT_PREVIEW_CHARS).collect();
            format!("{head}…")
        }
    };
    match arguments {
        serde_json::Value::Object(map) => {
            for value in map.values_mut() {
                match value {
                    serde_json::Value::String(text) => *text = cut(text),
                    other => {
                        let text = other.to_string();
                        if text.chars().count() > MAX_ARGUMENT_PREVIEW_CHARS {
                            *other = serde_json::Value::String(cut(&text));
                        }
                    }
                }
            }
        }
        other => {
            let text = other.to_string();
            if text.chars().count() > MAX_ARGUMENT_PREVIEW_CHARS {
                *other = serde_json::Value::String(cut(&text));
            }
        }
    }
}

/// The report as stored and delivered: without its tool calls (stored beside it), with the
/// saved file names, and with `result`, `stdout` and the traceback cut so the whole fits 32 KiB.
fn bounded_outcome(report: &ExecutionReport, files: &[String]) -> serde_json::Value {
    let mut value = serde_json::to_value(report).unwrap_or_default();
    if let Some(map) = value.as_object_mut() {
        map.remove("tool_calls");
        if !files.is_empty() {
            map.insert("files".into(), serde_json::json!(files));
        }
    }
    if value.to_string().len() <= MAX_OUTCOME_BYTES {
        return value;
    }
    let shrink = |value: &mut serde_json::Value, budget: usize| {
        let text = match &mut *value {
            serde_json::Value::String(text) => std::mem::take(text),
            other => other.to_string(),
        };
        *value = serde_json::Value::String(delivery::cut(text, budget));
    };
    if let Some(map) = value.as_object_mut() {
        if let Some(result) = map.get_mut("result") {
            shrink(result, MAX_OUTCOME_BYTES / 2);
        }
        if let Some(stdout) = map.get_mut("stdout") {
            shrink(stdout, MAX_OUTCOME_BYTES / 4);
        }
        if let Some(traceback) = map.get_mut("error").and_then(|e| e.get_mut("traceback")) {
            shrink(traceback, MAX_OUTCOME_BYTES / 8);
        }
    }
    value
}

#[cfg(test)]
mod tests;
