//! Trigger storage (`specs/071-triggers/data-model.md`).
//!
//! Two transactions carry the limits (research R2): [`TriggerStorage::admit_event`] records an
//! event and enforces the queue caps; [`TriggerStorage::start_trigger_run`] checks overlap, the hourly
//! limit and the cooldown and moves up to [`MAX_EVENTS_PER_RUN`] waiting events into a new run.
//! Both are single transactions, so concurrent webhook requests cannot pass a cap twice.
//!
//! [`MAX_EVENTS_PER_RUN`]: crate::schema::trigger::MAX_EVENTS_PER_RUN

use anyhow::Result;
use serde_json::Value;

use crate::{
    schema::{
        Admission, AgentId, EventStatus, Trigger, TriggerEvent, TriggerRun, TriggerRunState,
        WhenFull,
    },
    storage::VizierStorage,
    time::Timestamp,
};

/// One event to record.
#[derive(Debug, Clone)]
pub struct NewEvent {
    pub trigger_id: String,
    pub agent_id: AgentId,
    pub received_at: Timestamp,
    /// Already redacted and capped by the caller.
    pub payload: Option<Value>,
    /// Recognises a redelivery (FR-024).
    pub delivery_id: Option<String>,
    /// `Ignored` (filter), `Dropped` (overlap drop), `TriagePending` or `Waiting`.
    pub status: EventStatus,
    pub reason: Option<String>,
    pub when_full: WhenFull,
    /// A monitor's new state, saved only when the event is accepted or ignored (FR-027).
    pub monitor_state: Option<Value>,
}

/// One triage verdict (FR-043, FR-045).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub event_id: i64,
    pub important: bool,
    pub reason: String,
    /// Triage failed and the event is counted as important.
    pub untriaged: bool,
}

/// What a run took when it started.
#[derive(Debug, Clone)]
pub struct StartedRun {
    pub run: TriggerRun,
    pub events: Vec<TriggerEvent>,
}

/// What closing a run did.
#[derive(Debug, Clone)]
pub struct ClosedRun {
    pub run: TriggerRun,
    /// Events that were given their last try and are now `failed` (FR-049).
    pub failed_events: usize,
    /// Events put back to wait for one more run.
    pub retried_events: usize,
}

/// What one summary window holds (FR-050).
#[derive(Debug, Clone, Default)]
pub struct SummaryItems {
    pub unimportant: Vec<TriggerEvent>,
    pub quiet_runs: Vec<TriggerRun>,
}

/// What a monitor remembers between checks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MonitorRecord {
    pub state: Option<Value>,
    pub last_check_at: Option<Timestamp>,
    pub next_check_at: Option<Timestamp>,
    pub last_error: Option<String>,
    pub failures: u32,
    pub failed_at: Option<Timestamp>,
}

#[async_trait::async_trait]
pub trait TriggerStorage {
    /// Refused when the slug is taken on the agent.
    async fn create_trigger(&self, trigger: &Trigger) -> Result<()>;
    async fn save_trigger(&self, trigger: &Trigger) -> Result<()>;
    async fn get_trigger(&self, agent_id: &str, slug: &str) -> Result<Option<Trigger>>;
    async fn get_trigger_by_id(&self, id: &str) -> Result<Option<Trigger>>;
    /// One agent's, or every agent's when `agent_id` is `None`.
    async fn list_triggers(&self, agent_id: Option<&str>) -> Result<Vec<Trigger>>;
    /// Deletes its events, runs, deliveries, triage calls and monitor record too.
    async fn delete_trigger(&self, id: &str) -> Result<()>;

    /// Change one field of a stored trigger without saving a copy read earlier, which would undo
    /// an edit made meanwhile.
    async fn note_trigger_fired(&self, id: &str, at: Timestamp) -> Result<()>;
    async fn note_trigger_summarised(&self, id: &str, at: Timestamp) -> Result<()>;
    /// Sets or clears `needs_attention`; `true` when it changed.
    async fn set_trigger_attention(&self, id: &str, reason: Option<String>) -> Result<bool>;

    async fn admit_event(&self, event: NewEvent) -> Result<Admission>;
    async fn pending_triage(&self, trigger_id: &str, limit: usize) -> Result<Vec<TriggerEvent>>;
    /// Important verdicts move to `waiting` (applying the waiting cap), the rest to
    /// `unimportant`. Only events still `triage_pending` are touched.
    async fn record_verdicts(
        &self,
        trigger_id: &str,
        verdicts: &[Verdict],
        when_full: WhenFull,
    ) -> Result<()>;
    async fn triage_calls_since(&self, trigger_id: &str, since: Timestamp) -> Result<u32>;
    async fn record_triage_call(&self, trigger_id: &str, at: Timestamp) -> Result<()>;

