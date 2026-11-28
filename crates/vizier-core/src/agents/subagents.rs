//! Subagents as temporary workers (`specs/059-subagent-workers/`).
//!
//! A worker is a turn of the main agent on its own `Subagent` conversation, recorded as one
//! `subagent` row. The main agent spawns workers and gets handles back at once; it waits for
//! them, replies to their questions and stops them through the `subagents` tools, which call
//! this service. A worker lives as long as the conversation it was spawned in: its turns are
//! durable (resumed after a restart), a parent's `/abort` never touches it, and only a stop, its
//! time limit, or deleting the conversation or agent ends it early.
//!
//! The engine records outcomes, not a waiting tool (research D5): a worker's turn ending settles
//! its row in `agent_process`, `ask_main_agent` moves it to `asking`. Every change publishes a
//! `SessionFrame::Subagent` on the hiring conversation (the wake-up `wait_subagents` listens for)
//! and an outcome the main agent has not seen is delivered once through `agents::delivery`.

use std::{collections::HashMap, sync::Arc, time::Duration};

use parking_lot::Mutex;
use tokio::task::AbortHandle;

use crate::{
    agents::tools::ToolContext,
    error::VizierError,
    schema::{
        AgentId, Subagent, SubagentBrief, SubagentId, SubagentReplyText, SubagentState,
        SubagentStopReason, VizierChannelId, VizierRequest, VizierRequestContent, VizierSession,
        ConsultationFrame, ConsultationReplyKind,
        subagent::{
            DEFAULT_TIMEOUT_SECS, MAX_QUESTION_BYTES, MAX_RESULT_CHARS, MAX_TASK_BYTES,
            MAX_TIMEOUT_SECS, cut, new_consultation_id, new_group_id, new_subagent_id,
        },
    },
    storage::{
        trigger::TriggerStorage,
        VizierStorage,
        subagent::{Settled, StopOutcome, SubagentStorage},
    },
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

/// The list tool returns the unfinished workers plus this many final ones.
pub const LIST_FINAL_LIMIT: usize = 50;
/// A wait returns this long before its call's deadline, so the result still reaches the script.
const WAIT_GRACE: Duration = Duration::from_secs(2);
/// A wait with no deadline of its own (outside a script) waits at most this long.
const WAIT_CAP: Duration = Duration::from_secs(600);
/// Why a worker timed out.
fn timed_out_reason(secs: u64) -> String {
    format!("the worker's time limit ({secs}s) was reached")
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

/// A task or reply text: non-empty and at most 16 KiB.
pub fn validate_text(what: &str, text: &str, max_bytes: usize) -> crate::Result<()> {
    if text.trim().is_empty() {
        return Err(VizierError(format!("{what} must not be empty")));
    }
    if text.len() > max_bytes {
        return Err(VizierError(format!(
            "{what} is {} bytes; the limit is {max_bytes}",
            text.len()
        )));
    }
    Ok(())
}

/// The launching turn's project, for a worker (spec 041 FR-007a). A worker's own workers get
/// none: only a person's own project chat is the source of a scope.
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

/// Whether the main agent may manage workers from this context (FR-018): below the configured
/// depth, and not in the dream cycle.
pub fn may_hire(ctx: &ToolContext, max_depth: u8) -> bool {
    ctx.subagent_depth < max_depth && !matches!(ctx.session.1, VizierChannelId::Dream(..))
}

/// Whether this turn is a worker's own (it is offered `ask_main_agent`, FR-014).
pub fn is_worker(ctx: &ToolContext) -> bool {
    matches!(ctx.session.1, VizierChannelId::Subagent)
}

/// Whether this turn is a consulted agent's, in a consultation conversation (it is offered
/// `ask_consulting_agent`, spec 064 FR-007).
pub fn is_consultee(ctx: &ToolContext) -> bool {
    matches!(ctx.session.1, VizierChannelId::Consultation(_))
}

/// Which rows a family of tools manages: the subagent tools see workers, the consultation tools
/// consultations (spec 064 research D7). A handle of the other family reads as unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Workers,
    Consultations,
}

impl Family {
    fn holds(self, row: &Subagent) -> bool {
        row.is_consultation() == (self == Self::Consultations)
    }

    fn unknown(self, id: &str) -> Refusal {
        match self {
            Self::Workers => Refusal::UnknownSubagent(id.to_string()),
            Self::Consultations => Refusal::UnknownConsultation(id.to_string()),
        }
    }
}

/// One thing a group opens: a temporary worker for a task, or a consultation of a real agent.
#[derive(Debug, Clone)]
pub enum Target {
    Worker { task: String },
    Agent { agent_id: AgentId, request: String },
}

/// Who a consultation is for (spec 064 research D5): the person whose view right admitted it,
/// and the name the consultee is shown (`None` when no person is involved).
#[derive(Debug, Clone)]
pub struct OnBehalf {
    pub user_id: String,
    pub display: Option<String>,
}

/// What a refused tool call says, as data the agent can act on (FR-015).
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    UnknownSubagent(String),
    NotAsking { id: String, state: SubagentState },
    /// A consultation handle that does not exist in this conversation (spec 064).
    UnknownConsultation(String),
    /// A reply or follow-up to a consultation that is neither asking nor answered.
    NotWaiting { id: String, state: SubagentState },
}

