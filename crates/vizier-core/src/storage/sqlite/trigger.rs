//! `TriggerStorage` over the shared SQL layer (`specs/071-triggers/data-model.md`).
//!
//! The table is `agent_trigger`, not `trigger`, which is a keyword in both SQL dialects.

use anyhow::{Result, anyhow};
use chrono::{Local, TimeZone};
use serde_json::Value;

use crate::{
    encryption::fields::{decrypt_trigger_settings, protect_trigger_settings},
    schema::{
        AdmitOutcome, Admission, EventStatus, Trigger, TriggerEvent, TriggerRun, TriggerRunState,
        WhenFull,
        trigger::{
            MAX_EVENT_ATTEMPTS, MAX_EVENTS_PER_RUN, MAX_PENDING_TRIAGE, MAX_WAITING_EVENTS,
        },
    },
    storage::{
        fence::Scope,
        sql::{self, Connection, Row, ToSql, params, params_from_iter},
        sqlite::{SqlStorage, fence::check_fence},
        trigger::{
            ClosedRun, MonitorRecord, NewEvent, StartedRun, SummaryItems, TriggerStorage, Verdict,
        },
    },
    time::Timestamp,
};

/// How long a delivery id is remembered (FR-024: at least 24 hours).
const DELIVERY_MEMORY_MS: i64 = 24 * 60 * 60 * 1000;
/// Drops and refusals within this window collapse into one record (edge case "webhook storm").
const COLLAPSE_WINDOW_MS: i64 = 60 * 60 * 1000;
const HOUR_MS: i64 = 60 * 60 * 1000;

const EVENT_COLUMNS: &str =
    "id, trigger_id, agent_id, received_at, payload, status, reason, untriaged, count, attempts, run_id";
const RUN_COLUMNS: &str = "id, trigger_id, agent_id, trigger_slug, started_at, finished_at, state, \
     session_key, delivered_at, quiet_note";

fn at(ms: i64) -> Option<Timestamp> {
    Local.timestamp_millis_opt(ms).single()
}

fn event_from_row(row: &Row) -> sql::Result<Option<TriggerEvent>> {
    let received: i64 = row.get(3)?;
    let payload: Option<String> = row.get(4)?;
    let status: String = row.get(5)?;
    let (Some(received_at), Some(status)) = (at(received), EventStatus::parse(&status)) else {
        return Ok(None);
    };
    let untriaged: i64 = row.get(7)?;
    let attempts: i64 = row.get(9)?;
    Ok(Some(TriggerEvent {
        id: row.get(0)?,
        trigger_id: row.get(1)?,
        agent_id: row.get(2)?,
        received_at,
        payload: payload.and_then(|p| serde_json::from_str(&p).ok()),
        status,
        reason: row.get(6)?,
        untriaged: untriaged != 0,
        count: row.get(8)?,
        attempts: attempts.max(0) as u32,
        run_id: row.get(10)?,
    }))
}

fn run_from_row(row: &Row) -> sql::Result<Option<TriggerRun>> {
    let started: i64 = row.get(4)?;
    let finished: Option<i64> = row.get(5)?;
    let state: String = row.get(6)?;
    let delivered: Option<i64> = row.get(8)?;
    let (Some(started_at), Some(state)) = (at(started), TriggerRunState::parse(&state)) else {
        return Ok(None);
    };
    Ok(Some(TriggerRun {
        id: row.get(0)?,
        trigger_id: row.get(1)?,
        agent_id: row.get(2)?,
        trigger_slug: row.get(3)?,
        started_at,
        finished_at: finished.and_then(at),
        state,
        session_key: row.get(7)?,
        delivered_at: delivered.and_then(at),
        quiet_note: row.get(9)?,
    }))
}

impl SqlStorage {
    fn trigger_from_row(&self, row: &(String, String, String)) -> Result<Trigger> {
        let (id, settings, data) = row;
        let mut trigger: Trigger = serde_json::from_str(data)?;
        trigger.id = id.clone();
        trigger.settings = decrypt_trigger_settings(&self.cipher, id, settings)?;
        Ok(trigger)
    }