    /// Starts a run when no run of the trigger is in progress, the hourly limit and cooldown
    /// allow, and events are waiting. `session_key` is the run's conversation, which the caller
    /// derives from `started_at`.
    async fn start_trigger_run(
        &self,
        trigger: &Trigger,
        started_at: Timestamp,
        session_key: String,
    ) -> Result<Option<StartedRun>>;
    /// Guarded on `running`: a late close changes nothing and returns `None`.
    async fn close_trigger_run(
        &self,
        run_id: i64,
        state: TriggerRunState,
        finished_at: Timestamp,
        quiet_note: Option<String>,
    ) -> Result<Option<ClosedRun>>;
    /// Records "nothing to report" on the running run of this conversation (FR-050). `false`
    /// when no run of it is running.
    async fn note_quiet_run(&self, session_key: &str, note: &str) -> Result<bool>;
    /// Sets `delivered_at` once; `false` when it was already set (FR-046, at most once).
    async fn claim_delivery(&self, run_id: i64, at: Timestamp) -> Result<bool>;
    async fn get_trigger_run(&self, run_id: i64) -> Result<Option<TriggerRun>>;
    /// Newest first; `before` continues a page on `(started_at, id)`.
    async fn list_trigger_runs(
        &self,
        trigger_id: &str,
        before: Option<(Timestamp, i64)>,
        limit: usize,
    ) -> Result<Vec<TriggerRun>>;
    /// Newest first; `before` continues a page on `(received_at, id)`.
    async fn list_trigger_events(
        &self,
        trigger_id: &str,
        before: Option<(Timestamp, i64)>,
        limit: usize,
    ) -> Result<Vec<TriggerEvent>>;
    async fn trigger_run_events(&self, run_id: i64) -> Result<Vec<TriggerEvent>>;
    /// Runs started after `since`: what the hourly limit counts.
    async fn trigger_runs_since(&self, trigger_id: &str, since: Timestamp) -> Result<u32>;

    /// Every run still `running`, whoever owns it: what [`crate::triggers::resume`] watches.
    async fn open_trigger_runs(&self) -> Result<Vec<TriggerRun>>;
    /// Running runs whose owner is older than `below` (or null).
    async fn expired_trigger_runs(&self, below: u64) -> Result<Vec<TriggerRun>>;
    /// Recovery of `control`: the expired runs not in `keep` close as interrupted, with their
    /// events retried or failed; the kept ones are re-owned. Returns the closed ones.
    async fn interrupt_expired_trigger_runs_except(
        &self,
        below: u64,
        keep: &[i64],
    ) -> Result<Vec<ClosedRun>>;
    /// Triggers with events waiting for triage or for a run.
    async fn triggers_with_work(&self) -> Result<Vec<String>>;

    /// Unimportant events and quiet runs not yet summarised.
    async fn summary_items(&self, trigger_id: &str) -> Result<SummaryItems>;
    async fn mark_summarised(
        &self,
        trigger_id: &str,
        events: &[i64],
        runs: &[i64],
        at: Timestamp,
    ) -> Result<()>;
    /// Payloads of events received before `before` are deleted (FR-051).
    async fn purge_event_payloads(&self, before: Timestamp) -> Result<usize>;
    /// Delivery ids no longer remembered at `now` are deleted.
    async fn purge_deliveries(&self, now: Timestamp) -> Result<usize>;

    /// The tool scope of work a scoped turn started (a worker, a consultation, a detached
    /// script), so the turn its outcome is delivered into keeps it (FR-030).
    async fn set_work_scope(&self, work_id: &str, agent_id: &str, scope: &[String]) -> Result<()>;
    async fn work_scope(&self, work_id: &str) -> Result<Option<Vec<String>>>;

    async fn monitor_record(&self, trigger_id: &str) -> Result<MonitorRecord>;
    async fn save_monitor_record(&self, trigger_id: &str, record: &MonitorRecord) -> Result<()>;
}