impl Refusal {
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::UnknownSubagent(id) => serde_json::json!({
                "status": "unknown_subagent",
                "message": format!("no worker {id} was spawned in this conversation"),
            }),
            Self::NotAsking { id, state } => serde_json::json!({
                "status": "not_asking",
                "message": format!("{id} is {}, not asking; reply only to a worker that asked", state.as_str()),
            }),
            Self::UnknownConsultation(id) => serde_json::json!({
                "status": "unknown_consultation",
                "message": format!("no consultation {id} was started in this conversation"),
            }),
            Self::NotWaiting { id, state } => serde_json::json!({
                "status": "not_waiting",
                "message": format!(
                    "{id} is {}; reply to a consultation that is asking, or follow up one that answered",
                    state.as_str()
                ),
            }),
        }
    }
}

/// The tool whose call ends a worker's turn in `asking`.
pub const ASK_TOOL: &str = "ask_main_agent";
/// The tool whose call ends a consulted agent's turn in `asking` (spec 064).
pub const CONSULTEE_ASK_TOOL: &str = "ask_consulting_agent";

/// Whether a turn ended by asking, through either tool.
pub fn ended_by_asking(tool: Option<&str>) -> bool {
    matches!(tool, Some(ASK_TOOL) | Some(CONSULTEE_ASK_TOOL))
}

/// How a worker's turn ended, as `agent_process` saw it.
#[derive(Debug, Clone)]
pub enum TurnEnd {
    /// The turn returned a reply (its text).
    Replied(String),
    /// The turn ended by asking the main agent. Its outcome is the question, already recorded
    /// by the tool, so it settles nothing: the main agent may have replied (and the worker be
    /// running again) before this turn finished, and settling here would take the next turn's
    /// answer away.
    Asked,
    /// The turn failed or panicked.
    Failed(String),
}

/// Waiting mode of `wait_subagents`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    Any,
    All,
}

/// What `wait_subagents` returns.
#[derive(Debug, Clone)]
pub struct Waited {
    pub met: bool,
    pub workers: Vec<Subagent>,
}

/// Runs workers. One per process, on `VizierDependencies`; cheap to clone.
#[derive(Clone)]
pub struct Subagents {
    inner: Arc<Inner>,
}

struct Inner {
    storage: Arc<VizierStorage>,
    transport: VizierTransport,
    /// One time-limit task per running worker of this process.
    timers: Mutex<HashMap<SubagentId, AbortHandle>>,
    /// Workers a `wait_subagents` of this process is watching, with how many waits watch each.
    /// Delivery skips them, because the wait hands their outcome back itself: without this, an
    /// idle conversation's poke could deliver an answer the wait is about to return too.
    watched: Mutex<HashMap<SubagentId, usize>>,
}

/// A wait's hold on the workers it watches. Released on every way out of the wait (a return,
/// an error, the call dropped), and then anything it did not hand back is poked for delivery.
struct Watch {
    inner: Arc<Inner>,
    ids: Vec<SubagentId>,
    session: VizierSession,
}

impl Watch {
    fn new(inner: &Arc<Inner>, ids: &[SubagentId], session: &VizierSession) -> Self {
        let mut watched = inner.watched.lock();
        for id in ids {
            *watched.entry(id.clone()).or_default() += 1;
        }
        Self {
            inner: inner.clone(),
            ids: ids.to_vec(),
            session: session.clone(),
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        {
            let mut watched = self.inner.watched.lock();
            for id in &self.ids {
                if let Some(count) = watched.get_mut(id) {
                    *count -= 1;
                    if *count == 0 {
                        watched.remove(id);
                    }
                }
            }
        }
        // An outcome that arrived after the wait's last read was neither returned nor delivered:
        // poke, and the claim (no longer skipping it) takes it.
        let transport = self.inner.transport.clone();
        let session = self.session.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle
                .spawn(async move { crate::agents::delivery::poke(&transport, session, 0).await });
        }
    }
}