    fn load_triggers(&self, conn: &Connection, filter: &str, args: &[&dyn ToSql]) -> Result<Vec<Trigger>> {
        let sql = format!("SELECT id, settings, data FROM agent_trigger {filter} ORDER BY agent_id, slug");
        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<(String, String, String)> = stmt
            .query_map(params_from_iter(args.iter()), |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<sql::Result<_>>()?;
        rows.iter().map(|row| self.trigger_from_row(row)).collect()
    }

    /// The row's two stored forms: the sealed settings, and everything else as JSON.
    /// Reads and rewrites one trigger in a single transaction under `control`'s fence. `change`
    /// says whether there is anything to write.
    fn edit_trigger(&self, id: &str, change: impl FnOnce(&mut Trigger) -> bool) -> Result<bool> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;
        // Takes the row's lock first, so a concurrent writer cannot slip between read and write.
        tx.execute("UPDATE agent_trigger SET data = data WHERE id = ?1", params![id])?;
        let rows = tx.query_all("SELECT data FROM agent_trigger WHERE id = ?1", params![id])?;
        let Some(row) = rows.first() else {
            return Ok(false);
        };
        let data: String = row.get(0)?;
        let mut trigger: Trigger = serde_json::from_str(&data)?;
        if !change(&mut trigger) {
            return Ok(false);
        }
        // The stored JSON never holds the settings, which stay sealed in their own column.
        trigger.settings = Value::Null;
        tx.execute(
            "UPDATE agent_trigger SET data = ?2 WHERE id = ?1",
            params![id, serde_json::to_string(&trigger)?],
        )?;
        tx.commit()?;
        Ok(true)
    }

    fn trigger_columns(&self, trigger: &Trigger) -> Result<(String, String)> {
        let settings = protect_trigger_settings(&self.cipher, &trigger.id, &trigger.settings)?;
        let mut data = trigger.clone();
        data.settings = Value::Null;
        Ok((settings, serde_json::to_string(&data)?))
    }
}

fn select_events(
    conn: &Connection,
    filter: &str,
    args: &[&dyn ToSql],
) -> Result<Vec<TriggerEvent>> {
    let sql = format!("SELECT {EVENT_COLUMNS} FROM trigger_event {filter}");
    let mut stmt = conn.prepare(&sql)?;
    let events = stmt
        .query_map(params_from_iter(args.iter()), event_from_row)?
        .filter_map(|r| r.ok())
        .flatten()
        .collect();
    Ok(events)
}

fn select_runs(conn: &Connection, filter: &str, args: &[&dyn ToSql]) -> Result<Vec<TriggerRun>> {
    let sql = format!("SELECT {RUN_COLUMNS} FROM trigger_run {filter}");
    let mut stmt = conn.prepare(&sql)?;
    let runs = stmt
        .query_map(params_from_iter(args.iter()), run_from_row)?
        .filter_map(|r| r.ok())
        .flatten()
        .collect();
    Ok(runs)
}

fn count(conn: &Connection, sql: &str, args: &[&dyn ToSql]) -> Result<i64> {
    Ok(conn.query_row(sql, params_from_iter(args.iter()), |row| row.get(0))?)
}

/// Records a drop or a refusal, folding it into a recent one of the same kind (FR-008).
fn collapse_into(
    conn: &Connection,
    event: &NewEvent,
    status: EventStatus,
    reason: &str,
) -> Result<i64> {
    let now = event.received_at.timestamp_millis();
    let existing: Option<i64> = conn
        .query_all(
            "SELECT id FROM trigger_event WHERE trigger_id = ?1 AND status = ?2 AND reason = ?3 \
             AND received_at >= ?4 ORDER BY id DESC LIMIT 1",
            params![event.trigger_id, status.as_str(), reason, now - COLLAPSE_WINDOW_MS],
        )?
        .first()
        .map(|row| row.get(0))
        .transpose()?;
    if let Some(id) = existing {
        conn.execute("UPDATE trigger_event SET count = count + 1 WHERE id = ?1", params![id])?;
        return Ok(id);
    }
    insert_event(conn, event, status, Some(reason))
}

fn insert_event(
    conn: &Connection,
    event: &NewEvent,
    status: EventStatus,
    reason: Option<&str>,
) -> Result<i64> {
    let payload = event.payload.as_ref().map(serde_json::to_string).transpose()?;
    Ok(conn.query_row(
        "INSERT INTO trigger_event (trigger_id, agent_id, received_at, payload, status, reason, \
         untriaged, count, attempts) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 1, 0) RETURNING id",
        params![
            event.trigger_id,
            event.agent_id,
            event.received_at.timestamp_millis(),
            payload,
            status.as_str(),
            reason,
        ],
        |row| row.get(0),
    )?)
}