#[async_trait::async_trait]
impl TriggerStorage for VizierStorage {
    async fn create_trigger(&self, trigger: &Trigger) -> Result<()> {
        self.0.create_trigger(trigger).await
    }
    async fn save_trigger(&self, trigger: &Trigger) -> Result<()> {
        self.0.save_trigger(trigger).await
    }
    async fn get_trigger(&self, agent_id: &str, slug: &str) -> Result<Option<Trigger>> {
        self.0.get_trigger(agent_id, slug).await
    }
    async fn get_trigger_by_id(&self, id: &str) -> Result<Option<Trigger>> {
        self.0.get_trigger_by_id(id).await
    }
    async fn list_triggers(&self, agent_id: Option<&str>) -> Result<Vec<Trigger>> {
        self.0.list_triggers(agent_id).await
    }
    async fn delete_trigger(&self, id: &str) -> Result<()> {
        self.0.delete_trigger(id).await
    }
    async fn note_trigger_fired(&self, id: &str, at: Timestamp) -> Result<()> {
        self.0.note_trigger_fired(id, at).await
    }
    async fn note_trigger_summarised(&self, id: &str, at: Timestamp) -> Result<()> {
        self.0.note_trigger_summarised(id, at).await
    }
    async fn set_trigger_attention(&self, id: &str, reason: Option<String>) -> Result<bool> {
        self.0.set_trigger_attention(id, reason).await
    }
    async fn admit_event(&self, event: NewEvent) -> Result<Admission> {
        self.0.admit_event(event).await
    }
    async fn pending_triage(&self, trigger_id: &str, limit: usize) -> Result<Vec<TriggerEvent>> {
        self.0.pending_triage(trigger_id, limit).await
    }
    async fn record_verdicts(
        &self,
        trigger_id: &str,
        verdicts: &[Verdict],
        when_full: WhenFull,
    ) -> Result<()> {
        self.0.record_verdicts(trigger_id, verdicts, when_full).await
    }
    async fn triage_calls_since(&self, trigger_id: &str, since: Timestamp) -> Result<u32> {
        self.0.triage_calls_since(trigger_id, since).await
    }
    async fn record_triage_call(&self, trigger_id: &str, at: Timestamp) -> Result<()> {
        self.0.record_triage_call(trigger_id, at).await
    }
    async fn start_trigger_run(
        &self,
        trigger: &Trigger,
        started_at: Timestamp,
        session_key: String,
    ) -> Result<Option<StartedRun>> {
        self.0.start_trigger_run(trigger, started_at, session_key).await
    }
    async fn close_trigger_run(
        &self,
        run_id: i64,
        state: TriggerRunState,
        finished_at: Timestamp,
        quiet_note: Option<String>,
    ) -> Result<Option<ClosedRun>> {
        self.0.close_trigger_run(run_id, state, finished_at, quiet_note).await
    }
    async fn claim_delivery(&self, run_id: i64, at: Timestamp) -> Result<bool> {
        self.0.claim_delivery(run_id, at).await
    }
    async fn note_quiet_run(&self, session_key: &str, note: &str) -> Result<bool> {
        self.0.note_quiet_run(session_key, note).await
    }
    async fn get_trigger_run(&self, run_id: i64) -> Result<Option<TriggerRun>> {
        self.0.get_trigger_run(run_id).await
    }
    async fn list_trigger_runs(
        &self,
        trigger_id: &str,
        before: Option<(Timestamp, i64)>,
        limit: usize,
    ) -> Result<Vec<TriggerRun>> {
        self.0.list_trigger_runs(trigger_id, before, limit).await
    }
    async fn list_trigger_events(
        &self,
        trigger_id: &str,
        before: Option<(Timestamp, i64)>,
        limit: usize,
    ) -> Result<Vec<TriggerEvent>> {
        self.0.list_trigger_events(trigger_id, before, limit).await
    }
    async fn trigger_run_events(&self, run_id: i64) -> Result<Vec<TriggerEvent>> {
        self.0.trigger_run_events(run_id).await
    }
    async fn trigger_runs_since(&self, trigger_id: &str, since: Timestamp) -> Result<u32> {
        self.0.trigger_runs_since(trigger_id, since).await
    }
    async fn open_trigger_runs(&self) -> Result<Vec<TriggerRun>> {
        self.0.open_trigger_runs().await
    }
    async fn expired_trigger_runs(&self, below: u64) -> Result<Vec<TriggerRun>> {
        self.0.expired_trigger_runs(below).await
    }
    async fn interrupt_expired_trigger_runs_except(
        &self,
        below: u64,
        keep: &[i64],
    ) -> Result<Vec<ClosedRun>> {
        self.0.interrupt_expired_trigger_runs_except(below, keep).await
    }
    async fn triggers_with_work(&self) -> Result<Vec<String>> {
        self.0.triggers_with_work().await
    }
    async fn summary_items(&self, trigger_id: &str) -> Result<SummaryItems> {
        self.0.summary_items(trigger_id).await
    }
    async fn mark_summarised(
        &self,
        trigger_id: &str,
        events: &[i64],
        runs: &[i64],
        at: Timestamp,
    ) -> Result<()> {
        self.0.mark_summarised(trigger_id, events, runs, at).await
    }
    async fn purge_event_payloads(&self, before: Timestamp) -> Result<usize> {
        self.0.purge_event_payloads(before).await
    }
    async fn set_work_scope(&self, work_id: &str, agent_id: &str, scope: &[String]) -> Result<()> {
        self.0.set_work_scope(work_id, agent_id, scope).await
    }
    async fn work_scope(&self, work_id: &str) -> Result<Option<Vec<String>>> {
        self.0.work_scope(work_id).await
    }
    async fn purge_deliveries(&self, now: Timestamp) -> Result<usize> {
        self.0.purge_deliveries(now).await
    }
    async fn monitor_record(&self, trigger_id: &str) -> Result<MonitorRecord> {
        self.0.monitor_record(trigger_id).await
    }
    async fn save_monitor_record(&self, trigger_id: &str, record: &MonitorRecord) -> Result<()> {
        self.0.save_monitor_record(trigger_id, record).await
    }
}