impl Subagents {
    pub fn new(storage: Arc<VizierStorage>, transport: VizierTransport) -> Self {
        Self {
            inner: Arc::new(Inner {
                storage,
                transport,
                timers: Mutex::new(HashMap::new()),
                watched: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Record a group of workers and start them; returns their rows in the order given, at once.
    pub async fn spawn(
        &self,
        ctx: &ToolContext,
        tasks: Vec<String>,
        timeout_secs: u64,
    ) -> crate::Result<Vec<Subagent>> {
        if tasks.is_empty() {
            return Err(VizierError("tasks must not be empty".into()));
        }
        let targets = tasks.into_iter().map(|task| Target::Worker { task }).collect();
        self.open_group(ctx, targets, timeout_secs, None).await
    }

    /// `consult_agent` (spec 064): record a group of consultations of real agents and send each
    /// its first request; returns their rows in the order given, at once. Who may be consulted
    /// was decided by the caller (`consultations::access`); `on_behalf` is that person.
    pub async fn consult(
        &self,
        ctx: &ToolContext,
        consultations: Vec<(AgentId, String)>,
        timeout_secs: u64,
        on_behalf: OnBehalf,
    ) -> crate::Result<Vec<Subagent>> {
        if consultations.is_empty() {
            return Err(VizierError("consultations must not be empty".into()));
        }
        let targets = consultations
            .into_iter()
            .map(|(agent_id, request)| Target::Agent { agent_id, request })
            .collect();
        self.open_group(ctx, targets, timeout_secs, Some(on_behalf)).await
    }

    /// One group of workers or consultations: validated, recorded in one transaction, then each
    /// started. A row whose first request cannot be sent fails at once.
    async fn open_group(
        &self,
        ctx: &ToolContext,
        targets: Vec<Target>,
        timeout_secs: u64,
        on_behalf: Option<OnBehalf>,
    ) -> crate::Result<Vec<Subagent>> {
        for target in &targets {
            match target {
                Target::Worker { task } => validate_text("a task", task, MAX_TASK_BYTES)?,
                Target::Agent { request, .. } => validate_text("a request", request, MAX_TASK_BYTES)?,
            }
        }
        let now = crate::time::now();
        let group_id = new_group_id();
        let group_size = targets.len() as u32;
        let rows: Vec<Subagent> = targets
            .into_iter()
            .enumerate()
            .map(|(i, target)| {
                let (id, task, consultee_id) = match target {
                    Target::Worker { task } => (new_subagent_id(), task, None),
                    Target::Agent { agent_id, request } => {
                        (new_consultation_id(), request, Some(agent_id))
                    }
                };
                Subagent {
                id,
                agent_id: ctx.session.0.clone(),
                hiring_session: ctx.session.clone(),
                parent_detached_id: ctx.parent_detached_id.clone(),
                group_id: group_id.clone(),
                position: i as u32 + 1,
                group_size,
                worker_topic: uuid::Uuid::new_v4().to_string(),
                task,
                timeout_secs,
                state: SubagentState::Running,
                stop_reason: None,
                question: None,
                result: None,
                result_truncated: false,
                error: None,
                outcome_seq: 0,
                seen_seq: 0,
                delivered_seq: 0,
                deadline_at: Some(now + chrono::Duration::seconds(timeout_secs as i64)),
                remaining_ms: None,
                created_at: now,
                updated_at: now,
                ended_at: None,
                principal_user_id: consultee_id
                    .as_ref()
                    .and(on_behalf.as_ref().map(|who| who.user_id.clone())),
                consultee_id,
                follow_ups: 0,
            }})
            .collect();
        self.inner
            .storage
            .open_subagents(rows.clone())
            .await
            .map_err(|e| VizierError(format!("could not record the workers: {e}")))?;

        if let Some(tool_scope) = &ctx.tool_scope {
            for row in &rows {
                self.inner
                    .storage
                    .set_work_scope(&row.id, &row.agent_id, tool_scope)
                    .await
                    .map_err(|e| VizierError(format!("could not record the workers: {e}")))?;
            }
        }
        let scope = project_scope(ctx);
        let frame = on_behalf.as_ref().map(|who| ConsultationFrame {
            on_behalf_of: who.display.clone(),
        });
        for row in &rows {
            self.inner.publish(row);
            self.inner.arm(row);
            let brief = SubagentBrief {
                subagent_id: row.id.clone(),
                position: row.position,
                group_size: row.group_size,
                text: row.task.clone(),
                consultation: row.is_consultation().then(|| frame.clone()).flatten(),
            };
            let request = first_request(ctx, row, scope.clone(), VizierRequestContent::SubagentTask(brief));
            if let Err(e) = self
                .inner
                .transport
                .send_request(row.worker_session(), request, None)
                .await
            {
                tracing::warn!(subagent = %row.id, "could not start a worker: {e}");
                self.inner
                    .settle_row(&row.id, Settled::Failed(format!("could not start: {e}")))
                    .await;
            }
        }
        Ok(rows)
    }

    /// A worker's turn ended (`agent_process`). A no-op for anything that is not a worker's
    /// conversation, or a worker that is no longer running (it asked, or was stopped).
    pub async fn turn_ended(&self, session: &VizierSession, end: TurnEnd) {
        if !matches!(session.1, VizierChannelId::Subagent | VizierChannelId::Consultation(_)) {
            return;
        }
        let Some(topic) = session.2.as_deref() else {
            return;
        };
        let row = match self.inner.storage.subagent_by_worker(topic).await {
            Ok(Some(row)) => row,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!(topic, "could not read a worker: {e}");
                return;
            }
        };
        let settled = match end {
            TurnEnd::Asked => return,
            TurnEnd::Replied(text) => {
                let (result, truncated) = cut(&text, MAX_RESULT_CHARS);
                Settled::Answered { result, truncated }
            }
            TurnEnd::Failed(reason) => Settled::Failed(reason),
        };
        self.inner.settle_row(&row.id, settled).await;
    }

    /// `ask_main_agent`: the worker of `ctx.session` asks its main agent one question.
    pub async fn ask(&self, ctx: &ToolContext, question: &str) -> crate::Result<Subagent> {
        validate_text("the question", question, MAX_QUESTION_BYTES)?;
        let topic = match (&ctx.session.1, ctx.session.2.as_deref()) {
            (VizierChannelId::Subagent | VizierChannelId::Consultation(_), Some(topic)) => topic,
            _ => return Err(VizierError("only a worker can ask its main agent".into())),
        };
        let row = self
            .inner
            .storage
            .subagent_by_worker(topic)
            .await
            .map_err(|e| VizierError(format!("could not read this worker: {e}")))?
            .ok_or_else(|| VizierError("this conversation is not a worker's".into()))?;
        let now = crate::time::now();
        let remaining_ms = row
            .deadline_at
            .map(|deadline| (deadline - now).num_milliseconds().max(0))
            .unwrap_or(0);
        let asked = self
            .inner
            .storage
            .ask_subagent(&row.id, question.trim(), remaining_ms, now)
            .await
            .map_err(|e| VizierError(format!("could not record the question: {e}")))?
            .ok_or_else(|| {
                VizierError(format!(
                    "this worker is {}, not running",
                    row.state.as_str()
                ))
            })?;
        self.inner.disarm(&asked.id);
        self.inner.publish(&asked);
        self.inner.poke(&asked);
        Ok(asked)
    }

    /// `reply_subagent`: answer a worker that asked; it continues in its own conversation.
    pub async fn reply(
        &self,
        ctx: &ToolContext,
        id: &str,
        text: &str,
    ) -> crate::Result<Result<Subagent, Refusal>> {
        self.reply_in(ctx, id, text, Family::Workers).await
    }

    /// `reply_consultation` (spec 064 research D4): answer a consultation that asked, or follow
    /// up one that answered; either way the consultee continues the same conversation. Access
    /// was re-checked by the caller.
    pub async fn reply_consultation(
        &self,
        ctx: &ToolContext,
        id: &str,
        text: &str,
    ) -> crate::Result<Result<Subagent, Refusal>> {
        self.reply_in(ctx, id, text, Family::Consultations).await
    }

    async fn reply_in(
        &self,
        ctx: &ToolContext,
        id: &str,
        text: &str,
        family: Family,
    ) -> crate::Result<Result<Subagent, Refusal>> {
        validate_text("the reply", text, MAX_TASK_BYTES)?;
        let row = match self.owned_in(ctx, id, family).await? {
            Ok(row) => row,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let refused = |id: String, state: SubagentState| match family {
            Family::Workers => Refusal::NotAsking { id, state },
            Family::Consultations => Refusal::NotWaiting { id, state },
        };
        let follow_up = family == Family::Consultations && row.state == SubagentState::Answered;
        if row.state != SubagentState::Asking && !follow_up {
            return Ok(Err(refused(row.id, row.state)));
        }
        let now = crate::time::now();
        let moved = if follow_up {
            let deadline = now + chrono::Duration::seconds(row.timeout_secs as i64);
            self.inner.storage.follow_up_subagent(&row.id, deadline, now).await
        } else {
            let deadline =
                now + chrono::Duration::milliseconds(row.remaining_ms.unwrap_or(0).max(1_000));
            self.inner.storage.reply_subagent(&row.id, deadline, now).await
        };
        let Some(replied) =
            moved.map_err(|e| VizierError(format!("could not record the reply: {e}")))?
        else {
            // Stopped, answered or asked again in between.
            let state = self
                .inner
                .read(&row.id)
                .await
                .map_or(SubagentState::Stopped, |r| r.state);
            return Ok(Err(refused(row.id, state)));
        };
        self.inner.publish(&replied);
        self.inner.arm(&replied);
        let content = VizierRequestContent::SubagentReply(SubagentReplyText {
            subagent_id: replied.id.clone(),
            text: text.to_string(),
            consultation: replied.is_consultation().then_some(if follow_up {
                ConsultationReplyKind::FollowUp
            } else {
                ConsultationReplyKind::Reply
            }),
        });
        let mut request = first_request(ctx, &replied, project_scope(ctx), content);
        // Replying never widens the work: it keeps the scope it was started with, narrowed by the
        // replying turn's own, and its next outcome is delivered under the same.
        let stored = crate::agents::delivery::stored_scope(&self.inner.storage, &replied.id).await;
        request.tool_scope =
            crate::agents::tools::scope::intersect(stored.as_deref(), request.tool_scope.as_deref());
        if let Some(tool_scope) = &request.tool_scope {
            self.inner
                .storage
                .set_work_scope(&replied.id, &replied.agent_id, tool_scope)
                .await
                .map_err(|e| VizierError(format!("could not record the reply: {e}")))?;
        }
        if let Err(e) = self
            .inner
            .transport
            .send_request(replied.worker_session(), request, None)
            .await
        {
            tracing::warn!(subagent = %replied.id, "could not deliver a reply: {e}");
            self.inner
                .settle_row(
                    &replied.id,
                    Settled::Failed(format!("could not deliver the reply: {e}")),
                )
                .await;
        }
        Ok(Ok(replied))
    }

    /// `stop_subagent`: stop one running or asking worker; the others are untouched.
    pub async fn stop(
        &self,
        ctx: &ToolContext,
        id: &str,
    ) -> crate::Result<Result<(Subagent, bool), Refusal>> {
        self.stop_in(ctx, id, Family::Workers).await
    }

    /// `stop_consultation` (spec 064).
    pub async fn stop_consultation(
        &self,
        ctx: &ToolContext,
        id: &str,
    ) -> crate::Result<Result<(Subagent, bool), Refusal>> {
        self.stop_in(ctx, id, Family::Consultations).await
    }

    async fn stop_in(
        &self,
        ctx: &ToolContext,
        id: &str,
        family: Family,
    ) -> crate::Result<Result<(Subagent, bool), Refusal>> {
        if let Err(refusal) = self.owned_in(ctx, id, family).await? {
            return Ok(Err(refusal));
        }
        let outcome = self
            .inner
            .storage
            .stop_subagent(id, SubagentStopReason::ByAgent, crate::time::now())
            .await
            .map_err(|e| VizierError(format!("could not stop the worker: {e}")))?;
        Ok(match outcome {
            StopOutcome::Stopped(row) => {
                self.inner.halt(&row).await;
                Ok((row, true))
            }
            StopOutcome::AlreadyFinished(row) => Ok((row, false)),
            StopOutcome::NotFound => Err(family.unknown(id)),
        })
    }

    /// `list_subagents`: this conversation's workers. Marks nothing seen.
    pub async fn list(&self, ctx: &ToolContext) -> crate::Result<Vec<Subagent>> {
        self.list_in(ctx, Family::Workers).await
    }

    /// `list_consultations` (spec 064).
    pub async fn list_consultations(&self, ctx: &ToolContext) -> crate::Result<Vec<Subagent>> {
        self.list_in(ctx, Family::Consultations).await
    }

    async fn list_in(&self, ctx: &ToolContext, family: Family) -> crate::Result<Vec<Subagent>> {
        let rows = self
            .inner
            .storage
            .list_subagents(&ctx.session, LIST_FINAL_LIMIT)
            .await
            .map_err(|e| VizierError(format!("could not list the workers: {e}")))?;
        Ok(rows.into_iter().filter(|row| family.holds(row)).collect())
    }

    /// `get_subagent`: one worker; its settled or asking outcome counts as seen.
    pub async fn get(
        &self,
        ctx: &ToolContext,
        id: &str,
    ) -> crate::Result<Result<Subagent, Refusal>> {
        self.get_in(ctx, id, Family::Workers).await
    }

    /// A consultation of this conversation, read without counting as seen (the reply tool reads
    /// it to re-check access before replying).
    pub async fn get_consultation_quiet(
        &self,
        ctx: &ToolContext,
        id: &str,
    ) -> crate::Result<Result<Subagent, Refusal>> {
        self.owned_in(ctx, id, Family::Consultations).await
    }

    /// `get_consultation` (spec 064).
    pub async fn get_consultation(
        &self,
        ctx: &ToolContext,
        id: &str,
    ) -> crate::Result<Result<Subagent, Refusal>> {
        self.get_in(ctx, id, Family::Consultations).await
    }

    async fn get_in(
        &self,
        ctx: &ToolContext,
        id: &str,
        family: Family,
    ) -> crate::Result<Result<Subagent, Refusal>> {
        let row = match self.owned_in(ctx, id, family).await? {
            Ok(row) => row,
            Err(refusal) => return Ok(Err(refusal)),
        };
        self.inner.mark_seen(std::slice::from_ref(&row)).await;
        Ok(Ok(row))
    }

    /// `wait_subagents`: return when one (`Any`) or every (`All`) given worker has settled —
    /// asking counts — or when the call's own time runs out. Never changes a worker.
    pub async fn wait(
        &self,
        ctx: &ToolContext,
        ids: &[String],
        until: Until,
    ) -> crate::Result<Result<Waited, Refusal>> {
        self.wait_in(ctx, ids, until, Family::Workers).await
    }

    /// `wait_consultations` (spec 064): `wait_subagents` for consultations.
    pub async fn wait_consultations(
        &self,
        ctx: &ToolContext,
        ids: &[String],
        until: Until,
    ) -> crate::Result<Result<Waited, Refusal>> {
        self.wait_in(ctx, ids, until, Family::Consultations).await
    }

    async fn wait_in(
        &self,
        ctx: &ToolContext,
        ids: &[String],
        until: Until,
        family: Family,
    ) -> crate::Result<Result<Waited, Refusal>> {
        if ids.is_empty() {
            return Err(VizierError("ids must not be empty".into()));
        }
        // Watch before the first read, so no delivery takes these outcomes meanwhile, and
        // subscribe before it too, so a change in between still wakes us.
        let _watch = Watch::new(&self.inner, ids, &ctx.session);
        let mut events = self.inner.transport.subscribe_session_events();
        let deadline = ctx
            .deadline
            .map(|d| d.checked_sub(WAIT_GRACE).unwrap_or(d))
            .unwrap_or_else(|| tokio::time::Instant::now() + WAIT_CAP);
        loop {
            let mut workers = Vec::with_capacity(ids.len());
            for id in ids {
                match self.owned_in(ctx, id, family).await? {
                    Ok(row) => workers.push(row),
                    Err(refusal) => return Ok(Err(refusal)),
                }
            }
            let settled = workers.iter().filter(|w| w.state.is_settled()).count();
            let met = match until {
                Until::Any => settled > 0,
                Until::All => settled == workers.len(),
            };
            if met || tokio::time::Instant::now() >= deadline {
                self.inner.mark_seen(&workers).await;
                return Ok(Ok(Waited { met, workers }));
            }
            // Wake on any change to this conversation's workers; a lagged receiver just reads
            // the rows again.
            let woke = tokio::time::timeout_at(deadline, async {
                loop {
                    match events.recv().await {
                        Ok(SessionEvent {
                            session,
                            frame: SessionFrame::Subagent(_),
                        }) if session == ctx.session => return,
                        Ok(_) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            std::future::pending::<()>().await
                        }
                    }
                }
            })
            .await;
            if woke.is_err() {
                // Deadline: one last read decides.
                continue;
            }
        }
    }

    /// Recovery when this process claims `agent:{agent_id}`: running workers left by an older
    /// owner are re-owned and their time limits re-armed. Their turns are resumed by the turn
    /// recovery; asking workers need nothing.
    pub async fn recover_agent(&self, agent_id: &AgentId, below: u64) {
        match self
            .inner
            .storage
            .reown_running_subagents(agent_id, below)
            .await
        {
            Ok(rows) => {
                for row in &rows {
                    self.inner.arm(row);
                }
            }
            Err(e) => tracing::warn!(agent_id = %agent_id, "could not recover workers: {e}"),
        }
    }

    /// A hiring conversation is being deleted: stop its workers and delete their rows. Returns
    /// the workers' own conversations, for the caller to delete with it.
    pub async fn delete_session(&self, session: &VizierSession) -> Vec<VizierSession> {
        match self
            .inner
            .storage
            .stop_session_subagents(
                session,
                SubagentStopReason::ConversationDeleted,
                crate::time::now(),
            )
            .await
        {
            Ok(stopped) => {
                for row in &stopped {
                    self.inner.disarm(&row.id);
                    self.inner.abort_worker(row).await;
                }
            }
            Err(e) => {
                tracing::warn!(session = %session.to_slug(), "could not stop a conversation's workers: {e}")
            }
        }
        match self.inner.storage.delete_session_subagents(session).await {
            Ok(rows) => rows.iter().map(Subagent::worker_session).collect(),
            Err(e) => {
                tracing::warn!(session = %session.to_slug(), "could not delete a conversation's workers: {e}");
                vec![]
            }
        }
    }

    /// An agent is being deleted: drop its rows. A timer of a deleted worker finds nothing to
    /// settle when it fires. Returns the conversations its consultations opened on other agents
    /// (spec 064 FR-022), whose running turns it has aborted, for the caller to delete; its own
    /// workers' conversations go with the agent. Consultations *of* this agent stop with
    /// `consultee_deleted` and are delivered to the agents that asked.
    pub async fn delete_agent(&self, agent_id: &str) -> Vec<VizierSession> {
        self.consultee_deleted(&agent_id.to_string()).await;
        let rows = match self.inner.storage.delete_agent_subagents(agent_id).await {
            Ok(rows) => rows,
            Err(e) => {
                tracing::warn!(agent_id, "could not delete an agent's workers: {e}");
                return vec![];
            }
        };
        let mut elsewhere = vec![];
        for row in rows.iter().filter(|row| row.is_consultation()) {
            self.inner.disarm(&row.id);
            if !row.state.is_final() {
                self.inner.abort_worker(row).await;
            }
            elsewhere.push(row.worker_session());
        }
        elsewhere
    }

    /// The row `id` of `family`, if it was opened in this conversation.
    async fn owned_in(
        &self,
        ctx: &ToolContext,
        id: &str,
        family: Family,
    ) -> crate::Result<Result<Subagent, Refusal>> {
        let row = self
            .inner
            .storage
            .get_subagent(id)
            .await
            .map_err(|e| VizierError(format!("could not read worker {id}: {e}")))?;
        Ok(match row {
            Some(row) if row.hiring_session == ctx.session && family.holds(&row) => Ok(row),
            _ => Err(family.unknown(id)),
        })
    }

    /// A consultee is being deleted on this host (spec 064 FR-022): its open consultations stop
    /// with `consultee_deleted`, and each consulting conversation is told through a delivery.
    pub async fn consultee_deleted(&self, consultee: &AgentId) {
        match self
            .inner
            .storage
            .stop_consultations_of(consultee, crate::time::now())
            .await
        {
            Ok(rows) => {
                for row in &rows {
                    self.inner.disarm(&row.id);
                    self.inner.publish(row);
                    self.inner.poke(row);
                }
            }
            Err(e) => tracing::warn!(consultee = %consultee, "could not stop consultations of a deleted agent: {e}"),
        }
    }

    /// How many time limits this process is watching (tests).
    pub fn timer_count(&self) -> usize {
        self.inner.timers.lock().len()
    }
}

impl Inner {
    fn publish(&self, row: &Subagent) {
        self.transport.publish_session_event(SessionEvent {
            session: row.hiring_session.clone(),
            frame: SessionFrame::Subagent(row.snapshot()),
        });
    }

    async fn read(&self, id: &str) -> Option<Subagent> {
        self.storage.get_subagent(id).await.ok().flatten()
    }

    /// Settle a running worker; on success publish, drop its timer and poke its conversation.
    async fn settle_row(&self, id: &str, settled: Settled) {
        match self
            .storage
            .settle_subagent(id, settled, crate::time::now())
            .await
        {
            Ok(Some(row)) => {
                self.disarm(&row.id);
                self.publish(&row);
                self.poke(&row);
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(subagent = id, "could not record a worker's outcome: {e}"),
        }
    }

    /// A worker was stopped: drop its timer, publish, abort its running turn.
    async fn halt(&self, row: &Subagent) {
        self.disarm(&row.id);
        self.publish(row);
        self.abort_worker(row).await;
    }

    /// The existing `abort` command, sent to a worker's conversation.
    async fn abort_worker(&self, row: &Subagent) {
        let request = VizierRequest {
            timestamp: crate::time::now(),
            user: row.agent_id.clone(),
            content: VizierRequestContent::Command("abort".into()),
            metadata: serde_json::json!({}),
            ..Default::default()
        };
        if let Err(e) = self
            .transport
            .send_request(row.worker_session(), request, None)
            .await
        {
            tracing::debug!(subagent = %row.id, "could not abort a worker: {e}");
        }
    }

    /// Tell the hiring conversation an outcome may be ready.
    fn poke(&self, row: &Subagent) {
        let transport = self.transport.clone();
        let session = row.hiring_session.clone();
        tokio::spawn(async move { crate::agents::delivery::poke(&transport, session, 0).await });
    }

    async fn mark_seen(&self, workers: &[Subagent]) {
        let ids: Vec<SubagentId> = workers
            .iter()
            .filter(|w| w.is_deliverable())
            .map(|w| w.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }
        if let Err(e) = self.storage.mark_subagents_seen(&ids).await {
            tracing::warn!("could not mark worker outcomes seen: {e}");
        }
    }

    /// Watch a running worker's time limit.
    fn arm(self: &Arc<Self>, row: &Subagent) {
        let Some(deadline) = row.deadline_at else {
            return;
        };
        let wait = (deadline - crate::time::now())
            .to_std()
            .unwrap_or(Duration::ZERO);
        let inner = self.clone();
        let id = row.id.clone();
        let secs = row.timeout_secs;
        let task = tokio::spawn(async move {
            tokio::time::sleep(wait).await;
            inner.timers.lock().remove(&id);
            match inner
                .storage
                .settle_subagent(
                    &id,
                    Settled::TimedOut(timed_out_reason(secs)),
                    crate::time::now(),
                )
                .await
            {
                Ok(Some(row)) => {
                    inner.publish(&row);
                    inner.abort_worker(&row).await;
                    inner.poke(&row);
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(subagent = %id, "could not time out a worker: {e}"),
            }
        });
        if let Some(old) = self
            .timers
            .lock()
            .insert(row.id.clone(), task.abort_handle())
        {
            old.abort();
        }
    }

    fn disarm(&self, id: &str) {
        if let Some(handle) = self.timers.lock().remove(id) {
            handle.abort();
        }
    }
}

/// The request that carries a group's task or a reply to the row's own conversation. A worker
/// is one subagent level deeper and keeps the launching turn's project; a consultation is a
/// turn of another agent (spec 064 D8): one consultation level deeper, with the consulting agent
/// added to the chain, at subagent depth 0 (it may hire its own workers), with no project and no
/// parent script of the consulting agent's.
fn first_request(
    ctx: &ToolContext,
    row: &Subagent,
    scope: Option<crate::schema::ProjectScope>,
    content: VizierRequestContent,
) -> VizierRequest {
    let mut request = VizierRequest {
        timestamp: crate::time::now(),
        user: ctx.session.0.clone(),
        content,
        metadata: serde_json::json!({}),
        background_depth: ctx.background_depth.saturating_add(1),
        subagent_depth: ctx.subagent_depth.saturating_add(1),
        project_scope: scope,
        parent_detached_id: ctx.parent_detached_id.clone(),
        consultation_chain: ctx.consultation_chain.clone(),
        // A worker or consultee of a scoped turn keeps its scope (spec 071 FR-030).
        tool_scope: ctx.tool_scope.as_deref().cloned(),
        ..Default::default()
    };
    if row.is_consultation() {
        request.subagent_depth = 0;
        request.project_scope = None;
        request.parent_detached_id = None;
        request.consultation_chain.push(ctx.session.0.clone());
    }
    request
}

/// Delete the conversations of a deleted conversation's workers (`Subagents::delete_session`).
/// A worker whose turn was just aborted still reads as active for a moment, so each delete is
/// retried for up to five seconds before it is given up with a warning.
pub async fn delete_worker_conversations(
    files: &crate::session_files::SessionFiles,
    workers: Vec<VizierSession>,
) {
    for worker in workers {
        let mut attempts = 0;
        loop {
            match files.delete_conversation(&worker).await {
                Ok(_) => break,
                Err(crate::session_files::ClearError::TurnActive) if attempts < 50 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    tracing::warn!(session = %worker.to_slug(), "could not delete a worker's conversation: {e}");
                    break;
                }
            }
        }
    }
}

/// Workers are the second delivery source (research D9).
#[async_trait::async_trait]
impl crate::agents::delivery::DeliverySource for Subagents {
    async fn claim(&self, session: &VizierSession) -> Vec<crate::agents::delivery::Section> {
        let watched: Vec<SubagentId> = self.inner.watched.lock().keys().cloned().collect();
        match self
            .inner
            .storage
            .claim_subagent_deliveries(session, &watched)
            .await
        {
            Ok(rows) => {
                let mut sections = vec![];
                for row in &rows {
                    self.inner.publish(row);
                    sections.push(crate::agents::delivery::Section {
                        text: render_update(row),
                        scope: crate::agents::delivery::stored_scope(&self.inner.storage, &row.id).await,
                    });
                }
                sections
            }
            Err(e) => {
                tracing::warn!(session = %session.to_slug(), "could not claim worker outcomes: {e}");
                vec![]
            }
        }
    }

    async fn sessions_with_deliveries(&self, agent_id: &AgentId) -> Vec<VizierSession> {
        match self
            .inner
            .storage
            .subagent_delivery_sessions(agent_id)
            .await
        {
            Ok(sessions) => sessions,
            Err(e) => {
                tracing::warn!(agent_id = %agent_id, "could not list undelivered worker outcomes: {e}");
                vec![]
            }
        }
    }
}

/// The heading every worker section of a delivery starts with.
pub const UPDATE_HEADING: &str = "[Subagent update]";
/// A delivered result is cut at this many characters (the full one stays on the row).
const DELIVERED_RESULT_CHARS: usize = 4_000;

/// The heading every consultation section of a delivery starts with (spec 064).
pub const CONSULTATION_HEADING: &str = "[Consultation update]";

/// One worker's or consultation's section of a delivery (contracts/worker-turn.md).
pub fn render_update(row: &Subagent) -> String {
    let (title, _) = cut(&row.task.replace('\n', " "), 120);
    let (heading, reply_tool, get_tool) = match &row.consultee_id {
        Some(_) => (CONSULTATION_HEADING, "reply_consultation", "get_consultation"),
        None => (UPDATE_HEADING, "reply_subagent", "get_subagent"),
    };
    let mut out = match &row.consultee_id {
        Some(consultee) => format!(
            "{heading} {} — {consultee}, consultation {} of {} \"{title}\"\nstate: {}",
            row.id,
            row.position,
            row.group_size,
            row.state.as_str()
        ),
        None => format!(
            "{heading} {} — task {} of {} \"{title}\"\nstate: {}",
            row.id,
            row.position,
            row.group_size,
            row.state.as_str()
        ),
    };
    match row.state {
        SubagentState::Asking => {
            out.push_str(&format!(
                "\nquestion: {}\nreply with {reply_tool}(id=\"{}\", text=...)",
                row.question.as_deref().unwrap_or(""),
                row.id
            ));
        }
        SubagentState::Stopped => {
            if let Some(reason) = row.stop_reason {
                out.push_str(&format!("\nreason: {}", reason.describe(row.is_consultation())));
            }
        }
        SubagentState::Answered => {
            let (result, cut_here) =
                cut(row.result.as_deref().unwrap_or(""), DELIVERED_RESULT_CHARS);
            out.push_str(&format!("\nresult: {result}"));
            if cut_here || row.result_truncated {
                out.push_str(&format!(
                    "\n(read the whole result with {get_tool}(id=\"{}\"))",
                    row.id
                ));
            }
        }
        _ => {
            if let Some(error) = &row.error {
                out.push_str(&format!("\nerror: {error}"));
            }
        }
    }
    out
}

// --- Reply classification -------------------------------------------------------------------
//
// Shared with the transport's request/reply helper and the scheduler: how a turn's reply
// stream ended. Kept from the pre-059 fan-out, which was its first user.

const NO_ANSWER: &str = "the turn ended without an answer";
/// A reply stream that closed without an answer: its agent's process went away.
pub const AGENT_RESTARTED: &str = "the agent was restarted";

/// How a turn ended, from its first terminal frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PieceOutcome {
    Answered(String),
    Failed(String),
}

/// Map one frame of a turn to how it ended, or `None` for a mid-turn frame.
pub fn classify(content: &crate::schema::VizierResponseContent) -> Option<PieceOutcome> {
    use crate::schema::VizierResponseContent;
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
        // A worker's conversation has no person on it, so nothing is ever steered into one
        // (spec 062 FR-008).
        | VizierResponseContent::Steered
        | VizierResponseContent::Checkpoint { .. } => None,
    }
}

#[cfg(test)]
mod tests;