/// Makes room in a full queue, or says the new event must be refused (FR-015).
/// Returns `false` when the new event is refused.
fn make_room(
    conn: &Connection,
    trigger_id: &str,
    status: EventStatus,
    cap: usize,
    when_full: WhenFull,
) -> Result<bool> {
    let queued = count(
        conn,
        "SELECT COUNT(*) FROM trigger_event WHERE trigger_id = ?1 AND status = ?2",
        &[&trigger_id, &status.as_str()],
    )?;
    if (queued as usize) < cap {
        return Ok(true);
    }
    match when_full {
        WhenFull::RefuseNew => Ok(false),
        WhenFull::DropOldest => {
            let overflow = queued as usize + 1 - cap;
            conn.execute(
                &format!(
                    "UPDATE trigger_event SET status = ?1, reason = ?2 WHERE id IN (\
                     SELECT id FROM trigger_event WHERE trigger_id = ?3 AND status = ?4 \
                     ORDER BY received_at, id LIMIT {overflow})"
                ),
                params![EventStatus::Dropped.as_str(), "queue full", trigger_id, status.as_str()],
            )?;
            Ok(true)
        }
    }
}

fn save_monitor_state(conn: &Connection, trigger_id: &str, state: &Value) -> Result<()> {
    conn.execute(
        "INSERT INTO trigger_monitor (trigger_id, state, failures) VALUES (?1, ?2, 0) \
         ON CONFLICT (trigger_id) DO UPDATE SET state = excluded.state",
        params![trigger_id, serde_json::to_string(state)?],
    )?;
    Ok(())
}

/// Closes one run inside `tx`; `None` when it was no longer running.
fn close_in(
    tx: &Connection,
    run_id: i64,
    state: TriggerRunState,
    finished_at: Timestamp,
    quiet_note: Option<&str>,
    owner: Option<(&Option<String>, &Option<i64>)>,
) -> Result<Option<ClosedRun>> {
    let changed = match owner {
        Some((node, token)) => tx.execute(
            "UPDATE trigger_run SET state = ?1, finished_at = ?2, quiet_note = COALESCE(?3, quiet_note), owner_node = ?6, \
             owner_token = ?7 WHERE id = ?4 AND state = ?5",
            params![
                state.as_str(),
                finished_at.timestamp_millis(),
                quiet_note,
                run_id,
                TriggerRunState::Running.as_str(),
                node,
                token
            ],
        )?,
        None => tx.execute(
            "UPDATE trigger_run SET state = ?1, finished_at = ?2, quiet_note = COALESCE(?3, quiet_note) \
             WHERE id = ?4 AND state = ?5",
            params![
                state.as_str(),
                finished_at.timestamp_millis(),
                quiet_note,
                run_id,
                TriggerRunState::Running.as_str()
            ],
        )?,
    };
    if changed == 0 {
        return Ok(None);
    }
    let (mut failed, mut retried) = (0, 0);
    if matches!(state, TriggerRunState::NoResponse | TriggerRunState::Interrupted) {
        // FR-049: one more run, then failed.
        failed = tx.execute(
            "UPDATE trigger_event SET status = ?1, reason = ?2 WHERE run_id = ?3 AND attempts >= ?4",
            params![
                EventStatus::Failed.as_str(),
                "its runs did not finish",
                run_id,
                MAX_EVENT_ATTEMPTS as i64
            ],
        )?;
        retried = tx.execute(
            "UPDATE trigger_event SET status = ?1, run_id = NULL WHERE run_id = ?2 AND attempts < ?3",
            params![EventStatus::Waiting.as_str(), run_id, MAX_EVENT_ATTEMPTS as i64],
        )?;
    }
    let run = select_runs(tx, "WHERE id = ?1", &[&run_id])?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("trigger run {run_id} vanished while closing"))?;
    Ok(Some(ClosedRun { run, failed_events: failed, retried_events: retried }))
}

#[async_trait::async_trait]
impl TriggerStorage for SqlStorage {
    async fn create_trigger(&self, trigger: &Trigger) -> Result<()> {
        let (settings, data) = self.trigger_columns(trigger)?;
        let conn = self.conn.lock()?;
        let taken = count(
            &conn,
            "SELECT COUNT(*) FROM agent_trigger WHERE agent_id = ?1 AND slug = ?2",
            &[&trigger.agent_id, &trigger.slug],
        )?;
        if taken > 0 {
            return Err(anyhow!("a trigger named '{}' already exists on this agent", trigger.slug));
        }
        conn.execute(
            "INSERT INTO agent_trigger (id, agent_id, slug, kind, is_active, settings, data, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                trigger.id,
                trigger.agent_id,
                trigger.slug,
                trigger.kind,
                trigger.accepts_events() as i64,
                settings,
                data,
                trigger.created_at.timestamp_millis()
            ],
        )?;
        Ok(())
    }

    async fn save_trigger(&self, trigger: &Trigger) -> Result<()> {
        let (settings, data) = self.trigger_columns(trigger)?;
        let conn = self.conn.lock()?;
        let changed = conn.execute(
            "UPDATE agent_trigger SET slug = ?2, kind = ?3, is_active = ?4, settings = ?5, data = ?6 \
             WHERE id = ?1",
            params![
                trigger.id,
                trigger.slug,
                trigger.kind,
                trigger.accepts_events() as i64,
                settings,
                data
            ],
        )?;
        if changed == 0 {
            return Err(anyhow!("trigger '{}' does not exist", trigger.slug));
        }
        Ok(())
    }

    async fn get_trigger(&self, agent_id: &str, slug: &str) -> Result<Option<Trigger>> {
        let conn = self.conn.lock()?;
        Ok(self
            .load_triggers(&conn, "WHERE agent_id = ?1 AND slug = ?2", &[&agent_id, &slug])?
            .into_iter()
            .next())
    }

    async fn get_trigger_by_id(&self, id: &str) -> Result<Option<Trigger>> {
        let conn = self.conn.lock()?;
        Ok(self.load_triggers(&conn, "WHERE id = ?1", &[&id])?.into_iter().next())
    }

    async fn list_triggers(&self, agent_id: Option<&str>) -> Result<Vec<Trigger>> {
        let conn = self.conn.lock()?;
        match agent_id {
            Some(agent_id) => self.load_triggers(&conn, "WHERE agent_id = ?1", &[&agent_id]),
            None => self.load_triggers(&conn, "", &[]),
        }
    }

    async fn delete_trigger(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        for table in ["trigger_event", "trigger_run", "trigger_delivery", "trigger_triage_call", "trigger_monitor"] {
            tx.execute(&format!("DELETE FROM {table} WHERE trigger_id = ?1"), params![id])?;
        }
        tx.execute("DELETE FROM agent_trigger WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    async fn note_trigger_fired(&self, id: &str, at: Timestamp) -> Result<()> {
        self.edit_trigger(id, |trigger| {
            trigger.last_fired_at = Some(at);
            true
        })?;
        Ok(())
    }

    async fn note_trigger_summarised(&self, id: &str, at: Timestamp) -> Result<()> {
        self.edit_trigger(id, |trigger| {
            trigger.last_summary_at = Some(at);
            true
        })?;
        Ok(())
    }

    async fn set_trigger_attention(&self, id: &str, reason: Option<String>) -> Result<bool> {
        self.edit_trigger(id, |trigger| {
            let changed = trigger.needs_attention != reason;
            trigger.needs_attention = reason;
            changed
        })
    }

    async fn admit_event(&self, event: NewEvent) -> Result<Admission> {
        let now = event.received_at.timestamp_millis();
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;

        if let Some(delivery_id) = &event.delivery_id {
            let seen: Option<i64> = tx
                .query_all(
                    "SELECT event_id FROM trigger_delivery WHERE trigger_id = ?1 AND delivery_id = ?2 \
                     AND expires_at > ?3",
                    params![event.trigger_id, delivery_id, now],
                )?
                .first()
                .map(|row| row.get(0))
                .transpose()?;
            if let Some(event_id) = seen {
                return Ok(Admission { event_id, outcome: AdmitOutcome::Duplicate });
            }
        }

        let (event_id, outcome) = match event.status {
            EventStatus::Ignored => {
                let reason = event.reason.clone().unwrap_or_else(|| "filtered out".into());
                (insert_event(&tx, &event, EventStatus::Ignored, Some(&reason))?, AdmitOutcome::Ignored)
            }
            EventStatus::Dropped => {
                let reason = event.reason.clone().unwrap_or_else(|| "a run was in progress".into());
                (collapse_into(&tx, &event, EventStatus::Dropped, &reason)?, AdmitOutcome::Dropped)
            }
            status @ (EventStatus::TriagePending | EventStatus::Waiting) => {
                let cap = match status {
                    EventStatus::TriagePending => MAX_PENDING_TRIAGE,
                    _ => MAX_WAITING_EVENTS,
                };
                if make_room(&tx, &event.trigger_id, status, cap, event.when_full)? {
                    (insert_event(&tx, &event, status, event.reason.as_deref())?, AdmitOutcome::Accepted)
                } else {
                    (collapse_into(&tx, &event, EventStatus::Refused, "queue full")?, AdmitOutcome::Refused)
                }
            }
            other => return Err(anyhow!("an event cannot arrive as {}", other.as_str())),
        };

        if let Some(delivery_id) = &event.delivery_id {
            tx.execute(
                "INSERT INTO trigger_delivery (trigger_id, delivery_id, event_id, expires_at) \
                 VALUES (?1, ?2, ?3, ?4) ON CONFLICT (trigger_id, delivery_id) \
                 DO UPDATE SET event_id = excluded.event_id, expires_at = excluded.expires_at",
                params![event.trigger_id, delivery_id, event_id, now + DELIVERY_MEMORY_MS],
            )?;
        }
        if let Some(state) = &event.monitor_state
            && matches!(outcome, AdmitOutcome::Accepted | AdmitOutcome::Ignored)
        {
            save_monitor_state(&tx, &event.trigger_id, state)?;
        }
        tx.commit()?;
        Ok(Admission { event_id, outcome })
    }

    async fn pending_triage(&self, trigger_id: &str, limit: usize) -> Result<Vec<TriggerEvent>> {
        let conn = self.conn.lock()?;
        select_events(
            &conn,
            &format!("WHERE trigger_id = ?1 AND status = ?2 ORDER BY received_at, id LIMIT {limit}"),
            &[&trigger_id, &EventStatus::TriagePending.as_str()],
        )
    }

    async fn record_verdicts(
        &self,
        trigger_id: &str,
        verdicts: &[Verdict],
        when_full: WhenFull,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;
        for verdict in verdicts {
            let status = if verdict.important {
                if !make_room(&tx, trigger_id, EventStatus::Waiting, MAX_WAITING_EVENTS, when_full)? {
                    tx.execute(
                        "UPDATE trigger_event SET status = ?1, reason = ?2 WHERE id = ?3 AND status = ?4",
                        params![
                            EventStatus::Refused.as_str(),
                            "queue full",
                            verdict.event_id,
                            EventStatus::TriagePending.as_str()
                        ],
                    )?;
                    continue;
                }
                EventStatus::Waiting
            } else {
                EventStatus::Unimportant
            };
            tx.execute(
                "UPDATE trigger_event SET status = ?1, reason = ?2, untriaged = ?3 \
                 WHERE id = ?4 AND trigger_id = ?5 AND status = ?6",
                params![
                    status.as_str(),
                    verdict.reason,
                    verdict.untriaged as i64,
                    verdict.event_id,
                    trigger_id,
                    EventStatus::TriagePending.as_str()
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn triage_calls_since(&self, trigger_id: &str, since: Timestamp) -> Result<u32> {
        let conn = self.conn.lock()?;
        let n = count(
            &conn,
            "SELECT COUNT(*) FROM trigger_triage_call WHERE trigger_id = ?1 AND at >= ?2",
            &[&trigger_id, &since.timestamp_millis()],
        )?;
        Ok(n.max(0) as u32)
    }

    async fn record_triage_call(&self, trigger_id: &str, at: Timestamp) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;
        let at = at.timestamp_millis();
        tx.execute(
            "INSERT INTO trigger_triage_call (trigger_id, at) VALUES (?1, ?2)",
            params![trigger_id, at],
        )?;
        // Only the last hour is ever counted.
        tx.execute(
            "DELETE FROM trigger_triage_call WHERE trigger_id = ?1 AND at < ?2",
            params![trigger_id, at - HOUR_MS],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn start_trigger_run(
        &self,
        trigger: &Trigger,
        started_at: Timestamp,
        session_key: String,
    ) -> Result<Option<StartedRun>> {
        let (node, token) = self.fences.owner_of(&Scope::Control);
        let now = started_at.timestamp_millis();
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;

        let running = count(
            &tx,
            "SELECT COUNT(*) FROM trigger_run WHERE trigger_id = ?1 AND state = ?2",
            &[&trigger.id, &TriggerRunState::Running.as_str()],
        )?;
        if running > 0 {
            return Ok(None);
        }
        if let Some(limit) = trigger.hourly_limit {
            let recent = count(
                &tx,
                "SELECT COUNT(*) FROM trigger_run WHERE trigger_id = ?1 AND started_at > ?2",
                &[&trigger.id, &(now - HOUR_MS)],
            )?;
            if recent >= limit as i64 {
                return Ok(None);
            }
        }
        if trigger.cooldown_secs > 0 {
            let last: Option<i64> = tx
                .query_all(
                    "SELECT MAX(started_at) FROM trigger_run WHERE trigger_id = ?1",
                    params![trigger.id],
                )?
                .first()
                .map(|row| row.get::<_, Option<i64>>(0))
                .transpose()?
                .flatten();
            if last.is_some_and(|last| now - last < trigger.cooldown_secs as i64 * 1000) {
                return Ok(None);
            }
        }
        let waiting = select_events(
            &tx,
            &format!(
                "WHERE trigger_id = ?1 AND status = ?2 ORDER BY received_at, id LIMIT {MAX_EVENTS_PER_RUN}"
            ),
            &[&trigger.id, &EventStatus::Waiting.as_str()],
        )?;
        if waiting.is_empty() {
            return Ok(None);
        }
        let run_id: i64 = tx.query_row(
            "INSERT INTO trigger_run (trigger_id, agent_id, trigger_slug, started_at, state, \
             session_key, owner_node, owner_token) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) RETURNING id",
            params![
                trigger.id,
                trigger.agent_id,
                trigger.slug,
                now,
                TriggerRunState::Running.as_str(),
                session_key,
                node,
                token
            ],
            |row| row.get(0),
        )?;
        let ids: Vec<i64> = waiting.iter().map(|e| e.id).collect();
        let placeholders = ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
        tx.execute(
            &format!(
                "UPDATE trigger_event SET status = ?1, run_id = ?2, attempts = attempts + 1 \
                 WHERE id IN ({placeholders})"
            ),
            params![EventStatus::Handled.as_str(), run_id],
        )?;
        let run = select_runs(&tx, "WHERE id = ?1", &[&run_id])?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("trigger run {run_id} vanished while starting"))?;
        let events = select_events(&tx, "WHERE run_id = ?1 ORDER BY received_at, id", &[&run_id])?;
        tx.commit()?;
        Ok(Some(StartedRun { run, events }))
    }

    async fn close_trigger_run(
        &self,
        run_id: i64,
        state: TriggerRunState,
        finished_at: Timestamp,
        quiet_note: Option<String>,
    ) -> Result<Option<ClosedRun>> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;
        let closed = close_in(&tx, run_id, state, finished_at, quiet_note.as_deref(), None)?;
        tx.commit()?;
        Ok(closed)
    }

    async fn claim_delivery(&self, run_id: i64, at: Timestamp) -> Result<bool> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;
        let changed = tx.execute(
            "UPDATE trigger_run SET delivered_at = ?1 WHERE id = ?2 AND delivered_at IS NULL",
            params![at.timestamp_millis(), run_id],
        )?;
        tx.commit()?;
        Ok(changed == 1)
    }

    async fn note_quiet_run(&self, session_key: &str, note: &str) -> Result<bool> {
        let conn = self.conn.lock()?;
        let changed = conn.execute(
            "UPDATE trigger_run SET quiet_note = ?1 WHERE session_key = ?2 AND state = ?3",
            params![note, session_key, TriggerRunState::Running.as_str()],
        )?;
        Ok(changed > 0)
    }

    async fn get_trigger_run(&self, run_id: i64) -> Result<Option<TriggerRun>> {
        let conn = self.conn.lock()?;
        Ok(select_runs(&conn, "WHERE id = ?1", &[&run_id])?.into_iter().next())
    }

    async fn list_trigger_runs(
        &self,
        trigger_id: &str,
        before: Option<(Timestamp, i64)>,
        limit: usize,
    ) -> Result<Vec<TriggerRun>> {
        let conn = self.conn.lock()?;
        match before {
            Some((time, id)) => select_runs(
                &conn,
                &format!(
                    "WHERE trigger_id = ?1 AND (started_at < ?2 OR (started_at = ?2 AND id < ?3)) \
                     ORDER BY started_at DESC, id DESC LIMIT {limit}"
                ),
                &[&trigger_id, &time.timestamp_millis(), &id],
            ),
            None => select_runs(
                &conn,
                &format!("WHERE trigger_id = ?1 ORDER BY started_at DESC, id DESC LIMIT {limit}"),
                &[&trigger_id],
            ),
        }
    }

    async fn list_trigger_events(
        &self,
        trigger_id: &str,
        before: Option<(Timestamp, i64)>,
        limit: usize,
    ) -> Result<Vec<TriggerEvent>> {
        let conn = self.conn.lock()?;
        match before {
            Some((time, id)) => select_events(
                &conn,
                &format!(
                    "WHERE trigger_id = ?1 AND (received_at < ?2 OR (received_at = ?2 AND id < ?3)) \
                     ORDER BY received_at DESC, id DESC LIMIT {limit}"
                ),
                &[&trigger_id, &time.timestamp_millis(), &id],
            ),
            None => select_events(
                &conn,
                &format!("WHERE trigger_id = ?1 ORDER BY received_at DESC, id DESC LIMIT {limit}"),
                &[&trigger_id],
            ),
        }
    }

    async fn trigger_run_events(&self, run_id: i64) -> Result<Vec<TriggerEvent>> {
        let conn = self.conn.lock()?;
        select_events(&conn, "WHERE run_id = ?1 ORDER BY received_at, id", &[&run_id])
    }

    async fn trigger_runs_since(&self, trigger_id: &str, since: Timestamp) -> Result<u32> {
        let conn = self.conn.lock()?;
        let n = count(
            &conn,
            "SELECT COUNT(*) FROM trigger_run WHERE trigger_id = ?1 AND started_at > ?2",
            &[&trigger_id, &since.timestamp_millis()],
        )?;
        Ok(n.max(0) as u32)
    }

    async fn open_trigger_runs(&self) -> Result<Vec<TriggerRun>> {
        let conn = self.conn.lock()?;
        select_runs(&conn, "WHERE state = ?1 ORDER BY id", &[&TriggerRunState::Running.as_str()])
    }

    async fn expired_trigger_runs(&self, below: u64) -> Result<Vec<TriggerRun>> {
        let conn = self.conn.lock()?;
        select_runs(
            &conn,
            "WHERE state = ?1 AND (owner_token IS NULL OR owner_token < ?2) ORDER BY id",
            &[&TriggerRunState::Running.as_str(), &(below as i64)],
        )
    }

    async fn interrupt_expired_trigger_runs_except(
        &self,
        below: u64,
        keep: &[i64],
    ) -> Result<Vec<ClosedRun>> {
        let (node, token) = self.fences.owner_of(&Scope::Control);
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;
        let expired = select_runs(
            &tx,
            "WHERE state = ?1 AND (owner_token IS NULL OR owner_token < ?2) ORDER BY id",
            &[&TriggerRunState::Running.as_str(), &(below as i64)],
        )?;
        let mut closed = vec![];
        for run in expired {
            if keep.contains(&run.id) {
                tx.execute(
                    "UPDATE trigger_run SET owner_node = ?2, owner_token = ?3 WHERE id = ?1",
                    params![run.id, node, token],
                )?;
                continue;
            }
            if let Some(done) = close_in(
                &tx,
                run.id,
                TriggerRunState::Interrupted,
                crate::time::now(),
                None,
                Some((&node, &token)),
            )? {
                closed.push(done);
            }
        }
        tx.commit()?;
        Ok(closed)
    }

    async fn triggers_with_work(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT trigger_id FROM trigger_event WHERE status IN (?1, ?2) ORDER BY trigger_id",
        )?;
        let ids = stmt
            .query_map(
                params![EventStatus::TriagePending.as_str(), EventStatus::Waiting.as_str()],
                |row| row.get(0),
            )?
            .collect::<sql::Result<Vec<String>>>()?;
        Ok(ids)
    }

    async fn summary_items(&self, trigger_id: &str) -> Result<SummaryItems> {
        let conn = self.conn.lock()?;
        let unimportant = select_events(
            &conn,
            "WHERE trigger_id = ?1 AND status = ?2 AND summarised_at IS NULL \
             ORDER BY received_at, id LIMIT 500",
            &[&trigger_id, &EventStatus::Unimportant.as_str()],
        )?;
        let quiet_runs = select_runs(
            &conn,
            "WHERE trigger_id = ?1 AND quiet_note IS NOT NULL AND summarised_at IS NULL \
             ORDER BY started_at, id LIMIT 500",
            &[&trigger_id],
        )?;
        Ok(SummaryItems { unimportant, quiet_runs })
    }

    async fn mark_summarised(
        &self,
        trigger_id: &str,
        events: &[i64],
        runs: &[i64],
        at: Timestamp,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;
        let at = at.timestamp_millis();
        for (table, ids) in [("trigger_event", events), ("trigger_run", runs)] {
            if ids.is_empty() {
                continue;
            }
            let list = ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
            tx.execute(
                &format!("UPDATE {table} SET summarised_at = ?1 WHERE trigger_id = ?2 AND id IN ({list})"),
                params![at, trigger_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn purge_event_payloads(&self, before: Timestamp) -> Result<usize> {
        let conn = self.conn.lock()?;
        Ok(conn.execute(
            "UPDATE trigger_event SET payload = NULL WHERE received_at < ?1 AND payload IS NOT NULL",
            params![before.timestamp_millis()],
        )?)
    }

    async fn set_work_scope(&self, work_id: &str, agent_id: &str, scope: &[String]) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO trigger_work_scope (work_id, agent_id, scope) VALUES (?1, ?2, ?3) \
             ON CONFLICT (work_id) DO UPDATE SET scope = excluded.scope",
            params![work_id, agent_id, serde_json::to_string(scope)?],
        )?;
        Ok(())
    }

    async fn work_scope(&self, work_id: &str) -> Result<Option<Vec<String>>> {
        let conn = self.conn.lock()?;
        let rows = conn.query_all(
            "SELECT scope FROM trigger_work_scope WHERE work_id = ?1",
            params![work_id],
        )?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let scope: String = row.get(0)?;
        Ok(Some(serde_json::from_str(&scope)?))
    }

    async fn purge_deliveries(&self, now: Timestamp) -> Result<usize> {
        let conn = self.conn.lock()?;
        Ok(conn.execute(
            "DELETE FROM trigger_delivery WHERE expires_at <= ?1",
            params![now.timestamp_millis()],
        )?)
    }

    async fn monitor_record(&self, trigger_id: &str) -> Result<MonitorRecord> {
        let conn = self.conn.lock()?;
        let rows = conn.query_all(
            "SELECT state, last_check_at, next_check_at, last_error, failures, failed_at \
             FROM trigger_monitor WHERE trigger_id = ?1",
            params![trigger_id],
        )?;
        let Some(row) = rows.first() else {
            return Ok(MonitorRecord::default());
        };
        let state: Option<String> = row.get(0)?;
        let failures: i64 = row.get(4)?;
        Ok(MonitorRecord {
            state: state.and_then(|s| serde_json::from_str(&s).ok()),
            last_check_at: row.get::<_, Option<i64>>(1)?.and_then(at),
            next_check_at: row.get::<_, Option<i64>>(2)?.and_then(at),
            last_error: row.get(3)?,
            failures: failures.max(0) as u32,
            failed_at: row.get::<_, Option<i64>>(5)?.and_then(at),
        })
    }

    async fn save_monitor_record(&self, trigger_id: &str, record: &MonitorRecord) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_fence(&tx, &self.fences, &Scope::Control)?;
        let state = record.state.as_ref().map(serde_json::to_string).transpose()?;
        tx.execute(
            "INSERT INTO trigger_monitor (trigger_id, state, last_check_at, next_check_at, last_error, \
             failures, failed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (trigger_id) DO UPDATE \
             SET state = excluded.state, last_check_at = excluded.last_check_at, \
             next_check_at = excluded.next_check_at, last_error = excluded.last_error, \
             failures = excluded.failures, failed_at = excluded.failed_at",
            params![
                trigger_id,
                state,
                record.last_check_at.map(|t| t.timestamp_millis()),
                record.next_check_at.map(|t| t.timestamp_millis()),
                record.last_error,
                record.failures as i64,
                record.failed_at.map(|t| t.timestamp_millis())
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "trigger_tests.rs"]
mod tests;
